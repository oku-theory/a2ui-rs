//! The v1.0 specification schemas the runtime embeds and validates with.
//!
//! The runtime's models are the published schemas themselves, so the schema it generates from
//! its models is the embedded document. `vendor/sync.sh` refreshes the copies in `schemas/`.

use std::collections::BTreeMap;
use std::sync::LazyLock;

use jsonschema::Validator;
use serde_json::{Map, Value};
use snafu::{OptionExt, ensure};

use crate::bundle::{any_function_envelope, local_ref, localize_refs};
use crate::error::{EmbeddedSchemaSnafu, Error, UnknownDefinitionSnafu, ValueLimitSnafu};
use crate::limits::{MAX_JSON_DEPTH, deeper_than};
use crate::validator::{build, check, rooted};

const COMMON_TYPES: &str = include_str!("../schemas/v1_0/common_types.json");
const AGENT_TO_RENDERER: &str = include_str!("../schemas/v1_0/agent_to_renderer.json");

static COMMON_TYPES_DOC: LazyLock<Result<Value, String>> = LazyLock::new(|| parse(COMMON_TYPES));
static AGENT_TO_RENDERER_DOC: LazyLock<Result<Value, String>> =
    LazyLock::new(|| parse(AGENT_TO_RENDERER));

/// One compiled validator per common type, built on first use.
static COMMON_VALIDATORS: LazyLock<Result<BTreeMap<String, Validator>, String>> =
    LazyLock::new(common_validators);

fn parse(text: &str) -> Result<Value, String> {
    serde_json::from_str(text).map_err(|err| err.to_string())
}

fn embedded(
    name: &'static str,
    document: &'static Result<Value, String>,
) -> Result<&'static Value, Error> {
    document.as_ref().map_err(|message| {
        EmbeddedSchemaSnafu {
            name,
            message: message.clone(),
        }
        .build()
        .into()
    })
}

/// The v1.0 `common_types.json` schema, as the runtime validates with it.
pub fn common_types_schema() -> Result<&'static Value, Error> {
    embedded("common_types.json", &COMMON_TYPES_DOC)
}

/// The v1.0 `agent_to_renderer.json` message schema.
pub fn agent_to_renderer_schema() -> Result<&'static Value, Error> {
    embedded("agent_to_renderer.json", &AGENT_TO_RENDERER_DOC)
}

/// The `$defs` of `common_types.json`, by name.
pub(crate) fn common_type_defs() -> Result<&'static Map<String, Value>, Error> {
    Ok(common_types_schema()?
        .get("$defs")
        .and_then(Value::as_object)
        .with_context(|| EmbeddedSchemaSnafu {
            name: "common_types.json",
            message: "it has no $defs object".to_owned(),
        })?)
}

/// The common types as one schema document, every `$ref` local.
///
/// No catalog is in scope, so `anyFunction` is the envelope of a call to any function but
/// `@index`, whose own definition applies.
pub(crate) fn common_document() -> Result<Map<String, Value>, Error> {
    let mut defs = common_type_defs()?.clone();
    defs.values_mut().for_each(localize_refs);
    defs.insert("anyFunction".to_owned(), any_function_envelope());
    Ok(Map::from_iter([("$defs".to_owned(), Value::Object(defs))]))
}

/// Builds the validators; the error is the cause only (the document's own error, or the
/// `jsonschema` build message), which `embedded_error` prefixes once.
fn common_validators() -> Result<BTreeMap<String, Validator>, String> {
    let top = common_document().map_err(|err| err.to_string())?;
    common_type_defs()
        .map_err(|err| err.to_string())?
        .keys()
        .map(|name| {
            let reference = local_ref(&format!("/$defs/{name}"));
            Ok((name.clone(), build(&rooted(&top, &reference))?))
        })
        .collect()
}

fn embedded_error(message: &str) -> Error {
    EmbeddedSchemaSnafu {
        name: "common_types.json",
        message: message.to_owned(),
    }
    .build()
    .into()
}

/// Validates `value` as the v1.0 common type `definition`, such as `DynamicValue`.
///
/// No catalog is in scope, so a `FunctionCall` may call any function but `@index`, whose own
/// definition applies. The value is checked as is and never rewritten.
pub fn validate_common_type(definition: &str, value: &Value) -> Result<(), Error> {
    let validators = COMMON_VALIDATORS
        .as_ref()
        .map_err(|message| embedded_error(message))?;
    let validator = validators
        .get(definition)
        .context(UnknownDefinitionSnafu { definition })?;
    ensure!(
        !deeper_than(value, MAX_JSON_DEPTH),
        ValueLimitSnafu {
            limit: MAX_JSON_DEPTH
        }
    );
    check(validator, value)
}

#[cfg(test)]
#[path = "schema_tests.rs"]
mod tests;
