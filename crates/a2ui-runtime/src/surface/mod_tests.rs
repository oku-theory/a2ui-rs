use serde_json::json;
use speculoos::prelude::*;

use super::*;
use crate::{AgentMessage, MessageLimits};

/// The body of the `createSurface` message `body` reads as.
fn create(body: serde_json::Value) -> CreateSurface {
    let message = json!({"version": "1.0", "createSurface": body});
    match AgentMessage::from_json(&message, &MessageLimits::default()).unwrap() {
        AgentMessage::CreateSurface(create) => create,
        other => panic!("not a createSurface: {other:?}"),
    }
}

#[test]
fn surface_new_empty_catalog_id_is_some_empty() {
    let surface = Surface::new(&create(json!({"surfaceId": "s", "catalogId": ""})));
    assert_that!(surface.catalog_id()).is_equal_to(Some(""));
}

#[test]
fn surface_new_data_model_is_the_message_object() {
    let surface = Surface::new(&create(json!({"surfaceId": "s", "dataModel": {"n": 1}})));
    assert_that!(surface.data_model()).is_equal_to(&DataModel::new(json!({"n": 1})));
}
