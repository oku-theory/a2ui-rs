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

/// The `$ref` strings `find_ref` visits under `node`, each with its JSON Pointer.
fn visited(node: &Value) -> Vec<(String, String)> {
    let mut seen = Vec::new();
    find_ref(node, &mut |reference, path| {
        seen.push((reference.to_owned(), pointer(path)));
        None::<()>
    });
    seen
}

#[test]
fn find_ref_visits_refs_in_objects_arrays_and_data() {
    let node = json!({
        "a": {"$ref": "#/x"},
        "b": [{}, {"$ref": "#/y"}],
        "default": {"$ref": "#/z"}
    });
    assert_that!(visited(&node)).is_equal_to(vec![
        ("#/x".to_owned(), "/a/$ref".to_owned()),
        ("#/y".to_owned(), "/b/1/$ref".to_owned()),
        ("#/z".to_owned(), "/default/$ref".to_owned()),
    ]);
}

#[test]
fn find_ref_object_under_ref_key_is_walked_as_a_property() {
    let node = json!({"properties": {"$ref": {"$ref": "#/x"}}});
    assert_that!(visited(&node))
        .is_equal_to(vec![("#/x".to_owned(), "/properties/$ref/$ref".to_owned())]);
}

#[test]
fn find_ref_stops_at_the_first_some() {
    let node = json!({"a": {"$ref": "#/x"}, "b": {"$ref": "#/y"}});
    let mut calls = 0;
    let found = find_ref(&node, &mut |reference, _| {
        calls += 1;
        Some(reference.to_owned())
    });
    assert_that!((found, calls)).is_equal_to((Some("#/x".to_owned()), 1));
}

#[test]
fn rewrite_refs_rewrites_every_ref_string() {
    let mut node = json!({"a": {"$ref": "#/x"}, "b": [{"$ref": "#/y"}], "c": "#/z"});
    rewrite_refs(&mut node, &mut |reference| reference.push('!'));
    assert_that!(node).is_equal_to(json!({
        "a": {"$ref": "#/x!"},
        "b": [{"$ref": "#/y!"}],
        "c": "#/z"
    }));
}

#[test]
fn rewrite_refs_visits_what_for_each_ref_visits() {
    let mut node = json!({
        "properties": {"$ref": {"$ref": "#/a"}},
        "b": [[{"$ref": "#/b"}], {"default": {"$ref": "#/c"}}],
        "n": {"$ref": 5},
        "s": {"$ref": [{"$ref": "#/e"}]},
        "$ref": "#/d"
    });
    let mut expected = Vec::new();
    for_each_ref(&node, &mut |reference| expected.push(reference.to_owned()));
    let mut rewritten = Vec::new();
    rewrite_refs(&mut node, &mut |reference| {
        rewritten.push(reference.clone())
    });
    assert_that!(rewritten).is_equal_to(expected);
}

#[test]
fn find_ref_nested_arrays_spell_each_index() {
    assert_that!(visited(&json!([[{"$ref": "#/x"}]])))
        .is_equal_to(vec![("#/x".to_owned(), "/0/0/$ref".to_owned())]);
}

#[rstest]
#[case::number(json!({"$ref": 1}))]
#[case::null(json!({"$ref": null}))]
fn find_ref_non_string_ref_is_not_visited(#[case] node: Value) {
    assert_that!(visited(&node)).is_empty();
}

#[test]
fn def_names_lists_only_defs_entries() {
    let node = json!({
        "a": {"$ref": "#/$defs/A"},
        "b": {"$ref": "#/components/B"},
        "c": {"$ref": "#/theme"},
        "d": {"$ref": "#/$defs/D/properties/x"}
    });
    let mut names = Vec::new();
    def_names(&node, &mut names);
    assert_that!(names).is_equal_to(vec!["A".to_owned(), "D".to_owned()]);
}

/// The scalar members `find_member` visits under `node`: key, value and JSON Pointer.
fn members(node: &Value) -> Vec<(String, Value, String)> {
    let mut seen = Vec::new();
    find_member(node, &mut |key, value, path| {
        seen.push((key.to_owned(), value.clone(), pointer(path)));
        None::<()>
    });
    seen
}

#[test]
fn find_member_visits_each_scalar_member_with_its_path() {
    let node = json!({"a": {"$ref": "#/x", "n": 5}, "b": [{"$id": null}]});
    assert_that!(members(&node)).is_equal_to(vec![
        ("$ref".to_owned(), json!("#/x"), "/a/$ref".to_owned()),
        ("n".to_owned(), json!(5), "/a/n".to_owned()),
        ("$id".to_owned(), Value::Null, "/b/0/$id".to_owned()),
    ]);
}

#[rstest]
#[case::array_items(json!({"enum": ["#/x", 1]}))]
#[case::root(json!("#/x"))]
fn find_member_scalar_without_a_key_is_not_visited(#[case] node: Value) {
    assert_that!(members(&node)).is_empty();
}

#[test]
fn find_member_stops_at_the_first_some() {
    let node = json!({"a": [{"x": 1}], "b": 2});
    let mut calls = 0;
    let found = find_member(&node, &mut |key, _, _| {
        calls += 1;
        Some(key.to_owned())
    });
    assert_that!((found, calls)).is_equal_to((Some("x".to_owned()), 1));
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
#[case::key_and_index(vec![Segment::Key("a/b"), Segment::Index(1)], Some(json!(2)))]
#[case::root(vec![], Some(json!({"a/b": [1, 2]})))]
#[case::missing_key(vec![Segment::Key("x")], None)]
#[case::index_past_the_end(vec![Segment::Key("a/b"), Segment::Index(2)], None)]
#[case::key_into_an_array(vec![Segment::Key("a/b"), Segment::Key("0")], None)]
fn node_at_path_names_its_node(#[case] path: Vec<Segment<'_>>, #[case] expected: Option<Value>) {
    let root = json!({"a/b": [1, 2]});
    assert_that!(node_at(&root, &path).cloned()).is_equal_to(expected);
}

#[test]
fn first_not_a_schema_path_holds_raw_keys() {
    let root = json!({"$defs": {"a/b": {"$ref": "#/$defs/Missing"}}});
    let path = vec![
        Segment::Key("$defs"),
        Segment::Key("a/b"),
        Segment::Key("$ref"),
    ];
    assert_that!(first_not_a_schema(&root)).is_equal_to(Some((
        "#/$defs/Missing",
        path,
        NotASchema::Missing,
    )));
}
