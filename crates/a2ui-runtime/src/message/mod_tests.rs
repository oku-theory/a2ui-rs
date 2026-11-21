use rstest::rstest;
use serde_json::{Value, json};
use speculoos::prelude::*;

use super::*;
use crate::{Category, DetailCode, ErrorCode};

const V: &str = "1.0";

/// The function call the `callRendererFunction` cases carry.
fn function() -> Map<String, Value> {
    let mut call = Map::new();
    call.insert("@call".to_owned(), json!("f"));
    call.insert("catalogId".to_owned(), json!("c"));
    call
}

fn create(body: Value) -> Value {
    json!({"version": V, "createSurface": body})
}

fn call(body: Value) -> Value {
    json!({"version": V, "callRendererFunction": body})
}

#[rstest]
#[case::not_object(
    (Category::Validation, ErrorCode::ValidationFailed),
    json!([]), "must be a JSON object", None, None, None, None)]
#[case::conflicting(
    (Category::Validation, ErrorCode::ValidationFailed),
    json!({"version": V, "createSurface": {"surfaceId": "s"}, "deleteSurface": {"surfaceId": "s"}}),
    "multiple conflicting update actions", None, None, None, None
)]
#[case::no_version(
    (Category::Validation, ErrorCode::ValidationFailed),
    json!({"createSurface": {"surfaceId": "s1"}}),
    "Missing required version field", None, Some(("version", DetailCode::MissingField)), None, None
)]
#[case::version_type(
    (Category::Validation, ErrorCode::ValidationFailed),
    json!({"version": 1, "createSurface": {"surfaceId": "s1"}}),
    "version must be a string", None, Some(("version", DetailCode::TypeMismatch)), None, None
)]
#[case::version_value(
    (Category::Validation, ErrorCode::ValidationFailed),
    json!({"version": "v99.0", "createSurface": {"surfaceId": "s1"}}),
    "Unsupported protocol version 'v99.0'", None,
    Some(("version", DetailCode::InvalidValue)), None, None
)]
#[case::v08_action(
    (Category::Validation, ErrorCode::ValidationFailed),
    json!({"version": V, "beginRendering": {}}),
    "action 'beginRendering' is not supported in protocol version 1.0", None, None, None, None
)]
#[case::root_extra(
    (Category::Validation, ErrorCode::ValidationFailed),
    json!({"version": V, "createSurface": {"surfaceId": "s1"}, "extra": 1}),
    "Message has unexpected top-level field 'extra'", None,
    Some(("extra", DetailCode::InvalidValue)), Some("s1"), None
)]
#[case::no_action(
    (Category::Validation, ErrorCode::ValidationFailed),
    json!({"version": V}), "Message has no action", None, None, None, None)]
#[case::surface_id_type(
    (Category::Validation, ErrorCode::ValidationFailed),
    create(json!({"surfaceId": 123})),
    "createSurface.surfaceId must be a string", Some("/surfaceId"),
    Some(("createSurface.surfaceId", DetailCode::TypeMismatch)), None, None
)]
#[case::delete_missing(
    (Category::Validation, ErrorCode::ValidationFailed),
    json!({"version": V, "deleteSurface": {}}),
    "deleteSurface is missing required field 'surfaceId'", Some("/surfaceId"),
    Some(("deleteSurface.surfaceId", DetailCode::MissingField)), None, None
)]
#[case::theme(
    (Category::Validation, ErrorCode::ValidationFailed),
    create(json!({"surfaceId": "s1", "theme": {}})),
    "createSurface has unexpected field 'theme'", Some("/theme"),
    Some(("createSurface.theme", DetailCode::InvalidValue)), Some("s1"), None
)]
#[case::empty_components(
    (Category::Validation, ErrorCode::ValidationFailed),
    create(json!({"surfaceId": "s1", "components": []})),
    "createSurface.components is invalid", Some("/components"),
    Some(("createSurface.components", DetailCode::InvalidValue)), Some("s1"), None
)]
#[case::component_type(
    (Category::Validation, ErrorCode::ValidationFailed),
    json!({"version": V, "updateComponents": {"surfaceId": "s1", "components": [42]}}),
    "updateComponents.components.0 must be an object", Some("/components/0"),
    Some(("updateComponents.components.0", DetailCode::TypeMismatch)), Some("s1"), None
)]
#[case::bad_pointer(
    (Category::Validation, ErrorCode::ValidationFailed),
    json!({"version": V,
        "updateDataModel": {"surfaceId": "main", "path": "not_a_pointer", "value": 1}}),
    "Invalid path syntax 'not_a_pointer'", Some("/path"), None, Some("main"), None
)]
#[case::proto_pointer(
    (Category::Data, ErrorCode::DataError),
    json!({"version": V,
        "updateDataModel": {"surfaceId": "main", "path": "/__proto__/x", "value": 1}}),
    "__proto__", Some("/path"), None, Some("main"), None
)]
#[case::no_call_function(
    (Category::Validation, ErrorCode::InvalidFunctionCall),
    call(json!({"functionCallId": "call-err-1"})),
    "callFunction", Some("/callFunction"),
    Some(("callRendererFunction.callFunction", DetailCode::MissingField)), None, Some("call-err-1")
)]
#[case::args_type(
    (Category::Validation, ErrorCode::InvalidFunctionCall),
    call(json!({"functionCallId": "call-err-2",
        "callFunction": {"@call": "f", "catalogId": "c", "args": "not_an_object"}})),
    "callRendererFunction.callFunction.args must be an object", Some("/callFunction/args"),
    Some(("callRendererFunction.callFunction.args", DetailCode::TypeMismatch)),
    None, Some("call-err-2")
)]
#[case::no_catalog_id(
    (Category::Validation, ErrorCode::InvalidFunctionCall),
    call(json!({"functionCallId": "call-err-3", "callFunction": {"@call": "f"}})),
    "missing required field 'catalogId'", Some("/callFunction/catalogId"),
    Some(("callRendererFunction.callFunction.catalogId", DetailCode::MissingField)),
    None, Some("call-err-3")
)]
fn from_json_refuses_malformed_messages(
    #[case] class: (Category, ErrorCode),
    #[case] message: Value,
    #[case] fragment: &str,
    #[case] path: Option<&str>,
    #[case] detail: Option<(&str, DetailCode)>,
    #[case] surface: Option<&str>,
    #[case] function_call: Option<&str>,
) {
    let error = AgentMessage::from_json(&message).err();
    let error = error.expect("the message is malformed");
    assert_that!((error.category(), error.code())).is_equal_to(class);
    assert_that!(error.to_string()).contains(fragment);
    assert_that!(error.path()).is_equal_to(path);
    let details: Vec<_> = error
        .details()
        .iter()
        .map(|d| (d.path(), d.code()))
        .collect();
    assert_that!(details).is_equal_to(detail.into_iter().collect::<Vec<_>>());
    assert_that!(error.surface_id()).is_equal_to(surface);
    assert_that!(error.function_call_id()).is_equal_to(function_call);
}

