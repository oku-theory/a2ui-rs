use std::collections::BTreeSet;

use serde_json::json;
use speculoos::prelude::*;

use super::super::test_support::{load, v10};
use super::ComponentShape;

fn names(items: &BTreeSet<String>) -> Vec<&str> {
    items.iter().map(String::as_str).collect()
}

fn boxed() -> crate::Catalog {
    load(v10(json!({"components": {"Box": {"allOf": [
        {"$ref": "common_types.json#/$defs/ComponentCommon"},
        {"properties": {"component": {"const": "Box"}, "child": {}}, "required": ["component"]}
    ]}}})))
    .unwrap()
}

#[test]
fn shape_properties_follow_all_of_and_refs() {
    let catalog = boxed();
    let shape = catalog.component("Box").unwrap().shape();
    assert_that!(names(shape.properties())).is_equal_to(vec![
        "accessibility",
        "catalogId",
        "child",
        "component",
        "id",
        "metadata",
    ]);
}

#[test]
fn shape_required_follow_all_of_and_refs() {
    let catalog = boxed();
    let shape = catalog.component("Box").unwrap().shape();
    assert_that!(names(shape.required())).is_equal_to(vec!["component", "id"]);
}

#[test]
fn of_ref_cycle_terminates_with_every_name() {
    let root = json!({"$defs": {
        "X": {"properties": {"x": {}}, "allOf": [{"$ref": "#/$defs/Y"}]},
        "Y": {"properties": {"y": {}}, "allOf": [{"$ref": "#/$defs/X"}]}
    }});
    let shape = ComponentShape::of(&root, &json!({"$ref": "#/$defs/X"}));
    assert_that!(names(shape.properties())).is_equal_to(vec!["x", "y"]);
}

#[test]
fn shape_percent_encoded_ref_is_followed() {
    let document = v10(json!({
        "components": {"A": {"$ref": "#/$defs/Base_%E2%84%93"}},
        "$defs": {"Base_ℓ": {"properties": {"b": {}}}}
    }));
    let catalog = load(document).unwrap();
    let shape = catalog.component("A").unwrap().shape();
    assert_that!(names(shape.properties())).is_equal_to(vec!["b"]);
}
