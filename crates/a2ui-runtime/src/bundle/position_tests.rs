use rstest::rstest;
use serde_json::{Value, json};
use speculoos::prelude::*;

use super::*;
use crate::bundle::pointer;

fn data(schema: &Value) -> Vec<(String, Value)> {
    let mut seen = Vec::new();
    find_data(schema, &mut Vec::new(), &mut |value, path| {
        seen.push((pointer(path), value.clone()));
        None::<()>
    });
    seen.sort_by(|a, b| a.0.cmp(&b.0));
    seen
}

#[test]
fn find_data_visits_data_keywords_in_schema_positions() {
    let schema = json!({
        "const": 1,
        "properties": {"p": {"enum": [2]}, "const": {"default": 3}},
        "allOf": [{"examples": [4]}],
        "items": {"default": 5},
    });
    let expected = vec![
        ("/allOf/0/examples".to_owned(), json!([4])),
        ("/const".to_owned(), json!(1)),
        ("/items/default".to_owned(), json!(5)),
        ("/properties/const/default".to_owned(), json!(3)),
        ("/properties/p/enum".to_owned(), json!([2])),
    ];
    assert_that!(data(&schema)).is_equal_to(expected);
}

#[rstest]
#[case::unknown_keyword(json!({"x-note": {"const": 1}}), "/x-note/const")]
#[case::draft_7_dependencies(
    json!({"dependencies": {"a": {"default": 1}}}),
    "/dependencies/a/default"
)]
#[case::array_items(json!({"items": [{"enum": 1}]}), "/items/0/enum")]
#[case::list_of_maps(json!({"properties": [{"const": 1}]}), "/properties/0/const")]
fn find_data_keyword_outside_schema_positions_is_data(#[case] schema: Value, #[case] at: &str) {
    assert_that!(data(&schema)).is_equal_to(vec![(at.to_owned(), json!(1))]);
}

#[rstest]
#[case::properties("properties")]
#[case::pattern_properties("patternProperties")]
#[case::defs("$defs")]
#[case::dependent_schemas("dependentSchemas")]
fn find_data_data_keyword_as_a_name_is_a_schema(#[case] map: &str) {
    let schema = json!({map: {"const": {"enum": 1}}});
    let at = format!("/{map}/const/enum");
    assert_that!(data(&schema)).is_equal_to(vec![(at, json!(1))]);
}

#[test]
fn find_data_data_is_not_walked_further() {
    let schema = json!({"const": {"default": 2}});
    assert_that!(data(&schema)).is_equal_to(vec![("/const".to_owned(), json!({"default": 2}))]);
}

#[test]
fn find_data_stops_at_the_first_some() {
    let schema = json!({"allOf": [{"const": 1}, {"const": 2}]});
    let mut calls = 0;
    let found = find_data(&schema, &mut Vec::new(), &mut |value, _| {
        calls += 1;
        Some(value.clone())
    });
    assert_that!((found, calls)).is_equal_to((Some(json!(1)), 1));
}
