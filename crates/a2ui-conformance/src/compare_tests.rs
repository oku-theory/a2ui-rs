use a2ui_runtime::Pointer;
use rstest::rstest;
use serde_json::json;
use speculoos::prelude::*;

use super::*;

#[rstest]
#[case(json!(25000), json!(25000.0))]
#[case(json!({"a": [1, {"b": true}]}), json!({"a": [1.0, {"b": true}]}))]
#[case(json!(null), json!(null))]
fn json_eq_equivalent_values_are_equal(#[case] a: Value, #[case] b: Value) {
    assert_that!(json_eq(&a, &b)).is_true();
}

#[rstest]
#[case(json!(1), json!("1"))]
#[case(json!([1, 2]), json!([2, 1]))]
#[case(json!({"a": 1}), json!({"a": 1, "b": 2}))]
fn json_eq_different_values_are_not_equal(#[case] a: Value, #[case] b: Value) {
    assert_that!(json_eq(&a, &b)).is_false();
}

#[test]
fn expect_error_matching_category_and_message_passes() {
    let err = Pointer::parse("/__proto__").unwrap_err();
    let expected = json!({"category": "DataError", "message": "Forbidden path segment"});
    assert_that!(expect_error(&err, &expected, "here")).is_ok();
}

#[test]
fn expect_error_wrong_category_fails() {
    let err = Pointer::parse("/__proto__").unwrap_err();
    assert_that!(expect_error(
        &err,
        &json!({"category": "ParseError"}),
        "here"
    ))
    .is_err();
}

#[test]
fn expect_error_unknown_key_fails() {
    let err = Pointer::parse("/__proto__").unwrap_err();
    let expected = json!({"category": "DataError", "surprise": 1});
    assert_that!(expect_error(&err, &expected, "here")).is_err();
}

#[test]
fn expect_error_non_string_message_fails() {
    let err = Pointer::parse("/__proto__").unwrap_err();
    let expected = json!({"category": "DataError", "message": 5});
    let result = expect_error(&err, &expected, "here");
    assert_that!(result).is_err();
    assert_that!(result.unwrap_err().contains("must be a string")).is_true();
}

#[test]
fn expect_error_message_mismatch_fails_naming_the_mismatch() {
    let err = Pointer::parse("/__proto__").unwrap_err();
    let expected = json!({"category": "DataError", "message": "^nothing like this$"});
    let result = expect_error(&err, &expected, "here");
    assert_that!(result.unwrap_err().contains("does not match")).is_true();
}

#[test]
fn expect_error_non_mapping_expected_fails() {
    let err = Pointer::parse("/__proto__").unwrap_err();
    let result = expect_error(&err, &json!("DataError"), "here");
    assert_that!(result.unwrap_err().contains("must be a mapping")).is_true();
}

#[rstest]
#[case(json!({"enum": ["a", "b"]}), json!({"enum": ["b", "a"]}))]
#[case(json!({"required": ["x", "y"]}), json!({"required": ["y", "x"]}))]
#[case(json!({"properties": {"enum": {"enum": [1, 2]}}}), json!({"properties": {"enum": {"enum": [2, 1]}}}))]
#[case(json!({"a": [1, {"b": 2.0}]}), json!({"a": [1.0, {"b": 2}]}))]
fn schema_eq_equivalent_schemas_are_equal(#[case] a: Value, #[case] b: Value) {
    assert_that!(schema_eq(&a, &b)).is_true();
}

#[rstest]
#[case(json!({"oneOf": [{"type": "string"}, {"type": "number"}]}), json!({"oneOf": [{"type": "number"}, {"type": "string"}]}))]
#[case(json!({"enum": ["a", "a"]}), json!({"enum": ["a", "b"]}))]
#[case(json!({"required": ["x"]}), json!({"required": ["x", "y"]}))]
#[case(json!({"a": 1}), json!({"a": 1, "b": 2}))]
fn schema_eq_other_differences_are_detected(#[case] a: Value, #[case] b: Value) {
    assert_that!(schema_eq(&a, &b)).is_false();
}

#[test]
fn expect_error_literal_message_with_regex_metacharacters_matches() {
    let err = Pointer::parse("$x").unwrap_err();
    let expected = json!({"category": "DataError", "message": "Pointer '$x'"});
    assert_that!(expect_error(&err, &expected, "here")).is_ok();
}

#[test]
fn expect_error_literal_message_is_case_insensitive() {
    let err = Pointer::parse("$x").unwrap_err();
    let expected = json!({"category": "DataError", "message": "invalid json pointer"});
    assert_that!(expect_error(&err, &expected, "here")).is_ok();
}

#[test]
fn expect_error_regex_message_matches() {
    let err = Pointer::parse("$x").unwrap_err();
    let expected = json!({"category": "DataError", "message": "^Invalid JSON Pointer '.x'"});
    assert_that!(expect_error(&err, &expected, "here")).is_ok();
}

#[test]
fn expect_error_regex_message_is_case_insensitive() {
    let err = Pointer::parse("$x").unwrap_err();
    let expected = json!({"category": "DataError", "message": "^INVALID json pointer '.x'"});
    assert_that!(expect_error(&err, &expected, "here")).is_ok();
}

#[test]
fn expect_error_invalid_regex_found_nowhere_is_reported() {
    let err = Pointer::parse("$x").unwrap_err();
    let expected = json!({"category": "DataError", "message": "something else ("});
    let result = expect_error(&err, &expected, "here");
    assert_that!(result.unwrap_err()).contains("does not match");
}
