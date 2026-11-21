//! Checks a message against the branch of `agent_to_renderer.json` its action names.
//!
//! The top-level `oneOf` reports useless errors and the action key already names the branch, so
//! each action has its own validator. Components stay opaque here: a catalog checks them.

use std::sync::LazyLock;

use jsonschema::Validator;
use serde_json::{Map, Value, json};

use super::envelope::Action;
use crate::bundle::{local_ref, localize_refs};
use crate::detail::Detail;
use crate::error::{EmbeddedSchemaSnafu, Error, FunctionCallShapeSnafu, MessageFieldSnafu};
use crate::quoted::{Quoted, reason};
use crate::schema::{agent_to_renderer_schema, common_document};
use crate::validator::{Fault, FaultKind, build, fault, pointer, rooted};

/// The `agent_to_renderer.json` definitions and the common types, as one schema document.
type Document = Result<Map<String, Value>, String>;

static DOCUMENT: LazyLock<Document> = LazyLock::new(document);

/// One validator per action, in `Action::ALL` order (`validator_for` indexes them).
static VALIDATORS: [LazyLock<Result<Validator, String>>; 6] = [
    LazyLock::new(|| validator(Action::CreateSurface)),
    LazyLock::new(|| validator(Action::UpdateComponents)),
    LazyLock::new(|| validator(Action::UpdateDataModel)),
    LazyLock::new(|| validator(Action::DeleteSurface)),
    LazyLock::new(|| validator(Action::CallRendererFunction)),
    LazyLock::new(|| validator(Action::AgentFunctionResponse)),
];

fn validator_for(action: Action) -> &'static LazyLock<Result<Validator, String>> {
    match action {
        Action::CreateSurface => &VALIDATORS[0],
        Action::UpdateComponents => &VALIDATORS[1],
        Action::UpdateDataModel => &VALIDATORS[2],
        Action::DeleteSurface => &VALIDATORS[3],
        Action::CallRendererFunction => &VALIDATORS[4],
        Action::AgentFunctionResponse => &VALIDATORS[5],
    }
}

fn document() -> Document {
    let mut top = common_document().map_err(|err| err.to_string())?;
    let schema = agent_to_renderer_schema().map_err(|err| err.to_string())?;
    let own = schema
        .get("$defs")
        .and_then(Value::as_object)
        .ok_or("it has no $defs object")?;
    let defs = top
        .get_mut("$defs")
        .and_then(Value::as_object_mut)
        .ok_or("the common types have no $defs object")?;
    for (name, def) in own {
        let mut def = def.clone();
        localize_refs(&mut def);
        defs.insert(name.clone(), def);
    }
    defs.insert("Component".to_owned(), json!({"type": "object"}));
    Ok(top)
}

fn validator(action: Action) -> Result<Validator, String> {
    let top = DOCUMENT.as_ref().map_err(Clone::clone)?;
    build(&rooted(
        top,
        &local_ref(&format!("/$defs/{}", action.def())),
    ))
}

fn embedded_error(message: &str) -> Error {
    EmbeddedSchemaSnafu {
        name: "agent_to_renderer.json",
        message: message.to_owned(),
    }
    .build()
    .into()
}

/// Checks `message` against `action`'s branch; the first violation becomes an [`Error`].
///
/// A `callRendererFunction` fault carries its `functionCallId`, any other the `surfaceId`: for
/// a surface message naming none (or a non-string one), the empty id.
pub(crate) fn check(action: Action, message: &Value) -> Result<(), Error> {
    let validator = validator_for(action)
        .as_ref()
        .map_err(|message| embedded_error(message))?;
    let Some(violation) = validator.iter_errors(message).next() else {
        return Ok(());
    };
    Err(refusal(
        action,
        fault(&violation),
        message.get(action.key()),
    ))
}

/// The error for `fault` in `action`'s message, whose body is `body`.
fn refusal(action: Action, fault: Fault, body: Option<&Value>) -> Error {
    let nested = !fault.tokens.is_empty();
    let held = if nested {
        dotted(&fault.tokens)
    } else {
        action.key().to_owned()
    };
    let held = Quoted::new(&held);
    let reason = match &fault.kind {
        FaultKind::Missing(name) => {
            reason![held, " is missing required field '", Quoted::new(name), "'"]
        }
        FaultKind::Extra(name) if !nested => reason![
            "Message has unexpected top-level field '",
            Quoted::new(name),
            "'"
        ],
        FaultKind::Extra(name) => reason![held, " has unexpected field '", Quoted::new(name), "'"],
        FaultKind::Type(Some(expected)) => reason![held, " must be ", *expected],
        FaultKind::Type(None) | FaultKind::Other => reason![held, " is invalid"],
    };
    let (code, tokens) = fault.located();
    let id = |key: &str| body.and_then(|body| body.get(key)).and_then(Value::as_str);
    let mut error: Error = if action == Action::CallRendererFunction {
        FunctionCallShapeSnafu { reason }.build().into()
    } else {
        MessageFieldSnafu { reason }.build().into()
    };
    error = error.with_detail(Detail::new(dotted(&tokens), code));
    if nested && let Some((_, inside)) = tokens.split_first() {
        error = error.with_path(pointer(inside));
    }
    match action {
        Action::CallRendererFunction => {
            if let Some(call) = id("functionCallId") {
                error = error.with_function_call(call);
            }
        }
        // A surface message naming no string surface id reports the empty one, as upstream.
        Action::CreateSurface
        | Action::UpdateComponents
        | Action::UpdateDataModel
        | Action::DeleteSurface => error = error.with_surface(id("surfaceId").unwrap_or("")),
        Action::AgentFunctionResponse => {
            if let Some(surface) = id("surfaceId") {
                error = error.with_surface(surface);
            }
        }
    }
    error
}

fn dotted(tokens: &[String]) -> String {
    tokens.join(".")
}

#[cfg(test)]
#[path = "schema_tests.rs"]
mod tests;
