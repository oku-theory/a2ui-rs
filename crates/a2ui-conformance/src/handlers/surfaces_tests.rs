use a2ui_runtime::{Catalog, CatalogSet};
use rstest::rstest;
use serde_json::{Value, json};
use speculoos::prelude::*;

use super::*;

/// A processor holding surface `s1`: `root` (Column) over `t` (Text), data model `{n: 1}`.
fn processor() -> MessageProcessor {
    let document = json!({"catalogId": "c", "protocolVersion": "1.0", "components": {
        "Column": {"type": "object", "properties": {
            "children": {"$ref": "common_types.json#/$defs/ChildList"}}},
        "Text": {"type": "object", "properties": {"text": {"type": "string"}}}}});
    let mut set = CatalogSet::new();
    set.insert(Catalog::from_json(&document, None, None).unwrap())
        .unwrap();
    let mut processor = MessageProcessor::new(set);
    processor
        .handle_batch(&json!([{"version": "1.0", "createSurface": {
            "surfaceId": "s1", "catalogId": "c", "sendDataModel": true,
            "components": [
                {"id": "root", "component": "Column", "children": ["t"]},
                {"id": "t", "component": "Text", "text": "hi"}],
            "dataModel": {"n": 1}}}]))
        .unwrap();
    processor
}

fn checked(surface: Value) -> Result<(), String> {
    check(
        &processor(),
        &json!({"surfaces": {"s1": surface}}),
        "step 0",
    )
}

#[rstest]
#[case::pass(json!([{"id": "root"}, {"id": "t"}]), true)]
#[case::fail(json!([{"id": "root"}]), false)]
fn list_form_needs_the_same_count(#[case] components: Value, #[case] passes: bool) {
    let result = checked(json!({"components": components}));
    assert_that!(result.is_ok()).is_equal_to(passes);
}

#[rstest]
#[case::pass(json!(["t"]), true)]
#[case::fail(json!(["t", "u"]), false)]
fn list_form_compares_listed_values_whole(#[case] children: Value, #[case] passes: bool) {
    let components = json!([{"id": "root", "children": children}, {"id": "t"}]);
    let result = checked(json!({"components": components}));
    assert_that!(result.is_ok()).is_equal_to(passes);
}

#[rstest]
#[case::pass(json!({"t": {"text": "hi"}}), true)]
#[case::fail(json!({"t": {"text": "bye"}}), false)]
#[case::fail_unknown_id(json!({"u": {}}), false)]
fn map_form_is_a_subset_by_id(#[case] components: Value, #[case] passes: bool) {
    let result = checked(json!({"components": components}));
    assert_that!(result.is_ok()).is_equal_to(passes);
}

#[rstest]
#[case::pass(json!({"t": {"missing": null}}), true)]
#[case::fail(json!({"t": {"text": null}}), false)]
fn null_matches_only_an_absent_key(#[case] components: Value, #[case] passes: bool) {
    let result = checked(json!({"components": components}));
    assert_that!(result.is_ok()).is_equal_to(passes);
}

#[rstest]
#[case::pass(json!({"n": 1}), true)]
#[case::fail(json!({}), false)]
fn data_model_compares_the_whole_root(#[case] model: Value, #[case] passes: bool) {
    let result = checked(json!({"dataModel": model}));
    assert_that!(result.is_ok()).is_equal_to(passes);
}

#[rstest]
#[case::pass(json!({"exists": true, "sendDataModel": true, "metadata": null}), true)]
#[case::fail(json!({"exists": true, "theme": {}}), false)]
fn unknown_expectation_key_fails(#[case] surface: Value, #[case] passes: bool) {
    let result = checked(surface);
    assert_that!(result.is_ok()).is_equal_to(passes);
}

#[test]
fn absent_surface_expected_absent_passes() {
    let expected = json!({"surfaces": {"s9": {"exists": false}}});
    assert_that!(check(&processor(), &expected, "step 0")).is_ok();
}

#[test]
fn failure_names_the_context() {
    let result = checked(json!({"exists": false}));
    assert_that!(result.unwrap_err()).contains("step 0");
}

#[rstest]
#[case::no_surfaces(json!({}))]
#[case::empty_surfaces(json!({"surfaces": {}}))]
fn vacuous_expect_fails(#[case] expected: Value) {
    let result = check(&processor(), &expected, "step 0");
    assert_that!(result.unwrap_err()).contains("checks nothing");
}
