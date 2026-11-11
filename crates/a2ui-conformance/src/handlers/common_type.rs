//! The `validate_common_type` action: each step's value as one common type.
//!
//! The runtime validates the JSON value as is and never rebuilds it from a typed model, so the
//! suite's "serializes back to the same JSON" half holds by construction.

use serde_json::Value;

use crate::case::Case;
use crate::compare::expect_error;
use crate::keys::{assert_case_keys, assert_known_keys};

/// Validates each step's `value` as `definition`; `expectError` steps must be rejected.
pub fn run(case: &Case) -> Result<(), String> {
    assert_case_keys(case, &["protocolVersion", "definition", "steps"])?;
    let at = case.at();
    let definition = case
        .get("definition")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{at}: `definition` must be a string"))?;
    let steps = case
        .get("steps")
        .and_then(Value::as_array)
        .filter(|steps| !steps.is_empty())
        .ok_or_else(|| format!("{at}: a case needs non-empty `steps`"))?;
    for (index, step) in steps.iter().enumerate() {
        let at = format!("{at} step {index}");
        let step = step
            .as_object()
            .ok_or_else(|| format!("{at}: a step is a mapping"))?;
        assert_known_keys(step, &["value", "expectError"], &at)?;
        let value = step
            .get("value")
            .ok_or_else(|| format!("{at}: a step needs `value`"))?;
        match (
            step.get("expectError"),
            a2ui_runtime::validate_common_type(definition, value),
        ) {
            (None, Ok(())) => {}
            (None, Err(err)) => return Err(format!("{at}: rejected: {err}")),
            (Some(expected), Err(err)) => expect_error(&err, expected, &at)?,
            (Some(_), Ok(())) => return Err(format!("{at}: accepted, expected an error")),
        }
    }
    Ok(())
}
