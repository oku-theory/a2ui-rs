use rstest::rstest;
use serde_json::{Map, Value, json};
use speculoos::prelude::*;

use super::super::test_support::{catalog_error, load, v10};
use crate::Catalog;
use crate::limits::{
    MAX_CATALOG_BYTES, MAX_CATALOG_ENTRIES, MAX_CATALOG_NODES, MAX_JSON_DEPTH, MAX_KEY_BYTES,
    MAX_REF_BYTES,
};

/// A v1.0 catalog nesting exactly `levels` levels of objects (`levels` at least 2).
fn nested_document(levels: usize) -> Value {
    let deep = (2..levels).fold(
        json!(true),
        |inner, _| json!({"type": "object", "x": inner}),
    );
    v10(json!({"$defs": {"deep": deep}}))
}

/// A v1.0 catalog of `count` components.
fn components(count: usize) -> Value {
    let components: Map<String, Value> = (0..count)
        .map(|index| (format!("C{index}"), json!({"type": "object"})))
        .collect();
    v10(json!({"components": components}))
}

#[test]
fn from_json_array_document_is_catalog_error() {
    let message = catalog_error(load(json!([])));
    let expected = "Malformed catalog: a catalog document is a JSON object";
    assert_that!(message).is_equal_to(expected.to_owned());
}

#[test]
fn from_json_document_at_the_depth_cap_is_accepted() {
    assert_that!(load(nested_document(MAX_JSON_DEPTH))).is_ok();
}

#[test]
fn from_json_document_past_the_depth_cap_is_catalog_error() {
    let message = catalog_error(load(nested_document(MAX_JSON_DEPTH + 1)));
    assert_that!(message).is_equal_to("Catalog exceeds 64 levels of nesting".to_owned());
}

#[test]
fn from_json_document_past_the_node_cap_is_catalog_error() {
    let document = v10(json!({"$defs": {"big": {"enum": vec![0; MAX_CATALOG_NODES]}}}));
    let message = catalog_error(load(document));
    assert_that!(message).is_equal_to("Catalog exceeds 200000 JSON nodes".to_owned());
}

#[test]
fn from_json_long_object_key_is_catalog_error() {
    let document = v10(json!({"$defs": {"k".repeat(MAX_KEY_BYTES + 1): {}}}));
    let message = catalog_error(load(document));
    assert_that!(message).is_equal_to("Catalog exceeds 256 bytes in an object key".to_owned());
}

#[test]
fn from_json_long_ref_is_catalog_error() {
    let reference = format!("#/$defs/{}", "r".repeat(MAX_REF_BYTES));
    let document = v10(json!({"components": {"A": {"$ref": reference}}}));
    let message = catalog_error(load(document));
    assert_that!(message).is_equal_to("Catalog exceeds 4096 bytes in a $ref".to_owned());
}

#[test]
fn from_json_missing_catalog_id_is_catalog_error() {
    let message = catalog_error(load(json!({"protocolVersion": "1.0"})));
    assert_that!(message)
        .is_equal_to("catalog_id must be provided or exist in catalog schema".to_owned());
}

#[rstest]
#[case(json!({"catalogId": "", "protocolVersion": "1.0"}), None)]
#[case(json!({"catalogId": "c", "protocolVersion": "1.0"}), Some(""))]
fn from_json_empty_catalog_id_is_catalog_error(
    #[case] document: Value,
    #[case] catalog_id: Option<&str>,
) {
    let message = catalog_error(Catalog::from_json(&document, catalog_id, None));
    assert_that!(message).contains("catalog_id must be provided");
}

#[test]
fn from_json_non_string_catalog_id_is_catalog_error() {
    let message = catalog_error(load(json!({"catalogId": 7, "protocolVersion": "1.0"})));
    assert_that!(message).contains("`catalogId` must be a string");
}

#[test]
fn from_json_catalog_id_argument_overrides_the_document() {
    let catalog = Catalog::from_json(&v10(json!({})), Some("other"), None).unwrap();
    assert_that!(catalog.id()).is_equal_to("other");
}

#[rstest]
#[case(json!({"catalogId": "c"}), "'0.9'")]
#[case(json!({"catalogId": "c", "protocolVersion": "v0.9"}), "'v0.9'")]
#[case(json!({"catalogId": "c", "protocolVersion": "1.1"}), "'1.1'")]
#[case(json!({"catalogId": "c", "protocolVersion": "1.0."}), "'1.0.'")]
fn from_json_non_v1_0_version_is_catalog_error(#[case] document: Value, #[case] version: &str) {
    assert_that!(catalog_error(load(document))).contains(version);
}

#[test]
fn from_json_non_string_version_is_catalog_error() {
    let message = catalog_error(load(json!({"catalogId": "c", "protocolVersion": 1.0})));
    assert_that!(message).contains("`protocolVersion` must be a string");
}

