use rstest::rstest;
use serde_json::{Value, json};
use speculoos::prelude::*;

use super::*;
use crate::Category;

#[test]
fn common_types_schema_embedded_document_is_v1_0() {
    let schema = common_types_schema().unwrap();
    assert_that!(schema["$id"].as_str()).is_equal_to(Some(
        "https://a2ui.org/specification/v1_0/common_types.json",
    ));
}

#[test]
fn agent_to_renderer_schema_embedded_document_is_v1_0() {
    let schema = agent_to_renderer_schema().unwrap();
    assert_that!(schema["$id"].as_str()).is_equal_to(Some(
        "https://a2ui.org/specification/v1_0/agent_to_renderer.json",
    ));
}

#[test]
fn common_type_defs_embedded_document_holds_dynamic_string() {
    assert_that!(common_type_defs().unwrap().contains_key("DynamicString")).is_true();
}

#[rstest]
#[case("Extensions", json!({"vendor": {"any": "value"}}))]
#[case("Extensions", json!({"_private": 1}))]
#[case("Extensions", json!({"ünïcode": 1}))]
#[case("DynamicValue", json!({"@@path": "/a"}))]
#[case("DynamicValue", json!({"@call": "now"}))]
#[case("FunctionCall", json!({"@call": "x", "catalogId": "c", "args": {"a": 1}}))]
#[case("FunctionCall", json!({"@call": "@index"}))]
#[case("FunctionResponse", json!({"functionCallId": "call_1", "value": null}))]
#[case("Surface", json!({"component": "Surface", "child": "root"}))]
fn validate_common_type_valid_value_is_accepted(#[case] definition: &str, #[case] value: Value) {
    assert_that!(validate_common_type(definition, &value)).is_ok();
}

#[rstest]
#[case("Extensions", json!({"has-dash": 1}), "'has-dash' was unexpected")]
#[case("DynamicValue", json!({"@unknown": 1}), "'oneOf'")]
#[case("DynamicValue", json!({"@call": "now", "@path": "/a"}), "'oneOf'")]
#[case("FunctionCall", json!({"@call": "x", "bogus": 1}), "'bogus' was unexpected")]
#[case("FunctionResponse", json!({"functionCallId": "call_1"}), "'oneOf'")]
#[case("IndexSystemFunction", json!({"@call": "@index", "args": {"offset": 1, "step": 2}}), "'step' was unexpected")]
#[case("Surface", json!({"component": "Surface", "child": "root", "id": "surface"}), "'id' was unexpected")]
fn validate_common_type_invalid_value_is_validation_error(
    #[case] definition: &str,
    #[case] value: Value,
    #[case] fragment: &str,
) {
    let err = validate_common_type(definition, &value).unwrap_err();
    assert_that!(err.category()).is_equal_to(Category::Validation);
    assert_that!(err.to_string()).contains(fragment);
}

#[test]
fn validate_common_type_unknown_definition_is_validation_error() {
    let err = validate_common_type("Nope", &json!({})).unwrap_err();
    assert_that!(err.to_string()).contains("Unknown common type 'Nope'");
}

#[test]
fn validate_common_type_too_deep_value_is_validation_error() {
    let value = (0..65).fold(json!(1), |inner, _| json!([inner]));
    let err = validate_common_type("DynamicValue", &value).unwrap_err();
    assert_that!(err.to_string()).contains("nesting exceeds 64");
}
