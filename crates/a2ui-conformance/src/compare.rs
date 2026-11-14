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

/// [`json_eq`], except that arrays under `enum` and `required` compare as multisets.
pub fn schema_eq(a: &Value, b: &Value) -> bool {
    schema_eq_at(a, b, false)
}

fn schema_eq_at(a: &Value, b: &Value, unordered: bool) -> bool {
    match (a, b) {
        (Value::Array(xs), Value::Array(ys)) if unordered => same_multiset(xs, ys),
        (Value::Array(xs), Value::Array(ys)) => {
            xs.len() == ys.len() && xs.iter().zip(ys).all(|(x, y)| schema_eq_at(x, y, false))
        }
        (Value::Object(xs), Value::Object(ys)) => {
            xs.len() == ys.len()
                && xs.iter().all(|(key, x)| {
                    let unordered = key == "enum" || key == "required";
                    ys.get(key).is_some_and(|y| schema_eq_at(x, y, unordered))
                })
        }
        _ => json_eq(a, b),
    }
}

fn same_multiset(xs: &[Value], ys: &[Value]) -> bool {
    let mut left: Vec<&Value> = ys.iter().collect();
    xs.len() == ys.len()
        && xs
            .iter()
            .all(|x| match left.iter().position(|y| json_eq(x, y)) {
                Some(index) => {
                    left.swap_remove(index);
                    true
                }
                None => false,
            })
}

/// Checks `actual` against an `expectError` / `expect_error` value `{category, message?}`.
///
/// `message` matches as a case-insensitive substring or regular expression, as upstream's
/// reference harness does (`_exception_matches_expected_message`).
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
    let pattern = match fields.get("message") {
        None => return Ok(()),
        Some(Value::String(pattern)) => pattern,
        Some(other) => return Err(format!("{at}: message must be a string, got {other}")),
    };
    let text = actual.to_string();
    let substring = text.to_lowercase().contains(&pattern.to_lowercase());
    let regex = regex::RegexBuilder::new(pattern)
        .case_insensitive(true)
        .build()
        .is_ok_and(|regex| regex.is_match(&text));
    if substring || regex {
        Ok(())
    } else {
        Err(format!(
            "{at}: message {actual} does not match {pattern:?} as a substring or a regex"
        ))
    }
}

#[cfg(test)]
#[path = "compare_tests.rs"]
mod tests;
