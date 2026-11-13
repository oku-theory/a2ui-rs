use std::collections::BTreeSet;

use rstest::rstest;
use serde_json::{Value, json};
use speculoos::prelude::*;

use super::*;
use crate::catalog::test_support::{fan_out, nested};

#[rstest]
#[case(json!(1), 0, false)]
#[case(json!({"a": 1}), 1, false)]
#[case(json!({"a": [1]}), 1, true)]
#[case(json!([[], {}]), 2, false)]
fn deeper_than_small_values_are_measured(
    #[case] value: Value,
    #[case] limit: usize,
    #[case] expected: bool,
) {
    assert_that!(deeper_than(&value, limit)).is_equal_to(expected);
}

#[test]
fn deeper_than_value_at_the_cap_is_accepted() {
    assert_that!(deeper_than(&nested(MAX_JSON_DEPTH), MAX_JSON_DEPTH)).is_false();
}

#[test]
fn deeper_than_value_past_the_cap_is_rejected() {
    assert_that!(deeper_than(&nested(MAX_JSON_DEPTH + 1), MAX_JSON_DEPTH)).is_true();
}

#[test]
fn deeper_than_hundred_thousand_levels_in_a_small_stack_is_rejected() {
    let rejected = std::thread::Builder::new()
        .stack_size(256 * 1024)
        .spawn(|| {
            let value = nested(100_000);
            let rejected = deeper_than(&value, MAX_JSON_DEPTH);
            // Dropping 100 000 levels would recurse through serde_json's `Drop` and
            // overflow this thread's stack: the test leaks the value on purpose.
            std::mem::forget(value);
            rejected
        })
        .unwrap()
        .join()
        .unwrap();
    assert_that!(rejected).is_true();
}

#[test]
fn oversize_huge_array_is_rejected_by_the_node_cap() {
    let value = json!(vec![0; 300_000]);
    assert_that!(oversize(&value, MAX_CATALOG_NODES)).is_equal_to(Some(Oversize::Nodes));
}

#[rstest]
#[case(json!(1), 0, true)]
#[case(json!(1), 1, false)]
#[case(json!({"a": [1, 2]}), 4, false)]
#[case(json!({"a": [1, 2]}), 3, true)]
fn oversize_every_node_is_counted(
    #[case] value: Value,
    #[case] limit: usize,
    #[case] expected: bool,
) {
    assert_that!(oversize(&value, limit) == Some(Oversize::Nodes)).is_equal_to(expected);
}

#[rstest]
#[case(MAX_KEY_BYTES, None)]
#[case(MAX_KEY_BYTES + 1, Some(Oversize::Key))]
fn oversize_long_key_is_reported(#[case] length: usize, #[case] expected: Option<Oversize>) {
    let value = json!({"properties": {"k".repeat(length): {}}});
    assert_that!(oversize(&value, 100)).is_equal_to(expected);
}

#[rstest]
#[case(MAX_REF_BYTES, None)]
#[case(MAX_REF_BYTES + 1, Some(Oversize::Ref))]
fn oversize_long_ref_is_reported(#[case] length: usize, #[case] expected: Option<Oversize>) {
    let reference = format!("#/{}", "r".repeat(length - 2));
    let value = json!({"allOf": [{"$ref": reference}]});
    assert_that!(oversize(&value, 100)).is_equal_to(expected);
}

#[test]
fn oversize_long_string_outside_ref_is_accepted() {
    let value = json!({"description": "d".repeat(MAX_REF_BYTES + 1)});
    assert_that!(oversize(&value, 100)).is_none();
}

fn fan_out_document(levels: usize) -> Value {
    json!({"$defs": fan_out(levels, false)})
}

fn chain(links: usize) -> Value {
    let mut defs = serde_json::Map::new();
    defs.insert("D0".to_owned(), json!({}));
    for link in 1..=links {
        defs.insert(
            format!("D{link}"),
            json!({"$ref": format!("#/$defs/D{}", link - 1)}),
        );
    }
    json!({"$defs": defs})
}

