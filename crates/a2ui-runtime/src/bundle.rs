//! `$ref` rules of v1.0 catalogs, and the local references the runtime writes.
//!
//! A v1.0 catalog may reference itself (`#…`, `catalog.json#…`) and `common_types.json` (by a
//! relative or the published absolute URI). Localizing rewrites those to `#` fragments,
//! percent-encoded as URI references require (a raw `ℓ` makes the schema build fail).
//! Submodules: `walk` (the `$ref` walks), `check` (load rule 4), `target` (what a `$ref` names).

use std::borrow::Cow;

use percent_encoding::{AsciiSet, CONTROLS, percent_decode_str, utf8_percent_encode};
use serde_json::{Map, Value, json};

mod check;
mod target;
mod walk;

pub(crate) use check::check_references;
pub(crate) use target::{NotASchema, first_not_a_schema, resolve};
pub(crate) use walk::{Segment, for_each_ref, node_at, pointer, rewrite_refs};

/// The `common_types.json` URIs a v1.0 catalog may reference.
const COMMON_TYPES_URIS: [&str; 3] = [
    "common_types.json",
    "./common_types.json",
    "https://a2ui.org/specification/v1_0/common_types.json",
];

/// Characters a URI fragment cannot hold literally; non-ASCII bytes are always encoded.
const FRAGMENT: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'#')
    .add(b'%')
    .add(b'<')
    .add(b'>')
    .add(b'[')
    .add(b'\\')
    .add(b']')
    .add(b'^')
    .add(b'`')
    .add(b'{')
    .add(b'|')
    .add(b'}');

/// Whether a v1.0 catalog may use `reference` as a `$ref` target.
///
/// The reference needs a fragment: a whole document is never a target.
pub(crate) fn is_supported_ref(reference: &str) -> bool {
    reference.split_once('#').is_some_and(|(document, _)| {
        document.is_empty() || document == "catalog.json" || COMMON_TYPES_URIS.contains(&document)
    })
}

/// A `$ref` to the JSON Pointer `pointer` of the same document, such as `#/$defs/Child`.
pub(crate) fn local_ref(pointer: &str) -> String {
    format!("#{}", utf8_percent_encode(pointer, FRAGMENT))
}

/// Rewrites every `$ref` string under `node` with [`localize_ref`].
pub(crate) fn localize_refs(node: &mut Value) {
    rewrite_refs(node, &mut |reference| *reference = localize_ref(reference));
}

/// An `anyFunction` for checking a call with no catalog in scope: any function but `@index`.
pub(crate) fn any_function_envelope() -> Value {
    json!({
        "type": "object",
        "properties": {
            "@call": {"not": {"const": "@index"}},
            "args": {"type": "object"}
        }
    })
}

/// The entry sections of a catalog: definitions, components and functions.
const ENTRY_SECTIONS: [&str; 3] = ["$defs", "components", "functions"];

/// The decoded JSON Pointer a supported `$ref` names, such as `/$defs/Child`.
///
/// `None` for an unsupported reference, for a fragment that is neither empty nor a JSON
/// Pointer (an anchor such as `#Name`), and for one whose escapes are not UTF-8.
pub(crate) fn pointer_of(reference: &str) -> Option<String> {
    let (_, fragment) = reference
        .split_once('#')
        .filter(|(_, fragment)| is_supported_ref(reference) && is_pointer_fragment(fragment))?;
    percent_decode_str(fragment)
        .decode_utf8()
        .ok()
        .map(Cow::into_owned)
}

/// Whether `fragment` is empty or a JSON Pointer, the only fragments v1.0 supports.
fn is_pointer_fragment(fragment: &str) -> bool {
    fragment.is_empty() || fragment.starts_with('/')
}

/// The section and decoded name of the entry a JSON Pointer falls in.
///
/// `/components/Card/properties/child` falls in `("components", "Card")`. `None` when the
/// pointer names no node inside a `$defs`, `components` or `functions` entry.
pub(crate) fn entry_of(pointer: &str) -> Option<(&'static str, String)> {
    let (section, rest) = pointer.strip_prefix('/')?.split_once('/')?;
    let section = ENTRY_SECTIONS.into_iter().find(|known| *known == section)?;
    let token = rest.split_once('/').map_or(rest, |(token, _)| token);
    Some((section, unescape(token)))
}

/// A JSON Pointer reference token with `~1` and `~0` unescaped (RFC 6901, section 4).
fn unescape(token: &str) -> String {
    token.replace("~1", "/").replace("~0", "~")
}

/// `reference` as a local, percent-encoded `$ref`; any other one is returned unchanged.
pub(crate) fn localize_ref(reference: &str) -> String {
    pointer_of(reference).map_or_else(|| reference.to_owned(), |pointer| local_ref(&pointer))
}

/// Copies into `defs` each common type that `roots`, or a copied type, names as `#/$defs/<name>`.
///
/// `roots` must already be local. A name `defs` already holds is left alone, so a cycle
/// between common types ends; a name found in neither stays unresolved, and load reports the
/// dangling `$ref`: rule 11 for kept entries, the validation schema's check for the rest.
pub(crate) fn add_common_types(
    defs: &mut Map<String, Value>,
    roots: &[&Value],
    common: &Map<String, Value>,
) {
    let mut pending = Vec::new();
    for root in roots {
        def_names(root, &mut pending);
    }
    while let Some(name) = pending.pop() {
        if defs.contains_key(&name) {
            continue;
        }
        let Some(definition) = common.get(&name) else {
            continue;
        };
        let mut definition = definition.clone();
        localize_refs(&mut definition);
        def_names(&definition, &mut pending);
        defs.insert(name, definition);
    }
}

/// Appends to `out` the `$defs` entry each `$ref` under `node` names, data included.
pub(crate) fn def_names(node: &Value, out: &mut Vec<String>) {
    for_each_ref(node, &mut |reference| out.extend(def_name(reference)));
}

/// The `$defs` entry a local `$ref` names, decoded and unescaped.
fn def_name(reference: &str) -> Option<String> {
    entry_of(&pointer_of(reference)?)
        .filter(|(section, _)| *section == "$defs")
        .map(|(_, name)| name)
}

/// The `$defs` entry a local `$ref` names as a whole, decoded and unescaped.
///
/// `None` for a pointer below an entry (`#/$defs/A/properties`): only a whole entry is a mixin
/// or a described common type, because only a whole entry's `$ref`s were followed by `expand`.
pub(crate) fn whole_def_name(reference: &str) -> Option<String> {
    let pointer = pointer_of(reference)?;
    let token = pointer.strip_prefix("/$defs/")?;
    (!token.contains('/')).then(|| unescape(token))
}

/// A `FunctionCall` checking only the envelope: `@call`, `catalogId`, `args`, nothing else.
///
/// A call may run in another catalog, so a component check cannot judge its function; the
/// nested-call walk (Task 5) checks it against the catalog it runs in. It replaces
/// `FunctionCall` itself, not only `anyFunction` as Task 2's `any_function_envelope` does:
/// `FunctionCall` → `IndexSystemFunction` → `DynamicNumber` → `FunctionCall` is a cycle of the
/// common types, which load rule 11 rejects.
pub(crate) fn function_call_envelope() -> Value {
    json!({
        "type": "object",
        "allOf": [{"$ref": local_ref("/$defs/FunctionCommon")}],
        "properties": {"args": {"type": "object"}},
        "unevaluatedProperties": false
    })
}

#[cfg(test)]
#[path = "bundle_tests.rs"]
mod tests;
