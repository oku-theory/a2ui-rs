//! The v1.0 specification schemas the runtime embeds and validates with.
//!
//! The runtime's models are the published schemas themselves, so the schema it generates from
//! its models is the embedded document. `vendor/sync.sh` refreshes the copies in `schemas/`.

use std::sync::LazyLock;

use serde_json::Value;

use crate::error::{EmbeddedSchemaSnafu, Error};

const COMMON_TYPES: &str = include_str!("../schemas/v1_0/common_types.json");
const AGENT_TO_RENDERER: &str = include_str!("../schemas/v1_0/agent_to_renderer.json");

static COMMON_TYPES_DOC: LazyLock<Result<Value, String>> = LazyLock::new(|| parse(COMMON_TYPES));
static AGENT_TO_RENDERER_DOC: LazyLock<Result<Value, String>> =
    LazyLock::new(|| parse(AGENT_TO_RENDERER));

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

#[cfg(test)]
#[path = "schema_tests.rs"]
mod tests;
