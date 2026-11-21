//! Checks a message against the branch of `agent_to_renderer.json` its action names.
//!
//! The top-level `oneOf` reports useless errors and the action key already names the branch, so
//! each action has its own validator. Components stay opaque here: a catalog checks them.

use std::sync::LazyLock;

use jsonschema::error::{TypeKind, ValidationError, ValidationErrorKind};
use jsonschema::paths::LocationSegment;
use jsonschema::{JsonType, Validator};
use serde_json::{Map, Value, json};

use super::envelope::Action;
use crate::bundle::{local_ref, localize_refs};
use crate::detail::{Detail, DetailCode};
use crate::error::{EmbeddedSchemaSnafu, Error, FunctionCallShapeSnafu, MessageFieldSnafu};
use crate::pointer::escape_token;
use crate::quoted::{Quoted, reason};
use crate::schema::{agent_to_renderer_schema, common_document};
use crate::validator::{build, rooted};

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
/// A `callRendererFunction` fault carries its `functionCallId`, any other the `surfaceId`.
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

/// What one schema violation says, in the terms of the messages and details built from it.
struct Fault {
    /// The instance path's tokens, the action key first; the field's parent for a name below.
    tokens: Vec<String>,
    kind: FaultKind,
}

enum FaultKind {
    /// A required field is absent.
    Missing(String),
    /// A value has the wrong type, named with its article ("a string").
    Type(Option<&'static str>),
    /// A field the branch does not allow.
    Extra(String),
    /// Any other keyword.
    Other,
}

fn fault(error: &ValidationError<'_>) -> Fault {
    let kind = match error.kind() {
        ValidationErrorKind::OneOfNotValid { context } | ValidationErrorKind::AnyOf { context } => {
            // The branch that fails least is the one the author meant; a tie goes to the first.
            if let Some(first) = context
                .iter()
                .filter(|branch| !branch.is_empty())
                .min_by_key(|branch| branch.len())
                .and_then(|branch| branch.first())
            {
                return fault(first);
            }
            FaultKind::Other
        }
        ValidationErrorKind::Required { property } => FaultKind::Missing(
            property
                .as_str()
                .map_or_else(|| property.to_string(), str::to_owned),
        ),
        ValidationErrorKind::Type { kind } => FaultKind::Type(match kind {
            TypeKind::Single(single) => Some(article(*single)),
            _ => None,
        }),
        ValidationErrorKind::AdditionalProperties { unexpected } => unexpected
            .first()
            .map_or(FaultKind::Other, |name| FaultKind::Extra(name.clone())),
        _ => FaultKind::Other,
    };
    let tokens = error
        .instance_path()
        .into_iter()
        .map(|segment| match segment {
            LocationSegment::Property(name) => name.into_owned(),
            LocationSegment::Index(index) => index.to_string(),
        })
        .collect();
    Fault { tokens, kind }
}

fn article(kind: JsonType) -> &'static str {
    match kind {
        JsonType::Null => "null",
        JsonType::Boolean => "a boolean",
        JsonType::Integer => "an integer",
        JsonType::Number => "a number",
        JsonType::String => "a string",
        JsonType::Array => "an array",
        JsonType::Object => "an object",
    }
}

/// The error for `fault` in `action`'s message, whose body is `body`.
fn refusal(action: Action, fault: Fault, body: Option<&Value>) -> Error {
    let Fault { mut tokens, kind } = fault;
    let nested = !tokens.is_empty();
    let held = if nested {
        dotted(&tokens)
    } else {
        action.key().to_owned()
    };
    let held = Quoted::new(&held);
    let (reason, code, below) = match &kind {
        FaultKind::Missing(name) => (
            reason![held, " is missing required field '", Quoted::new(name), "'"],
            DetailCode::MissingField,
            Some(name),
        ),
        FaultKind::Extra(name) if !nested => (
            reason![
                "Message has unexpected top-level field '",
                Quoted::new(name),
                "'"
            ],
            DetailCode::InvalidValue,
            Some(name),
        ),
        FaultKind::Extra(name) => (
            reason![held, " has unexpected field '", Quoted::new(name), "'"],
            DetailCode::InvalidValue,
            Some(name),
        ),
        FaultKind::Type(Some(expected)) => (
            reason![held, " must be ", *expected],
            DetailCode::TypeMismatch,
            None,
        ),
        FaultKind::Type(None) | FaultKind::Other => {
            (reason![held, " is invalid"], DetailCode::InvalidValue, None)
        }
    };
    tokens.extend(below.cloned());
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
    if action == Action::CallRendererFunction {
        if let Some(call) = id("functionCallId") {
            error = error.with_function_call(call);
        }
    } else if let Some(surface) = id("surfaceId") {
        error = error.with_surface(surface);
    }
    error
}

fn dotted(tokens: &[String]) -> String {
    tokens.join(".")
}

/// `tokens` as a JSON Pointer, each escaped.
fn pointer(tokens: &[String]) -> String {
    tokens
        .iter()
        .map(|token| format!("/{}", escape_token(token)))
        .collect()
}

#[cfg(test)]
#[path = "schema_tests.rs"]
mod tests;
