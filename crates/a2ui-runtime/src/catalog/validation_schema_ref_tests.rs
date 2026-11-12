//! The validation schema's `$ref` targets: dangling ones, non-schema ones, and how each is named.

use rstest::rstest;
use serde_json::{Map, Value, json};
use speculoos::prelude::*;

use super::super::test_support::{catalog_error, load, v10};
use crate::bundle::Segment;

#[rstest]
#[case::theme_to_missing_def(
    json!({"theme": {"properties": {"brand": {"$ref": "#/$defs/Missing"}}}}),
    "#/$defs/Missing",
    "/theme/properties/brand/$ref"
)]
#[case::theme_to_missing_theme_node(
    json!({"theme": {"properties": {"brand": {"$ref": "#/theme/$defs/Missing"}}}}),
    "#/theme/$defs/Missing",
    "/theme/properties/brand/$ref"
)]
#[case::theme_to_missing_common_type(
    json!({"theme": {"properties": {"brand": {"$ref": "common_types.json#/$defs/Missing"}}}}),
    "common_types.json#/$defs/Missing",
    "/theme/properties/brand/$ref"
)]
#[case::unreached_def_to_missing_def(
    json!({"$defs": {"Unreached": {"$ref": "#/$defs/Missing"}}}),
    "#/$defs/Missing",
    "/$defs/Unreached/$ref"
)]
#[case::unreached_def_to_absent_theme(
    json!({"$defs": {"Unreached": {"$ref": "#/theme"}}}),
    "#/theme",
    "/$defs/Unreached/$ref"
)]
#[case::unreached_def_to_dropped_component(
    json!({
        "$defs": {
            "anyComponent": {"oneOf": [{"$ref": "#/components/A"}]},
            "Unreached": {"$ref": "#/components/B"}
        },
        "components": {"A": {}, "B": {}}
    }),
    "#/components/B",
    "/$defs/Unreached/$ref"
)]
#[case::unreached_def_to_dropped_function(
    json!({
        "$defs": {
            "anyFunction": {"oneOf": [{"$ref": "#/functions/g"}]},
            "Unreached": {"$ref": "#/functions/f"}
        },
        "functions": {"f": {}, "g": {}}
    }),
    "#/functions/f",
    "/$defs/Unreached/$ref"
)]
#[case::inside_an_array(
    json!({"$defs": {"Unreached": {"anyOf": [{}, {"$ref": "#/$defs/Missing"}]}}}),
    "#/$defs/Missing",
    "/$defs/Unreached/anyOf/1/$ref"
)]
#[case::in_data_like_rule_4(
    json!({"$defs": {"Unreached": {"default": {"$ref": "#/$defs/Missing"}}}}),
    "#/$defs/Missing",
    "/$defs/Unreached/default/$ref"
)]
#[case::percent_encoded_as_authored(
    json!({"$defs": {"Unreached": {"$ref": "catalog.json#/$defs/Missing_%E2%84%93"}}}),
    "catalog.json#/$defs/Missing_%E2%84%93",
    "/$defs/Unreached/$ref"
)]
#[case::unreached_def_to_missing_theme_node(
    json!({"theme": {}, "$defs": {"Unreached": {"$ref": "#/theme/missing"}}}),
    "#/theme/missing",
    "/$defs/Unreached/$ref"
)]
#[case::authored_defs_theme_without_top_level_theme(
    json!({"$defs": {"theme": {"properties": {"b": {"$ref": "#/$defs/Missing"}}}}}),
    "#/$defs/Missing",
    "/$defs/theme/properties/b/$ref"
)]
#[case::under_an_escaped_name(
    json!({"$defs": {"a/b~c": {"$ref": "#/$defs/Missing"}}}),
    "#/$defs/Missing",
    "/$defs/a~1b~0c/$ref"
)]
fn from_json_dangling_ref_outside_kept_entries_is_catalog_error_naming_it(
    #[case] extra: Value,
    #[case] reference: &str,
    #[case] at: &str,
) {
    let mut extra = extra;
    if let Some(map) = extra.as_object_mut() {
        map.entry("components").or_insert(json!({"A": {}}));
    }
    let expected = format!(
        "Malformed catalog: $ref '{reference}' at {at} does not resolve in the validation schema"
    );
    assert_that!(catalog_error(load(v10(extra)))).is_equal_to(expected);
}

#[test]
fn from_json_theme_ref_to_own_def_loads() {
    let document = v10(json!({
        "$defs": {"Brand": {"type": "string"}},
        "theme": {"properties": {"brand": {"$ref": "#/$defs/Brand"}}},
        "components": {"A": {}}
    }));
    assert_that!(load(document)).is_ok();
}

