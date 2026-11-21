//! A renderer `error` envelope compared with an expectation and upstream's JSON schema.

use std::sync::LazyLock;

use jsonschema::{Draft, Registry, Validator};
use serde_json::{Value, json};

use crate::compare::message_matches;
use crate::keys::assert_known_keys;
use crate::paths::{read_json, upstream_root};

const SCHEMA_DIR: &str = "specification/v1_0/json";

/// The id `renderer_to_agent.json` uses to reach `common_types.json`.
const COMMON_TYPES_ID: &str = "https://a2ui.org/specification/v1_0/common_types.json";

/// The catalog `common_types.json` points `anyFunction` at; an error envelope never reaches it.
const CATALOG_ID: &str = "https://a2ui.org/specification/v1_0/catalog.json";

static VALIDATOR: LazyLock<Result<Validator, String>> = LazyLock::new(validator);

fn validator() -> Result<Validator, String> {
    let dir = upstream_root()?.join(SCHEMA_DIR);
    let common = read_json(&dir.join("common_types.json"))?;
    let schema = read_json(&dir.join("renderer_to_agent.json"))?;
    let registry = Registry::new()
        .add(
            CATALOG_ID,
            json!({"$defs": {"anyFunction": {"type": "object"}}}),
        )
        .and_then(|registry| registry.add(COMMON_TYPES_ID, common))
        .and_then(|registry| registry.prepare())
        .map_err(|err| format!("common_types.json does not register: {err}"))?;
    jsonschema::options()
        .with_draft(Draft::Draft202012)
        .with_registry(&registry)
        .offline()
        .build(&schema)
        .map_err(|err| format!("renderer_to_agent.json does not compile: {err}"))
}

/// Checks the renderer error message built for `actual` against `expected`.
///
/// The envelope must validate against `renderer_to_agent.json`. `expected` holds `version` and
/// `error`; `error` holds `code`, `surfaceId`, `functionCallId`, `path` (all exact) and `message`
/// (a case-insensitive substring or regex).
pub fn check_renderer_error(
    actual: &a2ui_runtime::Error,
    expected: &Value,
    at: &str,
) -> Result<(), String> {
    let envelope = a2ui_runtime::RendererMessage::error(actual).to_json();
    VALIDATOR
        .as_ref()
        .map_err(|err| format!("{at}: {err}"))?
        .validate(&envelope)
        .map_err(|err| format!("{at}: renderer error {envelope} is invalid: {err}"))?;
    let Value::Object(fields) = expected else {
        return Err(format!("{at}: rendererError must be a mapping"));
    };
    assert_known_keys(fields, &["version", "error"], at)?;
    if let Some(version) = fields.get("version") {
        same(&envelope, "version", version, at)?;
    }
    let Some(expected_body) = fields.get("error") else {
        return Ok(());
    };
    let Value::Object(expected_body) = expected_body else {
        return Err(format!("{at}: rendererError.error must be a mapping"));
    };
    assert_known_keys(
        expected_body,
        &["code", "surfaceId", "functionCallId", "path", "message"],
        at,
    )?;
    let Some(body) = envelope.get("error").and_then(Value::as_object) else {
        return Err(format!("{at}: the renderer message has no `error` body"));
    };
    for (key, want) in expected_body {
        let have = body.get(key);
        if key == "message" {
            message(have, want, at)?;
        } else if have != Some(want) {
            return Err(format!(
                "{at}: rendererError.error.{key} expected {want}, got {}",
                have.map_or_else(|| "nothing".to_owned(), Value::to_string)
            ));
        }
    }
    Ok(())
}

fn same(envelope: &Value, key: &str, want: &Value, at: &str) -> Result<(), String> {
    match envelope.get(key) {
        Some(have) if have == want => Ok(()),
        have => Err(format!(
            "{at}: rendererError.{key} expected {want}, got {have:?}"
        )),
    }
}

fn message(have: Option<&Value>, want: &Value, at: &str) -> Result<(), String> {
    let Some(pattern) = want.as_str() else {
        return Err(format!(
            "{at}: rendererError.error.message must be a string"
        ));
    };
    let Some(text) = have.and_then(Value::as_str) else {
        return Err(format!(
            "{at}: rendererError.error.message is missing in the renderer message"
        ));
    };
    if message_matches(text, pattern) {
        Ok(())
    } else {
        Err(format!(
            "{at}: rendererError.error.message {text:?} does not match {pattern:?}"
        ))
    }
}

#[cfg(test)]
#[path = "outbound_tests.rs"]
mod tests;
