//! The `evaluate_function` and `select_catalog` actions: calls and catalog selection.

use std::sync::OnceLock;

use a2ui_catalog_basic::{BasicOptions, CATALOG_ID, CATALOG_JSON, functions};
use a2ui_runtime::{
    Catalog, CatalogSet, DataModel, Error, Evaluation, Evaluator, FunctionRegistry,
    MessageProcessor, Scope, select_supported,
};
use serde_json::{Map, Value, json};

use crate::case::Case;
use crate::compare::{expect_error, json_eq};
use crate::handlers::catalogs;
use crate::keys::{assert_case_keys, assert_known_keys};
use crate::manifest::record_aliased;

const RUN_KEYS: [&str; 9] = [
    "protocolVersion",
    "catalogPaths",
    "surface",
    "dataModel",
    "function",
    "catalogId",
    "args",
    "expect",
    "expectError",
];

const SURFACE_KEYS: [&str; 1] = ["catalogId"];

/// Keys a direct call cannot use: they describe a surface.
const SURFACE_ONLY_KEYS: [&str; 3] = ["catalogPaths", "dataModel", "catalogId"];

const SELECT_KEYS: [&str; 4] = ["catalog", "args", "expectSelected", "expectError"];

const SELECT_ARG_KEYS: [&str; 3] = ["surface", "components", "functionCall"];

const SELECT_SURFACE_KEYS: [&str; 3] = ["id", "defaultCatalogId", "supportedCatalogIds"];

const FUNCTION_CALL_KEYS: [&str; 3] = ["call", "catalogId", "args"];

/// The surface a call with a `surface` block is evaluated on.
const SURFACE_ID: &str = "main";

/// Runs an `evaluate_function` case and compares its result or first error.
///
/// Without `surface`, the basic catalog's implementation of `function` is called on `args`
/// inside a user activation, with no surface default to fall back on but the basic catalog.
/// With `surface`, the v1.0 `catalogPaths` are loaded, surface `main` is created with
/// `surface.catalogId` and `dataModel`, and `{"@call": function, "catalogId"?, "args"}` is
/// evaluated on it. The first error raised is the call's error.
pub fn run(case: &Case) -> Result<(), String> {
    assert_case_keys(case, &RUN_KEYS)?;
    let at = case.at();
    let function = case
        .get("function")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{at}: a case needs a string `function`"))?;
    let args = match case.get("args") {
        None => Value::Object(Map::new()),
        Some(args @ Value::Object(_)) => args.clone(),
        Some(other) => return Err(format!("{at}: `args` must be a mapping, got {other}")),
    };
    let result = match case.get("surface") {
        None => direct(case, function, args, &at)?,
        Some(surface) => on_surface(case, surface, function, args, &at)?,
    };
    compare(result, case, &at)
}

/// `function` of the basic catalog called on `args`, outside any surface.
fn direct(
    case: &Case,
    function: &str,
    args: Value,
    at: &str,
) -> Result<Result<Value, Error>, String> {
    if let Some(key) = SURFACE_ONLY_KEYS.iter().find(|key| case.get(key).is_some()) {
        return Err(format!("{at}: `{key}` needs a `surface` block"));
    }
    let set = basic_set(at)?;
    let registry = basic_functions(at)?;
    let data = DataModel::default();
    let scope = Scope::new(SURFACE_ID, Some(CATALOG_ID), &data).with_user_activation(true);
    let call = json!({"@call": function, "args": args});
    Ok(first_error(
        Evaluator::new(set, &registry).evaluate(&call, &scope),
    ))
}

/// A set holding only the basic catalog, loaded on the first direct call and then shared.
///
/// `a2ui_catalog_basic::catalog()` lends a `&'static Catalog` and `Catalog` is not `Clone`, so
/// the set keeps a catalog of its own, parsed once rather than once per case.
fn basic_set(at: &str) -> Result<&'static CatalogSet, String> {
    static BASIC: OnceLock<CatalogSet> = OnceLock::new();
    if let Some(set) = BASIC.get() {
        return Ok(set);
    }
    let catalog = Catalog::parse(CATALOG_JSON, None, None)
        .map_err(|err| format!("{at}: basic catalog: {err}"))?;
    let mut set = CatalogSet::new();
    set.insert(catalog)
        .map_err(|err| format!("{at}: basic catalog not registered: {err}"))?;
    Ok(BASIC.get_or_init(|| set))
}

