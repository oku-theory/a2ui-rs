//! Depth and size caps on JSON that agents send: catalogs and validated values.

use serde_json::Value;

/// Deepest nesting of arrays and objects accepted in a catalog or a validated value.
pub(crate) const MAX_JSON_DEPTH: usize = 64;

/// Compiled size cap of one schema `pattern`, in bytes, for the regex and its lazy DFA.
pub(crate) const MAX_PATTERN_BYTES: usize = 128 * 1024;

/// Whether `value` nests arrays and objects more than `limit` levels deep.
///
/// A scalar has depth 0 and `{"a": 1}` depth 1. The walk is iterative, allocates nothing per
/// node, and checks a container's level before pushing its children, so a value deeper than the
/// stack allows is rejected at the first level past `limit`, not followed.
pub(crate) fn deeper_than(value: &Value, limit: usize) -> bool {
    let mut stack = vec![(value, 0)];
    while let Some((node, level)) = stack.pop() {
        match node {
            Value::Array(items) => {
                if level >= limit {
                    return true;
                }
                stack.extend(items.iter().map(|child| (child, level + 1)));
            }
            Value::Object(map) => {
                if level >= limit {
                    return true;
                }
                stack.extend(map.values().map(|child| (child, level + 1)));
            }
            _ => {}
        }
    }
    false
}

#[cfg(test)]
#[path = "limits_tests.rs"]
mod tests;
