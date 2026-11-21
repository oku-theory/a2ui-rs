use rstest::rstest;
use serde_json::{Value, json};
use speculoos::prelude::*;

use crate::{AgentMessage, Category, Error, ErrorCode, MessageLimits};

/// A value of `levels` nested objects, the innermost empty.
fn nested(levels: usize) -> Value {
    let mut value = json!({});
    for _ in 1..levels {
        value = json!({"a": value});
    }
    value
}

/// `count` nested function calls, the innermost without a nested call.
fn chain(count: usize) -> Value {
    let mut call = json!({"@call": format!("f{}", count - 1), "args": {"value": "x"}});
    for index in (0..count - 1).rev() {
        call = json!({"@call": format!("f{index}"), "args": {"value": call}});
    }
    call
}

fn update(component: Value) -> Value {
    json!({"version": "1.0", "updateComponents": {"surfaceId": "s1", "components": [component]}})
}

fn read(message: &Value) -> Result<AgentMessage, Error> {
    AgentMessage::from_json(message, &MessageLimits::default())
}

fn text(value: Value) -> Value {
    json!({"id": "a", "component": "Text", "text": value})
}

#[test]
fn component_at_depth_limit_is_accepted() {
    assert_that!(read(&update(text(nested(49)))).is_ok()).is_true();
}

#[test]
fn component_past_depth_limit_is_refused() {
    let error = read(&update(text(nested(50)))).expect_err("51 containers");
    assert_that!((error.category(), error.code()))
        .is_equal_to((Category::Recursion, ErrorCode::RecursionLimitExceeded));
    assert_that!(error.to_string()).contains("Global recursion limit exceeded");
    assert_that!(error.path()).is_equal_to(Some("/components/0"));
    assert_that!(error.surface_id()).is_equal_to(Some("s1"));
}

#[rstest]
#[case::update_value(
    json!({"version": "1.0",
        "updateDataModel": {"surfaceId": "s1", "value": nested(200)}}),
    "/value", Some("s1"), None
)]
#[case::create_data_model(
    json!({"version": "1.0",
        "createSurface": {"surfaceId": "s1", "dataModel": nested(200)}}),
    "/dataModel", Some("s1"), None
)]
#[case::call_args(
    json!({"version": "1.0", "callRendererFunction": {"functionCallId": "c1",
        "callFunction": {"@call": "f", "catalogId": "c", "args": nested(200)}}}),
    "/callFunction/args", None, Some("c1")
)]
fn deep_values_are_refused_before_the_schema(
    #[case] message: Value,
    #[case] path: &str,
    #[case] surface: Option<&str>,
    #[case] call: Option<&str>,
) {
    let error = read(&message).expect_err("200 containers");
    assert_that!((error.category(), error.code()))
        .is_equal_to((Category::Recursion, ErrorCode::RecursionLimitExceeded));
    assert_that!(error.path()).is_equal_to(Some(path));
    assert_that!(error.surface_id()).is_equal_to(surface);
    assert_that!(error.function_call_id()).is_equal_to(call);
}

#[test]
fn call_chain_of_five_is_accepted() {
    assert_that!(read(&update(text(chain(5)))).is_ok()).is_true();
}

#[test]
fn call_chain_of_six_is_refused() {
    let error = read(&update(text(chain(6)))).expect_err("six nested calls");
    assert_that!((error.category(), error.code()))
        .is_equal_to((Category::Recursion, ErrorCode::RecursionLimitExceeded));
    assert_that!(error.to_string()).contains("functionCall depth 6");
    assert_that!(
        error
            .path()
            .is_some_and(|p| p.starts_with("/components/0/text"))
    )
    .is_true();
    assert_that!(error.surface_id()).is_equal_to(Some("s1"));
}

#[rstest]
#[case::empty("")]
#[case::relative("name")]
#[case::absolute("/a/b")]
fn component_paths_are_checked(#[case] path: &str) {
    assert_that!(read(&update(text(json!({"@path": path})))).is_ok()).is_true();
}

#[rstest]
#[case::relative("a/~2")]
#[case::absolute("/invalid/escape/~2")]
fn component_paths_with_a_bad_escape_are_refused(#[case] path: &str) {
    let error = read(&update(text(json!({"@path": path})))).expect_err("bad escape");
    assert_that!((error.category(), error.code()))
        .is_equal_to((Category::Validation, ErrorCode::ValidationFailed));
    assert_that!(error.to_string()).contains("Invalid path syntax");
    assert_that!(error.path()).is_equal_to(Some("/components/0/text"));
}

#[test]
fn component_path_naming_a_forbidden_segment_is_refused_as_data() {
    let error = read(&update(text(json!({"@path": "/__proto__/x"})))).expect_err("forbidden");
    assert_that!((error.category(), error.code()))
        .is_equal_to((Category::Data, ErrorCode::DataError));
    assert_that!(error.path()).is_equal_to(Some("/components/0/text"));
}

#[test]
fn bindings_in_a_create_surface_component_are_checked() {
    let message = json!({"version": "1.0", "createSurface": {"surfaceId": "s1",
        "components": [text(json!({"@path": "/ok"})), text(json!([{"@path": "a/~2"}]))]}});
    let error = read(&message).expect_err("bad escape");
    assert_that!(error.path()).is_equal_to(Some("/components/1/text/0"));
}