#[test]
fn from_json_patch_version_is_accepted() {
    let catalog = load(json!({"catalogId": "c", "protocolVersion": "1.0.2"})).unwrap();
    assert_that!(catalog.protocol_version()).is_equal_to("1.0.2");
}

#[test]
fn from_json_version_argument_overrides_the_document() {
    let catalog = Catalog::from_json(&json!({"catalogId": "c"}), None, Some("1.0")).unwrap();
    assert_that!(catalog.protocol_version()).is_equal_to("1.0");
}

#[test]
fn from_json_external_ref_is_catalog_error() {
    let document = v10(json!({"components": {"A": {"allOf": [
        {"$ref": "basic_catalog.json#/components/Text"}
    ]}}}));
    assert_that!(catalog_error(load(document))).contains("Unsupported external $ref");
}

#[test]
fn from_json_defs_shadowing_a_common_type_is_catalog_error() {
    let document = v10(json!({"$defs": {"DynamicString": {"type": "string"}}}));
    assert_that!(catalog_error(load(document))).contains("'DynamicString'");
}

#[rstest]
#[case(json!({"components": []}), "`components` must be an object")]
#[case(json!({"functions": "f"}), "`functions` must be an object")]
#[case(json!({"$defs": 1}), "`$defs` must be an object")]
fn from_json_non_object_section_is_catalog_error(#[case] extra: Value, #[case] fragment: &str) {
    assert_that!(catalog_error(load(v10(extra)))).contains(fragment);
}

#[test]
fn from_json_entries_at_the_cap_are_accepted() {
    assert_that!(load(components(MAX_CATALOG_ENTRIES))).is_ok();
}

#[test]
fn from_json_entries_past_the_cap_are_catalog_error() {
    let message = catalog_error(load(components(MAX_CATALOG_ENTRIES + 1)));
    assert_that!(message)
        .is_equal_to("Catalog exceeds 1024 components and functions entries".to_owned());
}

#[test]
fn parse_invalid_json_is_catalog_error() {
    assert_that!(catalog_error(Catalog::parse("{", None, None))).contains("not valid JSON");
}

#[test]
fn parse_text_past_the_size_cap_is_catalog_error() {
    let text = format!(
        "{{\"catalogId\": \"c\", \"pad\": \"{}\"}}",
        "x".repeat(MAX_CATALOG_BYTES)
    );
    let message = catalog_error(Catalog::parse(&text, None, None));
    assert_that!(message).is_equal_to("Catalog exceeds 4194304 bytes of catalog text".to_owned());
}

#[test]
fn to_json_returns_the_authored_document() {
    let document = v10(json!({
        "$id": "https://example.com/c.json",
        "instructions": "Use it.",
        "components": {"Label": {"allOf": [{"$ref": "common_types.json#/$defs/Checkable"}]}}
    }));
    assert_that!(load(document.clone()).unwrap().to_json()).is_equal_to(document);
}

#[test]
fn to_json_entries_left_out_of_unions_are_dropped() {
    let document = v10(json!({
        "components": {"A": {}, "B": {}},
        "$defs": {"anyComponent": {"oneOf": [{"$ref": "#/components/A"}]}}
    }));
    let json = load(document).unwrap().to_json();
    assert_that!(json["components"].clone()).is_equal_to(json!({"A": {}}));
}

#[test]
fn to_json_overridden_id_replaces_the_declared_one() {
    let catalog = Catalog::from_json(&v10(json!({})), Some("other"), None).unwrap();
    assert_that!(catalog.to_json()["catalogId"].clone()).is_equal_to(json!("other"));
}

#[test]
fn to_json_overridden_id_is_written_when_the_document_has_none() {
    let document = json!({"protocolVersion": "1.0"});
    let catalog = Catalog::from_json(&document, Some("given"), None).unwrap();
    assert_that!(catalog.to_json()["catalogId"].clone()).is_equal_to(json!("given"));
}

#[test]
fn to_json_overridden_version_reloads_without_the_override() {
    let catalog = Catalog::from_json(&json!({"catalogId": "c"}), None, Some("1.0")).unwrap();
    let reloaded = Catalog::from_json(&catalog.to_json(), None, None).unwrap();
    assert_that!(reloaded.protocol_version()).is_equal_to("1.0");
}

#[test]
fn accessors_theme_is_read_from_the_document() {
    let catalog = load(v10(json!({"theme": {"type": "object"}}))).unwrap();
    assert_that!(catalog.theme()).is_equal_to(Some(&json!({"type": "object"})));
}

#[test]
fn accessors_instructions_are_read_from_the_document() {
    let catalog = load(v10(json!({"instructions": "Be brief."}))).unwrap();
    assert_that!(catalog.instructions()).is_equal_to(Some("Be brief."));
}
