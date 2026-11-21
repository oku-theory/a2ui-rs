//! JSON Schema 2020-12 validation on the linear-time `regex` engine, offline.
//!
//! Catalog schemas come from agents, so patterns compile with `regex`, which never backtracks,
//! under a size cap, and no `$ref` is ever fetched: one to another document fails the build.

use jsonschema::error::{TypeKind, ValidationError, ValidationErrorKind};
use jsonschema::paths::LocationSegment;
use jsonschema::{Draft, JsonType, PatternOptions, Validator};
use serde_json::{Map, Value};

use crate::detail::DetailCode;
use crate::error::{Error, SchemaBuildSnafu, SchemaViolationSnafu};
use crate::limits::MAX_PATTERN_BYTES;
use crate::pointer::escape_token;

/// The JSON Schema dialect every runtime schema declares.
pub(crate) const DRAFT_2020_12: &str = "https://json-schema.org/draft/2020-12/schema";

/// Compiles `document`, whose root `$ref` names the schema instances must match.
pub(crate) fn compile(document: &Value) -> Result<Validator, Error> {
    build(document).map_err(|message| -> Error { SchemaBuildSnafu { message }.build().into() })
}

/// [`compile`] with the bare `jsonschema` build message as the error, for cached validators.
pub(crate) fn build(document: &Value) -> Result<Validator, String> {
    let patterns = PatternOptions::regex()
        .size_limit(MAX_PATTERN_BYTES)
        .dfa_size_limit(MAX_PATTERN_BYTES);
    jsonschema::options()
        .with_draft(Draft::Draft202012)
        .with_pattern_options(patterns)
        .offline()
        .build(document)
        .map_err(|err| err.to_string())
}

/// `top` as a schema document whose root `$ref` is `reference`, such as `#/components/Text`.
///
/// `reference` must come from `bundle::local_ref`, which percent-encodes it.
pub(crate) fn rooted(top: &Map<String, Value>, reference: &str) -> Value {
    let mut document = top.clone();
    document.insert("$schema".to_owned(), Value::from(DRAFT_2020_12));
    document.insert("$ref".to_owned(), Value::from(reference));
    Value::Object(document)
}

/// Checks `instance`; the first violation becomes a `ValidationError`.
pub(crate) fn check(validator: &Validator, instance: &Value) -> Result<(), Error> {
    validator
        .validate(instance)
        .map_err(|err| violation_error(&err))
}

/// The `ValidationError` for `violation`, its message ending with the instance path.
pub(crate) fn violation_error(violation: &ValidationError<'_>) -> Error {
    SchemaViolationSnafu {
        at: violation.instance_path().to_string(),
        message: violation.to_string(),
    }
    .build()
    .into()
}

/// What one schema violation says, in the terms of the messages and details built from it.
pub(crate) struct Fault {
    /// The instance path's tokens; the field's parent for a name below.
    pub(crate) tokens: Vec<String>,
    pub(crate) kind: FaultKind,
}

/// The kind of one schema violation.
pub(crate) enum FaultKind {
    /// A required field is absent.
    Missing(String),
    /// A value has the wrong type, named with its article ("a string").
    Type(Option<&'static str>),
    /// A field the schema does not allow.
    Extra(String),
    /// Any other keyword.
    Other,
}

impl Fault {
    /// The detail code of this fault and the tokens of the field it names.
    pub(crate) fn located(self) -> (DetailCode, Vec<String>) {
        let Self { mut tokens, kind } = self;
        let code = match kind {
            FaultKind::Missing(name) => {
                tokens.push(name);
                DetailCode::MissingField
            }
            FaultKind::Extra(name) => {
                tokens.push(name);
                DetailCode::InvalidValue
            }
            FaultKind::Type(Some(_)) => DetailCode::TypeMismatch,
            FaultKind::Type(None) | FaultKind::Other => DetailCode::InvalidValue,
        };
        (code, tokens)
    }
}

/// The fault `error` describes; in a `oneOf` or `anyOf`, that of the branch failing least.
pub(crate) fn fault(error: &ValidationError<'_>) -> Fault {
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
        ValidationErrorKind::AdditionalProperties { unexpected }
        | ValidationErrorKind::UnevaluatedProperties { unexpected } => unexpected
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

/// The detail code of `violation` and the JSON Pointer of the field it names.
pub(crate) fn violation_detail(violation: &ValidationError<'_>) -> (DetailCode, String) {
    let (code, tokens) = fault(violation).located();
    (code, pointer(&tokens))
}

/// `tokens` as a JSON Pointer, each escaped.
pub(crate) fn pointer(tokens: &[String]) -> String {
    tokens
        .iter()
        .map(|token| format!("/{}", escape_token(token)))
        .collect()
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

#[cfg(test)]
#[path = "validator_tests.rs"]
mod tests;
