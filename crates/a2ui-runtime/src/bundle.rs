//! `$ref` rules of v1.0 catalogs, and the local references the runtime writes.
//!
//! A v1.0 catalog may reference itself (`#…`, `catalog.json#…`) and `common_types.json` (by a
//! relative or the published absolute URI). Localizing rewrites those to `#` fragments,
//! percent-encoded as URI references require (a raw `ℓ` makes the schema build fail).

use std::borrow::Cow;

use percent_encoding::{AsciiSet, CONTROLS, percent_decode_str, utf8_percent_encode};
use serde_json::{Map, Value, json};
use snafu::ensure;

use crate::error::{Error, ExternalRefSnafu, ForbiddenKeywordSnafu, MalformedCatalogSnafu};
use crate::quoted::Quoted;

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

/// Calls `visit` on each `$ref` string and its path, until one returns `Some`.
///
/// A key holding an object or array is walked whatever its name, so a property named `$ref`
/// holding a schema is a property; every `$ref` string is visited, data included, since the
/// walk does not tell schemas from data. Keys are visited in map order, and `pointer(path)`
/// spells a path within this module. It recurses: callers pass a document that load rule 1
/// bounded to `MAX_JSON_DEPTH` levels, or one built from it.
/// [`check_references`] has its own walk, as it reads every reference keyword, not only `$ref`.
pub(crate) fn find_ref<'a, T>(
    node: &'a Value,
    visit: &mut impl FnMut(&'a str, &[Segment<'a>]) -> Option<T>,
) -> Option<T> {
    find_ref_under(node, &mut Vec::new(), visit)
}

fn find_ref_under<'a, T>(
    node: &'a Value,
    path: &mut Vec<Segment<'a>>,
    visit: &mut impl FnMut(&'a str, &[Segment<'a>]) -> Option<T>,
) -> Option<T> {
    match node {
        Value::Object(map) => map.iter().find_map(|(key, value)| {
            path.push(Segment::Key(key));
            let found = match value {
                Value::String(reference) if key == "$ref" => visit(reference, path),
                _ => find_ref_under(value, path, visit),
            };
            path.pop();
            found
        }),
        Value::Array(items) => items.iter().enumerate().find_map(|(index, item)| {
            path.push(Segment::Index(index));
            let found = find_ref_under(item, path, visit);
            path.pop();
            found
        }),
        _ => None,
    }
}

/// Calls `visit` on every `$ref` string under `node`, as [`find_ref`] visits them.
pub(crate) fn for_each_ref<'a>(node: &'a Value, visit: &mut impl FnMut(&'a str)) {
    find_ref(node, &mut |reference, _| {
        visit(reference);
        None::<()>
    });
}

/// Calls `rewrite` on each `$ref` string under `node`, as [`find_ref`] visits them.
///
/// It recurses under the same depth bound as [`find_ref`].
pub(crate) fn rewrite_refs(node: &mut Value, rewrite: &mut impl FnMut(&mut String)) {
    match node {
        Value::Object(map) => {
            for (key, value) in map.iter_mut() {
                match value {
                    Value::String(reference) if key == "$ref" => rewrite(reference),
                    _ => rewrite_refs(value, rewrite),
                }
            }
        }
        Value::Array(items) => items
            .iter_mut()
            .for_each(|item| rewrite_refs(item, rewrite)),
        _ => {}
    }
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

const ENTRY_SECTIONS: [&str; 3] = ["$defs", "components", "functions"];

/// The keywords the reference check reads when they hold a scalar.
const KEYWORDS: [&str; 7] = [
    "$anchor",
    "$dynamicAnchor",
    "$recursiveAnchor",
    "$dynamicRef",
    "$recursiveRef",
    "$id",
    "$schema",
];

const ANCHORS: &str = "anchors are not supported; use a JSON Pointer";
const DYNAMIC: &str = "dynamic references are not supported; use $ref";
const NESTED: &str = "only the catalog's top level may declare it";

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

fn is_pointer_fragment(fragment: &str) -> bool {
    fragment.is_empty() || fragment.starts_with('/')
}

/// The node of `root` a supported `$ref` names, decoding its fragment.
pub(crate) fn resolve<'a>(root: &'a Value, reference: &str) -> Option<&'a Value> {
    root.pointer(&pointer_of(reference)?)
}

