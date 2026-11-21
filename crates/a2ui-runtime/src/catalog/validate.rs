use serde_json::{Map, Value};
use snafu::{OptionExt, ensure};

use super::{AllowedCallers, Catalog};
use crate::detail::Detail;
use crate::error::{
    AgentOnlyCallSnafu, BadIdentifierSnafu, CallShapeSnafu, ComponentLimitSnafu,
    ComponentShapeSnafu, Error, MAX_VIOLATION_DETAILS, MissingComponentTypeSnafu, ProcessFailure,
    ReservedKeySnafu, SystemFunctionCatalogSnafu, UnknownComponentSnafu, UnknownFunctionSnafu,
    ValueLimitSnafu,
};
use crate::ident::is_identifier;
use crate::keys::{CALL_KEY, PATH_KEY};
use crate::limits::{
    ComponentOversize, MAX_COMPONENT_CALLS, MAX_COMPONENT_NODES, MAX_JSON_DEPTH,
    component_oversize, deeper_than,
};
use crate::quoted::{Quoted, reason};
use crate::schema::{validate_common_type, validate_component_metadata};
use crate::validator::{pointer, violation_detail, violation_error};
/// Keys a component may carry whether or not its schema declares them.
const ENVELOPE: [&str; 4] = ["id", "component", "catalogId", "metadata"];

/// Finds a loaded catalog by its id, for calls that name another catalog.
///
/// Implement this trait to let [`Catalog::validate_component`] check calls into the other
/// catalogs your surface has loaded, kept in a map or registry of your own. A [`Catalog`] finds
/// itself and a `[Catalog]` slice finds any of its catalogs. A call into a catalog that is not
/// found is checked only for its identifiers.
pub trait CatalogLookup {
    /// The loaded catalog whose id is `id`, if any.
    fn catalog(&self, id: &str) -> Option<&Catalog>;
}

impl CatalogLookup for Catalog {
    fn catalog(&self, id: &str) -> Option<&Catalog> {
        (self.id == id).then_some(self)
    }
}

impl CatalogLookup for [Catalog] {
    fn catalog(&self, id: &str) -> Option<&Catalog> {
        self.iter().find(|catalog| catalog.id == id)
    }
}

/// How many schema violations a component check reports.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Report<'a> {
    /// The first violation, with no detail.
    First,
    /// Every violation, each as a detail, up to `MAX_VIOLATION_DETAILS`, of the component whose
    /// checked id is `id`.
    All { id: &'a str },
}

/// What a call into a catalog the lookup does not find does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CallCatalogs {
    /// It is checked only for its identifiers.
    IdentifiersOnly,
    /// It is refused: the catalog was not found.
    Resolve,
}

/// Where a component is checked: its surface, the catalogs its calls run in, and how strictly.
pub(crate) struct Located<'a, L: ?Sized> {
    /// The surface's default catalog id, where calls naming none run.
    pub(crate) default_catalog: Option<&'a str>,
    /// The surface id, quoted when a call finds no catalog to run in.
    pub(crate) surface_id: &'a str,
    pub(crate) catalogs: &'a L,
    pub(crate) report: Report<'a>,
    pub(crate) calls: CallCatalogs,
    /// Whether an unknown component type or function is accepted.
    pub(crate) allow_unknown: bool,
}

impl Catalog {
    /// Validates one component instance, such as `{"id": "t1", "component": "Text"}`.
    ///
    /// A nested call runs in the catalog its `catalogId` names, else in `default_catalog` (the
    /// surface's). When that catalog is this one or `catalogs` finds it, the call is checked
    /// against its function there: the function exists, is not `agentOnly`, and the call
    /// matches its schema. A call into a catalog not found is checked only for its identifiers.
    ///
    /// Before any of that, the instance must hold at most `MAX_COMPONENT_NODES` (65 536) JSON
    /// nodes and `MAX_COMPONENT_CALLS` (1 024) calls (`@call` objects, nested ones included), then
    /// nest at most `MAX_JSON_DEPTH` (64) levels, so one component runs a bounded number of
    /// validators. These caps run on a value already parsed: a caller parsing untrusted text
    /// must cap its size first, as [`Catalog::parse`] does for a catalog.
    ///
    /// # Errors
    ///
    /// The first check the component fails, its [`Error::path`] a JSON Pointer into it.
    pub fn validate_component(
        &self,
        component: &Value,
        default_catalog: &str,
        catalogs: &(impl CatalogLookup + ?Sized),
    ) -> Result<(), Error> {
        let at = Located {
            default_catalog: Some(default_catalog),
            // Unread: a call always finds `default_catalog`.
            surface_id: "",
            catalogs,
            report: Report::First,
            calls: CallCatalogs::IdentifiersOnly,
            allow_unknown: false,
        };
        self.validate_located(component, &at)
    }

