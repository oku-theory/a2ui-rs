//! The `$ref` walks: every scalar member, or every `$ref` string, of a document.
//!
//! They do not tell schemas from data (plan 2b, Decision 15) and recurse: callers pass a
//! document that load rule 1 bounded to `MAX_JSON_DEPTH` levels, or one built from it.

use serde_json::{Map, Value};

use crate::pointer::escape_token;

/// One step of the path from a document root to a node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Segment<'a> {
    /// An object member, by its unescaped key.
    Key(&'a str),
    /// An array item, by its index.
    Index(usize),
}

/// The `$ref` keyword, named once for every walk that reads it.
pub(crate) const REF_KEY: &str = "$ref";

/// Defines a member descent and its `$ref` filter, shared or `mut`.
///
/// One body (Decision 17) yields [`find_member`] and [`find_member_mut`], which walk the same
/// members in the same order with the same paths, and [`find_ref`] and [`find_ref_mut`], which
/// keep the `$ref` strings among those members alike, so [`rewrite_refs`] cannot drift from
/// [`find_ref`]. Slots: the root walk, the descents from any node and from a map, the map
/// iterator, then the `$ref` walk with the type its strings are lent as.
macro_rules! member_descent {
    (
        $(#[$member_doc:meta])* $member:ident,
        $under:ident, $in_map_vis:vis $in_map:ident, $iter:ident,
        $(#[$ref_doc:meta])* $ref_vis:vis $find_ref:ident($ref_ty:ty)
        $(, $mutability:tt)?
    ) => {
        $(#[$member_doc])*
        fn $member<'a, T>(
            node: &'a $($mutability)? Value,
            visit: &mut impl FnMut(&'a str, &'a $($mutability)? Value, &[Segment<'a>]) -> Option<T>,
        ) -> Option<T> {
            $under(node, &mut Vec::new(), visit)
        }

        /// The descent from any node, with the path to it so far.
        fn $under<'a, T>(
            node: &'a $($mutability)? Value,
            path: &mut Vec<Segment<'a>>,
            visit: &mut impl FnMut(&'a str, &'a $($mutability)? Value, &[Segment<'a>]) -> Option<T>,
        ) -> Option<T> {
            match node {
                Value::Object(map) => $in_map(map, path, visit),
                Value::Array(items) => items.$iter().enumerate().find_map(|(index, item)| {
                    path.push(Segment::Index(index));
                    let found = $under(item, path, visit);
                    path.pop();
                    found
                }),
                _ => None,
            }
        }

        /// The descent from a map, with the path to it so far.
        $in_map_vis fn $in_map<'a, T>(
            map: &'a $($mutability)? Map<String, Value>,
            path: &mut Vec<Segment<'a>>,
            visit: &mut impl FnMut(&'a str, &'a $($mutability)? Value, &[Segment<'a>]) -> Option<T>,
        ) -> Option<T> {
            map.$iter().find_map(|(key, value)| {
                path.push(Segment::Key(key));
                let found = match value {
                    Value::Object(_) | Value::Array(_) => $under(value, path, visit),
                    _ => visit(key, value, path),
                };
                path.pop();
                found
            })
        }

        $(#[$ref_doc])*
        $ref_vis fn $find_ref<'a, T>(
            node: &'a $($mutability)? Value,
            visit: &mut impl FnMut(&'a $($mutability)? $ref_ty, &[Segment<'a>]) -> Option<T>,
        ) -> Option<T> {
            $member(node, &mut |key, value, path| match value {
                Value::String(reference) if key == REF_KEY => visit(reference, path),
                _ => None,
            })
        }
    };
}

member_descent!(
    /// Calls `visit` on each scalar object member under `node`, until one returns `Some`.
    ///
    /// `visit` gets the member's key, its value and its path; a member holding an object or
    /// array is walked whatever its key, and an array item or the root is never visited, having
    /// no key. Object members come in map order and array items in index order. [`find_ref`]
    /// walks with it, and [`check_references`](super::check_references) with its descent from a
    /// map ([`find_member_in`]).
    find_member,
    find_member_under, pub(super) find_member_in, iter,
    /// Calls `visit` on each `$ref` string and its path, until one returns `Some`.
    ///
    /// It walks with [`find_member`], so a property named `$ref` holding a schema is a property,
    /// and every `$ref` string is visited, data included, since the walk does not tell schemas
    /// from data. A `$ref` that is not a string is skipped (load rule 4 refuses it).
    pub(super) find_ref(str)
);

// The empty visibility slots keep the `mut` descent private to this module.
member_descent!(
    /// [`find_member`] over a `mut` value: `visit` may change in place each scalar it gets.
    ///
    /// A visitor must keep a scalar a scalar: a container put in its place is never walked, and
    /// could pass the depth that load rule 1 checked.
    find_member_mut,
    find_member_under_mut, find_member_in_mut, iter_mut,
    /// [`find_ref`] over a `mut` value: `visit` may rewrite each `$ref` string it gets.
    find_ref_mut(String),
    mut
);

/// Calls `visit` on every `$ref` string under `node`, as [`find_ref`] visits them.
pub(crate) fn for_each_ref<'a>(node: &'a Value, visit: &mut impl FnMut(&'a str)) {
    find_ref(node, &mut |reference, _| {
        visit(reference);
        None::<()>
    });
}

/// Calls `rewrite` on each `$ref` string under `node`, as [`find_ref`] visits them.
///
/// It walks with [`find_ref_mut`], under the same depth bound as [`find_ref`].
pub(crate) fn rewrite_refs(node: &mut Value, rewrite: &mut impl FnMut(&mut String)) {
    find_ref_mut(node, &mut |reference, _| {
        rewrite(reference);
        None::<()>
    });
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
            Segment::Key(key) => format!("/{}", escape_token(key)),
            Segment::Index(index) => format!("/{index}"),
        })
        .collect()
}

#[cfg(test)]
#[path = "walk_tests.rs"]
mod tests;
