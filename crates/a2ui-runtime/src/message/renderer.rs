//! Messages a renderer sends an agent.

use serde_json::{Map, Value, json};

use crate::error::{Error, ErrorCode};

/// One message a renderer sends an agent.
#[derive(Debug, Clone, PartialEq)]
pub struct RendererMessage {
    value: Value,
}

impl RendererMessage {
    /// The `error` message reporting `error` to the agent.
    ///
    /// Its body follows the branch of `renderer_to_agent.json` for the error's code: a
    /// validation error always names `surfaceId` and `path` (empty when unknown); any other
    /// names `surfaceId`, else `functionCallId`, else an empty `surfaceId`.
    pub fn error(error: &Error) -> Self {
        let code = error.code();
        let mut body = Map::new();
        body.insert("code".to_owned(), json!(code.as_str()));
        body.insert("message".to_owned(), json!(error.to_string()));
        match code {
            ErrorCode::ValidationFailed => {
                body.insert(
                    "surfaceId".to_owned(),
                    json!(error.surface_id().unwrap_or("")),
                );
                body.insert("path".to_owned(), json!(error.path().unwrap_or("")));
            }
            ErrorCode::DataError
            | ErrorCode::ExpressionError
            | ErrorCode::CatalogError
            | ErrorCode::InvalidFunctionCall
            | ErrorCode::RecursionLimitExceeded
            | ErrorCode::IntegrityError
            | ErrorCode::CatalogNotFound => {
                match (error.surface_id(), error.function_call_id()) {
                    (Some(surface), _) => body.insert("surfaceId".to_owned(), json!(surface)),
                    (None, Some(call)) => body.insert("functionCallId".to_owned(), json!(call)),
                    (None, None) => body.insert("surfaceId".to_owned(), json!("")),
                };
            }
        }
        Self {
            value: json!({"version": "1.0", "error": body}),
        }
    }

    /// The message as wire JSON.
    pub fn to_json(&self) -> Value {
        self.value.clone()
    }
}

#[cfg(test)]
#[path = "renderer_tests.rs"]
mod tests;
