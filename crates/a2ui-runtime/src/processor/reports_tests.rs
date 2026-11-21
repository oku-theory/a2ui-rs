use serde_json::{Value, json};
use speculoos::prelude::*;

use crate::catalog::test_support::{load, v10};
use crate::message::MessageLimits;
use crate::{AgentMessage, CatalogSet, ErrorCode, MessageProcessor};

const V: &str = "1.0";

/// A processor holding one catalog per id of `ids`, inserted in that order.
fn processor(ids: &[&str]) -> MessageProcessor {
    let mut catalogs = CatalogSet::new();
    for id in ids {
        catalogs
            .insert(load(v10(json!({"catalogId": id}))).unwrap())
            .unwrap();
    }
    MessageProcessor::new(catalogs)
}

/// Applies the message `value`.
fn apply(processor: &mut MessageProcessor, value: Value) {
    let message = AgentMessage::from_json(&value, &MessageLimits::default()).unwrap();
    processor.handle(&message).unwrap();
}

/// Creates surface `id` on catalog `c`, sending its data model or not.
fn create(processor: &mut MessageProcessor, id: &str, send: bool) {
    apply(
        processor,
        json!({"version": V, "createSurface":
            {"surfaceId": id, "catalogId": "c", "sendDataModel": send}}),
    );
}

#[test]
fn capabilities_inserted_out_of_order_lists_ids_sorted() {
    let report = processor(&["c", "a", "b"]).capabilities();

    assert_that!(report).is_equal_to(json!({"1.0": {"supportedCatalogIds": ["a", "b", "c"]}}));
}

#[test]
fn capabilities_has_no_inline_key() {
    let report = processor(&["c"]).capabilities();

    assert_that!(report["1.0"].get("inlineCatalogs")).is_none();
}

#[test]
fn capabilities_with_inline_catalogs_is_standalone() {
    let button = json!({"type": "object", "properties": {"label": {"type": "string"}}});
    let document = v10(json!({"catalogId": "cat-v10", "components": {"Button": button}}));
    let mut catalogs = CatalogSet::new();
    catalogs.insert(load(document).unwrap()).unwrap();

    let report = MessageProcessor::new(catalogs).capabilities_with_inline_catalogs();

    assert_that!(report).is_equal_to(json!({"1.0": {
        "supportedCatalogIds": ["cat-v10"],
        "inlineCatalogs": [{
            "catalogId": "cat-v10",
            "protocolVersion": "1.0",
            "components": {"Button": button}
        }]
    }}));
}

#[test]
fn capabilities_with_inline_catalogs_omits_empty_parts() {
    let document = v10(json!({"components": {}, "functions": {}}));
    let mut catalogs = CatalogSet::new();
    catalogs.insert(load(document).unwrap()).unwrap();

    let report = MessageProcessor::new(catalogs).capabilities_with_inline_catalogs();

    assert_that!(report["1.0"]["inlineCatalogs"].clone())
        .is_equal_to(json!([{"catalogId": "c", "protocolVersion": "1.0"}]));
}

#[test]
fn data_model_report_lists_sending_surfaces() {
    let mut processor = processor(&["c"]);
    create(&mut processor, "s", true);
    create(&mut processor, "quiet", false);
    create(&mut processor, "t", true);
    apply(
        &mut processor,
        json!({"version": V, "updateDataModel":
            {"surfaceId": "t", "path": "/a", "value": 1}}),
    );

    let report = processor.data_model_report();

    assert_that!(report).is_equal_to(Some(
        json!({"version": "1.0", "surfaces": {"s": {}, "t": {"a": 1}}}),
    ));
}

#[test]
fn data_model_report_without_sending_surfaces_is_none() {
    let mut processor = processor(&["c"]);
    assert_that!(processor.data_model_report()).is_none();

    create(&mut processor, "quiet", false);

    assert_that!(processor.data_model_report()).is_none();
}

#[test]
fn data_model_report_after_delete_drops_the_surface() {
    let mut processor = processor(&["c"]);
    create(&mut processor, "s", true);
    apply(
        &mut processor,
        json!({"version": V, "deleteSurface": {"surfaceId": "s"}}),
    );

    assert_that!(processor.data_model_report()).is_none();
}

#[test]
fn data_model_report_scalar_root_write_is_refused() {
    let mut processor = processor(&["c"]);
    create(&mut processor, "s", true);
    let message = AgentMessage::from_json(
        &json!({"version": V, "updateDataModel": {"surfaceId": "s", "value": 5}}),
        &MessageLimits::default(),
    )
    .unwrap();

    let error = processor.handle(&message).unwrap_err();

    assert_that!(error.code()).is_equal_to(ErrorCode::ValidationFailed);
    assert_that!(error.to_string()).is_equal_to("The data model root must be an object".to_owned());
    assert_that!(error.surface_id()).is_equal_to(Some("s"));
    assert_that!(processor.data_model_report())
        .is_equal_to(Some(json!({"version": "1.0", "surfaces": {"s": {}}})));
}
