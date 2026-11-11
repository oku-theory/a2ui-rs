//! Order-insensitive JSON comparison used by every expectation.

use serde_json::Value;

/// Deep equality where numbers compare as `f64` and object key order is ignored.
pub fn json_eq(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => x.as_f64() == y.as_f64(),
        (Value::Array(xs), Value::Array(ys)) => {
            xs.len() == ys.len() && xs.iter().zip(ys).all(|(x, y)| json_eq(x, y))
        }
        (Value::Object(xs), Value::Object(ys)) => {
            xs.len() == ys.len()
                && xs
                    .iter()
                    .all(|(key, x)| ys.get(key).is_some_and(|y| json_eq(x, y)))
        }
        _ => a == b,
    }
}

/// Checks `actual` against an `expectError` / `expect_error` value `{category, message?}`.
///
/// `message` is a regular expression matched against the error's message.
pub fn expect_error(
    actual: &a2ui_runtime::Error,
    expected: &Value,
    at: &str,
) -> Result<(), String> {
    let Value::Object(fields) = expected else {
        return Err(format!("{at}: expect_error must be a mapping"));
    };
    crate::keys::assert_known_keys(fields, &["category", "message"], at)?;
    let category = fields.get("category").and_then(Value::as_str);
    if category != Some(actual.category().as_str()) {
        return Err(format!(
            "{at}: expected {category:?}, got {}: {actual}",
            actual.category().as_str()
        ));
    }
    let Some(pattern) = fields.get("message").and_then(Value::as_str) else {
        return Ok(());
    };
    let regex =
        regex::Regex::new(pattern).map_err(|err| format!("{at}: bad message regex: {err}"))?;
    if regex.is_match(&actual.to_string()) {
        Ok(())
    } else {
        Err(format!("{at}: message {actual} does not match /{pattern}/"))
    }
}

#[cfg(test)]
#[path = "compare_tests.rs"]
mod tests;
