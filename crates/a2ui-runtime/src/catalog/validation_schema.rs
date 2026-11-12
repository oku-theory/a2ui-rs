//! The validation schema: the checked bundle in upstream's flattened shape.
//!
//! [`build`] takes a [`Checked`] bundle, so it only runs after `compile_entries` enforced the
//! expansion caps on every `$ref` it follows (see `flatten`). The result keeps `$defs` no entry
//! reaches, which those caps never budgeted, so it is output only: nothing compiles it. It
//! refuses a result holding a `$ref` that does not name a schema (an object or a boolean) of it.

use std::collections::BTreeSet;

use serde_json::{Map, Value};

use super::compile::Checked;
use super::flatten::flattened;
use crate::bundle::{
    NotASchema, Segment, add_common_types, def_names, first_not_a_schema, local_ref, localize_ref,
    localize_refs, node_at, pointer, pointer_of, rewrite_refs, whole_def_name,
};
use crate::error::{Error, MalformedCatalogSnafu};
use crate::quoted::{Quoted, reason};
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
/// (`#/$defs/Missing`, `#/theme` with no theme, a dropped component) or a node that is not a
/// schema (`#/$defs/X/title`), as the author wrote it and where (plan 2b, Decision 14). Load
/// rule 11 and the compile already refused those the kept entries reach, so one under
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
    check_refs_name_schemas(&out, header.theme, header.defs)?;
    Ok(out)
}

/// Fails on the first `$ref` string of `schema` that names no schema of `schema`.
///
/// The error names the `$ref` as authored in `theme` or `defs`, at its authored place. A `$ref`
/// the runtime wrote (a component's, a common type's, a synthesized union's) has no authored
/// place: it is named as the validation schema holds it, and only a runtime bug can leave one
/// dangling.
fn check_refs_name_schemas(
    schema: &Value,
    theme: Option<&Value>,
    defs: Option<&Map<String, Value>>,
) -> Result<(), Error> {
    let Some((reference, path, found)) = first_not_a_schema(schema) else {
        return Ok(());
    };
    // An authored place is the author's; a runtime-written one is the validation schema's.
    let (reference, at, of_schema, in_schema) = match authored(&path, reference, theme, defs) {
        Some((authored_at, authored_ref)) => (
            Quoted::new(authored_ref),
            Quoted::new(&authored_at),
            "",
            " in the validation schema",
        ),
        None => (
            Quoted::new(reference),
            Quoted::new(&pointer(&path)),
            " of the validation schema",
            "",
        ),
    };
    let kind = match found {
        NotASchema::Missing => None,
        NotASchema::String => Some("a string"),
        NotASchema::Number => Some("a number"),
        NotASchema::Array => Some("an array"),
        NotASchema::Null => Some("null"),
    };
    let reason = match kind {
        None => reason![
            "$ref '",
            reference,
            "' at ",
            at,
            of_schema,
            " does not resolve",
            in_schema
        ],
        Some(kind) => reason![
            "$ref '",
            reference,
            "' at ",
            at,
            of_schema,
            " names ",
            kind,
            in_schema,
            ", not a schema (an object or a boolean)",
        ],
    };
    Err(MalformedCatalogSnafu { reason }.build().into())
}

/// The authored place and `$ref` behind the validation schema's `written` `$ref` at `path`.
///
/// The theme moves to `/$defs/theme` and a definition stays at `/$defs/<name>`; both are copied
/// with every node at its path (localizing and describing only rewrite or add values). The
/// authored node is taken only when building it again gives `written`: localized, then pointed
/// at `#/$defs/theme` when there is a theme. `None`: no authored `$ref` rebuilds to it, so
/// the runtime wrote it.
fn authored<'a>(
    path: &[Segment<'_>],
    written: &str,
    theme: Option<&'a Value>,
    defs: Option<&'a Map<String, Value>>,
) -> Option<(String, &'a str)> {
    let [Segment::Key("$defs"), Segment::Key(name), below @ ..] = path else {
        return None;
    };
    let (node, in_theme) = match theme {
        Some(theme) if *name == "theme" => (theme, true),
        _ => (defs?.get(*name)?, false),
    };
    let reference = node_at(node, below)?.as_str()?;
    let rebuilt = localize_ref(reference);
    let rebuilt = match theme {
        Some(_) => theme_target(&rebuilt).unwrap_or(rebuilt),
        None => rebuilt,
    };
    if rebuilt != written {
        return None;
    }
    let place = if in_theme {
        format!("/theme{}", pointer(below))
    } else {
        pointer(path)
    };
    Some((place, reference))
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
/// [`localize_refs`], it rewrites every `$ref` string, data included. [`authored`] rebuilds
/// refs in this same order (localize, then retarget): a new `$defs` rewrite belongs there too.
fn retarget_theme(node: &mut Value) {
    rewrite_refs(node, &mut |reference| {
        if let Some(target) = theme_target(reference) {
            *reference = target;
        }
    });
}

/// The `$ref` to the same node under `#/$defs/theme`, for a `$ref` into `#/theme`.
fn theme_target(reference: &str) -> Option<String> {
    pointer_of(reference)
        .as_deref()
        .and_then(|pointer| pointer.strip_prefix("/theme"))
        .filter(|rest| rest.is_empty() || rest.starts_with('/'))
        .map(|rest| local_ref(&format!("/$defs/theme{rest}")))
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

#[cfg(test)]
#[path = "validation_schema_ref_tests.rs"]
mod ref_tests;
