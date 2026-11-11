//! v1.0 catalogs: the components and functions an agent may use on a surface.
//!
//! [`Catalog::from_json`] checks an authored catalog document, keeps it for
//! [`Catalog::to_json`], bundles the self-contained [`Catalog::validation_schema`] and compiles
//! one validator per entry, which its [`ComponentDef`] or [`FunctionDef`] holds.

mod compile;
mod document;
mod entries;
mod load;
mod shape;
#[cfg(test)]
pub(crate) mod test_support;

use std::collections::BTreeMap;
use std::sync::Arc;

use jsonschema::Validator;
use serde_json::{Map, Value};

pub use shape::ComponentShape;

/// Who may call a catalog function.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum AllowedCallers {
    /// Only the renderer, from component bindings and actions (the default).
    RendererOnly,
    /// Only the agent, through a function call message.
    AgentOnly,
    /// Both the renderer and the agent.
    RendererOrAgent,
}

impl AllowedCallers {
    /// The catalog spelling, such as `"rendererOnly"`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::RendererOnly => "rendererOnly",
            Self::AgentOnly => "agentOnly",
            Self::RendererOrAgent => "rendererOrAgent",
        }
    }

    /// The value a catalog's `allowedCallers` spells as `text`, if it is one.
    pub(crate) fn from_catalog_str(text: &str) -> Option<Self> {
        match text {
            "rendererOnly" => Some(Self::RendererOnly),
            "agentOnly" => Some(Self::AgentOnly),
            "rendererOrAgent" => Some(Self::RendererOrAgent),
            _ => None,
        }
    }
}

/// One component definition of a catalog.
#[derive(Debug, Clone)]
pub struct ComponentDef {
    name: String,
    schema: Value,
    allowed_parents: Option<Vec<String>>,
    allowed_children: Option<Vec<String>>,
    metadata: Option<Value>,
    shape: ComponentShape,
    validator: Arc<Validator>,
}

impl ComponentDef {
    /// The component type name, such as `Button`.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The schema as authored in the catalog document.
    pub fn schema(&self) -> &Value {
        &self.schema
    }

    /// The component types this one may be placed under, when restricted.
    pub fn allowed_parents(&self) -> Option<&[String]> {
        self.allowed_parents.as_deref()
    }

    /// The component types this one may hold, when restricted.
    pub fn allowed_children(&self) -> Option<&[String]> {
        self.allowed_children.as_deref()
    }

    /// The definition's `metadata` envelope (vendor `extensions`), if any.
    pub fn metadata(&self) -> Option<&Value> {
        self.metadata.as_ref()
    }

    /// The property and required names, through `allOf` and `$ref`s.
    pub fn shape(&self) -> &ComponentShape {
        &self.shape
    }

    /// The compiled schema, with function calls checked only as envelopes.
    #[expect(dead_code, reason = "validate_component (Task 5) reads it")]
    pub(crate) fn validator(&self) -> &Validator {
        &self.validator
    }
}

/// One function definition of a catalog.
#[derive(Debug, Clone)]
pub struct FunctionDef {
    name: String,
    schema: Value,
    return_type: Option<String>,
    allowed_callers: AllowedCallers,
    requires_user_activation: bool,
    validator: Arc<Validator>,
}

impl FunctionDef {
    /// The function name, such as `formatString`.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The schema as authored in the catalog document.
    pub fn schema(&self) -> &Value {
        &self.schema
    }

    /// The declared return type, such as `string` or `void`.
    pub fn return_type(&self) -> Option<&str> {
        self.return_type.as_deref()
    }

    /// Who may call the function.
    pub fn allowed_callers(&self) -> AllowedCallers {
        self.allowed_callers
    }

    /// Whether a call must come from a user gesture.
    pub fn requires_user_activation(&self) -> bool {
        self.requires_user_activation
    }

    /// The compiled schema of a call, with nested calls checked only as envelopes.
    #[expect(dead_code, reason = "validate_component (Task 5) reads it")]
    pub(crate) fn validator(&self) -> &Validator {
        &self.validator
    }
}

/// A loaded v1.0 catalog; see the module documentation.
#[derive(Debug)]
pub struct Catalog {
    id: String,
    protocol_version: String,
    document: Map<String, Value>,
    components: BTreeMap<String, ComponentDef>,
    functions: BTreeMap<String, FunctionDef>,
    bundle: Value,
}

impl Catalog {
    /// The catalog id, such as the basic catalog's URL.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// The protocol version the catalog targets, `1.0` or a `1.0.N` patch.
    pub fn protocol_version(&self) -> &str {
        &self.protocol_version
    }

    /// The component definition named `name`.
    pub fn component(&self, name: &str) -> Option<&ComponentDef> {
        self.components.get(name)
    }

    /// Every component definition, by name.
    pub fn components(&self) -> impl Iterator<Item = &ComponentDef> {
        self.components.values()
    }

    /// The function definition named `name`.
    pub fn function(&self, name: &str) -> Option<&FunctionDef> {
        self.functions.get(name)
    }

    /// Every function definition, by name.
    pub fn functions(&self) -> impl Iterator<Item = &FunctionDef> {
        self.functions.values()
    }

    /// The `theme` schema of the document, if any.
    pub fn theme(&self) -> Option<&Value> {
        self.document.get("theme")
    }

    /// The `instructions` for agents, if any.
    pub fn instructions(&self) -> Option<&str> {
        self.document.get("instructions").and_then(Value::as_str)
    }

    /// The self-contained schema: kept entries, `$defs` and the common types they reach.
    ///
    /// Until Plan 2b it is the unflattened bundle, not upstream's `validation_schema` golden.
    pub fn validation_schema(&self) -> &Value {
        &self.bundle
    }
}
