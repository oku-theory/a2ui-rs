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

#[cfg(test)]
#[path = "compare_tests.rs"]
mod tests;
