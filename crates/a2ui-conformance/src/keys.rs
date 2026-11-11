//! Strict key checks, so an unread key fails a case instead of passing silently.

use serde_json::{Map, Value};

use crate::case::Case;

const CASE_METADATA: [&str; 4] = ["name", "description", "features", "action"];

/// Fails when `obj` holds a key outside `allowed`, naming it and `at`.
pub fn assert_known_keys(
    obj: &Map<String, Value>,
    allowed: &[&str],
    at: &str,
) -> Result<(), String> {
    match obj.keys().find(|key| !allowed.contains(&key.as_str())) {
        Some(key) => Err(format!("Unknown key '{key}' in {at}")),
        None => Ok(()),
    }
}

/// Checks a case's top-level keys; `name`, `description`, `features`, `action` are implied.
pub fn assert_case_keys(case: &Case, allowed: &[&str]) -> Result<(), String> {
    let all: Vec<&str> = CASE_METADATA.iter().chain(allowed).copied().collect();
    assert_known_keys(&case.body, &all, &format!("{}#{}", case.suite, case.name))
}
