use rstest::rstest;
use serde_json::{Map, Value, json};
use speculoos::prelude::*;

use super::*;
use crate::Category;

#[rstest]
#[case("#/$defs/Weight", true)]
#[case("#", true)]
#[case("catalog.json#/$defs/anyFunction", true)]
#[case("common_types.json#/$defs/DynamicString", true)]
#[case("./common_types.json#/$defs/DynamicString", true)]
#[case(
    "https://a2ui.org/specification/v1_0/common_types.json#/$defs/DynamicString",
    true
)]
#[case("common_types.json", false)]
#[case("evil/common_types.json#/$defs/X", false)]
#[case("https://evil.example/common_types.json#/$defs/X", false)]
#[case("basic_catalog.json#/components/Text", false)]
#[case(
    "https://a2ui.org/specification/v1_0/catalogs/basic/catalog.json#/components/Button",
    false
)]
fn is_supported_ref_v1_0_targets_are_classified(#[case] reference: &str, #[case] expected: bool) {
    assert_that!(is_supported_ref(reference)).is_equal_to(expected);
}

#[rstest]
#[case("/$defs/Child", "#/$defs/Child")]
#[case("/components/Component_ℓ", "#/components/Component_%E2%84%93")]
#[case("/$defs/a b", "#/$defs/a%20b")]
#[case("/$defs/a~1b", "#/$defs/a~1b")]
fn local_ref_pointer_is_percent_encoded(#[case] pointer: &str, #[case] expected: &str) {
    assert_that!(local_ref(pointer)).is_equal_to(expected.to_owned());
}

#[rstest]
#[case("common_types.json#/$defs/Child", "#/$defs/Child")]
#[case(
    "https://a2ui.org/specification/v1_0/common_types.json#/$defs/Child",
    "#/$defs/Child"
)]
#[case("catalog.json#/$defs/anyFunction", "#/$defs/anyFunction")]
#[case("#/components/Text", "#/components/Text")]
#[case("#/components/Component_ℓ", "#/components/Component_%E2%84%93")]
#[case("#/components/Component_%E2%84%93", "#/components/Component_%E2%84%93")]
#[case("other.json#/x", "other.json#/x")]
fn localize_ref_supported_targets_become_local(#[case] reference: &str, #[case] expected: &str) {
    assert_that!(localize_ref(reference)).is_equal_to(expected.to_owned());
}

#[test]
fn localize_refs_rewrites_every_ref_string() {
    let mut node = json!({
        "a": {"$ref": "common_types.json#/$defs/X"},
        "b": [{"$ref": "catalog.json#/c"}]
    });
    localize_refs(&mut node);
    assert_that!(node).is_equal_to(json!({"a": {"$ref": "#/$defs/X"}, "b": [{"$ref": "#/c"}]}));
}

#[test]
fn localize_refs_property_named_ref_holding_a_schema_is_walked() {
    let mut node = json!({"properties": {"$ref": {"$ref": "common_types.json#/$defs/X"}}});
    localize_refs(&mut node);
    assert_that!(node).is_equal_to(json!({"properties": {"$ref": {"$ref": "#/$defs/X"}}}));
}

#[rstest]
#[case("#/$defs/Child", Some("/$defs/Child"))]
#[case("common_types.json#/$defs/Child", Some("/$defs/Child"))]
#[case("#/components/Component_%E2%84%93", Some("/components/Component_ℓ"))]
#[case("#", Some(""))]
#[case("catalog.json#", Some(""))]
#[case("other.json#/x", None)]
#[case("#A", None)]
#[case("#/components/%FF", None)]
fn pointer_of_supported_refs_are_decoded(#[case] reference: &str, #[case] expected: Option<&str>) {
    assert_that!(pointer_of(reference).as_deref()).is_equal_to(expected);
}

#[test]
fn local_ref_percent_sign_is_encoded() {
    assert_that!(local_ref("/$defs/a%b")).is_equal_to("#/$defs/a%25b".to_owned());
}

#[test]
fn pointer_of_encoded_percent_sign_is_decoded() {
    assert_that!(pointer_of("#/$defs/a%25b").as_deref()).is_equal_to(Some("/$defs/a%b"));
}

