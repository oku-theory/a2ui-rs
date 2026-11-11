//! Catalog documents and assertions shared by the catalog unit tests.

use serde_json::{Map, Value, json};
use speculoos::prelude::*;

use crate::{Catalog, Category, Error};

/// Loads `document` with no overrides.
pub(crate) fn load(document: Value) -> Result<Catalog, Error> {
    Catalog::from_json(&document, None, None)
}

/// A v1.0 catalog `c` holding the keys of `extra`.
pub(crate) fn v10(extra: Value) -> Value {
    let mut document = json!({"catalogId": "c", "protocolVersion": "1.0"});
    let target = document.as_object_mut().unwrap();
    for (key, value) in extra.as_object().unwrap() {
        target.insert(key.clone(), value.clone());
    }
    document
}

/// The message of a load that must fail with a `CatalogError`.
pub(crate) fn catalog_error(result: Result<Catalog, Error>) -> String {
    let err = result.unwrap_err();
    assert_that!(err.category()).is_equal_to(Category::Catalog);
    err.to_string()
}

/// A scalar wrapped in `levels` single-key objects.
pub(crate) fn nested(levels: usize) -> Value {
    // `json!({"a": inner})` clones `inner` recursively, so deep values are built by moving.
    (0..levels).fold(json!(1), |inner, _| {
        Value::Object(Map::from_iter([("a".to_owned(), inner)]))
    })
}

/// `$defs` `L0` to `L<levels>`, each level an `allOf` of two `$ref`s to the one below.
///
/// `unevaluated` adds `"unevaluatedProperties": false` to each level. Without it, the walk of
/// `L<n>` counts `8 × 2^n - 6` nodes, and the entries `L0` to `L<n-1>` it reaches add
/// `6n - 4` more: 8 242 at 10, 32 830 at 12.
pub(crate) fn fan_out(levels: usize, unevaluated: bool) -> Map<String, Value> {
    let mut defs = Map::from_iter([("L0".to_owned(), json!({"type": "string"}))]);
    for level in 1..=levels {
        let below = format!("#/$defs/L{}", level - 1);
        let mut definition = json!({"allOf": [{"$ref": below}, {"$ref": below}]});
        if unevaluated {
            definition["unevaluatedProperties"] = json!(false);
        }
        defs.insert(format!("L{level}"), definition);
    }
    defs
}
