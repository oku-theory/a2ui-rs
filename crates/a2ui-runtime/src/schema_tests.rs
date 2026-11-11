use speculoos::prelude::*;

use super::*;

#[test]
fn common_types_schema_embedded_document_is_v1_0() {
    let schema = common_types_schema().unwrap();
    assert_that!(schema["$id"].as_str()).is_equal_to(Some(
        "https://a2ui.org/specification/v1_0/common_types.json",
    ));
}

#[test]
fn agent_to_renderer_schema_embedded_document_is_v1_0() {
    let schema = agent_to_renderer_schema().unwrap();
    assert_that!(schema["$id"].as_str()).is_equal_to(Some(
        "https://a2ui.org/specification/v1_0/agent_to_renderer.json",
    ));
}