#[rstest]
#[case("/$defs/Child", Some(("$defs", "Child")))]
#[case("/components/Card/properties/child", Some(("components", "Card")))]
#[case("/functions/a~1b", Some(("functions", "a/b")))]
#[case("", None)]
#[case("/theme/x", None)]
#[case("/$defs", None)]
fn entry_of_pointer_names_its_entry(#[case] pointer: &str, #[case] expected: Option<(&str, &str)>) {
    let entry = entry_of(pointer);
    let entry = entry
        .as_ref()
        .map(|(section, name)| (*section, name.as_str()));
    assert_that!(entry).is_equal_to(expected);
}

#[test]
fn resolve_percent_encoded_ref_finds_the_entry() {
    let root = json!({"components": {"Component_ℓ": {"type": "object"}}});
    let found = resolve(&root, "#/components/Component_%E2%84%93");
    assert_that!(found).is_equal_to(Some(&json!({"type": "object"})));
}

fn references(document: Value) -> Result<(), crate::Error> {
    check_references(document.as_object().unwrap())
}

fn reference_error(document: Value) -> String {
    let err = references(document).unwrap_err();
    assert_that!(err.category()).is_equal_to(Category::Catalog);
    err.to_string()
}

#[test]
fn check_references_supported_refs_pass() {
    let document = json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": "https://example.com/c.json",
        "components": {"A": {"allOf": [{"$ref": "common_types.json#/$defs/Checkable"}]}},
        "$defs": {"whole": {"$ref": "catalog.json#"}, "local": {"$ref": "#/$defs/whole"}}
    });
    assert_that!(references(document)).is_ok();
}

#[test]
fn check_references_properties_named_like_keywords_pass() {
    let document = json!({"$defs": {"x": {"properties": {
        "$ref": {"type": "string"},
        "$anchor": {"type": "string"},
        "$id": {"type": "string"}
    }}}});
    assert_that!(references(document)).is_ok();
}

#[rstest]
#[case(
    json!({"components": {"A": {"allOf": [{"$ref": "x.json#/a"}]}}}),
    "Unsupported external $ref 'x.json#/a' at /components/A/allOf/0/$ref"
)]
#[case(json!({"$defs": {"a/b": {"$ref": "x.json#/a"}}}), "at /$defs/a~1b/$ref")]
#[case(
    json!({"components": {"A": {"$id": "https://example.com/a"}}}),
    "Unsupported '$id' at /components/A/$id: only the catalog's top level may declare it"
)]
#[case(
    json!({"components": {"A": {"$schema": "https://json-schema.org/draft/2020-12/schema"}}}),
    "Unsupported '$schema' at /components/A/$schema"
)]
#[case(
    json!({"$defs": {
        "x": {"$anchor": "A"},
        "y": {"allOf": [{"$ref": "#A"}, {"$ref": "#A"}]}
    }}),
    "Unsupported '$anchor' at /$defs/x/$anchor: anchors are not supported; use a JSON Pointer"
)]
#[case(
    json!({"$defs": {"x": {"$dynamicAnchor": "meta"}}}),
    "Unsupported '$dynamicAnchor' at /$defs/x/$dynamicAnchor"
)]
#[case(
    json!({"$defs": {"x": {"$recursiveAnchor": true}}}),
    "Unsupported '$recursiveAnchor' at /$defs/x/$recursiveAnchor"
)]
#[case(
    json!({"$defs": {"x": {"$dynamicRef": "#meta"}}}),
    "Unsupported '$dynamicRef' at /$defs/x/$dynamicRef: dynamic references are not supported"
)]
#[case(
    json!({"$defs": {"x": {"$recursiveRef": "#"}}}),
    "Unsupported '$recursiveRef' at /$defs/x/$recursiveRef"
)]
#[case(
    json!({"$defs": {"x": {"$ref": "#A"}}}),
    "$ref '#A' at /$defs/x/$ref has a fragment that is neither empty nor a JSON Pointer"
)]
#[case(
    json!({"$defs": {"x": {"$ref": "#/components/%FF"}}}),
    "$ref '#/components/%FF' at /$defs/x/$ref has a fragment that is not UTF-8"
)]
#[case(
    json!({"$defs": {"x": {"$ref": 5}}}),
    "Malformed catalog: $ref at /$defs/x/$ref must be a string"
)]
fn check_references_unsupported_reference_is_catalog_error(
    #[case] document: Value,
    #[case] fragment: &str,
) {
    assert_that!(reference_error(document)).contains(fragment);
}