    /// Validates one component on the surface `at` describes; paths are relative to it.
    pub(crate) fn validate_located<L: CatalogLookup + ?Sized>(
        &self,
        component: &Value,
        at: &Located<'_, L>,
    ) -> Result<(), Error> {
        bounded(component)?;
        let map = component.as_object().with_context(|| ComponentShapeSnafu {
            reason: "a component is a JSON object",
        });
        let map = map.map_err(|error| Error::from(error).with_path("".into()))?;
        for key in map.keys() {
            unreserved(key).map_err(|error| error.with_path(pointer(std::slice::from_ref(key))))?;
        }
        let kind = map.get("component").and_then(Value::as_str);
        let kind = kind
            .context(MissingComponentTypeSnafu)
            .map_err(|error| Error::from(error).with_path("/component".into()))?;
        identifier("component type", kind).map_err(|error| error.with_path("/component".into()))?;
        let def = self.components.get(kind);
        if def.is_none() && !at.allow_unknown {
            let error = Error::from(UnknownComponentSnafu { component: kind }.build());
            return Err(error.with_path("/component".into()));
        }
        let mut instance = map.clone();
        for key in ENVELOPE {
            if def.is_some_and(|def| def.shape().properties().contains(key)) {
                continue;
            }
            envelope(key, instance.remove(key))
                .map_err(|error| error.with_path(format!("/{key}")))?;
        }
        if let Some(def) = def {
            check_schema(def.validator(), &Value::Object(instance), kind, at.report)?;
        }
        let walk = Walk { home: self, at };
        let mut tokens = Vec::new();
        for (key, value) in map
            .iter()
            .filter(|(key, _)| !ENVELOPE.contains(&key.as_str()))
        {
            tokens.push(key.clone());
            walk.node(value, &mut tokens)?;
            tokens.pop();
        }
        Ok(())
    }
}

/// Checks that `component` holds at most `MAX_COMPONENT_NODES` nodes and `MAX_COMPONENT_CALLS`
/// calls, then nests at most `MAX_JSON_DEPTH` levels; errors point at the component.
pub(crate) fn bounded(component: &Value) -> Result<(), Error> {
    if let Some(oversize) = component_oversize(component) {
        let (what, limit) = match oversize {
            ComponentOversize::Nodes => ("JSON nodes", MAX_COMPONENT_NODES),
            ComponentOversize::Calls => ("function calls", MAX_COMPONENT_CALLS),
        };
        return Err(Error::from(ComponentLimitSnafu { what, limit }.build()).with_path("".into()));
    }
    if deeper_than(component, MAX_JSON_DEPTH) {
        let limit = MAX_JSON_DEPTH;
        return Err(Error::from(ValueLimitSnafu { limit }.build()).with_path("".into()));
    }
    Ok(())
}

/// Checks the envelope key `key`, holding `value`, which the component's schema leaves out.
fn envelope(key: &'static str, value: Option<Value>) -> Result<(), Error> {
    match (key, value) {
        ("id" | "catalogId", Some(value)) if !value.is_string() => Err(ComponentShapeSnafu {
            reason: reason![key, " must be a string"],
        }
        .build()
        .into()),
        ("metadata", Some(value)) => validate_component_metadata(&value),
        _ => Ok(()),
    }
}

/// Checks `instance`, a component of type `kind`, against `validator`, reporting as `report`
/// says.
fn check_schema(
    validator: &jsonschema::Validator,
    instance: &Value,
    kind: &str,
    report: Report<'_>,
) -> Result<(), Error> {
    let take = match report {
        Report::First => 1,
        Report::All { .. } => MAX_VIOLATION_DETAILS + 1,
    };
    let violations: Vec<_> = validator.iter_errors(instance).take(take).collect();
    let Some(first) = violations.first() else {
        return Ok(());
    };
    let (code, path) = violation_detail(first);
    let error = violation_error(first).with_path(path.clone());
    match (report, violations.len()) {
        (Report::First, _) => Err(error),
        (Report::All { .. }, 1) => Err(error.with_detail(Detail::new(path, code))),
        (Report::All { id }, count) => {
            let failure = ProcessFailure::SchemaViolations {
                id: id.into(),
                kind: kind.into(),
                count,
                first: Quoted::new(&error.to_string()),
            };
            let mut error = Error::from(failure).with_path(String::new());
            for violation in violations.iter().take(MAX_VIOLATION_DETAILS) {
                let (code, path) = violation_detail(violation);
                error = error.with_detail(Detail::new(path, code));
            }
            Err(error)
        }
    }
}

/// The nested-call walk of one component: where its calls run and which catalogs are known.
struct Walk<'a, L: ?Sized> {
    home: &'a Catalog,
    at: &'a Located<'a, L>,
}

