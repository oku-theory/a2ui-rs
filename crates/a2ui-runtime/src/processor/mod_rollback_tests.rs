use rstest::rstest;
use serde_json::{Map, Value, json};
use speculoos::prelude::*;

use super::*;
use crate::processor::test_support::{column, slot_catalogs, text};
use crate::{DataModel, ErrorCode};

const V: &str = "1.0";

/// The message `value` reads as.
fn message(value: Value) -> AgentMessage {
    AgentMessage::from_json(&value, &MessageLimits::default()).unwrap()
}

/// An `updateComponents` sending `components` to `s1`.
fn update(components: Vec<Value>) -> Value {
    json!({"version": V, "updateComponents": {"surfaceId": "s1", "components": components}})
}

/// A processor under `limits` holding `s1` on catalog `t`: `root` lists the `Text` `a`.
fn processor(limits: StateLimits) -> MessageProcessor {
    let mut processor = MessageProcessor::new(slot_catalogs()).with_state_limits(limits);
    processor
        .handle(&message(json!({"version": V, "createSurface": {
            "surfaceId": "s1", "catalogId": "t", "sendDataModel": true,
            "metadata": {"extensions": {}}, "dataModel": {"n": 1},
            "components": [column("root", &["a"]), text("a")]
        }})))
        .unwrap();
    processor
}

/// What a processor holds, as `refused_message_leaves_the_surface_unchanged` compares it.
#[derive(Debug, PartialEq)]
struct Snapshot {
    components: Vec<Map<String, Value>>,
    by_id: Vec<Option<Map<String, Value>>>,
    metadata: Option<Map<String, Value>>,
    data_model: DataModel,
    catalog_id: Option<String>,
    sends_data_model: bool,
    surfaces: Vec<bool>,
}

fn snapshot(processor: &MessageProcessor) -> Snapshot {
    let surface = processor.surface("s1").unwrap();
    Snapshot {
        components: surface.components().cloned().collect(),
        by_id: ["root", "a", "o", "missing"]
            .iter()
            .map(|id| surface.component(id).cloned())
            .collect(),
        metadata: surface.metadata().cloned(),
        data_model: surface.data_model().clone(),
        catalog_id: surface.catalog_id().map(str::to_owned),
        sends_data_model: surface.sends_data_model(),
        surfaces: ["s1", "s2"]
            .iter()
            .map(|id| processor.surface(id).is_some())
            .collect(),
    }
}

/// An update replacing `a` with a chain of 50 `Card`s: the last is 51 levels deep.
fn too_deep() -> Value {
    let mut components: Vec<Value> = (0..50)
        .map(|index| {
            let id = if index == 0 {
                "a".to_owned()
            } else {
                format!("d{index}")
            };
            json!({"id": id, "component": "Card", "child": format!("d{}", index + 1)})
        })
        .collect();
    components.push(text("d50"));
    update(components)
}

#[rstest]
#[case::duplicate_id(
    StateLimits::default(),
    update(vec![text("a"), text("a")]),
    ErrorCode::IntegrityError,
    "Duplicate component ID"
)]
#[case::bad_component(
    StateLimits::default(),
    update(vec![json!({"id": "a", "component": "Text", "text": 5})]),
    ErrorCode::ValidationFailed,
    "is not of type \"string\" at '/text'"
)]
#[case::unknown_catalog(
    StateLimits::default(),
    update(vec![json!({"id": "a", "component": "Text", "catalogId": "nope"})]),
    ErrorCode::CatalogNotFound,
    "Catalog not found: nope"
)]
#[case::cycle(
    StateLimits::default(),
    update(vec![json!({"id": "a", "component": "Card", "child": "root"})]),
    ErrorCode::RecursionLimitExceeded,
    "Circular reference"
)]
#[case::dangling(
    StateLimits::default(),
    update(vec![column("root", &["a", "missing"])]),
    ErrorCode::IntegrityError,
    "Dangling reference"
)]
#[case::orphan(
    StateLimits::default(),
    update(vec![text("o")]),
    ErrorCode::IntegrityError,
    "is not reachable from 'root'"
)]
#[case::self_reference(
    StateLimits::default(),
    update(vec![json!({"id": "a", "component": "Card", "child": "a"})]),
    ErrorCode::RecursionLimitExceeded,
    "Self-reference"
)]
#[case::missing_root(
    StateLimits::default(),
    json!({"version": V, "createSurface": {"surfaceId": "s2", "catalogId": "t",
        "components": [text("a")]}}),
    ErrorCode::ValidationFailed,
    "Missing root component 'root'"
)]
#[case::undeclared_property(
    StateLimits::default(),
    update(vec![json!({"id": "a", "component": "Text", "text": "x", "bogus": 1})]),
    ErrorCode::ValidationFailed,
    "bogus"
)]
#[case::depth(
    StateLimits::default(),
    too_deep(),
    ErrorCode::RecursionLimitExceeded,
    "Global recursion limit exceeded"
)]
#[case::composition(
    StateLimits::default(),
    update(vec![json!({"id": "a", "component": "Header"})]),
    ErrorCode::UnallowedParent,
    "cannot be placed under parent 'root' (Column). Allowed parents: ['Card']."
)]
#[case::state_limit(
    StateLimits::default().with_max_components_per_surface(2),
    update(vec![column("root", &["a", "o"]), text("o")]),
    ErrorCode::ValidationFailed,
    "Components per surface limit of 2 exceeded"
)]
#[case::create_with_a_bad_component(
    StateLimits::default(),
    json!({"version": V, "createSurface": {"surfaceId": "s2", "catalogId": "t",
        "components": [{"id": "root", "component": "Text", "text": 5}]}}),
    ErrorCode::ValidationFailed,
    "is not of type \"string\" at '/text'"
)]
fn refused_message_leaves_the_surface_unchanged(
    #[case] limits: StateLimits,
    #[case] refused: Value,
    #[case] code: ErrorCode,
    #[case] fragment: &str,
) {
    let mut processor = processor(limits);
    let before = snapshot(&processor);

    let error = processor.handle(&message(refused)).unwrap_err();

    assert_that!(error.code()).is_equal_to(code);
    assert_that!(error.to_string()).contains(fragment);
    assert_that!(snapshot(&processor)).is_equal_to(before);
}
