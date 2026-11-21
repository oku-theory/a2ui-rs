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

/// Checks `actual` against an `expectError` / `expect_error` value.
///
/// A string matches the message only. A mapping takes `category`, `message`, `code`, `path`,
/// `surfaceId`, `functionCallId`, `details` and `rendererError`; any other key is an error.
/// `message` matches as a case-insensitive literal substring, else a case-insensitive regex, as
/// upstream's reference harness does. `ValidationError` also accepts `IntegrityError` and
/// `RecursionError`, which subclass it upstream.
pub fn expect_error(
    actual: &a2ui_runtime::Error,
    expected: &Value,
    at: &str,
) -> Result<(), String> {
    let fields = match expected {
        Value::String(_) => {
            return check_message(actual, expected, at);
        }
        Value::Object(fields) => fields,
        _ => return Err(format!("{at}: expect_error must be a mapping or a string")),
    };
    crate::keys::assert_known_keys(fields, KEYS, at)?;
    if let Some(category) = fields.get("category") {
        check_category(actual, category, at)?;
    }
    if let Some(message) = fields.get("message") {
        check_message(actual, message, at)?;
    }
    if let Some(code) = fields.get("code") {
        check_exact("code", Some(actual.code().as_str()), code, at)?;
    }
    for (key, have) in [
        ("path", actual.path()),
        ("surfaceId", actual.surface_id()),
        ("functionCallId", actual.function_call_id()),
    ] {
        if let Some(want) = fields.get(key) {
            check_exact(key, have, want, at)?;
        }
    }
    if let Some(details) = fields.get("details") {
        check_details(actual, details, at)?;
    }
    match fields.get("rendererError") {
        Some(renderer) => crate::outbound::check_renderer_error(actual, renderer, at),
        None => Ok(()),
    }
}

const KEYS: &[&str] = &[
    "category",
    "message",
    "code",
    "path",
    "surfaceId",
    "functionCallId",
    "details",
    "rendererError",
];

fn check_category(actual: &a2ui_runtime::Error, want: &Value, at: &str) -> Result<(), String> {
    let have = actual.category().as_str();
    let Some(want) = want.as_str() else {
        return Err(format!("{at}: category must be a string, got {want}"));
    };
    let subclass = want == "ValidationError" && matches!(have, "IntegrityError" | "RecursionError");
    if want == have || subclass {
        Ok(())
    } else {
        Err(format!("{at}: expected {want:?}, got {have}: {actual}"))
    }
}

fn check_exact(key: &str, have: Option<&str>, want: &Value, at: &str) -> Result<(), String> {
    let Some(want) = want.as_str() else {
        return Err(format!("{at}: {key} must be a string, got {want}"));
    };
    if have == Some(want) {
        Ok(())
    } else {
        Err(format!("{at}: {key} expected {want:?}, got {have:?}"))
    }
}

fn check_message(actual: &a2ui_runtime::Error, want: &Value, at: &str) -> Result<(), String> {
    let Some(pattern) = want.as_str() else {
        return Err(format!("{at}: message must be a string, got {want}"));
    };
    if message_matches(&actual.to_string(), pattern) {
        Ok(())
    } else {
        Err(format!(
            "{at}: message {actual} does not match {pattern:?} as a substring or a regex"
        ))
    }
}

/// Whether `text` holds `pattern` as a case-insensitive literal, else matches it as a regex.
pub(crate) fn message_matches(text: &str, pattern: &str) -> bool {
    text.to_lowercase().contains(&pattern.to_lowercase())
        || regex::RegexBuilder::new(pattern)
            .case_insensitive(true)
            .build()
            .is_ok_and(|regex| regex.is_match(text))
}

fn check_details(actual: &a2ui_runtime::Error, want: &Value, at: &str) -> Result<(), String> {
    let Some(want) = want.as_array() else {
        return Err(format!("{at}: details must be a list, got {want}"));
    };
    let have = actual.details();
    if want.len() != have.len() {
        return Err(format!(
            "{at}: expected {} details, got {}: {actual}",
            want.len(),
            have.len()
        ));
    }
    for (index, (want, have)) in want.iter().zip(have).enumerate() {
        let Value::Object(want) = want else {
            return Err(format!("{at}: details[{index}] must be a mapping"));
        };
        let here = format!("{at}: details[{index}]");
        crate::keys::assert_known_keys(want, &["path", "code", "surfaceId"], &here)?;
        let fields = [
            ("path", Some(have.path())),
            ("code", Some(have.code().as_str())),
            ("surfaceId", have.surface_id()),
        ];
        for (key, value) in fields {
            if let Some(want) = want.get(key) {
                check_exact(&format!("details[{index}].{key}"), value, want, at)?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "compare_tests.rs"]
mod tests;
