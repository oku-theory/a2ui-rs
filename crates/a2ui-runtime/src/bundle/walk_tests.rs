use rstest::rstest;
use serde_json::{Value, json};
use speculoos::prelude::*;

use super::*;

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
