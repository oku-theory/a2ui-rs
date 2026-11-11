//! Validating a component instance against a catalog, nested function calls included.

use serde_json::{Map, Value};
use snafu::{OptionExt, ensure};

use super::{AllowedCallers, Catalog};
use crate::error::{
    AgentOnlyCallSnafu, BadIdentifierSnafu, CallShapeSnafu, ComponentShapeSnafu, Error,
    MissingComponentTypeSnafu, ReservedKeySnafu, SystemFunctionCatalogSnafu, UnknownComponentSnafu,
    UnknownFunctionSnafu, ValueLimitSnafu,
};
use crate::ident::is_identifier;
use crate::limits::{MAX_JSON_DEPTH, deeper_than};
use crate::schema::{validate_common_type, validate_component_metadata};
use crate::validator::check;

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

impl Catalog {
    /// Validates one component instance, such as `{"id": "t1", "component": "Text"}`.
    ///
    /// A nested call runs in the catalog its `catalogId` names, else in `default_catalog` (the
    /// surface's). When that catalog is this one or `catalogs` finds it, the call is checked
    /// against its function there: the function exists, is not `agentOnly`, and the call
    /// matches its schema. A call into a catalog not found is checked only for its identifiers.
    pub fn validate_component(
        &self,
        component: &Value,
        default_catalog: &str,
        catalogs: &(impl CatalogLookup + ?Sized),
    ) -> Result<(), Error> {
        ensure!(
            !deeper_than(component, MAX_JSON_DEPTH),
            ValueLimitSnafu {
                limit: MAX_JSON_DEPTH
            }
        );
        let map = component.as_object().context(ComponentShapeSnafu {
            reason: "a component is a JSON object",
        })?;
        map.keys().map(String::as_str).try_for_each(unreserved)?;
        let kind = map
            .get("component")
            .and_then(Value::as_str)
            .context(MissingComponentTypeSnafu)?;
        identifier("component type", kind)?;
        let def = self
            .components
            .get(kind)
            .context(UnknownComponentSnafu { component: kind })?;
        let mut instance = map.clone();
        for key in ENVELOPE {
            if def.shape().properties().contains(key) {
                continue;
            }
            match (key, instance.remove(key)) {
                ("id" | "catalogId", Some(value)) => ensure!(
                    value.is_string(),
                    ComponentShapeSnafu {
                        reason: format!("{key} must be a string")
                    }
                ),
                ("metadata", Some(value)) => validate_component_metadata(&value)?,
                _ => {}
            }
        }
        check(def.validator(), &Value::Object(instance))?;
        let walk = Walk {
            home: self,
            default_catalog,
            catalogs,
        };
        map.iter()
            .filter(|(key, _)| !ENVELOPE.contains(&key.as_str()))
            .try_for_each(|(_, value)| walk.node(value))
    }
}

/// The nested-call walk of one component: where its calls run and which catalogs are known.
struct Walk<'a, L: ?Sized> {
    home: &'a Catalog,
    default_catalog: &'a str,
    catalogs: &'a L,
}

impl<'a, L: CatalogLookup + ?Sized> Walk<'a, L> {
    /// Checks reserved keys and calls under `node`; it is depth-checked, so this recurses.
    fn node(&self, node: &Value) -> Result<(), Error> {
        match node {
            Value::Object(map) => {
                map.keys().map(String::as_str).try_for_each(unreserved)?;
                if map.contains_key("@call") {
                    self.call(node, map)?;
                }
                map.values().try_for_each(|value| self.node(value))
            }
            Value::Array(items) => items.iter().try_for_each(|item| self.node(item)),
            _ => Ok(()),
        }
    }

    /// The catalog whose id is `id`: the component's own, else one `catalogs` finds.
    fn find(&self, id: &str) -> Option<&'a Catalog> {
        if self.home.id == id {
            Some(self.home)
        } else {
            self.catalogs.catalog(id)
        }
    }

    /// Checks the call `node`, whose entries are `call`, in the catalog it runs in.
    fn call(&self, node: &Value, call: &Map<String, Value>) -> Result<(), Error> {
        let name = call
            .get("@call")
            .and_then(Value::as_str)
            .context(CallShapeSnafu {
                reason: "@call must be a string",
            })?;
        let named = call
            .get("catalogId")
            .map(|id| {
                id.as_str().with_context(|| CallShapeSnafu {
                    reason: format!("catalogId of '{name}' must be a string"),
                })
            })
            .transpose()?;
        if let Some(args) = call.get("args") {
            let args = args.as_object().with_context(|| CallShapeSnafu {
                reason: format!("args of '{name}' must be an object"),
            })?;
            args.keys()
                .try_for_each(|argument| identifier("argument", argument))?;
        }
        if name.starts_with('@') {
            ensure!(
                named.is_none(),
                SystemFunctionCatalogSnafu { function: name }
            );
            ensure!(name == "@index", UnknownFunctionSnafu { function: name });
            return validate_common_type("IndexSystemFunction", node);
        }
        identifier("function", name)?;
        let Some(catalog) = self.find(named.unwrap_or(self.default_catalog)) else {
            return Ok(());
        };
        let function = catalog
            .functions
            .get(name)
            .context(UnknownFunctionSnafu { function: name })?;
        ensure!(
            function.allowed_callers() != AllowedCallers::AgentOnly,
            AgentOnlyCallSnafu { function: name }
        );
        if named.is_none() {
            return check(function.validator(), node);
        }
        let mut instance = call.clone();
        instance.remove("catalogId");
        check(function.validator(), &Value::Object(instance))
    }
}

/// Checks that `name`, a `what` such as `"function"`, is a UAX #31 identifier.
fn identifier(what: &'static str, name: &str) -> Result<(), Error> {
    ensure!(is_identifier(name), BadIdentifierSnafu { what, name });
    Ok(())
}

/// Rejects a key starting with one `@` other than `@path` and `@call`, bare `@` included.
fn unreserved(key: &str) -> Result<(), Error> {
    let reserved =
        key.starts_with('@') && !key.starts_with("@@") && key != "@path" && key != "@call";
    ensure!(!reserved, ReservedKeySnafu { key });
    Ok(())
}

#[cfg(test)]
#[path = "validate_tests.rs"]
mod tests;
