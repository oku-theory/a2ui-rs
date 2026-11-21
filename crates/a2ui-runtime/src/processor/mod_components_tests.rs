use serde_json::{Value, json};
use speculoos::prelude::*;

use super::*;
use crate::catalog::test_support::catalog_set;

const V: &str = "1.0";

/// The message `value` reads as.
fn message(value: Value) -> AgentMessage {
    AgentMessage::from_json(&value, &MessageLimits::default()).unwrap()
}

/// A processor holding catalogs `a` and `b`, with surface `main` on `a`.
fn widgets() -> MessageProcessor {
    let mut processor = MessageProcessor::new(catalog_set());
    processor
        .handle(&message(json!({"version": V,
            "createSurface": {"surfaceId": "main", "catalogId": "a"}})))
        .unwrap();
    processor
}

/// An `updateComponents` sending `components` to `main`.
fn components(components: Value) -> AgentMessage {
    message(json!({"version": V,
        "updateComponents": {"surfaceId": "main", "components": components}}))
}

/// The components `main` stores, in arrival order.
fn stored(processor: &MessageProcessor) -> Vec<Value> {
    let surface = processor.surface("main").unwrap();
    surface.components().cloned().map(Value::Object).collect()
}

#[test]
fn update_replaces_a_component_whole_in_place() {
    let mut processor = widgets();
    let root = json!({"id": "root", "component": "Column", "children": ["t1", "t2"]});
    let middle = json!({"id": "t1", "component": "Text", "text": "x",
        "metadata": {"extensions": {"com_example": 1}}});
    let last = json!({"id": "t2", "component": "Text", "text": "y"});
    processor
        .handle(&components(json!([root, middle, last])))
        .unwrap();
    let replacement = json!({"id": "t1", "component": "Text", "text": "z"});

    processor.handle(&components(json!([replacement]))).unwrap();

    assert_that!(stored(&processor)).is_equal_to(vec![root, replacement, last]);
}

#[test]
fn refused_second_component_stores_nothing() {
    let mut processor = widgets();
    let valid = json!({"id": "t1", "component": "Text", "text": "x"});
    let invalid = json!({"id": "t2", "component": "Text"});

    let error = processor
        .handle(&components(json!([valid, invalid])))
        .unwrap_err();

    assert_that!(error.surface_id()).is_equal_to(Some("main"));
    assert_that!(error.path()).is_equal_to(Some("/components/1/text"));
    assert_that!(error.details()[0].surface_id()).is_equal_to(Some("main"));
    assert_that!(stored(&processor).is_empty()).is_true();
}

#[test]
fn create_surface_components_are_checked_and_stored() {
    let mut processor = MessageProcessor::new(catalog_set());
    let text = json!({"id": "root", "component": "Text", "text": "x"});

    processor
        .handle(&message(json!({"version": V, "createSurface": {
            "surfaceId": "main", "catalogId": "a", "components": [text]}})))
        .unwrap();

    assert_that!(stored(&processor)).is_equal_to(vec![text]);
}

#[test]
fn create_surface_refused_component_creates_nothing() {
    let mut processor = MessageProcessor::new(catalog_set());

    let error = processor
        .handle(&message(json!({"version": V, "createSurface": {
            "surfaceId": "main", "catalogId": "a",
            "components": [{"id": "t1", "component": "Nope"}]}})))
        .unwrap_err();

    assert_that!(error.surface_id()).is_equal_to(Some("main"));
    assert_that!(error.path()).is_equal_to(Some("/components/0/component"));
    assert_that!(processor.surface("main").is_none()).is_true();
}

#[test]
fn handle_with_rules_applies_to_one_call() {
    let mut processor = widgets();
    let unknown = components(json!([{"id": "n1", "component": "Nope"}]));

    let permissive = processor.handle_with(&unknown, ValidationRules::permissive());
    let strict = processor.handle(&unknown);

    assert_that!(permissive).is_ok();
    assert_that!(strict.unwrap_err().path()).is_equal_to(Some("/components/0/component"));
}

#[test]
fn handle_batch_with_rules_applies_to_each_message() {
    let mut processor = MessageProcessor::new(catalog_set());
    let batch = json!([
        {"version": V, "createSurface": {"surfaceId": "main", "catalogId": "a"}},
        {"version": V, "updateComponents": {"surfaceId": "main",
            "components": [{"id": "n1", "component": "Nope"}]}},
    ]);

    let outcome = processor.handle_batch_with(&batch, ValidationRules::permissive());

    assert_that!(outcome).is_ok();
    assert_that!(stored(&processor)).has_length(1);
}

#[test]
fn permissive_rules_accept_an_unknown_component_type() {
    let mut processor = widgets().with_rules(ValidationRules::permissive());
    let unknown = json!({"id": "n1", "component": "Nope", "x": {"@call": "nope"}});

    processor.handle(&components(json!([unknown]))).unwrap();

    assert_that!(stored(&processor)).is_equal_to(vec![unknown]);
}

#[test]
fn refused_replacement_keeps_the_stored_components_and_order() {
    let mut processor = widgets();
    let root = json!({"id": "root", "component": "Column", "children": ["t1"]});
    let text = json!({"id": "t1", "component": "Text", "text": "x"});
    processor.handle(&components(json!([root, text]))).unwrap();
    let replaced = json!({"id": "t1", "component": "Text", "text": "y"});
    let invalid = json!({"id": "t2", "component": "Text"});

    let refused = processor.handle(&components(json!([replaced, invalid])));

    assert_that!(refused).is_err();
    assert_that!(stored(&processor)).is_equal_to(vec![root, text]);
}

#[test]
fn batch_component_error_keeps_its_pointer_detail() {
    let mut processor = MessageProcessor::new(catalog_set());
    let batch = json!([
        {"version": V, "createSurface": {"surfaceId": "main", "catalogId": "a"}},
        {"version": V, "updateComponents": {"surfaceId": "main",
            "components": [{"id": "t1", "component": "Text", "text": 5}]}},
    ]);

    let error = processor.handle_batch(&batch).unwrap_err();

    assert_that!(error.message_index()).is_equal_to(Some(1));
    assert_that!(error.path()).is_equal_to(Some("/components/0/text"));
    let paths: Vec<&str> = error.details().iter().map(|detail| detail.path()).collect();
    assert_that!(paths).is_equal_to(vec!["/text"]);
}
