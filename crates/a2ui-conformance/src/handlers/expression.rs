//! The `parse_expression_template` action: template text into parts.

use a2ui_runtime::parse_template;
use serde_json::Value;

use crate::case::Case;
use crate::compare::{expect_error, json_eq};
use crate::keys::assert_case_keys;

/// Parses `input` and compares the parts with `expect`, or the error with `expect_error`.
pub fn run(case: &Case) -> Result<(), String> {
    assert_case_keys(case, &["input", "expect", "expect_error"])?;
    let at = case.at();
    let input = case
        .get("input")
        .and_then(Value::as_str)
        .ok_or("a case needs `input`")?;
    if case.get("expect").is_some() && case.get("expect_error").is_some() {
        return Err(format!("{at}: `expect` and `expect_error` are exclusive"));
    }
    match (
        parse_template(input),
        case.get("expect_error"),
        case.get("expect"),
    ) {
        (Err(err), Some(expected), _) => expect_error(&err, expected, &at),
        (Err(err), None, _) => Err(format!("{at}: {err}")),
        (Ok(parts), Some(_), _) => Err(format!("{at}: expected an error, got {parts:?}")),
        (Ok(parts), None, Some(expected)) => {
            let actual = Value::Array(parts.iter().map(|part| part.to_json()).collect());
            if json_eq(&actual, expected) {
                Ok(())
            } else {
                Err(format!("{at}: got {actual}, expected {expected}"))
            }
        }
        (Ok(_), None, None) => Err(format!("{at}: a case needs `expect` or `expect_error`")),
    }
}
