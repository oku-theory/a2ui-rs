//! The validation schema: the checked bundle in upstream's flattened shape.
//!
//! [`build`] takes a [`Checked`] bundle, so it only runs after `compile_entries` enforced the
//! expansion caps on every `$ref` it follows (see `flatten`). The result keeps `$defs` no entry
//! reaches, which those caps never budgeted, so it is output only: nothing compiles it.

use std::collections::BTreeSet;

use serde_json::{Map, Value};

use super::compile::Checked;
use super::flatten::flattened;
use crate::bundle::{add_common_types, def_names, whole_def_name};
use crate::validator::DRAFT_2020_12;

/// Keywords whose value maps names to subschemas.
const SCHEMA_MAPS: [&str; 4] = [
    "properties",
    "patternProperties",
    "$defs",
    "dependentSchemas",
];

/// Keywords whose value is a list of subschemas.
const SCHEMA_LISTS: [&str; 4] = ["allOf", "anyOf", "oneOf", "prefixItems"];

/// Keywords whose value is one subschema.
const SCHEMA_ONE: [&str; 10] = [
    "items",
    "not",
    "if",
    "then",
    "else",
    "additionalProperties",
    "unevaluatedProperties",
    "unevaluatedItems",
    "contains",
    "propertyNames",
];

/// Builds the validation schema of a checked bundle with the catalog's effective id and version.
pub(super) fn build(
    checked: Checked<'_>,
    id: &str,
    version: &str,
    instructions: Option<&str>,
    common: &Map<String, Value>,
) -> Value {
    let bundle = checked.bundle();
    let mut merged = BTreeSet::new();
    let mut components = Map::new();
    for (name, schema) in bundle.components() {
        let flat = flattened(name, schema, bundle.defs(), common);
        merged.extend(flat.merged);
        let mut flat = Value::Object(flat.schema);
        describe(&mut flat, common);
        components.insert(name.clone(), flat);
    }
    let mut functions = Map::new();
    for (name, schema) in bundle.functions() {
        let mut schema = schema.clone();
        close_args(&mut schema);
        describe(&mut schema, common);
        functions.insert(name.clone(), schema);
    }
    let mut defs: Map<String, Value> = bundle
        .defs()
        .iter()
        .filter(|(name, _)| !common.contains_key(*name))
        .map(|(name, definition)| {
            let mut definition = definition.clone();
            describe(&mut definition, common);
            (name.clone(), definition)
        })
        .collect();
    prune(&mut defs, &merged, &components, &functions);
    let mut reached = Map::new();
    let roots: Vec<&Value> = components
        .values()
        .chain(functions.values())
        .chain(defs.values())
        .collect();
    add_common_types(&mut reached, &roots, common);
    defs.extend(reached);
    let mut out = Map::from_iter([
        ("$schema".to_owned(), Value::from(DRAFT_2020_12)),
        ("catalogId".to_owned(), Value::from(id)),
        ("protocolVersion".to_owned(), Value::from(version)),
    ]);
    if let Some(text) = instructions.filter(|text| !text.is_empty()) {
        out.insert("instructions".to_owned(), Value::from(text));
    }
    out.insert("components".to_owned(), Value::Object(components));
    if !functions.is_empty() {
        out.insert("functions".to_owned(), Value::Object(functions));
    }
    out.insert("$defs".to_owned(), Value::Object(defs));
    Value::Object(out)
}

/// Gives each whole-entry `$ref` to a common type that type's description, unless it has one.
///
/// It walks schema positions only, never `const`, `enum`, `default` or `examples` data; the
/// draft-7 forms (`dependencies`, an array `items`) are not schema positions here.
fn describe(node: &mut Value, common: &Map<String, Value>) {
    let Value::Object(map) = node else {
        return;
    };
    for (key, value) in map.iter_mut() {
        let key = key.as_str();
        if SCHEMA_MAPS.contains(&key) {
            if let Value::Object(entries) = value {
                entries
                    .values_mut()
                    .for_each(|schema| describe(schema, common));
            }
        } else if SCHEMA_LISTS.contains(&key) {
            if let Value::Array(items) = value {
                items.iter_mut().for_each(|schema| describe(schema, common));
            }
        } else if SCHEMA_ONE.contains(&key) {
            describe(value, common);
        }
    }
    if map.contains_key("description") {
        return;
    }
    let description = map
        .get("$ref")
        .and_then(Value::as_str)
        .and_then(whole_def_name)
        .and_then(|name| common.get(&name)?.get("description"))
        .filter(|description| description.is_string())
        .cloned();
    if let Some(description) = description {
        map.insert("description".to_owned(), description);
    }
}

/// Closes a function's inline object `properties.args`, as upstream does.
///
/// `additionalProperties` moves to `unevaluatedProperties`, which defaults to `false`. Upstream
/// closes only `properties.args`, not the `args`/`parameters` spellings catalogs may also use.
fn close_args(schema: &mut Value) {
    let Some(properties) = schema.get_mut("properties").and_then(Value::as_object_mut) else {
        return;
    };
    if !properties.contains_key("@call") && !properties.contains_key("call") {
        return;
    }
    let Some(Value::Object(args)) = properties.get_mut("args") else {
        return;
    };
    if args.get("type").and_then(Value::as_str) != Some("object") || args.contains_key("$ref") {
        return;
    }
    let open = args.remove("additionalProperties");
    if !args.contains_key("unevaluatedProperties") {
        args.insert(
            "unevaluatedProperties".to_owned(),
            open.unwrap_or(Value::Bool(false)),
        );
    }
}

/// Drops each merged mixin of `defs` that nothing kept still names.
///
/// Components, functions and every definition that is not a merged mixin are kept, so a
/// dropped mixin leaves no dangling `$ref`. Names are read from every `$ref` string, data
/// included, as `add_common_types` does: it may keep more, never less.
fn prune(
    defs: &mut Map<String, Value>,
    merged: &BTreeSet<String>,
    components: &Map<String, Value>,
    functions: &Map<String, Value>,
) {
    if merged.is_empty() {
        return;
    }
    let mut pending = Vec::new();
    for root in components.values().chain(functions.values()) {
        def_names(root, &mut pending);
    }
    for (name, definition) in defs.iter() {
        if !merged.contains(name) {
            def_names(definition, &mut pending);
        }
    }
    let mut reached = BTreeSet::new();
    while let Some(name) = pending.pop() {
        if reached.insert(name.clone())
            && let Some(definition) = defs.get(&name)
        {
            def_names(definition, &mut pending);
        }
    }
    defs.retain(|name, _| !merged.contains(name) || reached.contains(name));
}

#[cfg(test)]
#[path = "validation_schema_tests.rs"]
mod tests;
