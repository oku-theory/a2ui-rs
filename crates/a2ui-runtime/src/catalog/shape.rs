//! The property names a component declares, for envelope checks and catalog comparison.

use std::collections::BTreeSet;

use serde_json::Value;

use crate::bundle::resolve;

/// The property and required names of a component schema.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ComponentShape {
    properties: BTreeSet<String>,
    required: BTreeSet<String>,
}

impl ComponentShape {
    /// Reads `schema` through `allOf` and `$ref`s resolved in `root`, each `$ref` once.
    pub(crate) fn of(root: &Value, schema: &Value) -> Self {
        let mut shape = Self::default();
        let mut seen = BTreeSet::new();
        let mut pending = vec![schema];
        while let Some(node) = pending.pop() {
            if let Some(properties) = node.get("properties").and_then(Value::as_object) {
                shape.properties.extend(properties.keys().cloned());
            }
            if let Some(required) = node.get("required").and_then(Value::as_array) {
                let names = required.iter().filter_map(Value::as_str).map(str::to_owned);
                shape.required.extend(names);
            }
            pending.extend(
                node.get("allOf")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten(),
            );
            if let Some(reference) = node.get("$ref").and_then(Value::as_str)
                && seen.insert(reference)
                && let Some(target) = resolve(root, reference)
            {
                pending.push(target);
            }
        }
        shape
    }

    /// Property names, through `allOf` and `$ref`s.
    pub fn properties(&self) -> &BTreeSet<String> {
        &self.properties
    }

    /// Required property names, through `allOf` and `$ref`s.
    pub fn required(&self) -> &BTreeSet<String> {
        &self.required
    }
}

#[cfg(test)]
#[path = "shape_tests.rs"]
mod tests;
