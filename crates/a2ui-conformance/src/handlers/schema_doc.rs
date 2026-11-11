//! The `common_types_schema` and `agent_to_renderer_schema` actions.
//!
//! The runtime's models are the embedded published schemas, so the generated schema must equal
//! the specification document exactly (compared as parsed JSON: key order is ignored, array
//! order is not).

use serde_json::Value;

use crate::case::Case;
use crate::compare::json_eq;
use crate::keys::assert_case_keys;
use crate::paths::{read_json, upstream_root};

/// Compares the runtime's common types schema with `expectFile`.
pub fn run_common_types(case: &Case) -> Result<(), String> {
    compare(case, a2ui_runtime::common_types_schema())
}

/// Compares the runtime's agent-to-renderer schema with `expectFile`.
pub fn run_agent_to_renderer(case: &Case) -> Result<(), String> {
    compare(case, a2ui_runtime::agent_to_renderer_schema())
}

fn compare(case: &Case, generated: Result<&Value, a2ui_runtime::Error>) -> Result<(), String> {
    assert_case_keys(case, &["protocolVersion", "expectFile"])?;
    let at = case.at();
    let generated = generated.map_err(|err| format!("{at}: {err}"))?;
    let path = case
        .get("expectFile")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{at}: `expectFile` must be a string"))?;
    let expected = read_json(&upstream_root()?.join(path))?;
    if json_eq(generated, &expected) {
        Ok(())
    } else {
        Err(format!("{at}: the runtime's schema differs from {path}"))
    }
}