/// The first unresolvable `$ref` string under `root`, with its JSON Pointer.
///
/// It only looks pointers up, never follows one, so its cost is linear in `root` whatever the
/// `$ref`s chain. The pointer is built only for the `$ref` found. It walks with [`find_ref`].
pub(crate) fn first_unresolved(root: &Value) -> Option<(&str, String)> {
    find_ref(root, &mut |reference, path| {
        resolve(root, reference)
            .is_none()
            .then(|| (reference, pointer(path)))
    })
}

/// The section and decoded name of the entry a JSON Pointer falls in.
///
/// `/components/Card/properties/child` falls in `("components", "Card")`. `None` when the
/// pointer names no node inside a `$defs`, `components` or `functions` entry.
pub(crate) fn entry_of(pointer: &str) -> Option<(&'static str, String)> {
    let (section, rest) = pointer.strip_prefix('/')?.split_once('/')?;
    let section = ENTRY_SECTIONS.into_iter().find(|known| *known == section)?;
    let token = rest.split_once('/').map_or(rest, |(token, _)| token);
    Some((section, token.replace("~1", "/").replace("~0", "~")))
}

/// `reference` as a local, percent-encoded `$ref`; any other one is returned unchanged.
pub(crate) fn localize_ref(reference: &str) -> String {
    pointer_of(reference).map_or_else(|| reference.to_owned(), |pointer| local_ref(&pointer))
}

/// Checks the reference keywords of a catalog document (load rule 4).
///
/// A key holding an object or array is walked whatever its name, so a property named `$ref`
/// holding a schema is a property. A scalar under a keyword is checked: `$ref` must be a
/// supported string with an empty or JSON Pointer fragment, anchors and dynamic references are
/// refused, and `$id` and `$schema` may only sit at the top level. A pointer must name the
/// theme or a `$defs`, component or function entry (common types: a `$defs` entry), or a node
/// in one, since the validation schema keeps nothing else. A pointer below a component
/// or function entry (`#/components/A/properties`) is refused: the validation schema rewrites
/// those entries, so it would dangle there. The walk does not tell schemas from data, so a
/// `const` or `default` holding such a key is refused too. A numeric `$anchor` or `$id` is left
/// to the schema build, whose meta-schema check rejects it.
///
/// `#` and `catalog.json#` name the whole document, which the validation schema does not keep,
/// so they are refused like `#/title`. Pointers are built only for an error. The document is at
/// most `MAX_JSON_DEPTH` deep, so the walk recurses.
pub(crate) fn check_references(document: &Map<String, Value>) -> Result<(), Error> {
    check_map(document, &mut Vec::new())
}

/// One step of the path from a document root to a node.
pub(crate) enum Segment<'a> {
    /// An object member, by its unescaped key.
    Key(&'a str),
    /// An array item, by its index.
    Index(usize),
}

fn check_map<'a>(map: &'a Map<String, Value>, path: &mut Vec<Segment<'a>>) -> Result<(), Error> {
    for (key, value) in map {
        path.push(Segment::Key(key));
        match value {
            Value::Object(_) | Value::Array(_) => check_node(value, path)?,
            Value::String(reference) if key == "$ref" => check_ref(reference, path)?,
            _ if key == "$ref" => {
                return Err(MalformedCatalogSnafu {
                    reason: format!("$ref at {} must be a string", Quoted::new(&pointer(path))),
                }
                .build()
                .into());
            }
            _ => check_keyword(key, value, path)?,
        }
        path.pop();
    }
    Ok(())
}

