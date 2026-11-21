//! A2UI v1.0 agent-to-renderer messages, read into typed values.
//!
//! The vendored JSON Schema is the wire contract: a message passes the envelope prechecks, then
//! the schema branch of its action, and only then is read by hand into an [`AgentMessage`]. The
//! runtime keeps no serde dependency. Components stay raw JSON for a catalog to check.

mod envelope;
mod limits;
mod renderer;
mod schema;
mod typed;
mod values;

use serde_json::{Map, Value};

use crate::error::{Error, MessageEnvelopeSnafu, MessageJsonSnafu, MessageTooLargeSnafu};
use crate::pointer::Pointer;
use crate::quoted::{Quoted, reason};

pub use limits::MessageLimits;
pub use renderer::RendererMessage;

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
    /// Reads one message from JSON text: exact size, then syntax, then the stages of
    /// [`AgentMessage::from_json`] after its estimated size check.
    ///
    /// # Errors
    ///
    /// A validation error when the text exceeds `limits`; a parse error when it is not JSON;
    /// otherwise those of [`AgentMessage::from_json`].
    pub fn parse(text: &str, limits: &MessageLimits) -> Result<Self, Error> {
        if text.len() > limits.max_bytes() {
            return Err(too_large(limits));
        }
        let value: Value = serde_json::from_str(text).map_err(|e| {
            MessageJsonSnafu {
                message: Quoted::new(&e.to_string()),
            }
            .build()
        })?;
        Self::read(&value)
    }

    /// Reads one message from JSON: size, envelope, depth, schema, typed fields, then bindings.
    ///
    /// # Errors
    ///
    /// A [`Category::Validation`](crate::Category) error naming the first fault; the error's
    /// path, ids and details say where.
    pub fn from_json(value: &Value, limits: &MessageLimits) -> Result<Self, Error> {
        if limits::encoded_len_exceeds(value, limits.max_bytes()) {
            return Err(too_large(limits));
        }
        Self::read(value)
    }

    /// Envelope, depth, schema, typed fields and bindings: every stage after the size check.
    fn read(value: &Value) -> Result<Self, Error> {
        let (action, body) = envelope::precheck(value)?;
        if let Value::Object(map) = body {
            values::check_depth(action, map)?;
        }
        schema::check(action, value)?;
        let message = typed::read(action, body)?;
        values::check_bindings(&message)?;
        Ok(message)
    }

    /// Reads a batch: an array of messages, or an object whose only key is `messages`.
    ///
    /// # Errors
    ///
    /// A validation error when `value` has neither shape.
    pub fn batch<'a>(value: &'a Value, limits: &'a MessageLimits) -> Result<Batch<'a>, Error> {
        let messages = match value {
            Value::Array(items) => Some(items),
            Value::Object(entries) if entries.len() == 1 => {
                entries.get("messages").and_then(Value::as_array)
            }
            _ => None,
        };
        messages
            .map(|items| Batch {
                items: items.iter().enumerate(),
                limits,
            })
            .ok_or_else(|| {
                MessageEnvelopeSnafu {
                    reason: reason![
                        "A batch is an array of messages or an object with only 'messages', \
                         an array"
                    ],
                }
                .build()
                .into()
            })
    }
}

fn too_large(limits: &MessageLimits) -> Error {
    MessageTooLargeSnafu {
        limit: limits.max_bytes(),
    }
    .build()
    .into()
}

/// The messages of a batch, each read on its own: an error does not end the batch.
#[derive(Debug)]
pub struct Batch<'a> {
    items: std::iter::Enumerate<std::slice::Iter<'a, Value>>,
    limits: &'a MessageLimits,
}

impl Iterator for Batch<'_> {
    type Item = Result<AgentMessage, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        let (index, value) = self.items.next()?;
        Some(AgentMessage::from_json(value, self.limits).map_err(|e| e.at_message(index)))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.items.size_hint()
    }
}

impl ExactSizeIterator for Batch<'_> {}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