/// `function` called on surface `main`, created from the case's catalogs and `surface` block.
fn on_surface(
    case: &Case,
    surface: &Value,
    function: &str,
    args: Value,
    at: &str,
) -> Result<Result<Value, Error>, String> {
    let surface = surface
        .as_object()
        .ok_or_else(|| format!("{at}: `surface` must be a mapping"))?;
    assert_known_keys(surface, &SURFACE_KEYS, at)?;
    let loaded = catalogs::load(case)?;
    record_aliased(case, &loaded.aliased)?;
    let mut processor = MessageProcessor::new(loaded.set).with_functions(basic_functions(at)?);
    let mut create = json!({"surfaceId": SURFACE_ID});
    if let Some(id) = surface.get("catalogId") {
        create["catalogId"] = id.clone();
    }
    if let Some(data) = case.get("dataModel") {
        create["dataModel"] = data.clone();
    }
    processor
        .handle_batch(&json!([{"version": "1.0", "createSurface": create}]))
        .map_err(|err| format!("{at}: surface not created: {err}"))?;
    let mut call = json!({"@call": function, "args": args});
    if let Some(id) = case.get("catalogId") {
        call["catalogId"] = id.clone();
    }
    let evaluation = processor
        .evaluate(SURFACE_ID, &call, false)
        .map_err(|err| format!("{at}: not evaluated: {err}"))?;
    Ok(first_error(evaluation))
}

/// The basic catalog's functions, with the default options.
fn basic_functions(at: &str) -> Result<FunctionRegistry, String> {
    let mut registry = FunctionRegistry::new();
    functions(&mut registry, BasicOptions::default())
        .map_err(|err| format!("{at}: basic functions not registered: {err}"))?;
    Ok(registry)
}

/// The evaluated value, or the first error raised on the way.
fn first_error(evaluation: Evaluation) -> Result<Value, Error> {
    match evaluation.errors.into_iter().next() {
        Some(error) => Err(error),
        None => Ok(evaluation.value),
    }
}

/// Compares a call's result with `expect` (`null` included) or its error with `expectError`.
fn compare(result: Result<Value, Error>, case: &Case, at: &str) -> Result<(), String> {
    let expected_error = case.get("expectError");
    let expected = case.get("expect");
    if expected_error.is_some() && expected.is_some() {
        return Err(format!("{at}: `expect` and `expectError` are exclusive"));
    }
    match (result, expected_error, expected) {
        (Err(err), Some(wanted), _) => expect_error(&err, wanted, at),
        (Err(err), None, _) => Err(format!("{at}: raised {err}")),
        (Ok(value), Some(_), _) => Err(format!("{at}: got {value}, expected an error")),
        (Ok(value), None, Some(wanted)) if json_eq(&value, wanted) => Ok(()),
        (Ok(value), None, Some(wanted)) => Err(format!("{at}: got {value}, expected {wanted}")),
        (Ok(_), None, None) => Err(format!("{at}: a case needs `expect` or `expectError`")),
    }
}

/// One catalog selection: the id the entry names, what it is, and its name.
type Request<'a> = (Option<&'a str>, &'static str, &'a str);