impl<'a, L: CatalogLookup + ?Sized> Walk<'a, L> {
    /// Checks reserved keys and calls under `node`, at `tokens`; it is depth-checked, so this
    /// recurses.
    fn node(&self, node: &Value, tokens: &mut Vec<String>) -> Result<(), Error> {
        match node {
            Value::Object(map) => {
                for key in map.keys() {
                    unreserved(key).map_err(|error| error.with_path(pointer(tokens)))?;
                }
                if map.contains_key(CALL_KEY) {
                    self.call(node, map, tokens)?;
                }
                for (key, value) in map {
                    tokens.push(key.clone());
                    self.node(value, tokens)?;
                    tokens.pop();
                }
                Ok(())
            }
            Value::Array(items) => {
                for (index, item) in items.iter().enumerate() {
                    tokens.push(index.to_string());
                    self.node(item, tokens)?;
                    tokens.pop();
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    /// The catalog whose id is `id`: the component's own, else one the lookup finds.
    fn find(&self, id: &str) -> Option<&'a Catalog> {
        if self.home.id == id {
            Some(self.home)
        } else {
            self.at.catalogs.catalog(id)
        }
    }

    /// Checks the call `node`, at `tokens`, whose entries are `call`, where it runs.
    fn call(
        &self,
        node: &Value,
        call: &Map<String, Value>,
        tokens: &[String],
    ) -> Result<(), Error> {
        let here = pointer(tokens);
        let below = |key: &str| format!("{here}/{key}");
        let name = call.get(CALL_KEY).and_then(Value::as_str);
        let name = name
            .context(CallShapeSnafu {
                reason: "@call must be a string",
            })
            .map_err(|error| Error::from(error).with_path(below(CALL_KEY)))?;
        let named = call
            .get("catalogId")
            .map(|id| {
                id.as_str().with_context(|| CallShapeSnafu {
                    reason: reason!["catalogId of '", Quoted::new(name), "' must be a string"],
                })
            })
            .transpose()
            .map_err(|error| Error::from(error).with_path(below("catalogId")))?;
        if let Some(args) = call.get("args") {
            let args = args.as_object().with_context(|| CallShapeSnafu {
                reason: reason!["args of '", Quoted::new(name), "' must be an object"],
            });
            let args = args.map_err(|error| Error::from(error).with_path(below("args")))?;
            for argument in args.keys() {
                identifier("argument", argument).map_err(|error| error.with_path(below("args")))?;
            }
        }
        self.function(node, call, name, named)
            .map_err(|error| match error.path() {
                Some(_) => error.prefix_path(&here),
                None => error.with_path(here.clone()),
            })
    }

    /// Checks the call `node` to `name`, in catalog `named` else the surface default.
    ///
    /// An error without a path is the call's own; one with a path is relative to the call.
    fn function(
        &self,
        node: &Value,
        call: &Map<String, Value>,
        name: &str,
        named: Option<&str>,
    ) -> Result<(), Error> {
        if name.starts_with('@') {
            ensure!(
                named.is_none(),
                SystemFunctionCatalogSnafu { function: name }
            );
            ensure!(name == "@index", UnknownFunctionSnafu { function: name });
            return validate_common_type("IndexSystemFunction", node);
        }
        identifier("function", name)?;
        let Some(id) = named.or(self.at.default_catalog) else {
            return Err(ProcessFailure::NoDefaultCatalog {
                surface: self.at.surface_id.into(),
                what: "function",
                name: name.into(),
            }
            .into());
        };
        let Some(catalog) = self.find(id) else {
            return match self.at.calls {
                CallCatalogs::IdentifiersOnly => Ok(()),
                CallCatalogs::Resolve => {
                    Err(ProcessFailure::CatalogNotFound { id: id.into() }.into())
                }
            };
        };
        let Some(function) = catalog.functions.get(name) else {
            ensure!(
                self.at.allow_unknown,
                UnknownFunctionSnafu { function: name }
            );
            return Ok(());
        };
        ensure!(
            function.allowed_callers() != AllowedCallers::AgentOnly,
            AgentOnlyCallSnafu { function: name }
        );
        let mut instance = call.clone();
        if named.is_some() {
            instance.remove("catalogId");
        }
        let instance = Value::Object(instance);
        let Some(violation) = function.validator().iter_errors(&instance).next() else {
            return Ok(());
        };
        let (_, path) = violation_detail(&violation);
        Err(violation_error(&violation).with_path(path))
    }
}

/// Checks that `name`, a `what` such as `"function"`, is a UAX #31 identifier.
pub(crate) fn identifier(what: &'static str, name: &str) -> Result<(), Error> {
    ensure!(is_identifier(name), BadIdentifierSnafu { what, name });
    Ok(())
}

/// Rejects a key starting with one `@` other than `@path` and `@call`, bare `@` included.
fn unreserved(key: &str) -> Result<(), Error> {
    let reserved =
        key.starts_with('@') && !key.starts_with("@@") && key != PATH_KEY && key != CALL_KEY;
    ensure!(!reserved, ReservedKeySnafu { key });
    Ok(())
}

#[cfg(test)]
#[path = "validate_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "validate_located_tests.rs"]
mod located_tests;
