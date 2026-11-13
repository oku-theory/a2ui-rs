//! `$ref` rules of v1.0 catalogs, and the local references the runtime writes.
//!
//! A v1.0 catalog may reference itself (`#…`, `catalog.json#…`) and `common_types.json` (by a
//! relative or the published absolute URI). Localizing rewrites those to `#` fragments,
//! percent-encoded as URI references require (a raw `ℓ` makes the schema build fail).
//! Submodules: `walk` (the `$ref` walks), `check` (load rule 4), `ref_pointer` (a `$ref`'s
//! decoded tokens), `target` (what a `$ref` names), `position` (which keywords hold schemas,
//! and which data).

use std::borrow::Cow;

use percent_encoding::{AsciiSet, CONTROLS, percent_decode_str, utf8_percent_encode};
use serde_json::{Map, Value};

mod check;
mod position;
mod ref_pointer;
mod target;
mod walk;

pub(crate) use check::check_references;
pub(crate) use position::{SCHEMA_LISTS, SCHEMA_MAPS, SCHEMA_ONE};
pub(crate) use ref_pointer::RefPointer;
pub(crate) use target::{NotASchema, first_not_a_schema, resolve};
pub(crate) use walk::{REF_KEY, Segment, for_each_ref, node_at, pointer, rewrite_refs};

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

/// `reference` as a local, percent-encoded `$ref`; any other one is returned unchanged.
pub(crate) fn localize_ref(reference: &str) -> String {
    pointer_of(reference).map_or_else(|| reference.to_owned(), |pointer| local_ref(&pointer))
}

/// The `$ref` the validation schema holds for `reference`, authored in `$defs` or the theme.
///
/// Localized, then pointed at the same node under `#/$defs/theme` when the catalog has a
/// theme (`theme_def` only adds keys, so every theme node keeps its path there). The
/// validation schema's `build` writes every `$defs` and theme `$ref` with it, `authored`
/// rebuilds with it, and load rule 4 refuses a `$ref` in their data it would change (plan 2b,
/// Decision 19), so a new rewrite of those `$ref`s belongs here, once. It is idempotent, so
/// the bundle's own localizing of definitions changes nothing.
pub(crate) fn written_ref(reference: &str, with_theme: bool) -> String {
    let local = localize_ref(reference);
    if with_theme && let Some(target) = theme_target(&local) {
        target
    } else {
        local
    }
}

/// The `$ref` to the same node under `#/$defs/theme`, for a `$ref` into `#/theme`.
fn theme_target(reference: &str) -> Option<String> {
    pointer_of(reference)
        .as_deref()
        .and_then(|pointer| pointer.strip_prefix("/theme"))
        .filter(|rest| rest.is_empty() || rest.starts_with('/'))
        .map(|rest| local_ref(&format!("/$defs/theme{rest}")))
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
    let pointer = RefPointer::of(reference)?;
    let (section, name) = pointer.entry()?;
    (section == "$defs").then(|| name.to_owned())
}

/// The `$defs` entry a local `$ref` names as a whole, decoded and unescaped.
///
/// `None` for a pointer below an entry (`#/$defs/A/properties`): only a whole entry is a mixin
/// or a described common type, because only a whole entry's `$ref`s were followed by `expand`.
pub(crate) fn whole_def_name(reference: &str) -> Option<String> {
    match RefPointer::of(reference)?.tokens() {
        [section, name] if section == "$defs" => Some(name.clone()),
        _ => None,
    }
}

#[cfg(test)]
#[path = "bundle_tests.rs"]
mod tests;
