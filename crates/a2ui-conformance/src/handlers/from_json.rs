//! The `from_json` action: load a catalog, then check entries and component validation.

use a2ui_runtime::{Catalog, Category};
use serde_json::{Map, Value};

use crate::case::Case;
use crate::compare::{expect_error, json_eq};
use crate::handlers::catalog_checks::{entries, matches_builtin, self_contained};
use crate::handlers::catalog_input::{document, load};
use crate::keys::{assert_case_keys, assert_known_keys};

const EXPECT_KEYS: [&str; 10] = [
    "catalogId",
    "protocolVersion",
    "components",
    "functions",
    "theme",
    "matchesBuiltinCatalog",
    "selfContained",
    "validComponents",
    "invalidComponents",
    "componentMetadata",
];

/// Loads the case's catalog and checks `expect`, or the `expectError` it must raise.
pub fn run(case: &Case) -> Result<(), String> {
    assert_case_keys(
        case,
        &[
            "catalog",
            "catalogPath",
            "catalogId",
            "protocolVersion",
            "expect",
            "expectError",
        ],
    )?;
    let at = case.at();
    let input = document(case)?;
    match (case.get("expectError"), load(case, &input)?) {
        (Some(expected), Err(err)) => expect_error(&err, expected, &at),
        (Some(_), Ok(_)) => Err(format!("{at}: loaded, expected an error")),
        (None, Err(err)) => Err(format!("{at}: load failed: {err}")),
        (None, Ok(catalog)) => match case.get("expect") {
            None => Ok(()),
            Some(Value::Object(expect)) => check(expect, &catalog, &at),
            Some(_) => Err(format!("{at}: `expect` must be a mapping")),
        },
    }
}

fn check(expect: &Map<String, Value>, catalog: &Catalog, at: &str) -> Result<(), String> {
    assert_known_keys(expect, &EXPECT_KEYS, at)?;
    entries(expect, catalog, at)?;
    if flag(expect, "selfContained", at)? {
        self_contained(catalog.validation_schema()).map_err(|err| format!("{at}: {err}"))?;
    }
    if flag(expect, "matchesBuiltinCatalog", at)? {
        let builtin = a2ui_catalog_basic::catalog().map_err(|err| format!("{at}: {err}"))?;
        matches_builtin(catalog, &builtin).map_err(|err| format!("{at}: {err}"))?;
    }
    for (index, component) in list(expect, "validComponents", at)?.iter().enumerate() {
        catalog
            .validate_component(component, catalog.id(), catalog)
            .map_err(|err| format!("{at}: validComponents[{index}] rejected: {err}"))?;
    }
    for (index, component) in list(expect, "invalidComponents", at)?.iter().enumerate() {
        match catalog.validate_component(component, catalog.id(), catalog) {
            Err(err) if err.category() == Category::Validation => {}
            Err(err) => return Err(format!("{at}: invalidComponents[{index}]: {err}")),
            Ok(()) => return Err(format!("{at}: invalidComponents[{index}] accepted")),
        }
    }
    if let Some(expected) = expect.get("componentMetadata") {
        component_metadata(expected, catalog, at)?;
    }
    Ok(())
}

fn component_metadata(expected: &Value, catalog: &Catalog, at: &str) -> Result<(), String> {
    let expected = expected
        .as_object()
        .ok_or_else(|| format!("{at}: componentMetadata must be a mapping"))?;
    for (name, metadata) in expected {
        let def = catalog
            .component(name)
            .ok_or_else(|| format!("{at}: no component {name}"))?;
        let actual = def.metadata().cloned().unwrap_or(Value::Null);
        if !json_eq(&actual, metadata) {
            return Err(format!(
                "{at}: {name} metadata {actual}, expected {metadata}"
            ));
        }
    }
    Ok(())
}

fn flag(expect: &Map<String, Value>, key: &str, at: &str) -> Result<bool, String> {
    match expect.get(key) {
        None => Ok(false),
        Some(Value::Bool(true)) => Ok(true),
        Some(other) => Err(format!(
            "{at}: only `{key}: true` is supported, got {other}"
        )),
    }
}

fn list<'a>(expect: &'a Map<String, Value>, key: &str, at: &str) -> Result<&'a [Value], String> {
    match expect.get(key) {
        None => Ok(&[]),
        Some(Value::Array(items)) => Ok(items),
        Some(_) => Err(format!("{at}: `{key}` must be a list")),
    }
}
