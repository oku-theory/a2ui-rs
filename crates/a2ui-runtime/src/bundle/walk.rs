//! The `$ref` walks: every scalar member, or every `$ref` string, of a document.
//!
//! They do not tell schemas from data (plan 2b, Decision 15) and recurse: callers pass a
//! document that load rule 1 bounded to `MAX_JSON_DEPTH` levels, or one built from it.

use serde_json::{Map, Value};

/// One step of the path from a document root to a node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Segment<'a> {
    /// An object member, by its unescaped key.
    Key(&'a str),
    /// An array item, by its index.
    Index(usize),
}

/// Calls `visit` on each scalar object member under `node`, until one returns `Some`.
///
/// `visit` gets the member's key, its value and its path; a member holding an object or array
/// is walked whatever its key, and an array item or the root is never visited, having no key.
/// Object members come in map order and array items in index order. [`find_ref`] walks with
/// it, and [`check_references`](super::check_references) with its descent from a map
/// ([`find_member_in`]).
fn find_member<'a, T>(
    node: &'a Value,
    visit: &mut impl FnMut(&'a str, &'a Value, &[Segment<'a>]) -> Option<T>,
) -> Option<T> {
    find_member_under(node, &mut Vec::new(), visit)
}

/// [`find_member`]'s descent from any node, with the path to it so far.
fn find_member_under<'a, T>(
    node: &'a Value,
    path: &mut Vec<Segment<'a>>,
    visit: &mut impl FnMut(&'a str, &'a Value, &[Segment<'a>]) -> Option<T>,
) -> Option<T> {
    match node {
        Value::Object(map) => find_member_in(map, path, visit),
        Value::Array(items) => items.iter().enumerate().find_map(|(index, item)| {
            path.push(Segment::Index(index));
            let found = find_member_under(item, path, visit);
            path.pop();
            found
        }),
        _ => None,
    }
}

/// [`find_member`]'s descent from a map, for a root that is one (load rule 4's document).
pub(super) fn find_member_in<'a, T>(
    map: &'a Map<String, Value>,
    path: &mut Vec<Segment<'a>>,
    visit: &mut impl FnMut(&'a str, &'a Value, &[Segment<'a>]) -> Option<T>,
) -> Option<T> {
    map.iter().find_map(|(key, value)| {
        path.push(Segment::Key(key));
        let found = match value {
            Value::Object(_) | Value::Array(_) => find_member_under(value, path, visit),
            _ => visit(key, value, path),
        };
        path.pop();
        found
    })
}

/// Calls `visit` on each `$ref` string and its path, until one returns `Some`.
///
/// It walks with [`find_member`], so a property named `$ref` holding a schema is a property,
/// and every `$ref` string is visited, data included, since the walk does not tell schemas
/// from data. A `$ref` that is not a string is skipped (load rule 4 refuses it).
pub(super) fn find_ref<'a, T>(
    node: &'a Value,
    visit: &mut impl FnMut(&'a str, &[Segment<'a>]) -> Option<T>,
) -> Option<T> {
    find_member(node, &mut |key, value, path| match value {
        Value::String(reference) if key == "$ref" => visit(reference, path),
        _ => None,
    })
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

/// The node of `root` at `path`, if every step names one.
pub(crate) fn node_at<'a>(root: &'a Value, path: &[Segment<'_>]) -> Option<&'a Value> {
    path.iter().try_fold(root, |node, segment| match segment {
        Segment::Key(key) => node.as_object()?.get(*key),
        Segment::Index(index) => node.as_array()?.get(*index),
    })
}

/// The JSON Pointer of `path`, each key escaped.
pub(crate) fn pointer(path: &[Segment<'_>]) -> String {
    path.iter()
        .map(|segment| match segment {
            Segment::Key(key) => format!("/{}", key.replace('~', "~0").replace('/', "~1")),
            Segment::Index(index) => format!("/{index}"),
        })
        .collect()
}

#[cfg(test)]
#[path = "walk_tests.rs"]
mod tests;
