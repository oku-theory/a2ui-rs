//! `$ref` rules of v1.0 catalogs, and the local references the runtime writes.
//!
//! A v1.0 catalog may reference itself (`#…`, `catalog.json#…`) and `common_types.json` (by a
//! relative or the published absolute URI). Localizing rewrites those to `#` fragments,
//! percent-encoded as URI references require (a raw `ℓ` makes the schema build fail).

use percent_encoding::{AsciiSet, CONTROLS, percent_decode_str, utf8_percent_encode};
use serde_json::{Value, json};

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

/// `reference` as a local, percent-encoded `$ref`; an unsupported one is returned unchanged.
///
/// A fragment whose escapes are not UTF-8 is kept as written; the schema build then reports
/// it (catalogs never get here with one: Task 4's reference check rejects it first).
pub(crate) fn localize_ref(reference: &str) -> String {
    match reference.split_once('#') {
        Some((_, fragment)) if is_supported_ref(reference) => {
            match percent_decode_str(fragment).decode_utf8() {
                Ok(pointer) => local_ref(&pointer),
                Err(_) => format!("#{fragment}"),
            }
        }
        _ => reference.to_owned(),
    }
}

/// Rewrites every `$ref` string under `node` with [`localize_ref`].
pub(crate) fn localize_refs(node: &mut Value) {
    match node {
        Value::Object(map) => {
            for (key, value) in map.iter_mut() {
                match value {
                    Value::String(reference) if key == "$ref" => {
                        *reference = localize_ref(reference);
                    }
                    _ => localize_refs(value),
                }
            }
        }
        Value::Array(items) => items.iter_mut().for_each(localize_refs),
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

#[cfg(test)]
#[path = "bundle_tests.rs"]
mod tests;
