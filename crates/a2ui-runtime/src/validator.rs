//! JSON Schema 2020-12 validation on the linear-time `regex` engine, offline.
//!
//! Catalog schemas come from agents, so patterns compile with `regex`, which never backtracks,
//! under a size cap, and no `$ref` is ever fetched: one to another document fails the build.

use jsonschema::{Draft, PatternOptions, Validator};
use serde_json::{Map, Value};

use crate::error::{Error, SchemaBuildSnafu, SchemaViolationSnafu};
use crate::limits::MAX_PATTERN_BYTES;

/// The JSON Schema dialect every runtime schema declares.
pub(crate) const DRAFT_2020_12: &str = "https://json-schema.org/draft/2020-12/schema";

/// Compiles `document`, whose root `$ref` names the schema instances must match.
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "catalog loading (Task 4) compiles with it; only tests do now"
    )
)]
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
    validator.validate(instance).map_err(|err| -> Error {
        SchemaViolationSnafu {
            at: err.instance_path().to_string(),
            message: err.to_string(),
        }
        .build()
        .into()
    })
}

#[cfg(test)]
#[path = "validator_tests.rs"]
mod tests;
