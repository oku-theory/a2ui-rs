//! The value conversions function authors share: boolean, number and text.

use serde_json::Value;

/// `value` as a boolean: `true`, `"true"` (any ASCII case), a nonzero number, or `{"valid": true}`.
///
/// Everything else, `null` and arrays included, is `false`.
pub fn boolean(value: &Value) -> bool {
    match value {
        Value::Bool(flag) => *flag,
        Value::String(text) => text.eq_ignore_ascii_case("true"),
        Value::Number(number) => number.as_f64().is_some_and(|n| n != 0.0),
        Value::Object(members) => members.get("valid").and_then(Value::as_bool) == Some(true),
        Value::Null | Value::Array(_) => false,
    }
}

/// `value` as a finite number: a number, or a string that parses as one once trimmed.
///
/// Anything else, including `"inf"` and booleans, is `None`.
pub fn number(value: &Value) -> Option<f64> {
    let parsed = match value {
        Value::Number(number) => number.as_f64(),
        Value::String(text) => text.trim().parse::<f64>().ok(),
        _ => None,
    };
    parsed.filter(|n| n.is_finite())
}

/// `value` as text: strings as they are, `null` as `""`, the rest as compact JSON.
pub fn text(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

#[cfg(test)]
#[path = "coerce_tests.rs"]
mod tests;
