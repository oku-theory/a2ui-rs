//! The envelope checks that run before the schema: object, one action, protocol version.

use serde_json::{Map, Value};

use crate::detail::{Detail, DetailCode};
use crate::error::{Error, MessageEnvelopeSnafu};
use crate::quoted::{Quoted, Reason, reason};

/// The protocol version this runtime implements.
const VERSION: &str = "1.0";

/// One of the six v1.0 agent-to-renderer actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Action {
    CreateSurface,
    UpdateComponents,
    UpdateDataModel,
    DeleteSurface,
    CallRendererFunction,
    AgentFunctionResponse,
}

impl Action {
    /// Every action, in the schema's order.
    pub(crate) const ALL: [Action; 6] = [
        Self::CreateSurface,
        Self::UpdateComponents,
        Self::UpdateDataModel,
        Self::DeleteSurface,
        Self::CallRendererFunction,
        Self::AgentFunctionResponse,
    ];

    /// The message key that names this action, such as `createSurface`.
    pub(crate) fn key(self) -> &'static str {
        match self {
            Self::CreateSurface => "createSurface",
            Self::UpdateComponents => "updateComponents",
            Self::UpdateDataModel => "updateDataModel",
            Self::DeleteSurface => "deleteSurface",
            Self::CallRendererFunction => "callRendererFunction",
            Self::AgentFunctionResponse => "agentFunctionResponse",
        }
    }

    /// The `$defs` entry of `agent_to_renderer.json` that describes this action's message.
    pub(crate) fn def(self) -> &'static str {
        match self {
            Self::CreateSurface => "CreateSurfaceMessage",
            Self::UpdateComponents => "UpdateComponentsMessage",
            Self::UpdateDataModel => "UpdateDataModelMessage",
            Self::DeleteSurface => "DeleteSurfaceMessage",
            Self::CallRendererFunction => "CallRendererFunctionMessage",
            Self::AgentFunctionResponse => "AgentFunctionResponseMessage",
        }
    }
}

fn refuse(reason: Reason) -> Error {
    MessageEnvelopeSnafu { reason }.build().into()
}

fn version_error(reason: Reason, code: DetailCode) -> Error {
    refuse(reason).with_detail(Detail::new("version".to_owned(), code))
}

/// Finds the message's action and its body, or the envelope fault that comes first.
///
/// The body is the action's value, which the schema has yet to check.
pub(crate) fn precheck(value: &Value) -> Result<(Action, &Value), Error> {
    let message = value
        .as_object()
        .ok_or_else(|| refuse(reason!["A2UI message must be a JSON object"]))?;
    let mut present = Action::ALL
        .into_iter()
        .filter_map(|action| message.get(action.key()).map(|body| (action, body)));
    let found = present.next();
    if present.next().is_some() {
        return Err(refuse(reason![
            "Message contains multiple conflicting update actions"
        ]));
    }
    check_version(message)?;
    found.ok_or_else(|| match message.keys().find(|key| *key != "version") {
        Some(key) => refuse(reason![
            "action '",
            Quoted::new(key),
            "' is not supported in protocol version 1.0"
        ]),
        None => refuse(reason!["Message has no action"]),
    })
}

fn check_version(message: &Map<String, Value>) -> Result<(), Error> {
    match message.get("version") {
        None => Err(version_error(
            reason!["Missing required version field"],
            DetailCode::MissingField,
        )),
        Some(Value::String(version)) if version == VERSION => Ok(()),
        Some(Value::String(version)) => Err(version_error(
            reason!["Unsupported protocol version '", Quoted::new(version), "'"],
            DetailCode::InvalidValue,
        )),
        Some(_) => Err(version_error(
            reason!["version must be a string"],
            DetailCode::TypeMismatch,
        )),
    }
}

#[cfg(test)]
#[path = "envelope_tests.rs"]
mod tests;
