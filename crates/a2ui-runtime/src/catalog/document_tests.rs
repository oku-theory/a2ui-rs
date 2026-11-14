use rstest::rstest;
use serde_json::{Map, Value, json};
use speculoos::prelude::*;

use super::*;

fn map(value: Value) -> Map<String, Value> {
    value.as_object().unwrap().clone()
}

#[rstest]
#[case(json!({"properties": {"args": {"type": "object", "properties": {"url": {}}}}}), vec!["url"])]
#[case(json!({"parameters": {"arg_ℓ": {"type": "string"}}}), vec!["arg_ℓ"])]
#[case(
    json!({"args": {
        "type": "object",
        "properties": {"a": {}},
        "allOf": [{"properties": {"b": {}}}]
    }}),
    vec!["a", "b"]
)]
#[case(json!({"args": {"a": {"type": "string"}}}), vec![])]
#[case(
    json!({"parameters": {"type": {"type": "string"}, "properties": {"type": "object"}}}),
    vec!["properties", "type"]
)]
#[case(json!({"returnType": "string"}), vec![])]
fn argument_names_every_form_is_read(#[case] schema: Value, #[case] expected: Vec<&str>) {
    let names = argument_names(&map(schema));
    assert_that!(names).is_equal_to(expected.into_iter().map(str::to_owned).collect::<Vec<_>>());
}

#[test]
fn property_names_composition_branches_are_included() {
    let schema = map(json!({
        "properties": {"a": {}},
        "allOf": [{"$ref": "#/x"}, {"properties": {"b": {}}}]
    }));
    assert_that!(property_names(&schema)).is_equal_to(vec!["a".to_owned(), "b".to_owned()]);
}

#[rstest]
#[case(json!({"returnType": "void"}), Some("void"))]
#[case(json!({"properties": {"returnType": {"const": "string"}}}), Some("string"))]
#[case(json!({"type": "object"}), None)]
fn return_type_both_forms_are_read(#[case] schema: Value, #[case] expected: Option<&str>) {
    assert_that!(return_type(&map(schema)).as_deref()).is_equal_to(expected);
}

#[test]
fn union_members_referenced_entries_are_listed() {
    let defs = map(json!({"anyComponent": {"oneOf": [
        {"$ref": "#/components/A"},
        {"$ref": "catalog.json#/components/B"},
        {"$ref": "#/components/Component_%E2%84%93"}
    ]}}));
    let members = union_members(&defs, "anyComponent", "components").unwrap();
    assert_that!(members.into_iter().collect::<Vec<_>>()).is_equal_to(vec![
        "A".to_owned(),
        "B".to_owned(),
        "Component_ℓ".to_owned(),
    ]);
}

#[test]
fn union_members_escaped_name_is_decoded() {
    let defs = map(json!({"anyFunction": {"oneOf": [{"$ref": "#/functions/a~1b~0c"}]}}));
    let members = union_members(&defs, "anyFunction", "functions").unwrap();
    assert_that!(members.into_iter().collect::<Vec<_>>()).is_equal_to(vec!["a/b~c".to_owned()]);
}

#[rstest]
#[case(json!({}))]
#[case(json!({"anyFunction": {"not": {}}}))]
#[case(json!({"anyFunction": {"oneOf": []}}))]
#[case(json!({"anyFunction": {"oneOf": [{"$ref": "#/$defs/Other"}]}}))]
#[case(json!({"anyFunction": {"oneOf": [{"$ref": "#/functions/f/properties"}]}}))]
#[case(json!({"anyFunction": {"oneOf": [{"$ref": "#/functions/a~2b"}]}}))]
fn union_members_no_listed_entry_is_none(#[case] defs: Value) {
    assert_that!(union_members(&map(defs), "anyFunction", "functions")).is_none();
}

#[test]
fn union_with_discriminator_lists_local_refs() {
    let value = union("components", &["A", "B"], Some("component"));
    assert_that!(value).is_equal_to(json!({
        "oneOf": [{"$ref": "#/components/A"}, {"$ref": "#/components/B"}],
        "discriminator": {"propertyName": "component"}
    }));
}

#[test]
fn union_names_are_sorted() {
    let value = union("components", &["B", "A"], None);
    assert_that!(value)
        .is_equal_to(json!({"oneOf": [{"$ref": "#/components/A"}, {"$ref": "#/components/B"}]}));
}

#[test]
fn union_non_ascii_name_is_percent_encoded() {
    let value = union("functions", &["func_ℓ"], None);
    assert_that!(value).is_equal_to(json!({"oneOf": [{"$ref": "#/functions/func_%E2%84%93"}]}));
}

#[test]
fn union_without_entries_matches_nothing() {
    assert_that!(union("functions", &[], None)).is_equal_to(json!({"not": {}}));
}
