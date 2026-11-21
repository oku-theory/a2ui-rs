//! Catalog documents and assertions shared by the catalog unit tests.

use serde_json::{Map, Value, json};
use speculoos::prelude::*;

use crate::{Catalog, CatalogSet, Category, Error};

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
    wrapped(json!(1), "a", levels)
}

/// `leaf` wrapped in `levels` objects, each holding the one below under `key`.
pub(crate) fn wrapped(leaf: Value, key: &str, levels: usize) -> Value {
    // `json!({key: inner})` clones `inner` recursively, so deep values are built by moving.
    (0..levels).fold(leaf, |inner, _| {
        Value::Object(Map::from_iter([(key.to_owned(), inner)]))
    })
}

/// The stack a wasm32 build gets by default, 1 MiB; native frames only approximate wasm ones.
pub(crate) const WASM_STACK: usize = 1 << 20;

/// Loads `document` as [`load`] does, on a thread with a [`WASM_STACK`] stack.
pub(crate) fn load_on_a_small_stack(document: Value) -> Result<Catalog, Error> {
    on_a_small_stack(move || load(document))
}

/// Runs `work` on a thread with a [`WASM_STACK`] stack.
pub(crate) fn on_a_small_stack<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> T {
    std::thread::Builder::new()
        .stack_size(WASM_STACK)
        .spawn(work)
        .unwrap()
        .join()
        .unwrap()
}

/// The common type `name` as a `$ref`.
fn common(name: &str) -> Value {
    json!({"$ref": format!("common_types.json#/$defs/{name}")})
}

/// Catalog `a`: `Text`, `Chart`, `Column`, `Button`, `Any`, `Many`; functions `upper`,
/// `fetch` (agentOnly) and `both` (no arguments).
pub(crate) fn catalog_a() -> Catalog {
    let many: Map<String, Value> = (0..17)
        .map(|index| (format!("p{index}"), json!({"type": "string"})))
        .collect();
    let required: Vec<String> = many.keys().cloned().collect();
    load(json!({
        "catalogId": "a",
        "protocolVersion": "1.0",
        "components": {
            "Text": {
                "type": "object",
                "properties": {"component": {"const": "Text"}, "text": common("DynamicString")},
                "required": ["component", "text"]
            },
            "Chart": {
                "type": "object",
                "properties": {
                    "component": {"const": "Chart"},
                    "chartType": {"type": "string", "enum": ["bar", "line"]}
                }
            },
            "Column": {
                "type": "object",
                "properties": {"component": {"const": "Column"}, "children": common("ChildList")},
                "required": ["children"]
            },
            "Button": {
                "type": "object",
                "properties": {
                    "component": {"const": "Button"},
                    "child": common("ComponentId"),
                    "action": common("Action")
                },
                "required": ["child"],
                "unevaluatedProperties": false
            },
            "Any": {"type": "object", "properties": {"component": {"const": "Any"}}},
            "Many": {"type": "object", "properties": many, "required": required}
        },
        "functions": {
            "upper": {
                "type": "object",
                "returnType": "string",
                "properties": {
                    "@call": {"const": "upper"},
                    "args": {
                        "type": "object",
                        "properties": {"value": common("DynamicString")},
                        "required": ["value"],
                        "unevaluatedProperties": false
                    }
                },
                "required": ["@call", "args"]
            },
            "fetch": {"type": "object", "returnType": "string", "allowedCallers": "agentOnly"},
            "both": {"type": "object", "returnType": "string", "allowedCallers": "rendererOrAgent"}
        }
    }))
    .unwrap()
}

/// Catalog `b`: a `Badge` component and an `onlyB` function.
pub(crate) fn catalog_b() -> Catalog {
    load(json!({
        "catalogId": "b",
        "protocolVersion": "1.0",
        "components": {
            "Badge": {
                "type": "object",
                "properties": {"component": {"const": "Badge"}, "label": common("DynamicString")}
            }
        },
        "functions": {"onlyB": {"type": "object", "returnType": "string"}}
    }))
    .unwrap()
}

/// A set holding [`catalog_a`] and [`catalog_b`].
pub(crate) fn catalog_set() -> CatalogSet {
    let mut set = CatalogSet::new();
    set.insert(catalog_a()).unwrap();
    set.insert(catalog_b()).unwrap();
    set
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

/// A catalog of `count` components that each mix in a 3 000-property `$defs` entry.
///
/// Each component's expansion charges about 12 010 nodes: 21 fit `MAX_CATALOG_EXPANSION`
/// (262 144), 22 do not.
pub(crate) fn mixin_copies(count: usize) -> Value {
    let properties: Map<String, Value> = (0..3000)
        .map(|index| (format!("p{index}"), json!({"type": "string"})))
        .collect();
    let components: Map<String, Value> = (0..count)
        .map(|index| {
            (
                format!("C{index}"),
                json!({"allOf": [{"$ref": "#/$defs/Big"}]}),
            )
        })
        .collect();
    v10(json!({"$defs": {"Big": {"properties": properties}}, "components": components}))
}
