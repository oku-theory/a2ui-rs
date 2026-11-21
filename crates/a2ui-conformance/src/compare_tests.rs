use a2ui_runtime::{AgentMessage, MessageLimits, Pointer};
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
    let result = expect_error(&err, &json!(5), "here");
    assert_that!(
        result
            .unwrap_err()
            .contains("must be a mapping or a string")
    )
    .is_true();
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
    let expected = json!({"category": "ValidationError", "message": "path syntax '$x'"});
    assert_that!(expect_error(&err, &expected, "here")).is_ok();
}

#[test]
fn expect_error_literal_message_is_case_insensitive() {
    let err = Pointer::parse("$x").unwrap_err();
    let expected = json!({"category": "ValidationError", "message": "invalid PATH syntax"});
    assert_that!(expect_error(&err, &expected, "here")).is_ok();
}

#[test]
fn expect_error_regex_message_matches() {
    let err = Pointer::parse("$x").unwrap_err();
    let expected = json!({"category": "ValidationError", "message": "^Invalid path syntax '.x'"});
    assert_that!(expect_error(&err, &expected, "here")).is_ok();
}

#[test]
fn expect_error_regex_message_is_case_insensitive() {
    let err = Pointer::parse("$x").unwrap_err();
    let expected = json!({"category": "ValidationError", "message": "^INVALID path syntax '.x'"});
    assert_that!(expect_error(&err, &expected, "here")).is_ok();
}

#[test]
fn expect_error_invalid_regex_found_nowhere_is_reported() {
    let err = Pointer::parse("$x").unwrap_err();
    let expected = json!({"category": "ValidationError", "message": "something else ("});
    let result = expect_error(&err, &expected, "here");
    assert_that!(result.unwrap_err()).contains("does not match");
}

fn error_of(message: Value) -> a2ui_runtime::Error {
    AgentMessage::from_json(&message, &MessageLimits::default()).unwrap_err()
}

fn deep_component() -> a2ui_runtime::Error {
    let text = (0..51).fold(json!("x"), |inner, _| json!({"a": inner}));
    error_of(json!({"version": "1.0", "updateComponents": {
        "surfaceId": "s1", "components": [{"id": "root", "component": "Text", "text": text}]}}))
}

fn delete_empty() -> a2ui_runtime::Error {
    error_of(json!({"version": "1.0", "deleteSurface": {}}))
}

#[test]
fn expect_error_string_matches_message_only() {
    let err = Pointer::parse("/__proto__").unwrap_err();
    assert_that!(expect_error(&err, &json!("forbidden path"), "here")).is_ok();
    assert_that!(expect_error(&err, &json!("nothing like this"), "here")).is_err();
}

#[test]
fn expect_error_validation_accepts_recursion() {
    let expected = json!({"category": "ValidationError"});
    assert_that!(expect_error(&deep_component(), &expected, "here")).is_ok();
}

#[test]
fn expect_error_recursion_refuses_validation() {
    let expected = json!({"category": "RecursionError"});
    assert_that!(expect_error(&delete_empty(), &expected, "here")).is_err();
}

#[test]
fn expect_error_message_is_literal_before_regex() {
    let err = error_of(json!({"version": "1.0", "a['Column']": {}}));
    let expected = json!({"category": "ValidationError", "message": "a['Column']"});
    assert_that!(expect_error(&err, &expected, "here")).is_ok();
}

fn create_with_theme() -> a2ui_runtime::Error {
    error_of(json!({"version": "1.0", "createSurface": {"surfaceId": "s1", "theme": {}}}))
}

fn call_err_1() -> a2ui_runtime::Error {
    error_of(json!({"version": "1.0", "callRendererFunction": {"functionCallId": "call-err-1"}}))
}

#[test]
fn expect_error_compares_code_path_and_ids_when_equal() {
    let expected = json!({"category": "ValidationError", "code": "VALIDATION_FAILED",
        "path": "/surfaceId"});
    assert_that!(expect_error(&delete_empty(), &expected, "here")).is_ok();
    let expected = json!({"surfaceId": "s1", "path": "/theme"});
    assert_that!(expect_error(&create_with_theme(), &expected, "here")).is_ok();
    let expected = json!({"code": "INVALID_FUNCTION_CALL", "functionCallId": "call-err-1"});
    assert_that!(expect_error(&call_err_1(), &expected, "here")).is_ok();
}

#[rstest]
#[case::code(delete_empty(), json!({"code": "DATA_ERROR"}), "code")]
#[case::path(delete_empty(), json!({"path": "/other"}), "path")]
#[case::surface_id(create_with_theme(), json!({"surfaceId": "s2"}), "surfaceId")]
#[case::absent_surface_id(delete_empty(), json!({"surfaceId": "s2"}), "surfaceId")]
#[case::function_call_id(call_err_1(), json!({"functionCallId": "c2"}), "functionCallId")]
fn expect_error_compares_code_path_and_ids(
    #[case] err: a2ui_runtime::Error,
    #[case] expected: Value,
    #[case] key: &str,
) {
    let result = expect_error(&err, &expected, "here");
    assert_that!(result.unwrap_err()).contains(key);
}

#[test]
fn expect_error_compares_details_by_index_and_length() {
    let err = create_with_theme();
    let detail = json!({"path": "createSurface.theme", "code": "invalid_value"});
    assert_that!(expect_error(&err, &json!({"details": [detail]}), "here")).is_ok();
    assert_that!(expect_error(&err, &json!({"details": []}), "here")).is_err();
    let wrong = json!({"details": [{"path": "createSurface.other"}]});
    assert_that!(expect_error(&err, &wrong, "here").unwrap_err()).contains("details[0].path");
    let wrong = json!({"details": [{"code": "missing_field"}]});
    assert_that!(expect_error(&err, &wrong, "here").unwrap_err()).contains("details[0].code");
}

fn processed_create_with_theme() -> a2ui_runtime::Error {
    let mut processor = a2ui_runtime::MessageProcessor::new(a2ui_runtime::CatalogSet::new());
    let batch = json!([{"version": "1.0", "createSurface": {"surfaceId": "s1", "theme": {}}}]);
    processor.handle_batch(&batch).unwrap_err()
}

#[test]
fn expect_error_details_surface_id_is_compared() {
    let err = processed_create_with_theme();
    let good = json!({"details": [{"surfaceId": "s1"}]});
    assert_that!(expect_error(&err, &good, "here")).is_ok();
    let wrong = json!({"details": [{"surfaceId": "s2"}]});
    assert_that!(expect_error(&err, &wrong, "here").unwrap_err()).contains("details[0].surfaceId");
}

#[test]
fn expect_error_refuses_unknown_keys() {
    let expected = json!({"category": "ValidationError", "surprise": 1});
    let result = expect_error(&delete_empty(), &expected, "here");
    assert_that!(result.unwrap_err()).contains("Unknown key 'surprise'");
}

#[test]
fn expect_error_renderer_error_is_checked() {
    let err = call_err_1();
    let good =
        json!({"rendererError": {"version": "1.0", "error": {"code": "INVALID_FUNCTION_CALL"}}});
    assert_that!(expect_error(&err, &good, "here")).is_ok();
    let bad = json!({"rendererError": {"version": "1.0", "error": {"code": "DATA_ERROR"}}});
    assert_that!(expect_error(&err, &bad, "here").unwrap_err()).contains("rendererError");
}
