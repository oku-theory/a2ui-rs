//! Flattening a component schema into one object, as upstream's validation schema writes it.
//!
//! A component mixes in common types (`ComponentCommon`, `Checkable`), own `$defs` and inline
//! schemas through `allOf`. Flattening merges their `properties` and `required`, writes the
//! `id` and `component` keys itself, drops `catalogId` and keeps what it cannot merge in a
//! residual `allOf`. A later piece's property replaces an earlier one where `allOf` would
//! require both, so the result can accept instances the entry's validator rejects.
//!
//! Unlike [`ComponentShape`](super::ComponentShape), which lists every property name the
//! validator may see through any `$ref`, flattening follows only `allOf` members that are a
//! whole-entry `$ref`, as upstream does.
//!
//! It runs only on a bundle `compile_entries` accepted: every mixin it follows is a whole
//! `$defs` entry that `expand` walked from the same site, so its copies stay within
//! `MAX_CATALOG_EXPANSION` and its recursion within `MAX_SCHEMA_DEPTH` / 3 mixin hops.

use std::collections::BTreeSet;

use serde_json::{Map, Value, json};

use crate::bundle::{REF_KEY, local_ref, whole_def_name};

/// Properties the protocol owns: flattening writes `id` and `component` and drops `catalogId`.
///
/// Unlike the instance envelope `validate` strips, `metadata` is a catalog property here.
const PROTOCOL_KEYS: [&str; 3] = ["id", "component", "catalogId"];

/// Keys flattening merges or rewrites; a component's other keys are copied.
const MERGED: [&str; 9] = [
    "type",
    "description",
    "allOf",
    "properties",
    "required",
    "unevaluatedProperties",
    "additionalProperties",
    "allowedParents",
    "allowedChildren",
];

/// The union definitions, never merged as mixins.
const UNIONS: [&str; 2] = ["anyComponent", "anyFunction"];

/// Where a piece comes from; pieces merge in this order, so a later kind wins.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Kind {
    Mixin,
    Local,
    Common,
    Specific,
}

/// The `properties`, `required` and `description` one schema contributes.
struct Piece<'a> {
    kind: Kind,
    properties: Option<&'a Map<String, Value>>,
    required: Option<&'a Vec<Value>>,
    description: Option<&'a str>,
}

/// What collecting one component found.
#[derive(Default)]
struct Collected<'a> {
    pieces: Vec<Piece<'a>>,
    residual: Vec<Value>,
    /// Every `$defs` entry followed as a mixin, common or own.
    seen: BTreeSet<String>,
    /// The own `$defs` entries merged in.
    merged: BTreeSet<String>,
}

/// A flattened component and the own `$defs` it merged in as mixins.
pub(super) struct Flattened {
    /// The flattened schema.
    pub(super) schema: Map<String, Value>,
    /// Names of the own `$defs` merged in, which `$defs` may then drop.
    pub(super) merged: BTreeSet<String>,
}

/// Flattens the component `name`; `defs` is the bundle's `$defs`, `common` the common types.
///
/// A loaded entry is always an object; any other value reads as an object with no keys.
pub(super) fn flattened(
    name: &str,
    schema: &Value,
    defs: &Map<String, Value>,
    common: &Map<String, Value>,
) -> Flattened {
    let mut found = Collected::default();
    collect(schema, defs, common, false, true, &mut found);
    found.pieces.sort_by_key(|piece| piece.kind);
    let mut properties = Map::new();
    let mut required = BTreeSet::new();
    let mut description = None;
    let mut constant = Value::from(name);
    for piece in &found.pieces {
        for (key, value) in piece.properties.into_iter().flatten() {
            if key == "component" {
                constant = value
                    .get("const")
                    .cloned()
                    .unwrap_or_else(|| Value::from(name));
            }
            if !PROTOCOL_KEYS.contains(&key.as_str()) {
                properties.insert(key.clone(), value.clone());
            }
        }
        let names = piece
            .required
            .into_iter()
            .flatten()
            .filter_map(Value::as_str);
        required.extend(
            names
                .filter(|name| !PROTOCOL_KEYS.contains(name))
                .map(str::to_owned),
        );
        if let Some(text) = piece.description.filter(|text| !text.is_empty()) {
            description = Some(text);
        }
    }
    properties.insert(
        "id".to_owned(),
        json!({"$ref": local_ref("/$defs/ComponentId")}),
    );
    properties.insert("component".to_owned(), json!({"const": constant}));
    let required: Vec<Value> = ["id".to_owned()]
        .into_iter()
        .chain(required)
        .chain(["component".to_owned()])
        .map(Value::from)
        .collect();
    let mut out = Map::from_iter([("type".to_owned(), Value::from("object"))]);
    for (key, value) in schema.as_object().into_iter().flatten() {
        if !MERGED.contains(&key.as_str()) {
            out.insert(key.clone(), value.clone());
        }
    }
    if let Some(text) = description {
        out.insert("description".to_owned(), Value::from(text));
    }
    out.insert("properties".to_owned(), Value::Object(properties));
    out.insert("required".to_owned(), Value::Array(required));
    if !found.residual.is_empty() {
        out.insert("allOf".to_owned(), Value::Array(found.residual));
    }
    let closed = schema
        .get("unevaluatedProperties")
        .or_else(|| schema.get("additionalProperties"));
    if let Some(closed) = closed {
        out.insert("unevaluatedProperties".to_owned(), closed.clone());
    }
    for key in ["allowedParents", "allowedChildren"] {
        let listed = schema
            .get(key)
            .filter(|list| list.as_array().is_some_and(|items| !items.is_empty()));
        if let Some(listed) = listed {
            out.insert(key.to_owned(), listed.clone());
        }
    }
    Flattened {
        schema: out,
        merged: found.merged,
    }
}

