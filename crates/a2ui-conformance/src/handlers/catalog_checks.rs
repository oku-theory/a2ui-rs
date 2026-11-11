//! Expectations on a loaded catalog, shared by the `from_json` and `derive` actions.

use std::collections::BTreeSet;

use a2ui_runtime::Catalog;
use percent_encoding::percent_decode_str;
use serde_json::{Map, Value};

use crate::compare::schema_eq;

/// Checks `catalogId`, `protocolVersion`, `components`, `functions` and `theme` of `expect`.
///
/// `components` / `functions` are a list of the exact names, or a map of the exact names to
/// their authored schemas.
pub fn entries(expect: &Map<String, Value>, catalog: &Catalog, at: &str) -> Result<(), String> {
    if let Some(expected) = expect.get("catalogId") {
        same_text(expected, catalog.id(), "catalogId", at)?;
    }
    if let Some(expected) = expect.get("protocolVersion") {
        same_text(expected, catalog.protocol_version(), "protocolVersion", at)?;
    }
    if let Some(expected) = expect.get("components") {
        let actual = catalog.components().map(|def| (def.name(), def.schema()));
        named(expected, actual.collect(), "components", at)?;
    }
    if let Some(expected) = expect.get("functions") {
        let actual = catalog.functions().map(|def| (def.name(), def.schema()));
        named(expected, actual.collect(), "functions", at)?;
    }
    if let Some(expected) = expect.get("theme") {
        match catalog.theme() {
            Some(theme) if schema_eq(theme, expected) => {}
            Some(theme) => return Err(format!("{at}: theme {theme}, expected {expected}")),
            None => return Err(format!("{at}: no theme, expected {expected}")),
        }
    }
    Ok(())
}

fn same_text(expected: &Value, actual: &str, what: &str, at: &str) -> Result<(), String> {
    if expected.as_str() == Some(actual) {
        Ok(())
    } else {
        Err(format!("{at}: {what} is '{actual}', expected {expected}"))
    }
}

fn named(
    expected: &Value,
    actual: Vec<(&str, &Value)>,
    what: &str,
    at: &str,
) -> Result<(), String> {
    let actual_names: BTreeSet<&str> = actual.iter().map(|(name, _)| *name).collect();
    let expected_names: BTreeSet<&str> = match expected {
        Value::Array(names) => names
            .iter()
            .map(|name| {
                name.as_str()
                    .ok_or_else(|| format!("{at}: {what} lists a non-string"))
            })
            .collect::<Result<_, _>>()?,
        Value::Object(entries) => entries.keys().map(String::as_str).collect(),
        _ => return Err(format!("{at}: {what} must be a list or a map")),
    };
    if expected_names != actual_names {
        return Err(format!(
            "{at}: {what} {actual_names:?}, expected {expected_names:?}"
        ));
    }
    let Value::Object(entries) = expected else {
        return Ok(());
    };
    for (name, schema) in actual {
        let expected = entries.get(name);
        if expected.is_none_or(|expected| !schema_eq(schema, expected)) {
            return Err(format!(
                "{at}: {what}.{name} is {schema}, expected {expected:?}"
            ));
        }
    }
    Ok(())
}

/// Checks every `$ref` of `schema` is local and resolves.
///
/// Fragments are percent-decoded first, as a URI reference requires.
pub fn self_contained(schema: &Value) -> Result<(), String> {
    let mut refs = Vec::new();
    collect_refs(schema, &mut refs);
    if refs.is_empty() {
        return Err("the validation schema has no $ref".to_owned());
    }
    let resolves = |target: &str| {
        target
            .strip_prefix('#')
            .and_then(|fragment| percent_decode_str(fragment).decode_utf8().ok())
            .is_some_and(|pointer| schema.pointer(&pointer).is_some())
    };
    match refs.into_iter().find(|target| !resolves(target)) {
        Some(target) => Err(format!(
            "$ref '{target}' does not resolve in the validation schema"
        )),
        None => Ok(()),
    }
}

fn collect_refs<'a>(node: &'a Value, out: &mut Vec<&'a str>) {
    match node {
        Value::Object(map) => {
            for (key, value) in map {
                match value {
                    Value::String(target) if key == "$ref" => out.push(target),
                    _ => collect_refs(value, out),
                }
            }
        }
        Value::Array(items) => items.iter().for_each(|item| collect_refs(item, out)),
        _ => {}
    }
}

/// `loaded` defines what `builtin` does: id, version, component shapes, function signatures.
///
/// Component shapes are compared without the envelope keys `id` and `component`.
pub fn matches_builtin(loaded: &Catalog, builtin: &Catalog) -> Result<(), String> {
    if (loaded.id(), loaded.protocol_version()) != (builtin.id(), builtin.protocol_version()) {
        return Err(format!(
            "catalog {} {} differs from the builtin {} {}",
            loaded.id(),
            loaded.protocol_version(),
            builtin.id(),
            builtin.protocol_version()
        ));
    }
    let names = |catalog: &Catalog| -> BTreeSet<String> {
        catalog
            .components()
            .map(|def| def.name().to_owned())
            .collect()
    };
    if names(loaded) != names(builtin) {
        return Err("component names differ from the builtin catalog".to_owned());
    }
    for name in names(builtin) {
        if shape(loaded, &name) != shape(builtin, &name) {
            return Err(format!("component {name} differs from the builtin catalog"));
        }
    }
    let signatures = |catalog: &Catalog| {
        catalog
            .functions()
            .map(|def| {
                let signature = (
                    def.return_type().map(str::to_owned),
                    def.allowed_callers(),
                    def.requires_user_activation(),
                );
                (def.name().to_owned(), signature)
            })
            .collect::<Vec<_>>()
    };
    if signatures(loaded) == signatures(builtin) {
        Ok(())
    } else {
        Err("function signatures differ from the builtin catalog".to_owned())
    }
}

fn shape(catalog: &Catalog, name: &str) -> Option<(BTreeSet<String>, BTreeSet<String>)> {
    let shape = catalog.component(name)?.shape();
    let strip = |names: &BTreeSet<String>| -> BTreeSet<String> {
        names
            .iter()
            .filter(|key| !matches!(key.as_str(), "id" | "component"))
            .cloned()
            .collect()
    };
    Some((strip(shape.properties()), strip(shape.required())))
}

#[cfg(test)]
#[path = "catalog_checks_tests.rs"]
mod tests;
