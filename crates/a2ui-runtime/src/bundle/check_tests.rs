use rstest::rstest;
use serde_json::{Value, json};
use speculoos::prelude::*;

use super::*;
use crate::Category;

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
        "$defs": {
            "whole": {"$ref": "catalog.json#/$defs/local"},
            "local": {"$ref": "#/$defs/whole"}
        }
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

#[rstest]
#[case("#/components/A/allOf/1/properties/x")]
#[case("#/components/A/properties")]
#[case("catalog.json#/components/A/properties/x")]
#[case("#/functions/f/properties/args/additionalProperties")]
#[case("#/functions/Func_%E2%84%93/properties")]
#[case("#/components/A%2Fx")]
#[case("#/components//x")]
fn check_references_pointer_inside_a_component_or_function_is_catalog_error(
    #[case] reference: &str,
) {
    let document = json!({"$defs": {"x": {"$ref": reference}}});
    assert_that!(reference_error(document)).contains(format!(
        "$ref '{reference}' at /$defs/x/$ref points inside a component or function"
    ));
}

#[rstest]
#[case("#/components/A")]
#[case("#/components/A~1B")]
#[case("#/functions/Func_%E2%84%93")]
fn check_references_whole_component_or_function_passes(#[case] reference: &str) {
    let document = json!({"$defs": {"x": {"$ref": reference}}});
    assert_that!(references(document)).is_ok();
}

#[rstest]
#[case("#/$defs/A/properties/x")]
#[case("common_types.json#/$defs/DynamicString/oneOf/0")]
#[case("#/theme/properties/brand")]
fn check_references_pointer_inside_defs_or_theme_passes(#[case] reference: &str) {
    let document = json!({"$defs": {"x": {"$ref": reference}}});
    assert_that!(references(document)).is_ok();
}

#[rstest]
#[case::bare("#")]
#[case::bare_catalog("catalog.json#")]
#[case::bare_common_types("common_types.json#")]
#[case::title("#/title")]
#[case::vendor_key("#/x-shared/Brand")]
#[case::catalog_id("catalog.json#/catalogId")]
#[case::defs_section("#/$defs")]
#[case::components_section("#/components")]
#[case::functions_section("#/functions")]
#[case::common_types_entry_section("common_types.json#/components/A")]
#[case::common_types_theme("common_types.json#/theme")]
#[case::common_types_defs_section("common_types.json#/$defs")]
#[case::longer_than_theme("#/themes")]
#[case::escaped_theme_key("#/theme~1x")]
fn check_references_target_the_validation_schema_drops_is_catalog_error(#[case] reference: &str) {
    let document = json!({"$defs": {"x": {"$ref": reference}}});
    assert_that!(reference_error(document)).contains(format!(
        "$ref '{reference}' at /$defs/x/$ref names no definition, entry or theme"
    ));
}

#[rstest]
#[case::theme("#/theme")]
#[case::catalog_theme("catalog.json#/theme")]
#[case::def("catalog.json#/$defs/A")]
#[case::def_named_empty("#/$defs/")]
#[case::component_named_empty("#/components/")]
#[case::absolute_common_type("https://a2ui.org/specification/v1_0/common_types.json#/$defs/Weight")]
fn check_references_target_the_validation_schema_keeps_passes(#[case] reference: &str) {
    let document = json!({"$defs": {"x": {"$ref": reference}}});
    assert_that!(references(document)).is_ok();
}

#[test]
fn check_references_property_named_like_a_keyword_is_walked() {
    let document = json!({"$defs": {"x": {"properties": {"$ref": {"$ref": "x.json#/a"}}}}});
    assert_that!(reference_error(document)).contains("at /$defs/x/properties/$ref/$ref");
}

#[test]
fn check_references_two_faults_reports_the_first_in_walk_order() {
    let document = json!({"$defs": {"a": {"$ref": 5}, "b": {"$ref": "x.json#/a"}}});
    assert_that!(reference_error(document)).contains("$ref at /$defs/a/$ref must be a string");
}

#[test]
fn check_references_array_under_ref_key_is_walked() {
    let document = json!({"$defs": {"x": {"$ref": [{"$ref": "x.json#/a"}]}}});
    assert_that!(reference_error(document)).contains("at /$defs/x/$ref/0/$ref");
}

#[rstest]
#[case::default_data("default", json!({"$ref": "other.json#/a"}), "/default/$ref")]
#[case::examples_data("examples", json!([{"$ref": "other.json#/a"}]), "/examples/0/$ref")]
#[case::const_data("const", json!({"$ref": "other.json#/a"}), "/const/$ref")]
#[case::enum_data("enum", json!([{"$ref": "other.json#/a"}]), "/enum/0/$ref")]
fn check_references_unsupported_ref_in_data_is_refused(
    #[case] keyword: &str,
    #[case] data: Value,
    #[case] below: &str,
) {
    let document = json!({"$defs": {"X": {keyword: data}}});
    let expected = format!("Unsupported external $ref 'other.json#/a' at /$defs/X{below}");
    assert_that!(reference_error(document)).contains(expected);
}

