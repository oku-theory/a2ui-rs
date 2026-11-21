use serde_json::{Value, json};
use speculoos::prelude::*;

use super::*;
use crate::{AgentMessage, MessageLimits};

/// The error reading `message` raises.
fn fault(message: &Value) -> Error {
    AgentMessage::from_json(message, &MessageLimits::default()).expect_err("an invalid message")
}

/// A value of `levels` nested objects, the innermost empty.
fn nested(levels: usize) -> Value {
    let mut value = json!({});
    for _ in 1..levels {
        value = json!({"a": value});
    }
    value
}

#[test]
fn validation_error_without_surface_writes_empty_ids() {
    let error = fault(&json!({"version": "1.0", "deleteSurface": {}}));

    assert_that!(RendererMessage::error(&error).to_json()).is_equal_to(json!({
        "version": "1.0",
        "error": {
            "code": "VALIDATION_FAILED",
            "message": error.to_string(),
            "surfaceId": "",
            "path": "/surfaceId",
        },
    }));
}

#[test]
fn validation_error_with_surface_and_path() {
    let error = fault(&json!({"version": "1.0",
        "updateDataModel": {"surfaceId": "main", "path": "not_a_pointer", "value": 1}}));

    assert_that!(RendererMessage::error(&error).to_json()).is_equal_to(json!({
        "version": "1.0",
        "error": {
            "code": "VALIDATION_FAILED",
            "message": error.to_string(),
            "surfaceId": "main",
            "path": "/path",
        },
    }));
}

#[test]
fn envelope_error_writes_empty_path() {
    let error = fault(&json!({"createSurface": {"surfaceId": "s1"}}));

    assert_that!(RendererMessage::error(&error).to_json()).is_equal_to(json!({
        "version": "1.0",
        "error": {
            "code": "VALIDATION_FAILED",
            "message": error.to_string(),
            "surfaceId": "",
            "path": "",
        },
    }));
}

#[test]
fn function_call_error_writes_only_function_call_id() {
    let error = fault(&json!({"version": "1.0",
        "callRendererFunction": {"functionCallId": "call-err-1"}}));

    assert_that!(RendererMessage::error(&error).to_json()).is_equal_to(json!({
        "version": "1.0",
        "error": {
            "code": "INVALID_FUNCTION_CALL",
            "message": error.to_string(),
            "functionCallId": "call-err-1",
        },
    }));
}

#[test]
fn data_error_writes_only_surface_id() {
    let error = fault(&json!({"version": "1.0",
        "updateDataModel": {"surfaceId": "main", "path": "/__proto__/x", "value": 1}}));

    assert_that!(RendererMessage::error(&error).to_json()).is_equal_to(json!({
        "version": "1.0",
        "error": {
            "code": "DATA_ERROR",
            "message": error.to_string(),
            "surfaceId": "main",
        },
    }));
}

#[test]
fn recursion_error_writes_only_surface_id() {
    let error = fault(&json!({"version": "1.0",
        "createSurface": {"surfaceId": "main", "dataModel": nested(51)}}));

    assert_that!(RendererMessage::error(&error).to_json()).is_equal_to(json!({
        "version": "1.0",
        "error": {
            "code": "RECURSION_LIMIT_EXCEEDED",
            "message": error.to_string(),
            "surfaceId": "main",
        },
    }));
}

/// The error `MessageProcessor` raises on the last of `messages`, with no catalog loaded.
fn processed(messages: &[Value]) -> Error {
    let mut processor = crate::MessageProcessor::new(crate::CatalogSet::new());
    let limits = MessageLimits::default();
    let (last, first) = messages.split_last().expect("one message at least");
    for message in first {
        processor
            .handle(&AgentMessage::from_json(message, &limits).unwrap())
            .unwrap();
    }
    processor
        .handle(&AgentMessage::from_json(last, &limits).unwrap())
        .expect_err("a refused message")
}

#[test]
fn integrity_error_writes_only_surface_id() {
    let create = json!({"version": "1.0", "createSurface": {"surfaceId": "main"}});
    let error = processed(&[create.clone(), create]);

    assert_that!(RendererMessage::error(&error).to_json()).is_equal_to(json!({
        "version": "1.0",
        "error": {
            "code": "INTEGRITY_ERROR",
            "message": "Surface 'main' already exists",
            "surfaceId": "main",
        },
    }));
}

#[test]
fn catalog_not_found_writes_only_surface_id() {
    let error = processed(&[json!({"version": "1.0",
        "createSurface": {"surfaceId": "main", "catalogId": "missing-catalog"}})]);

    assert_that!(RendererMessage::error(&error).to_json()).is_equal_to(json!({
        "version": "1.0",
        "error": {
            "code": "CATALOG_NOT_FOUND",
            "message": "Catalog not found: missing-catalog",
            "surfaceId": "main",
        },
    }));
}

#[test]
fn unallowed_parent_writes_the_validation_shape() {
    let catalog = crate::Catalog::from_json(
        &json!({"catalogId": "custom", "protocolVersion": "1.0", "components": {
            "ColumnHeader": {"type": "object", "allowedParents": ["Column"]}}}),
        None,
        None,
    )
    .unwrap();
    let mut catalogs = crate::CatalogSet::new();
    catalogs.insert(catalog).unwrap();
    let mut processor = crate::MessageProcessor::new(catalogs);
    let create = json!({"version": "1.0", "createSurface": {"surfaceId": "s1",
        "catalogId": "custom", "components": [{"id": "root", "component": "ColumnHeader"}]}});
    let message = AgentMessage::from_json(&create, &MessageLimits::default()).unwrap();

    let error = processor.handle(&message).unwrap_err();

    assert_that!(RendererMessage::error(&error).to_json()).is_equal_to(json!({
        "version": "1.0",
        "error": {
            "code": "UNALLOWED_PARENT",
            "message": "Component 'root' (ColumnHeader) cannot be placed under parent 'Surface' \
                        (Surface). Allowed parents: ['Column'].",
            "surfaceId": "s1",
            "path": "/components/0",
        },
    }));
}