/// Runs a `select_catalog` case: each component in key order, or the one `functionCall`.
///
/// Each selection checks the surface's `supportedCatalogIds`; the first error stops the run
/// and is compared with `expectError`, else the last selection with `expectSelected`.
pub fn select(case: &Case) -> Result<(), String> {
    assert_case_keys(case, &SELECT_KEYS)?;
    let at = case.at();
    let args = mapping(case.get("args"), "args", &at)?;
    assert_known_keys(args, &SELECT_ARG_KEYS, &at)?;
    let surface = mapping(args.get("surface"), "args.surface", &at)?;
    assert_known_keys(surface, &SELECT_SURFACE_KEYS, &at)?;
    let surface_id = text(surface.get("id"), "args.surface.id", &at)?
        .ok_or_else(|| format!("{at}: `args.surface.id` is needed"))?;
    let default = text(surface.get("defaultCatalogId"), "defaultCatalogId", &at)?;
    let supported = supported(surface.get("supportedCatalogIds"), &at)?;
    let mut selected = None;
    let mut raised = None;
    for (own, what, name) in requests(args, &at)? {
        match select_supported(own, default, &supported, surface_id, what, name) {
            Ok(id) => selected = Some(id),
            Err(err) => {
                raised = Some(err);
                break;
            }
        }
    }
    match (raised, case.get("expectError"), case.get("expectSelected")) {
        (_, Some(_), Some(_)) => Err(format!(
            "{at}: `expectSelected` and `expectError` are exclusive"
        )),
        (Some(err), Some(wanted), None) => expect_error(&err, wanted, &at),
        (Some(err), None, _) => Err(format!("{at}: raised {err}")),
        (None, Some(_), None) => Err(format!("{at}: selected {selected:?}, expected an error")),
        (None, None, Some(wanted)) => match selected {
            Some(id) if wanted.as_str() == Some(id) => Ok(()),
            _ => Err(format!("{at}: selected {selected:?}, expected {wanted}")),
        },
        (None, None, None) => Err(format!(
            "{at}: a case needs `expectSelected` or `expectError`"
        )),
    }
}

/// The selections `args` asks for: its `components` in key order, or its `functionCall`.
fn requests<'a>(args: &'a Map<String, Value>, at: &str) -> Result<Vec<Request<'a>>, String> {
    match (args.get("components"), args.get("functionCall")) {
        (Some(components), None) => {
            let components = components
                .as_object()
                .ok_or_else(|| format!("{at}: `args.components` must be a mapping"))?;
            components
                .iter()
                .map(|(id, component)| {
                    let here = format!("{at} component {id}");
                    let component = mapping(Some(component), "component", &here)?;
                    let kind = text(component.get("component"), "component", &here)?
                        .ok_or_else(|| format!("{here}: `component` is needed"))?;
                    let own = text(component.get("catalogId"), "catalogId", &here)?;
                    Ok((own, "component", kind))
                })
                .collect()
        }
        (None, Some(call)) => {
            let call = mapping(Some(call), "args.functionCall", at)?;
            assert_known_keys(call, &FUNCTION_CALL_KEYS, at)?;
            let name = text(call.get("call"), "functionCall.call", at)?
                .ok_or_else(|| format!("{at}: `args.functionCall.call` is needed"))?;
            let own = text(call.get("catalogId"), "functionCall.catalogId", at)?;
            Ok(vec![(own, "function", name)])
        }
        _ => Err(format!(
            "{at}: exactly one of `args.components` and `args.functionCall` is needed"
        )),
    }
}

/// `value` as a mapping named `what`; absent or another type is an error.
fn mapping<'a>(
    value: Option<&'a Value>,
    what: &str,
    at: &str,
) -> Result<&'a Map<String, Value>, String> {
    value
        .and_then(Value::as_object)
        .ok_or_else(|| format!("{at}: `{what}` must be a mapping"))
}

/// `value` as a string named `what`, if present; another type is an error.
fn text<'a>(value: Option<&'a Value>, what: &str, at: &str) -> Result<Option<&'a str>, String> {
    match value {
        None => Ok(None),
        Some(Value::String(text)) => Ok(Some(text)),
        Some(other) => Err(format!("{at}: `{what}` must be a string, got {other}")),
    }
}

/// `supportedCatalogIds`, a list of strings.
fn supported(value: Option<&Value>, at: &str) -> Result<Vec<String>, String> {
    let ids = value
        .and_then(Value::as_array)
        .ok_or_else(|| format!("{at}: `args.surface.supportedCatalogIds` must be a list"))?;
    ids.iter()
        .map(|id| {
            id.as_str()
                .map(str::to_owned)
                .ok_or_else(|| format!("{at}: `supportedCatalogIds` entries must be strings"))
        })
        .collect()
}

#[cfg(test)]
#[path = "functions_tests.rs"]
mod tests;
