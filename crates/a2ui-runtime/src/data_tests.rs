use rstest::rstest;
use serde_json::{Value, json};
use speculoos::prelude::*;

use super::*;
use crate::error::Category;

fn p(path: &str) -> Pointer {
    Pointer::parse(path).unwrap()
}

fn sample() -> DataModel {
    DataModel::new(json!({"user": {"name": "Alice"}, "items": ["a", "b", "c"]}))
}

#[test]
fn default_model_is_empty_object() {
    assert_that!(DataModel::default().get(&Pointer::root())).is_equal_to(Some(&json!({})));
}

#[rstest]
#[case("/user/name", json!("Alice"))]
#[case("/items/1", json!("b"))]
fn get_existing_path_returns_value(#[case] path: &str, #[case] expected: Value) {
    assert_that!(sample().get(&p(path)).cloned()).is_equal_to(Some(expected));
}

#[rstest]
#[case("/user/age")]
#[case("/items/99")]
#[case("/items/-1")]
#[case("/items/01")]
#[case("/user/name/first")]
fn get_missing_path_is_absent(#[case] path: &str) {
    assert_that!(sample().get(&p(path))).is_none();
}

#[test]
fn set_nested_new_path_creates_objects_and_lists() {
    let mut model = DataModel::default();
    model.set(&p("/a/b/0/c"), json!(1)).unwrap();
    assert_that!(model.get(&Pointer::root()).cloned())
        .is_equal_to(Some(json!({"a": {"b": [{"c": 1}]}})));
}

#[test]
fn set_past_list_end_pads_with_null() {
    let mut model = sample();
    model.set(&p("/items/4"), json!("e")).unwrap();
    assert_that!(model.get(&p("/items")).cloned())
        .is_equal_to(Some(json!(["a", "b", "c", null, "e"])));
}

#[test]
fn set_through_primitive_is_data_error() {
    let err = sample()
        .set(&p("/user/name/first"), json!("x"))
        .unwrap_err();
    assert_that!(err.to_string()).contains("Cannot set path");
}

#[rstest]
#[case("/items/foo")]
#[case("/items/01")]
#[case("/items/foo/bar")]
fn set_non_numeric_list_segment_is_rejected(#[case] path: &str) {
    let err = sample().set(&p(path), json!("x")).unwrap_err();
    assert_that!(err.to_string()).contains("non-numeric segment");
}

#[test]
fn set_index_past_limit_is_data_error() {
    let err = sample().set(&p("/items/10001"), json!("x")).unwrap_err();
    assert_that!(err.category()).is_equal_to(Category::Data);
}

#[test]
fn set_refused_new_list_leaves_no_container() {
    let mut model = sample();
    let _ = model.set(&p("/newArray/10000000/field"), json!("x"));
    assert_that!(model.get(&p("/newArray"))).is_none();
}

#[test]
fn set_null_on_absent_path_is_noop() {
    let mut model = sample();
    assert_that!(model.set(&p("/ghost/child"), Value::Null).unwrap()).is_none();
}

#[test]
fn set_null_on_absent_path_creates_no_parent() {
    let mut model = sample();
    model.set(&p("/ghost/child"), Value::Null).unwrap();
    assert_that!(model.get(&p("/ghost"))).is_none();
}

#[test]
fn set_same_value_returns_none() {
    let mut model = sample();
    assert_that!(model.set(&p("/user/name"), json!("Alice")).unwrap()).is_none();
}

#[test]
fn delete_object_key_removes_it() {
    let mut model = sample();
    model.delete(&p("/user/name")).unwrap();
    assert_that!(model.get(&p("/user")).cloned()).is_equal_to(Some(json!({})));
}

#[test]
fn delete_list_element_keeps_length() {
    let mut model = sample();
    model.delete(&p("/items/1")).unwrap();
    assert_that!(model.get(&p("/items")).cloned()).is_equal_to(Some(json!(["a", null, "c"])));
}

#[test]
fn delete_root_resets_to_empty_object() {
    let mut model = sample();
    model.delete(&Pointer::root()).unwrap();
    assert_that!(model.get(&Pointer::root()).cloned()).is_equal_to(Some(json!({})));
}

#[rstest]
#[case("/user/name", "/user/name", true)]
#[case("/user/name", "/user", true)]
#[case("/user", "/user/name", true)]
#[case("/user/name", "/items", false)]
fn write_changes_related_paths_only(
    #[case] written: &str,
    #[case] watched: &str,
    #[case] fired: bool,
) {
    let mut model = sample();
    let write = model
        .set(&p(written), json!({"name": "Bob"}))
        .unwrap()
        .unwrap();
    assert_that!(write.changes(&p(watched), &model)).is_equal_to(fired);
}

#[test]
fn write_unchanged_descendant_does_not_change() {
    let mut model = DataModel::new(json!({"a": {"c": 1}}));
    let write = model
        .set(&p("/a"), json!({"c": 1, "d": 2}))
        .unwrap()
        .unwrap();
    assert_that!(write.changes(&p("/a/c"), &model)).is_false();
}