#[rstest]
#[case::create_all(
    json!({"version": V, "createSurface": {
        "surfaceId": "s1", "catalogId": "basic", "sendDataModel": true,
        "components": [{"id": "root", "component": "Text"}],
        "dataModel": {"a": 1}, "metadata": {"extensions": {}}
    }}),
    AgentMessage::CreateSurface(CreateSurface {
        surface_id: "s1".to_owned(),
        catalog_id: Some("basic".to_owned()),
        send_data_model: true,
        components: vec![json!({"id": "root", "component": "Text"})],
        data_model: json!({"a": 1}).as_object().cloned(),
        metadata: json!({"extensions": {}}).as_object().cloned(),
    })
)]
#[case::create_minimal(
    create(json!({"surfaceId": "s1"})),
    AgentMessage::CreateSurface(CreateSurface {
        surface_id: "s1".to_owned(),
        catalog_id: None,
        send_data_model: false,
        components: vec![],
        data_model: None,
        metadata: None,
    })
)]
#[case::update_components(
    json!({"version": V, "updateComponents": {"surfaceId": "s1", "components": [{"id": "a"}]}}),
    AgentMessage::UpdateComponents(UpdateComponents {
        surface_id: "s1".to_owned(),
        components: vec![json!({"id": "a"})],
    })
)]
#[case::update_data_without_path(
    json!({"version": V, "updateDataModel": {"surfaceId": "s1", "value": null}}),
    AgentMessage::UpdateDataModel(UpdateDataModel {
        surface_id: "s1".to_owned(),
        path: Pointer::root(),
        value: Value::Null,
    })
)]
#[case::update_data_with_path(
    json!({"version": V, "updateDataModel": {"surfaceId": "s1", "path": "/a/b", "value": 2}}),
    AgentMessage::UpdateDataModel(UpdateDataModel {
        surface_id: "s1".to_owned(),
        path: Pointer::parse("/a/b").expect("a valid pointer"),
        value: json!(2),
    })
)]
#[case::delete(
    json!({"version": V, "deleteSurface": {"surfaceId": "s1"}}),
    AgentMessage::DeleteSurface(DeleteSurface { surface_id: "s1".to_owned() })
)]
#[case::call_without_surface(
    call(json!({"functionCallId": "c1", "callFunction": {"@call": "f", "catalogId": "c"}})),
    AgentMessage::CallRendererFunction(CallRendererFunction {
        function_call_id: "c1".to_owned(),
        surface_id: None,
        call_function: function(),
    })
)]
#[case::call_with_surface(
    call(json!({"functionCallId": "c1", "surfaceId": "s1",
        "callFunction": {"@call": "f", "catalogId": "c"}})),
    AgentMessage::CallRendererFunction(CallRendererFunction {
        function_call_id: "c1".to_owned(),
        surface_id: Some("s1".to_owned()),
        call_function: function(),
    })
)]
#[case::response(
    json!({"version": V, "agentFunctionResponse": {"functionCallId": "c1", "value": 3}}),
    AgentMessage::AgentFunctionResponse(AgentFunctionResponse {
        body: json!({"functionCallId": "c1", "value": 3}).as_object().cloned().unwrap_or_default(),
    })
)]
fn from_json_reads_each_action(#[case] message: Value, #[case] expected: AgentMessage) {
    assert_that!(AgentMessage::from_json(&message))
        .is_ok()
        .is_equal_to(expected);
}