#[rstest]
#[case::theme_node(
    &["$defs", "theme", "properties", "b", "$ref"],
    "#/x",
    true,
    Some(("/theme/properties/b/$ref", "#/x"))
)]
#[case::theme_node_retargeted(
    &["$defs", "theme", "properties", "c", "$ref"],
    "#/$defs/theme/properties/b",
    true,
    Some(("/theme/properties/c/$ref", "catalog.json#/theme/properties/b"))
)]
#[case::defs_theme_without_theme(
    &["$defs", "theme", "$ref"],
    "#/z",
    false,
    Some(("/$defs/theme/$ref", "#/z"))
)]
#[case::def_node(&["$defs", "a/b", "$ref"], "#/y", true, Some(("/$defs/a~1b/$ref", "#/y")))]
#[case::def_node_localized(
    &["$defs", "u", "$ref"],
    "#/$defs/Missing_%E2%84%93",
    false,
    Some(("/$defs/u/$ref", "catalog.json#/$defs/Missing_ℓ"))
)]
#[case::def_node_retargeted(
    &["$defs", "v", "$ref"],
    "#/$defs/theme/properties/b",
    true,
    Some(("/$defs/v/$ref", "catalog.json#/theme/properties/b"))
)]
#[case::theme_node_in_an_array(
    &["$defs", "theme", "allOf", "1", "$ref"],
    "#/w",
    true,
    Some(("/theme/allOf/1/$ref", "#/w"))
)]
#[case::theme_ref_not_retargeted(
    &["$defs", "theme", "properties", "c", "$ref"],
    "catalog.json#/theme/properties/b",
    true,
    None
)]
#[case::other_ref_at_the_authored_place(&["$defs", "a/b", "$ref"], "#/other", true, None)]
#[case::common_type(&["$defs", "DynamicString", "$ref"], "#/y", true, None)]
#[case::component(&["components", "A", "$ref"], "#/y", true, None)]
#[case::not_a_ref_string(&["$defs", "theme", "properties"], "#/x", true, None)]
fn authored_validation_schema_ref_is_authored_place_and_ref_or_none(
    #[case] keys: &[&str],
    #[case] written: &str,
    #[case] with_theme: bool,
    #[case] expected: Option<(&str, &str)>,
) {
    let theme = json!({
        "properties": {
            "b": {"$ref": "#/x"},
            "c": {"$ref": "catalog.json#/theme/properties/b"}
        },
        "allOf": [{}, {"$ref": "#/w"}]
    });
    let defs = Map::from_iter([
        ("a/b".to_owned(), json!({"$ref": "#/y"})),
        ("theme".to_owned(), json!({"$ref": "#/z"})),
        (
            "u".to_owned(),
            json!({"$ref": "catalog.json#/$defs/Missing_ℓ"}),
        ),
        (
            "v".to_owned(),
            json!({"$ref": "catalog.json#/theme/properties/b"}),
        ),
    ]);
    let path: Vec<Segment<'_>> = keys
        .iter()
        .map(|key| key.parse().map_or(Segment::Key(key), Segment::Index))
        .collect();
    let found = super::authored(&path, written, with_theme.then_some(&theme), Some(&defs));
    let found = found
        .as_ref()
        .map(|(place, reference)| (place.as_str(), *reference));
    assert_that!(found).is_equal_to(expected);
}

#[test]
fn check_refs_name_schemas_runtime_written_ref_is_named_as_the_schema_holds_it() {
    let schema = json!({"components": {"A": {"$ref": "#/$defs/Gone"}}, "$defs": {}});
    let message = super::check_refs_name_schemas(&schema, None, None)
        .unwrap_err()
        .to_string();
    let expected = "Malformed catalog: $ref '#/$defs/Gone' at /components/A/$ref of the \
                    validation schema does not resolve";
    assert_that!(message).is_equal_to(expected.to_owned());
}

#[test]
fn from_json_dangling_ref_with_control_characters_is_named_escaped() {
    let document = v10(json!({
        "$defs": {"Unreached": {"$ref": "#/$defs/Missing\n\u{1b}[2J"}},
        "components": {"A": {}}
    }));
    let expected = "Malformed catalog: $ref '#/$defs/Missing\\n\\u{1b}[2J' at \
                    /$defs/Unreached/$ref does not resolve in the validation schema";
    assert_that!(catalog_error(load(document))).is_equal_to(expected.to_owned());
}

#[test]
fn from_json_long_dangling_ref_keeps_the_wording_around_a_capped_ref() {
    let reference = format!("#/$defs/{}", "M".repeat(2000));
    let document = v10(json!({
        "$defs": {"Unreached": {"$ref": reference}},
        "components": {"A": {}}
    }));
    let message = catalog_error(load(document));
    assert_that!(message.contains("characters elided")).is_true();
    assert_that!(message)
        .ends_with(" at /$defs/Unreached/$ref does not resolve in the validation schema");
    assert_that!(message.len()).is_less_than(700);
}