#[rstest]
#[case::unknown_escape("#/$defs/a~2b")]
#[case::trailing_tilde("catalog.json#/$defs/a~")]
#[case::section_token("#/$def~s/A")]
#[case::below_the_entry("#/$defs/A/properties/x~2")]
#[case::percent_encoded_tilde("#/$defs/a%7E2b")]
fn check_references_pointer_with_a_bad_escape_is_catalog_error(#[case] reference: &str) {
    let document = json!({"$defs": {"x": {"$ref": reference}}});
    assert_that!(reference_error(document)).contains(format!(
        "$ref '{reference}' at /$defs/x/$ref has a JSON Pointer where '~' must be followed by \
         '0' or '1'"
    ));
}

#[test]
fn check_references_percent_encoded_escape_digit_is_decoded_first() {
    let document = json!({"$defs": {"a/b": {}, "x": {"$ref": "#/$defs/a~%31b"}}});
    assert_that!(references(document)).is_ok();
}

#[test]
fn check_references_pointer_with_both_escapes_passes() {
    let document = json!({"$defs": {"a/b~c": {}, "x": {"$ref": "#/$defs/a~1b~0c"}}});
    assert_that!(references(document)).is_ok();
}

#[rstest]
#[case::const_catalog_ref("const", json!({"$ref": "catalog.json#/$defs/X"}), "/const/$ref")]
#[case::enum_common_type(
    "enum",
    json!([{"$ref": "common_types.json#/$defs/DynamicString"}]),
    "/enum/0/$ref"
)]
#[case::default_theme_ref("default", json!({"$ref": "#/theme"}), "/default/$ref")]
#[case::examples_unencoded(
    "examples",
    json!([{"a": {"$ref": "#/$defs/µ"}}]),
    "/examples/0/a/$ref"
)]
fn check_references_data_ref_the_rewrite_would_change_is_catalog_error(
    #[case] keyword: &str,
    #[case] data: Value,
    #[case] below: &str,
) {
    let document = json!({"theme": {}, "$defs": {"X": {"properties": {"p": {keyword: data}}}}});
    assert_that!(reference_error(document)).contains(format!(
        "at /$defs/X/properties/p{below} is data, which the runtime would rewrite"
    ));
}

#[test]
fn check_references_data_ref_names_the_form_to_write() {
    let document = json!({"theme": {}, "$defs": {"X": {"const": {"$ref": "catalog.json#/theme"}}}});
    assert_that!(reference_error(document)).ends_with(
        "Malformed catalog: $ref 'catalog.json#/theme' at /$defs/X/const/$ref is data, which \
         the runtime would rewrite; write it '#/$defs/theme'",
    );
}

#[rstest]
#[case::theme(json!({"theme": {"properties": {"p": {"const": {"$ref": "catalog.json#/theme"}}}}}))]
#[case::component(json!({"components": {"A": {"enum": [{"$ref": "catalog.json#/$defs/X"}]}}}))]
#[case::function(json!({"functions": {"f": {"default": {"$ref": "catalog.json#/$defs/X"}}}}))]
#[case::outside_a_schema_position(
    json!({"$defs": {"X": {"x-note": {"const": {"$ref": "catalog.json#/$defs/X"}}}}})
)]
#[case::draft_7_dependencies(
    json!({"$defs": {"X": {"dependencies": {"a": {"enum": [{"$ref": "catalog.json#/$defs/X"}]}}}}})
)]
fn check_references_data_ref_in_any_entry_is_catalog_error(#[case] document: Value) {
    assert_that!(reference_error(document)).contains("is data, which the runtime would rewrite");
}

#[rstest]
#[case::local_form(json!({"theme": {}, "$defs": {"X": {"const": {"$ref": "#/$defs/X"}}}}))]
#[case::theme_ref_without_a_theme(json!({"$defs": {"X": {"const": {"$ref": "#/theme/a"}}}}))]
#[case::retargeted_theme_ref(
    json!({"theme": {}, "$defs": {"X": {"enum": [{"$ref": "#/$defs/theme"}]}}})
)]
#[case::property_named_const(
    json!({"$defs": {"X": {"properties": {"const": {"$ref": "catalog.json#/$defs/X"}}}}})
)]
#[case::component_theme_ref(
    json!({"theme": {}, "components": {"A": {"const": {"$ref": "#/theme"}}}})
)]
fn check_references_data_ref_the_rewrite_keeps_passes(#[case] document: Value) {
    assert_that!(references(document)).is_ok();
}
