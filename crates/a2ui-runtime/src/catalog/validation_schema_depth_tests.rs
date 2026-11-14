//! Load at the depth limits, on the stack a wasm32 build gets.
//!
//! Each deepest case has a companion one level deeper that load refuses, so the case provably
//! sits at the limit. A stack overflow aborts the test process; nextest reports it as a failure.

use rstest::rstest;
use serde_json::{Map, Value, json};
use speculoos::prelude::*;

use super::super::test_support::{
    WASM_STACK, catalog_error, load, load_on_a_small_stack, v10, wrapped,
};
use crate::limits::{MAX_JSON_DEPTH, MAX_SCHEMA_DEPTH};

/// A catalog holding a dangling `$ref` under `levels` `not` schemas, placed under `keys`.
///
/// `not` is a schema keyword, so `describe` recurses through the chain too.
fn deep_dangling(keys: &[&str], levels: usize) -> Value {
    let chain = wrapped(json!({"$ref": "#/$defs/Missing"}), "not", levels);
    let mut document = keys
        .iter()
        .rev()
        .fold(chain, |inner, key| wrapped(inner, key, 1));
    document["components"] = json!({"A": {}});
    v10(document)
}

/// The deepest catalog load rule 1 accepts, with a dangling `$ref` at the bottom.
///
/// The root, the place keys and the `$ref` object take the levels the chain does not.
#[rstest]
#[case::theme(&["theme"], MAX_JSON_DEPTH - 2, "/theme/not/")]
#[case::unreached_def(&["$defs", "Deep"], MAX_JSON_DEPTH - 3, "/$defs/Deep/not/")]
fn from_json_deepest_dangling_ref_on_a_small_stack_is_catalog_error_at_authored_place(
    #[case] keys: &[&str],
    #[case] levels: usize,
    #[case] place: &str,
) {
    let message = catalog_error(load_on_a_small_stack(deep_dangling(keys, levels)));
    let prefix = format!("Malformed catalog: $ref '#/$defs/Missing' at {place}");
    assert_that!(message.starts_with(&prefix)).is_true();
    assert_that!(message).ends_with("does not resolve in the validation schema");
}

/// One level past the cases above: proves they sit exactly at the depth limit.
#[rstest]
#[case::theme(&["theme"], MAX_JSON_DEPTH - 1)]
#[case::unreached_def(&["$defs", "Deep"], MAX_JSON_DEPTH - 2)]
fn from_json_one_level_past_the_deepest_is_depth_error(
    #[case] keys: &[&str],
    #[case] levels: usize,
) {
    let message = catalog_error(load_on_a_small_stack(deep_dangling(keys, levels)));
    assert_that!(message).contains("levels of nesting");
}

/// `leaf` under `levels` inline `allOf`s, built by moving; each step takes two levels.
fn all_of_chain(leaf: Value, levels: usize) -> Value {
    (0..levels).fold(leaf, |inner, _| {
        Value::Object(Map::from_iter([(
            "allOf".to_owned(),
            Value::Array(vec![inner]),
        )]))
    })
}

/// A catalog whose kept component `A` is `schema`, with `$defs.X` for its leaf to name.
fn deep_component(schema: Value) -> Value {
    let mut document = wrapped(wrapped(schema, "A", 1), "components", 1);
    document["$defs"] = json!({"X": {"type": "string"}});
    v10(document)
}

/// The deepest kept component load rule 1 accepts loads: compiled, flattened, described.
#[rstest]
#[case::not_chain(wrapped(json!({"$ref": "#/$defs/X"}), "not", MAX_JSON_DEPTH - 3))]
#[case::all_of_chain(all_of_chain(json!({"$ref": "#/$defs/X"}), (MAX_JSON_DEPTH - 3) / 2))]
fn from_json_deepest_kept_component_on_a_small_stack_loads(#[case] schema: Value) {
    assert_that!(load_on_a_small_stack(deep_component(schema))).is_ok();
}

/// One `not` past the deepest component: proves the `not` case sits at the depth limit.
#[test]
fn from_json_component_one_level_past_the_deepest_is_depth_error() {
    let schema = wrapped(json!({"$ref": "#/$defs/X"}), "not", MAX_JSON_DEPTH - 2);
    let message = catalog_error(load_on_a_small_stack(deep_component(schema)));
    assert_that!(message).contains("levels of nesting");
}

/// A catalog whose component `A` reaches `$defs` `D0` through `hops` `$defs`, two `not`s each.
fn ref_chain(hops: usize) -> Value {
    let mut defs = Map::from_iter([("D0".to_owned(), json!({"type": "string"}))]);
    for hop in 1..=hops {
        let below = format!("#/$defs/D{}", hop - 1);
        defs.insert(format!("D{hop}"), json!({"not": {"not": {"$ref": below}}}));
    }
    let top = format!("#/$defs/D{hops}");
    v10(json!({"$defs": defs, "components": {"A": {"properties": {"p": {"$ref": top}}}}}))
}

/// The most hops whose expansion stays within `MAX_SCHEMA_DEPTH`: three levels each, under
/// the five of `A`'s envelope.
const DEEPEST_HOPS: usize = (MAX_SCHEMA_DEPTH - 5) / 3;

#[test]
fn validate_component_deepest_expanded_entry_on_a_small_stack_is_accepted() {
    let document = ref_chain(DEEPEST_HOPS);
    let valid = std::thread::Builder::new()
        .stack_size(WASM_STACK)
        .spawn(move || {
            let catalog = load(document).unwrap();
            let instance = json!({"id": "x", "component": "A", "p": "s"});
            catalog.validate_component(&instance, "c", &catalog).is_ok()
        })
        .unwrap()
        .join()
        .unwrap();
    assert_that!(valid).is_true();
}

/// One hop past the case above: proves it sits at the expanded-depth limit.
#[test]
fn from_json_one_hop_past_the_deepest_expansion_is_catalog_error() {
    let message = catalog_error(load(ref_chain(DEEPEST_HOPS + 1)));
    assert_that!(message).contains("levels of 'A' once its $refs are expanded");
}
