//! The validation schema: the checked bundle in upstream's flattened shape.
//!
//! [`build`] takes a [`Checked`] bundle, so it only runs after `compile_entries` enforced the
//! expansion caps on every `$ref` it follows (see `flatten`). The result keeps `$defs` no entry
//! reaches, which those caps never budgeted, so it is output only: nothing compiles it. It
//! refuses a result holding a `$ref` that does not resolve in it.

use std::collections::BTreeSet;

use serde_json::{Map, Value};

use super::compile::Checked;
use super::flatten::flattened;
use crate::bundle::{
    add_common_types, def_names, first_unresolved, local_ref, localize_refs, pointer_of,
    rewrite_refs, whole_def_name,
};
use crate::error::{Error, MalformedCatalogSnafu};
use crate::quoted::Quoted;
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

/// What the catalog document adds to the checked bundle's validation schema.
pub(super) struct Header<'a> {
    /// The effective catalog id.
    pub(super) id: &'a str,
    /// The effective protocol version.
    pub(super) version: &'a str,
    /// The catalog's `instructions`, if any.
    pub(super) instructions: Option<&'a str>,
    /// The catalog's `theme` schema, if any.
    pub(super) theme: Option<&'a Value>,
    /// The catalog's own `$defs`, as authored, if any.
    pub(super) defs: Option<&'a Map<String, Value>>,
}

/// Builds the validation schema of a checked bundle under the catalog's `header`.
///
/// A `MalformedCatalog` error reports the first `$ref` of the result that names no node of it
/// (`#/$defs/Missing`, `#/theme` with no theme, a dropped component), as the author wrote it
/// and where. Load rule 11 already resolved those the kept entries reach, so one under
/// `/components` or `/functions` would be a runtime bug; this completes rule 11 for the theme
/// and unreached definitions.
pub(super) fn build(
    checked: Checked<'_>,
    header: Header<'_>,
    common: &Map<String, Value>,
) -> Result<Value, Error> {
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
    if let Some(theme) = header.theme {
        defs.insert("theme".to_owned(), theme_def(theme, common));
        defs.values_mut().for_each(retarget_theme);
    }
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
        ("catalogId".to_owned(), Value::from(header.id)),
        ("protocolVersion".to_owned(), Value::from(header.version)),
    ]);
    if let Some(text) = header.instructions.filter(|text| !text.is_empty()) {
        out.insert("instructions".to_owned(), Value::from(text));
    }
    out.insert("components".to_owned(), Value::Object(components));
    if !functions.is_empty() {
        out.insert("functions".to_owned(), Value::Object(functions));
    }
    out.insert("$defs".to_owned(), Value::Object(defs));
    let out = Value::Object(out);
    check_resolved(&out, header.theme, header.defs)?;
    Ok(out)
}

/// Fails on the first `$ref` string of `schema` that names no node of `schema`.
///
/// The error names the `$ref` as authored in `theme` or `defs`, at its authored place. A `$ref`
/// the runtime wrote (a component's, a common type's, a synthesized union's) has no authored
/// place: it is named as the validation schema holds it, and only a runtime bug can leave one
/// dangling.
fn check_resolved(
    schema: &Value,
    theme: Option<&Value>,
    defs: Option<&Map<String, Value>>,
) -> Result<(), Error> {
    let Some((reference, at)) = first_unresolved(schema) else {
        return Ok(());
    };
    let reason = match authored(&at, theme, defs) {
        Some((authored_at, authored_ref)) => format!(
            "$ref '{}' at {} does not resolve in the validation schema",
            Quoted::new(authored_ref),
            Quoted::new(&authored_at)
        ),
        None => format!(
            "$ref '{}' at {} of the validation schema does not resolve",
            Quoted::new(reference),
            Quoted::new(&at)
        ),
    };
    Err(MalformedCatalogSnafu { reason }.build().into())
}

/// The authored place and `$ref` of the validation schema's `$ref` at `at`, if authored.
///
/// The theme moves to `/$defs/theme` and a definition stays at `/$defs/<name>`; both are copied
/// with every node at its path (localizing and describing only rewrite or add values).
fn authored<'a>(
    at: &str,
    theme: Option<&'a Value>,
    defs: Option<&'a Map<String, Value>>,
) -> Option<(String, &'a str)> {
    let rest = at.strip_prefix("/$defs/")?;
    let (token, below) = rest
        .find('/')
        .map_or((rest, ""), |slash| rest.split_at(slash));
    let (place, node) = match theme {
        Some(theme) if token == "theme" => (format!("/theme{below}"), theme.pointer(below)?),
        _ => {
            let name = token.replace("~1", "/").replace("~0", "~");
            (at.to_owned(), defs?.get(&name)?.pointer(below)?)
        }
    };
    node.as_str().map(|reference| (place, reference))
}

/// The theme as upstream writes it in `$defs`: localized, described, open unless it says.
///
/// An object schema with neither `additionalProperties` nor `unevaluatedProperties` gets
/// `additionalProperties: true`; any other theme is copied as it is.
fn theme_def(theme: &Value, common: &Map<String, Value>) -> Value {
    let mut theme = theme.clone();
    localize_refs(&mut theme);
    describe(&mut theme, common);
    if let Value::Object(map) = &mut theme
        && !map.contains_key("additionalProperties")
        && !map.contains_key("unevaluatedProperties")
    {
        map.insert("additionalProperties".to_owned(), Value::Bool(true));
    }
    theme
}

/// Points each `$ref` into `#/theme` at the same node under `#/$defs/theme`.
///
/// `theme_def` only adds keys, so every node of the authored theme keeps its path there. Load
/// rule 11 keeps such refs out of components, functions and the definitions they reach, so only
/// `$defs` (the theme itself, definitions no entry reaches) can hold one. Like
/// [`localize_refs`], it rewrites every `$ref` string, data included.
fn retarget_theme(node: &mut Value) {
    rewrite_refs(node, &mut |reference| {
        if let Some(rest) = pointer_of(reference)
            .as_deref()
            .and_then(|pointer| pointer.strip_prefix("/theme"))
            .filter(|rest| rest.is_empty() || rest.starts_with('/'))
        {
            *reference = local_ref(&format!("/$defs/theme{rest}"));
        }
    });
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

#[cfg(test)]
#[path = "validation_schema_depth_tests.rs"]
mod depth_tests;
