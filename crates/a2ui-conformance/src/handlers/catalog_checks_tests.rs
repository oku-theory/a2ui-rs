use a2ui_runtime::Catalog;
use rstest::rstest;
use serde_json::{Value, json};
use speculoos::prelude::*;

use super::*;

fn catalog(extra: Value) -> Catalog {
    let mut document = json!({"catalogId": "c", "protocolVersion": "1.0"});
    for (key, value) in extra.as_object().unwrap() {
        document[key] = value.clone();
    }
    Catalog::from_json(&document, None, None).unwrap()
}

fn expect(value: Value) -> serde_json::Map<String, Value> {
    value.as_object().unwrap().clone()
}

#[test]
fn self_contained_resolving_refs_pass() {
    let schema = json!({"a": {"$ref": "#/$defs/B_%E2%84%93"}, "$defs": {"B_ℓ": {}}});
    assert_that!(self_contained(&schema)).is_ok();
}

#[rstest]
#[case(json!({"a": {"type": "string"}}), "has no $ref")]
#[case(
    json!({"a": {"$ref": "common_types.json#/$defs/B"}}),
    "'common_types.json#/$defs/B' does not resolve"
)]
#[case(json!({"a": {"$ref": "#/$defs/Missing"}}), "'#/$defs/Missing' does not resolve")]
#[case(json!({"a": {"$ref": "#/$defs/%FF"}}), "'#/$defs/%FF' does not resolve")]
fn self_contained_broken_schema_is_reported(#[case] schema: Value, #[case] fragment: &str) {
    assert_that!(self_contained(&schema).unwrap_err()).contains(fragment);
}

#[test]
fn matches_builtin_same_catalog_passes() {
    let one = catalog(json!({"components": {"A": {"properties": {"x": {}}}}}));
    let two = catalog(json!({"components": {"A": {"properties": {"x": {}}}}}));
    assert_that!(matches_builtin(&one, &two)).is_ok();
}

#[rstest]
#[case(json!({"components": {"B": {}}}), "component names differ")]
#[case(json!({"components": {"A": {"properties": {"y": {}}}}}), "component A differs")]
#[case(
    json!({
        "components": {"A": {"properties": {"x": {}}}},
        "functions": {"f": {"returnType": "string"}}
    }),
    "function signatures differ"
)]
fn matches_builtin_difference_is_reported(#[case] extra: Value, #[case] fragment: &str) {
    let builtin = catalog(json!({"components": {"A": {"properties": {"x": {}}}}}));
    assert_that!(matches_builtin(&catalog(extra), &builtin).unwrap_err()).contains(fragment);
}

#[test]
fn matches_builtin_other_id_is_reported() {
    let builtin = catalog(json!({}));
    let document = json!({"catalogId": "d", "protocolVersion": "1.0"});
    let other = Catalog::from_json(&document, None, None).unwrap();
    assert_that!(matches_builtin(&other, &builtin).unwrap_err())
        .contains("differs from the builtin");
}

#[rstest]
#[case(json!({"components": ["A"]}))]
#[case(json!({"components": {"A": {"properties": {"x": {}}}}}))]
#[case(json!({"catalogId": "c", "protocolVersion": "1.0"}))]
fn entries_matching_expectation_passes(#[case] expected: Value) {
    let loaded = catalog(json!({"components": {"A": {"properties": {"x": {}}}}}));
    assert_that!(entries(&expect(expected), &loaded, "t")).is_ok();
}

#[test]
fn entries_matching_theme_passes() {
    let theme = json!({"type": "object", "properties": {"primaryColor": {"type": "string"}}});
    let loaded = catalog(json!({"theme": theme.clone()}));
    assert_that!(entries(&expect(json!({"theme": theme})), &loaded, "t")).is_ok();
}

#[test]
fn entries_other_theme_is_reported() {
    let loaded = catalog(json!({"theme": {"type": "object"}}));
    let expected = expect(json!({"theme": {"type": "string"}}));
    assert_that!(entries(&expected, &loaded, "t").unwrap_err())
        .is_equal_to(r#"t: theme {"type":"object"}, expected {"type":"string"}"#.to_owned());
}

#[rstest]
#[case(json!({"components": ["A", "B"]}), "components")]
#[case(json!({"components": {"A": {"properties": {"y": {}}}}}), "components.A")]
#[case(json!({"components": "A"}), "must be a list or a map")]
#[case(json!({"components": [1]}), "lists a non-string")]
#[case(json!({"catalogId": "d"}), "catalogId is 'c'")]
#[case(json!({"theme": {"type": "object"}}), "t: no theme, expected {\"type\":\"object\"}")]
fn entries_mismatch_is_reported(#[case] expected: Value, #[case] fragment: &str) {
    let loaded = catalog(json!({"components": {"A": {"properties": {"x": {}}}}}));
    assert_that!(entries(&expect(expected), &loaded, "t").unwrap_err()).contains(fragment);
}