#[rstest]
#[case::theme_to_a_string(
    json!({"theme": {"title": "T", "properties": {"b": {"$ref": "#/theme/title"}}}}),
    "#/theme/title",
    "/theme/properties/b/$ref",
    "a string"
)]
#[case::unreached_def_to_an_array(
    json!({"$defs": {"X": {"enum": [1]}, "Unreached": {"$ref": "#/$defs/X/enum"}}}),
    "#/$defs/X/enum",
    "/$defs/Unreached/$ref",
    "an array"
)]
#[case::unreached_def_to_a_number(
    json!({"$defs": {"X": {"minimum": 1}, "Unreached": {"$ref": "#/$defs/X/minimum"}}}),
    "#/$defs/X/minimum",
    "/$defs/Unreached/$ref",
    "a number"
)]
#[case::unreached_def_to_null(
    json!({"$defs": {"X": {"const": null}, "Unreached": {"$ref": "#/$defs/X/const"}}}),
    "#/$defs/X/const",
    "/$defs/Unreached/$ref",
    "null"
)]
fn from_json_ref_to_a_non_schema_node_is_catalog_error_naming_it(
    #[case] mut extra: Value,
    #[case] reference: &str,
    #[case] at: &str,
    #[case] kind: &str,
) {
    extra["components"] = json!({"A": {}});
    let expected = format!(
        "Malformed catalog: $ref '{reference}' at {at} names {kind} in the validation schema, \
         not a schema (an object or a boolean)"
    );
    assert_that!(catalog_error(load(v10(extra)))).is_equal_to(expected);
}

#[rstest]
#[case::boolean_schema(json!({"B": true}), "#/$defs/B")]
#[case::object_in_a_data_position(json!({"X": {"default": {}}}), "#/$defs/X/default")]
#[case::boolean_in_a_data_position(json!({"X": {"uniqueItems": true}}), "#/$defs/X/uniqueItems")]
fn from_json_ref_to_an_object_or_boolean_node_keeps_it(
    #[case] mut defs: Value,
    #[case] reference: &str,
) {
    defs["Unreached"] = json!({"$ref": reference});
    let catalog = load(v10(json!({"$defs": defs, "components": {"A": {}}})));
    let kept = catalog.map(|catalog| catalog.validation_schema()["$defs"]["Unreached"].clone());
    assert_that!(kept).is_ok_containing(json!({"$ref": reference}));
}

#[rstest]
#[case::default_data("default", json!({"$ref": "#/$defs/X/title"}), "/default/$ref")]
#[case::examples_data("examples", json!([{"$ref": "#/$defs/X/title"}]), "/examples/0/$ref")]
#[case::const_data("const", json!({"$ref": "#/$defs/X/title"}), "/const/$ref")]
#[case::enum_data("enum", json!([{"$ref": "#/$defs/X/title"}]), "/enum/0/$ref")]
fn from_json_data_ref_to_a_string_is_catalog_error_like_any_ref(
    #[case] keyword: &str,
    #[case] data: Value,
    #[case] below: &str,
) {
    let document = v10(json!({
        "$defs": {"X": {"title": "T"}, "Unreached": {keyword: data}},
        "components": {"A": {}}
    }));
    let expected = format!(
        "Malformed catalog: $ref '#/$defs/X/title' at /$defs/Unreached{below} names a string \
         in the validation schema, not a schema (an object or a boolean)"
    );
    assert_that!(catalog_error(load(document))).is_equal_to(expected);
}

#[test]
fn from_json_kept_component_ref_to_a_string_fails_its_compile() {
    let document = v10(json!({
        "$defs": {"X": {"title": "T"}},
        "components": {"A": {"properties": {"p": {"$ref": "#/$defs/X/title"}}}}
    }));
    let expected = "Schema does not compile: \"T\" is not of types \"boolean\", \"object\"";
    assert_that!(catalog_error(load(document))).is_equal_to(expected.to_owned());
}

#[test]
fn check_refs_name_schemas_runtime_written_ref_to_a_string_is_named_as_the_schema_holds_it() {
    let schema =
        json!({"components": {"A": {"$ref": "#/$defs/X/title"}}, "$defs": {"X": {"title": "T"}}});
    let message = super::check_refs_name_schemas(&schema, None, None)
        .unwrap_err()
        .to_string();
    let expected = "Malformed catalog: $ref '#/$defs/X/title' at /components/A/$ref of the \
                    validation schema names a string, not a schema (an object or a boolean)";
    assert_that!(message).is_equal_to(expected.to_owned());
}

#[rstest]
#[case::examples_data("examples", json!([{"$ref": "#/$defs/Missing"}]), "/examples/0/$ref")]
#[case::const_data("const", json!({"$ref": "#/$defs/Missing"}), "/const/$ref")]
#[case::enum_data("enum", json!([{"$ref": "#/$defs/Missing"}]), "/enum/0/$ref")]
fn from_json_dangling_ref_in_data_is_catalog_error_like_any_ref(
    #[case] keyword: &str,
    #[case] data: Value,
    #[case] below: &str,
) {
    let document = v10(json!({"$defs": {"X": {keyword: data}}, "components": {"A": {}}}));
    let expected = format!(
        "Malformed catalog: $ref '#/$defs/Missing' at /$defs/X{below} does not resolve in the \
         validation schema"
    );
    assert_that!(catalog_error(load(document))).is_equal_to(expected);
}