fn within(expansion: Expansion) -> (usize, usize, BTreeSet<(&'static str, String)>) {
    match expansion {
        Expansion::Within {
            size,
            patterns,
            reached,
        } => (size, patterns, reached),
        other => panic!("expected an expansion within the caps, got {other:?}"),
    }
}

#[test]
fn expand_plain_schema_counts_every_node() {
    let root = json!({"type": "object", "properties": {"a": {"type": "string"}}});
    assert_that!(within(expand(&root, &root, 100)).0).is_equal_to(5);
}

#[test]
fn expand_ref_counts_its_target_and_its_entry() {
    let root = json!({"$defs": {"S": {"type": "string"}}, "x": {"$ref": "#/$defs/S"}});
    assert_that!(within(expand(&root, &root["x"], 100)).0).is_equal_to(6);
}

#[test]
fn expand_pointer_into_an_entry_charges_the_whole_entry() {
    let root = json!({
        "$defs": {"Big": {"properties": {"a": {}, "b": {}, "c": {}}}},
        "x": {"$ref": "#/$defs/Big/properties/a"}
    });
    assert_that!(within(expand(&root, &root["x"], 100)).0).is_equal_to(8);
}

#[test]
fn expand_pointer_into_an_entry_past_the_budget_is_too_large() {
    let properties: serde_json::Map<String, Value> = (0..100)
        .map(|index| (format!("p{index}"), json!({})))
        .collect();
    let root = json!({
        "$defs": {"Big": {"properties": properties}},
        "x": {"$ref": "#/$defs/Big/properties/p0"}
    });
    assert_that!(expand(&root, &root["x"], 50)).is_equal_to(Expansion::TooLarge);
}

#[test]
fn expand_memoized_ref_site_counts_its_target_each_time() {
    let root = json!({"$defs": {
        "X": {"allOf": [{"$ref": "#/$defs/M"}, {"$ref": "#/$defs/M"}]},
        "M": {"$ref": "#/$defs/S"},
        "S": {"type": "string"}
    }});
    assert_that!(within(expand(&root, &root["$defs"]["X"], 100)).0).is_equal_to(18);
}

#[test]
fn expand_memoized_ref_site_still_detects_the_cycle() {
    let root = json!({
        "$defs": {"T": {"properties": {"p": {"$ref": "#/$defs/T"}}}},
        "x": {"$ref": "#/$defs/T/properties/p"}
    });
    let expansion = expand(&root, &root["x"], 100);
    assert_that!(expansion).is_equal_to(Expansion::Cycle("#/$defs/T".to_owned()));
}

#[test]
fn expand_fan_out_counts_every_followed_ref() {
    let root = fan_out_document(6);
    assert_that!(within(expand(&root, &root["$defs"]["L6"], 10_000)).0).is_equal_to(538);
}

#[test]
fn expand_past_the_budget_is_too_large() {
    let root = fan_out_document(40);
    assert_that!(expand(&root, &root["$defs"]["L40"], 1000)).is_equal_to(Expansion::TooLarge);
}

#[test]
fn expand_ref_cycle_is_named() {
    let root = json!({"$defs": {"A": {"$ref": "#/$defs/B"}, "B": {"$ref": "#/$defs/A"}}});
    let expansion = expand(&root, &root["$defs"]["A"], 100);
    assert_that!(expansion).is_equal_to(Expansion::Cycle("#/$defs/A".to_owned()));
}

#[test]
fn expand_sibling_refs_to_one_target_are_no_cycle() {
    let root = json!({
        "$defs": {"S": {"type": "string"}},
        "x": {"$ref": "#/$defs/S", "properties": {"a": {"$ref": "#/$defs/S"}}}
    });
    assert_that!(within(expand(&root, &root["x"], 100)).0).is_equal_to(11);
}

#[rstest]
#[case("#/$defs/Missing")]
#[case("#/theme")]
#[case("#")]
#[case("#A")]
fn expand_ref_outside_any_entry_is_unresolved(#[case] reference: &str) {
    let root = json!({"theme": {}, "x": {"$ref": reference}});
    let expansion = expand(&root, &root["x"], 100);
    assert_that!(expansion).is_equal_to(Expansion::Unresolved(reference.to_owned()));
}

#[test]
fn expand_ref_chain_at_the_depth_cap_is_within() {
    let root = chain(MAX_SCHEMA_DEPTH);
    let start = &root["$defs"][format!("D{MAX_SCHEMA_DEPTH}")];
    // The walk counts 2n + 1 nodes; the entries D0 to D(n - 1) add 2n - 1 more.
    assert_that!(within(expand(&root, start, 10_000)).0).is_equal_to(4 * MAX_SCHEMA_DEPTH);
}

#[test]
fn expand_ref_chain_past_the_depth_cap_is_too_deep() {
    let links = MAX_SCHEMA_DEPTH + 1;
    let root = chain(links);
    let start = &root["$defs"][format!("D{links}")];
    assert_that!(expand(&root, start, 10_000)).is_equal_to(Expansion::TooDeep);
}

#[test]
fn expand_patterns_and_pattern_properties_are_counted() {
    let root = json!({
        "properties": {"a": {"type": "string", "pattern": "^a"}},
        "patternProperties": {"^x": {}, "^y": {}}
    });
    assert_that!(within(expand(&root, &root, 100)).1).is_equal_to(3);
}

#[test]
fn expand_reached_entries_are_listed() {
    let root = json!({
        "$defs": {"S": {"$ref": "#/components/C"}},
        "components": {"C": {}},
        "x": {"allOf": [{"$ref": "#/$defs/S"}]}
    });
    let expected = BTreeSet::from([("$defs", "S".to_owned()), ("components", "C".to_owned())]);
    assert_that!(within(expand(&root, &root["x"], 100)).2).is_equal_to(expected);
}

#[test]
fn expand_escaped_entry_name_is_listed_once_by_its_key() {
    let root = json!({
        "$defs": {"a/b": {}},
        "x": {"allOf": [{"$ref": "#/$defs/a~1b"}, {"$ref": "#/$defs/a%7E1b"}]}
    });
    let expected = BTreeSet::from([("$defs", "a/b".to_owned())]);
    assert_that!(within(expand(&root, &root["x"], 100)).2).is_equal_to(expected);
}

#[test]
fn expand_long_string_weighs_one_node_per_64_bytes() {
    let root = json!({"description": "d".repeat(128)});
    assert_that!(within(expand(&root, &root, 100)).0).is_equal_to(4);
}

#[test]
fn expand_long_key_weighs_one_node_per_64_bytes() {
    let root = json!({"k".repeat(128): 1});
    assert_that!(within(expand(&root, &root, 100)).0).is_equal_to(4);
}

#[test]
fn expand_reached_entry_charge_weighs_its_strings() {
    let root = json!({
        "$defs": {"S": {"description": "d".repeat(640)}},
        "x": {"$ref": "#/$defs/S"}
    });
    // `x` and its `$ref` (2), `S` walked (1 + 11), then `S` charged whole (12).
    assert_that!(within(expand(&root, &root["x"], 100)).0).is_equal_to(26);
}