/// Collects the pieces of `schema` and of the `allOf` members it can merge.
///
/// `local` marks a schema reached through an own mixin; `top` the component itself, whose
/// unmerged keys the result copies rather than the residual `allOf`. Recursion follows inline
/// members (bounded by the document's 64 levels) and whole-entry mixins, each once per
/// component (bounded by `expand`'s depth cap, see the module documentation).
fn collect<'a>(
    schema: &'a Value,
    defs: &'a Map<String, Value>,
    common: &Map<String, Value>,
    local: bool,
    top: bool,
    out: &mut Collected<'a>,
) {
    let members = schema.get("allOf").and_then(Value::as_array);
    for member in members.into_iter().flatten() {
        if !member.is_object() {
            out.residual.push(member.clone());
            continue;
        }
        if member.get(REF_KEY).is_none() {
            collect(member, defs, common, local, false, out);
            continue;
        }
        match mixin_target(member, defs) {
            Some((name, target)) if common.contains_key(&name) => {
                let Some(properties) = target.get("properties").and_then(Value::as_object) else {
                    out.residual.push(member.clone());
                    continue;
                };
                if out.seen.insert(name) {
                    out.pieces.push(Piece {
                        kind: Kind::Mixin,
                        properties: Some(properties),
                        required: target.get("required").and_then(Value::as_array),
                        description: None,
                    });
                }
            }
            Some((name, target)) => {
                if out.seen.insert(name.clone()) {
                    out.merged.insert(name);
                    collect(target, defs, common, true, false, out);
                }
            }
            None => out.residual.push(member.clone()),
        }
    }
    if !top {
        let rest: Map<String, Value> = schema
            .as_object()
            .into_iter()
            .flatten()
            .filter(|(key, _)| !MERGED.contains(&key.as_str()))
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect();
        if !rest.is_empty() {
            out.residual.push(Value::Object(rest));
        }
    }
    if ["properties", "required", "description"]
        .iter()
        .any(|key| schema.get(*key).is_some())
    {
        let properties = schema.get("properties").and_then(Value::as_object);
        let kind = if local {
            Kind::Local
        } else if properties.is_some_and(|map| map.contains_key("component")) {
            Kind::Specific
        } else {
            Kind::Common
        };
        out.pieces.push(Piece {
            kind,
            properties,
            required: schema.get("required").and_then(Value::as_array),
            description: schema.get("description").and_then(Value::as_str),
        });
    }
}

/// The whole `$defs` entry a member names, when `$ref` is its only key and it is no union.
fn mixin_target<'a>(member: &Value, defs: &'a Map<String, Value>) -> Option<(String, &'a Value)> {
    let member = member.as_object()?;
    if member.len() != 1 {
        return None;
    }
    let name = whole_def_name(member.get(REF_KEY)?.as_str()?)?;
    if UNIONS.contains(&name.as_str()) {
        return None;
    }
    let target = defs.get(&name)?;
    Some((name, target))
}

#[cfg(test)]
#[path = "flatten_tests.rs"]
mod tests;
