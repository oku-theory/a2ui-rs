//! Reading the parts of a catalog document that JSON Schema does not model.

use std::collections::BTreeSet;

use serde_json::{Map, Value, json};

use crate::bundle::{local_ref, pointer_of};

const BRANCHES: [&str; 3] = ["allOf", "anyOf", "oneOf"];

/// Property names a component schema declares, at top level and in composition branches.
pub(crate) fn property_names(schema: &Map<String, Value>) -> Vec<String> {
    let mut names = Vec::new();
    schema_keys(schema, &mut names);
    names
}

/// Argument names of a function, read by the key that holds them.
///
/// `properties.args` and `args` are schemas, whose `properties` (also in composition
/// branches) name the arguments; `parameters` is a shorthand map from argument name to schema,
/// so an argument may be named `type` or `properties`.
pub(crate) fn argument_names(schema: &Map<String, Value>) -> Vec<String> {
    let nested = schema
        .get("properties")
        .and_then(|properties| properties.get("args"));
    let mut names = Vec::new();
    for arguments in [nested, schema.get("args")]
        .into_iter()
        .flatten()
        .filter_map(Value::as_object)
    {
        schema_keys(arguments, &mut names);
    }
    if let Some(parameters) = schema.get("parameters").and_then(Value::as_object) {
        names.extend(parameters.keys().cloned());
    }
    names
}

/// Collects `properties` keys of `schema` and of its composition branches.
///
/// It recurses on branches of a depth-checked catalog document.
fn schema_keys(schema: &Map<String, Value>, out: &mut Vec<String>) {
    if let Some(properties) = schema.get("properties").and_then(Value::as_object) {
        out.extend(properties.keys().cloned());
    }
    for branch in BRANCHES {
        let items = schema.get(branch).and_then(Value::as_array);
        for item in items.into_iter().flatten().filter_map(Value::as_object) {
            schema_keys(item, out);
        }
    }
}

/// A function's return type: `returnType`, or the `const` of `properties.returnType`.
pub(crate) fn return_type(schema: &Map<String, Value>) -> Option<String> {
    schema
        .get("returnType")
        .and_then(Value::as_str)
        .or_else(|| {
            schema
                .get("properties")?
                .get("returnType")?
                .get("const")?
                .as_str()
        })
        .map(str::to_owned)
}

/// The entries of `section` that `$defs.<union>` lists; `None` when it lists none.
pub(crate) fn union_members(
    defs: &Map<String, Value>,
    union: &str,
    section: &str,
) -> Option<BTreeSet<String>> {
    let prefix = format!("/{section}/");
    let members: BTreeSet<String> = defs
        .get(union)?
        .get("oneOf")?
        .as_array()?
        .iter()
        .filter_map(|item| pointer_of(item.get("$ref")?.as_str()?))
        .filter_map(|pointer| pointer.strip_prefix(prefix.as_str()).map(str::to_owned))
        .collect();
    (!members.is_empty()).then_some(members)
}

/// A `oneOf` union of local refs to `section` entries; an empty union matches nothing.
///
/// The names are sorted here, so the union never depends on the caller's order. They are
/// UAX #31 identifiers, so they need no JSON Pointer escaping; `local_ref` percent-encodes the
/// non-ASCII ones.
pub(crate) fn union(section: &str, names: &[&str], discriminator: Option<&str>) -> Value {
    if names.is_empty() {
        return json!({"not": {}});
    }
    let mut names = names.to_vec();
    names.sort_unstable();
    let refs: Vec<Value> = names
        .iter()
        .map(|name| json!({"$ref": local_ref(&format!("/{section}/{name}"))}))
        .collect();
    match discriminator {
        Some(property) => json!({"oneOf": refs, "discriminator": {"propertyName": property}}),
        None => json!({"oneOf": refs}),
    }
}

#[cfg(test)]
#[path = "document_tests.rs"]
mod tests;
