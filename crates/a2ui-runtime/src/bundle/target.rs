//! What a `$ref` names: the node it resolves to, and whether that node is a schema.

use serde_json::Value;

use super::RefPointer;
use super::walk::{Segment, find_ref};

/// The node of `root` a supported `$ref` names, decoding its fragment.
pub(crate) fn resolve<'a>(root: &'a Value, reference: &str) -> Option<&'a Value> {
    RefPointer::of(reference)?.resolve(root)
}

/// What a `$ref` naming no schema resolves to: nothing, or a non-schema value.
///
/// It carries no wording; `catalog::validation_schema` words the message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NotASchema {
    /// No node: the `$ref` does not resolve.
    Missing,
    /// A string node.
    String,
    /// A number node.
    Number,
    /// An array node.
    Array,
    /// A `null` node.
    Null,
}

/// The first `$ref` under `root` naming no schema of it, with its path.
///
/// A schema is an object or a boolean (JSON Schema 2020-12, 4.3.1): a target that is one
/// passes, even in a data position. It only looks pointers up, never follows one, so its cost
/// is linear in `root` whatever the `$ref`s chain. The path holds the keys as they are, never
/// escaped, and is copied only for the `$ref` found. It walks with [`find_ref`].
pub(crate) fn first_not_a_schema(root: &Value) -> Option<(&str, Vec<Segment<'_>>, NotASchema)> {
    find_ref(root, &mut |reference, path| {
        let found = match resolve(root, reference) {
            None => NotASchema::Missing,
            Some(Value::Object(_) | Value::Bool(_)) => return None,
            Some(Value::String(_)) => NotASchema::String,
            Some(Value::Number(_)) => NotASchema::Number,
            Some(Value::Array(_)) => NotASchema::Array,
            Some(Value::Null) => NotASchema::Null,
        };
        Some((reference, path.to_vec(), found))
    })
}

#[cfg(test)]
#[path = "target_tests.rs"]
mod tests;
