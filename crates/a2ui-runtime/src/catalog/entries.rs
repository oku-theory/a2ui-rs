//! Component and function attributes read from a catalog document (load rules 6 to 8).

use std::sync::Arc;

use jsonschema::Validator;
use serde_json::{Map, Value};
use snafu::{OptionExt, ensure};

use super::document::{argument_names, property_names, return_type};
use super::{AllowedCallers, ChildSlot, ComponentDef, ComponentShape, FunctionDef};
use crate::error::{
    Error, InvalidIdentifierSnafu, MalformedCatalogSnafu, ReservedComponentSnafu,
    UserActivationSnafu,
};
use crate::ident::is_identifier;
use crate::quoted::{Quoted, Reason, reason};
use crate::schema::validate_component_metadata;

/// A checked component definition, before its shape and validator exist.
pub(super) struct ComponentAttrs {
    name: String,
    schema: Value,
    allowed_parents: Option<Vec<String>>,
    allowed_children: Option<Vec<String>>,
    metadata: Option<Value>,
    child_slots: Vec<ChildSlot>,
}

impl ComponentAttrs {
    /// Checks and reads the component `name` of the document.
    pub(super) fn read(name: &str, schema: &Value) -> Result<Self, Error> {
        ensure!(name != "Surface", ReservedComponentSnafu);
        check_identifier("component", name, None)?;
        let map = entry("component", name, schema)?;
        for property in property_names(map) {
            check_identifier("property", &property, Some(("component", name)))?;
        }
        let metadata = map.get("metadata").cloned();
        if let Some(metadata) = &metadata {
            validate_component_metadata(metadata)?;
        }
        Ok(Self {
            name: name.to_owned(),
            schema: schema.clone(),
            allowed_parents: names_list(map, "allowedParents", name)?,
            allowed_children: names_list(map, "allowedChildren", name)?,
            metadata,
            child_slots: Vec::new(),
        })
    }

    /// The component type name.
    pub(super) fn name(&self) -> &str {
        &self.name
    }

    /// The schema as authored.
    pub(super) fn schema(&self) -> &Value {
        &self.schema
    }

    /// Records where the schema holds child component ids.
    pub(super) fn set_child_slots(&mut self, slots: Vec<ChildSlot>) {
        self.child_slots = slots;
    }

    /// The definition, completed with its shape and validator.
    pub(super) fn into_def(self, shape: ComponentShape, validator: Arc<Validator>) -> ComponentDef {
        ComponentDef {
            name: self.name,
            schema: self.schema,
            allowed_parents: self.allowed_parents,
            allowed_children: self.allowed_children,
            metadata: self.metadata,
            shape,
            child_slots: self.child_slots,
            validator,
        }
    }
}

/// A checked function definition, before its validator exists.
pub(super) struct FunctionAttrs {
    name: String,
    schema: Value,
    return_type: Option<String>,
    allowed_callers: AllowedCallers,
    requires_user_activation: bool,
}

impl FunctionAttrs {
    /// Checks and reads the function `name` of the document.
    pub(super) fn read(name: &str, schema: &Value) -> Result<Self, Error> {
        check_identifier("function", name, None)?;
        let map = entry("function", name, schema)?;
        for argument in argument_names(map) {
            check_identifier("argument", &argument, Some(("function", name)))?;
        }
        let allowed_callers = match map.get("allowedCallers") {
            None => AllowedCallers::RendererOnly,
            Some(value) => value
                .as_str()
                .and_then(AllowedCallers::from_catalog_str)
                .with_context(|| MalformedCatalogSnafu {
                    reason: reason![
                        "function '",
                        Quoted::new(name),
                        "' has an unknown allowedCallers",
                    ],
                })?,
        };
        let requires_user_activation = match map.get("requiresUserActivation") {
            None => false,
            Some(value) => value.as_bool().with_context(|| MalformedCatalogSnafu {
                reason: reason![
                    "requiresUserActivation of function '",
                    Quoted::new(name),
                    "' must be a boolean",
                ],
            })?,
        };
        ensure!(
            !requires_user_activation || allowed_callers == AllowedCallers::RendererOnly,
            UserActivationSnafu { function: name }
        );
        Ok(Self {
            name: name.to_owned(),
            schema: schema.clone(),
            return_type: return_type(map),
            allowed_callers,
            requires_user_activation,
        })
    }

    /// The function name.
    pub(super) fn name(&self) -> &str {
        &self.name
    }

    /// The schema as authored.
    pub(super) fn schema(&self) -> &Value {
        &self.schema
    }

    /// The definition, completed with its validator.
    pub(super) fn into_def(self, validator: Arc<Validator>) -> FunctionDef {
        FunctionDef {
            name: self.name,
            schema: self.schema,
            return_type: self.return_type,
            allowed_callers: self.allowed_callers,
            requires_user_activation: self.requires_user_activation,
            validator,
        }
    }
}

/// Checks `name` is a UAX #31 identifier; `owner` is the `(kind, name)` declaring it.
///
/// `ensure!` builds the context only on failure, so a valid name formats nothing.
fn check_identifier(
    kind: &'static str,
    name: &str,
    owner: Option<(&'static str, &str)>,
) -> Result<(), Error> {
    ensure!(
        is_identifier(name),
        InvalidIdentifierSnafu {
            reason: identifier_reason(kind, name, owner)
        }
    );
    Ok(())
}

/// The wording of an invalid identifier, naming its owner when it has one.
fn identifier_reason(
    kind: &'static str,
    name: &str,
    owner: Option<(&'static str, &str)>,
) -> Reason {
    match owner {
        Some((owner_kind, owner)) => reason![
            "Invalid UAX #31 ",
            kind,
            " identifier: '",
            Quoted::new(name),
            "' in ",
            owner_kind,
            " '",
            Quoted::new(owner),
            "'"
        ],
        None => reason![
            "Invalid UAX #31 ",
            kind,
            " identifier: '",
            Quoted::new(name),
            "'"
        ],
    }
}

fn entry<'a>(
    kind: &'static str,
    name: &str,
    schema: &'a Value,
) -> Result<&'a Map<String, Value>, Error> {
    Ok(schema.as_object().with_context(|| MalformedCatalogSnafu {
        reason: reason![kind, " '", Quoted::new(name), "' must be a schema object"],
    })?)
}

fn names_list(
    map: &Map<String, Value>,
    key: &'static str,
    component: &str,
) -> Result<Option<Vec<String>>, Error> {
    let Some(value) = map.get(key) else {
        return Ok(None);
    };
    let names = value
        .as_array()
        .and_then(|items| {
            items
                .iter()
                .map(|item| item.as_str().map(str::to_owned))
                .collect::<Option<Vec<_>>>()
        })
        .with_context(|| MalformedCatalogSnafu {
            reason: reason![
                "`",
                key,
                "` of component '",
                Quoted::new(component),
                "' must be a list of names",
            ],
        })?;
    Ok(Some(names))
}

#[cfg(test)]
#[path = "entries_tests.rs"]
mod tests;
