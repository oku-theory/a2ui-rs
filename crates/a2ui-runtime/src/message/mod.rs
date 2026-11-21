//! A2UI v1.0 agent-to-renderer messages, read into typed values.
//!
//! The vendored JSON Schema is the wire contract: a message passes the envelope prechecks, then
//! the schema branch of its action, and only then is read by hand into an [`AgentMessage`]. The
//! runtime keeps no serde dependency. Components stay raw JSON for a catalog to check.

mod envelope;
mod schema;
mod typed;

use serde_json::{Map, Value};

use crate::error::Error;
use crate::pointer::Pointer;

/// One message an agent sends a renderer, read from JSON.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum AgentMessage {
    /// Creates a surface, optionally with its components and data model.
    CreateSurface(CreateSurface),
    /// Adds or replaces the components of a surface.
    UpdateComponents(UpdateComponents),
    /// Writes a value into a surface's data model.
    UpdateDataModel(UpdateDataModel),
    /// Deletes a surface.
    DeleteSurface(DeleteSurface),
    /// Asks the renderer to run a function and answer.
    CallRendererFunction(CallRendererFunction),
    /// The answer to a function the renderer asked the agent to run.
    AgentFunctionResponse(AgentFunctionResponse),
}

/// The body of a `createSurface` message.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct CreateSurface {
    /// The surface's id.
    pub surface_id: String,
    /// The surface's default catalog id, if the message names one.
    pub catalog_id: Option<String>,
    /// Whether the renderer sends the surface's data model back to the agent.
    pub send_data_model: bool,
    /// The initial components, raw, empty when the message has none.
    pub components: Vec<Value>,
    /// The initial data model, if the message has one.
    pub data_model: Option<Map<String, Value>>,
    /// The surface's metadata, if the message has any.
    pub metadata: Option<Map<String, Value>>,
}

/// The body of an `updateComponents` message.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct UpdateComponents {
    /// The surface to update.
    pub surface_id: String,
    /// The components, raw.
    pub components: Vec<Value>,
}

/// The body of an `updateDataModel` message.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct UpdateDataModel {
    /// The surface to update.
    pub surface_id: String,
    /// Where to write; the root when the message has no `path`.
    pub path: Pointer,
    /// The value to write; `null` deletes.
    pub value: Value,
}

/// The body of a `deleteSurface` message.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct DeleteSurface {
    /// The surface to delete.
    pub surface_id: String,
}

/// The body of a `callRendererFunction` message.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct CallRendererFunction {
    /// The id the answer must echo.
    pub function_call_id: String,
    /// The surface whose data model is the call's context, if any.
    pub surface_id: Option<String>,
    /// The function call, raw.
    pub call_function: Map<String, Value>,
}

/// The body of an `agentFunctionResponse` message.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct AgentFunctionResponse {
    /// The response, raw; a later plan types it.
    pub body: Map<String, Value>,
}

impl AgentMessage {
    /// Reads one message from JSON: envelope, then schema, then typed fields.
    ///
    /// # Errors
    ///
    /// A [`Category::Validation`](crate::Category) error naming the first fault; the error's
    /// path, ids and details say where.
    pub fn from_json(value: &Value) -> Result<Self, Error> {
        let (action, body) = envelope::precheck(value)?;
        schema::check(action, value)?;
        typed::read(action, body)
    }
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
