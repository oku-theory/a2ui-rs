//! The byte cap on one agent message, and the walk that estimates a value against it.

use serde_json::Value;

/// The default cap on one message: 1 MiB.
const DEFAULT_MAX_BYTES: usize = 1_048_576;

/// What a number counts for: the longest `f64` text, not its real length.
const NUMBER_BYTES: usize = 24;

/// How large an agent message a host accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MessageLimits {
    max_bytes: usize,
}

impl MessageLimits {
    /// Limits that accept messages of at most `max_bytes` bytes.
    pub fn new(max_bytes: usize) -> Self {
        Self { max_bytes }
    }

    /// The largest message accepted, in bytes.
    pub fn max_bytes(&self) -> usize {
        self.max_bytes
    }
}

impl Default for MessageLimits {
    fn default() -> Self {
        Self::new(DEFAULT_MAX_BYTES)
    }
}

/// Whether the compact encoding of `value` is estimated to pass `max` bytes.
///
/// Counts each string's and key's bytes plus two quotes, one per `:` and `,`, two per container,
/// four per `true` or `null`, five per `false` and 24 per number. Escapes are not counted: the
/// cap bounds work, it is not an exact size. The walk is iterative and stops once past `max`.
pub(crate) fn encoded_len_exceeds(value: &Value, max: usize) -> bool {
    let mut total = 0usize;
    let mut stack = vec![value];
    while let Some(next) = stack.pop() {
        total = total.saturating_add(match next {
            Value::Null | Value::Bool(true) => 4,
            Value::Bool(false) => 5,
            Value::Number(_) => NUMBER_BYTES,
            Value::String(text) => text.len() + 2,
            Value::Array(items) => 2 + items.len().saturating_sub(1),
            Value::Object(entries) => {
                let keys: usize = entries.keys().map(|key| key.len() + 3).sum();
                2 + entries.len().saturating_sub(1) + keys
            }
        });
        if total > max {
            return true;
        }
        match next {
            Value::Array(items) => stack.extend(items),
            Value::Object(entries) => stack.extend(entries.values()),
            _ => {}
        }
    }
    false
}

#[cfg(test)]
#[path = "limits_tests.rs"]
mod tests;