fn check_node<'a>(node: &'a Value, path: &mut Vec<Segment<'a>>) -> Result<(), Error> {
    match node {
        Value::Object(map) => check_map(map, path),
        Value::Array(items) => {
            for (index, item) in items.iter().enumerate() {
                path.push(Segment::Index(index));
                check_node(item, path)?;
                path.pop();
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn check_ref(reference: &str, path: &[Segment<'_>]) -> Result<(), Error> {
    ensure!(
        is_supported_ref(reference),
        ExternalRefSnafu {
            reference,
            pointer: pointer(path),
        }
    );
    let fragment = reference.split_once('#').map(|(_, fragment)| fragment);
    ensure!(
        fragment.is_some_and(is_pointer_fragment),
        MalformedCatalogSnafu {
            reason: format!(
                "$ref '{}' at {} has a fragment that is neither empty nor a JSON Pointer",
                Quoted::new(reference),
                Quoted::new(&pointer(path))
            ),
        }
    );
    let Some(target) = pointer_of(reference) else {
        return MalformedCatalogSnafu {
            reason: format!(
                "$ref '{}' at {} has a fragment that is not UTF-8",
                Quoted::new(reference),
                Quoted::new(&pointer(path))
            ),
        }
        .fail()
        .map_err(Into::into);
    };
    ensure!(
        is_kept_target(reference, &target),
        MalformedCatalogSnafu {
            reason: format!(
                "$ref '{}' at {} names no definition, entry or theme, which are the only parts \
                 of the catalog the validation schema keeps",
                Quoted::new(reference),
                Quoted::new(&pointer(path))
            ),
        }
    );
    ensure!(
        !is_inside_entry(&target),
        MalformedCatalogSnafu {
            reason: format!(
                "$ref '{}' at {} points inside a component or function, which the validation \
                 schema flattens; reference it whole or move the schema to $defs",
                Quoted::new(reference),
                Quoted::new(&pointer(path))
            ),
        }
    );
    Ok(())
}

/// Whether a supported `reference` to `pointer` names a node the validation schema keeps.
///
/// A common types reference must name a `$defs` entry or a node in one. A catalog reference
/// must name the theme, or a `$defs`, component or function entry or a node in one. The whole
/// document (`#`), a bare section (`#/$defs`) and any other top-level key (`#/title`) are not
/// kept as they are, so a `$ref` to them would dangle or change meaning there.
fn is_kept_target(reference: &str, pointer: &str) -> bool {
    let from_common_types = reference
        .split_once('#')
        .is_some_and(|(document, _)| COMMON_TYPES_URIS.contains(&document));
    let Some((section, rest)) = pointer
        .strip_prefix('/')
        .map(|tokens| tokens.split_once('/').unwrap_or((tokens, "")))
    else {
        return false;
    };
    if from_common_types {
        return section == "$defs" && !rest.is_empty();
    }
    section == "theme" || (ENTRY_SECTIONS.contains(&section) && !rest.is_empty())
}

/// Whether `pointer` names a node below a component or function, such as `/components/A/x`.
///
/// The validation schema rewrites those entries (flattened components, closed function
/// `args`), so no stable node is left for such a pointer; `$defs` entries are copied as is.
fn is_inside_entry(pointer: &str) -> bool {
    pointer
        .strip_prefix("/components/")
        .or_else(|| pointer.strip_prefix("/functions/"))
        .is_some_and(|rest| rest.contains('/'))
}

/// `key` as one of the static [`KEYWORDS`], when it is one.
fn keyword(key: &str) -> Option<&'static str> {
    KEYWORDS.into_iter().find(|keyword| *keyword == key)
}

fn check_keyword(key: &str, value: &Value, path: &[Segment<'_>]) -> Result<(), Error> {
    let Some(name) = keyword(key) else {
        return Ok(());
    };
    let reason = match (name, value) {
        ("$anchor" | "$dynamicAnchor", Value::String(_)) | ("$recursiveAnchor", Value::Bool(_)) => {
            ANCHORS
        }
        ("$dynamicRef" | "$recursiveRef", Value::String(_)) => DYNAMIC,
        ("$id" | "$schema", Value::String(_)) if path.len() > 1 => NESTED,
        _ => return Ok(()),
    };
    Err(ForbiddenKeywordSnafu {
        keyword: name,
        pointer: pointer(path),
        reason,
    }
    .build()
    .into())
}

/// The JSON Pointer of `path`, each key escaped.
fn pointer(path: &[Segment<'_>]) -> String {
    path.iter()
        .map(|segment| match segment {
            Segment::Key(key) => format!("/{}", key.replace('~', "~0").replace('/', "~1")),
            Segment::Index(index) => format!("/{index}"),
        })
        .collect()
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
    (!token.contains('/')).then(|| token.replace("~1", "/").replace("~0", "~"))
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