#[test]
fn check_references_property_named_like_a_keyword_is_walked() {
    let document = json!({"$defs": {"x": {"properties": {"$ref": {"$ref": "x.json#/a"}}}}});
    assert_that!(reference_error(document)).contains("at /$defs/x/properties/$ref/$ref");
}

#[test]
fn add_common_types_copies_the_reachable_closure() {
    let common = json!({
        "A": {"$ref": "#/$defs/B"},
        "B": {"type": "string"},
        "C": {"type": "number"}
    });
    let mut defs = Map::new();
    add_common_types(
        &mut defs,
        &[&json!({"$ref": "#/$defs/A"})],
        common.as_object().unwrap(),
    );
    let names: Vec<&str> = defs.keys().map(String::as_str).collect();
    assert_that!(names).is_equal_to(vec!["A", "B"]);
}

#[test]
fn add_common_types_cycle_between_common_types_terminates() {
    let common = json!({"A": {"$ref": "#/$defs/B"}, "B": {"$ref": "#/$defs/A"}});
    let mut defs = Map::new();
    add_common_types(
        &mut defs,
        &[&json!({"$ref": "#/$defs/A"})],
        common.as_object().unwrap(),
    );
    let names: Vec<&str> = defs.keys().map(String::as_str).collect();
    assert_that!(names).is_equal_to(vec!["A", "B"]);
}

#[test]
fn add_common_types_definition_already_present_is_kept() {
    let common = json!({"A": {"type": "string"}});
    let mut defs = Map::from_iter([("A".to_owned(), json!({"type": "number"}))]);
    add_common_types(
        &mut defs,
        &[&json!({"$ref": "#/$defs/A"})],
        common.as_object().unwrap(),
    );
    assert_that!(defs["A"].clone()).is_equal_to(json!({"type": "number"}));
}

#[test]
fn add_common_types_encoded_and_escaped_names_are_decoded() {
    let common = json!({"a/ℓ": {"type": "string"}});
    let mut defs = Map::new();
    let root = json!({"$ref": "#/$defs/a~1%E2%84%93"});
    add_common_types(&mut defs, &[&root], common.as_object().unwrap());
    assert_that!(defs.contains_key("a/ℓ")).is_true();
}

#[test]
fn add_common_types_copied_refs_are_localized() {
    let common = json!({"A": {"$ref": "common_types.json#/$defs/B"}, "B": {}});
    let mut defs = Map::new();
    add_common_types(
        &mut defs,
        &[&json!({"$ref": "#/$defs/A"})],
        common.as_object().unwrap(),
    );
    assert_that!(defs["A"].clone()).is_equal_to(json!({"$ref": "#/$defs/B"}));
}

#[test]
fn function_call_envelope_closes_the_call_over_function_common() {
    assert_that!(function_call_envelope()).is_equal_to(json!({
        "type": "object",
        "allOf": [{"$ref": "#/$defs/FunctionCommon"}],
        "properties": {"args": {"type": "object"}},
        "unevaluatedProperties": false
    }));
}

#[rstest]
#[case::entry("#/$defs/Weight", Some("Weight"))]
#[case::escaped("#/$defs/a~1b~0c", Some("a/b~c"))]
#[case::percent_encoded("#/$defs/%C2%B5", Some("µ"))]
#[case::below_the_entry("#/$defs/Weight/description", None)]
#[case::other_section("#/components/Weight", None)]
#[case::section_only("#/$defs", None)]
fn whole_def_name_names_only_a_whole_entry(#[case] reference: &str, #[case] name: Option<&str>) {
    assert_that!(whole_def_name(reference)).is_equal_to(name.map(str::to_owned));
}
