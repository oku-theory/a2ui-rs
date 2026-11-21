use a2ui_runtime::{AgentMessage, MessageLimits};
use rstest::rstest;
use serde_json::{Value, json};
use speculoos::prelude::*;

use super::*;

fn error_of(message: Value) -> a2ui_runtime::Error {
    AgentMessage::from_json(&message, &MessageLimits::default()).unwrap_err()
}

fn call_err_1() -> a2ui_runtime::Error {
    error_of(json!({"version": "1.0", "callRendererFunction": {"functionCallId": "call-err-1"}}))
}

fn deep(depth: usize) -> Value {
    (0..depth).fold(json!("x"), |inner, _| json!({"a": inner}))
}

#[test]
fn renderer_error_matches_expected() {
    let expected = json!({"version": "1.0", "error": {
        "code": "INVALID_FUNCTION_CALL", "functionCallId": "call-err-1",
        "message": "callFunction"}});
    assert_that!(check_renderer_error(&call_err_1(), &expected, "here")).is_ok();
}

#[rstest]
#[case::missing_version(json!({"createSurface": {"surfaceId": "s1", "catalogId": "c"}}))]
#[case::empty_delete(json!({"version": "1.0", "deleteSurface": {}}))]
#[case::call_err_1(json!({"version": "1.0",
    "callRendererFunction": {"functionCallId": "call-err-1"}}))]
#[case::deep_component(json!({"version": "1.0", "updateComponents": {
    "surfaceId": "s1", "components": [{"id": "root", "component": "Text", "text": deep(51)}]}}))]
fn renderer_error_validates_against_upstream_schema(#[case] message: Value) {
    let err = error_of(message);
    assert_that!(check_renderer_error(
        &err,
        &json!({"version": "1.0"}),
        "here"
    ))
    .is_ok();
}

#[rstest]
#[case::code(json!({"code": "DATA_ERROR"}), "code")]
#[case::function_call_id(json!({"functionCallId": "other"}), "functionCallId")]
#[case::message(json!({"message": "nothing like this"}), "message")]
#[case::path(json!({"path": "/x"}), "path")]
fn renderer_error_mismatch_names_the_field(#[case] error: Value, #[case] key: &str) {
    let expected = json!({"version": "1.0", "error": error});
    let result = check_renderer_error(&call_err_1(), &expected, "here");
    assert_that!(result.unwrap_err()).contains(key);
}

#[test]
fn renderer_error_unknown_key_fails() {
    let expected = json!({"version": "1.0", "error": {"surprise": 1}});
    let result = check_renderer_error(&call_err_1(), &expected, "here");
    assert_that!(result.unwrap_err()).contains("Unknown key 'surprise'");
}
