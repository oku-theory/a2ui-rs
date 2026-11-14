use serde_json::{Map, Value, json};
use speculoos::prelude::*;

use super::*;
use crate::Category;

fn top(value: Value) -> Map<String, Value> {
    value.as_object().unwrap().clone()
}

fn compile_def(definition: Value) -> Result<Validator, crate::Error> {
    compile(&rooted(
        &top(json!({"$defs": {"D": definition}})),
        "#/$defs/D",
    ))
}

#[test]
fn check_matching_instance_passes() {
    let validator = compile_def(json!({"type": "string"})).unwrap();
    assert_that!(check(&validator, &json!("a"))).is_ok();
}

#[test]
fn check_violation_is_validation_error() {
    let validator = compile_def(json!({"type": "string"})).unwrap();
    let err = check(&validator, &json!(1)).unwrap_err();
    assert_that!(err.category()).is_equal_to(Category::Validation);
}

#[test]
fn check_violation_message_names_the_instance_path() {
    let validator = compile_def(json!({"properties": {"a": {"type": "string"}}})).unwrap();
    let err = check(&validator, &json!({"a": 1})).unwrap_err();
    assert_that!(err.to_string()).ends_with(" at '/a'");
}

fn extensions() -> Value {
    json!({
        "type": "object",
        "patternProperties": {"^[\\p{XID_Start}_][\\p{XID_Continue}]*$": {}},
        "additionalProperties": false
    })
}

#[test]
fn check_unicode_property_pattern_rejects_a_dash() {
    let validator = compile_def(extensions()).unwrap();
    assert_that!(check(&validator, &json!({"has-dash": 1}))).is_err();
}

#[test]
fn check_unicode_property_pattern_accepts_non_ascii_letters() {
    let validator = compile_def(extensions()).unwrap();
    assert_that!(check(&validator, &json!({"ünïcode": 1}))).is_ok();
}

#[test]
fn compile_lookahead_pattern_is_catalog_error() {
    let err = compile_def(json!({"type": "string", "pattern": "(?=a)b"})).unwrap_err();
    assert_that!(err.category()).is_equal_to(Category::Catalog);
    assert_that!(err.to_string()).contains("is not a \"regex\"");
}

#[test]
fn compile_pattern_past_the_size_cap_is_catalog_error() {
    let err = compile_def(json!({"type": "string", "pattern": "\\w{5000}"})).unwrap_err();
    assert_that!(err.to_string()).contains("is not a \"regex\"");
}

#[test]
fn compile_external_ref_is_catalog_error() {
    let err = compile_def(json!({"$ref": "https://example.com/x.json"})).unwrap_err();
    assert_that!(err.to_string()).contains("Retrieval is disabled");
}

#[test]
fn compile_external_ref_no_root_reaches_is_catalog_error() {
    let document = json!({
        "$defs": {"D": {"type": "string"}, "E": {"$ref": "https://example.com/x.json"}}
    });
    let err = compile(&rooted(&top(document), "#/$defs/D")).unwrap_err();
    assert_that!(err.category()).is_equal_to(Category::Catalog);
    assert_that!(err.to_string()).contains("Retrieval is disabled");
}

#[test]
fn compile_non_ascii_name_through_local_ref_builds() {
    let document = json!({"$defs": {"Component_ℓ": {"type": "string"}}});
    let reference = crate::bundle::local_ref("/$defs/Component_ℓ");
    assert_that!(compile(&rooted(&top(document), &reference))).is_ok();
}
