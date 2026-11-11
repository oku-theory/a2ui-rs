use serde_json::json;
use speculoos::prelude::*;

use super::*;

fn p(path: &str) -> Pointer {
    Pointer::parse(path).unwrap()
}

#[test]
fn notified_same_path_twice_lists_both_observers() {
    let mut model = DataModel::new(json!({"user": {"name": "Alice"}}));
    let mut watchers = Watchers::default();
    watchers.watch(1, p("/user/name"));
    watchers.watch(2, p("/user/name"));
    let write = model.set(&p("/user/name"), json!("Eve")).unwrap().unwrap();
    assert_that!(watchers.notified(&write, &model)).is_equal_to(vec![1, 2]);
}

#[test]
fn notified_unrelated_path_is_empty() {
    let mut model = DataModel::default();
    let mut watchers = Watchers::default();
    watchers.watch("b", p("/b"));
    let write = model.set(&p("/a"), json!(99)).unwrap().unwrap();
    assert_that!(watchers.notified(&write, &model)).is_empty();
}

#[test]
fn clear_removes_every_observer() {
    let mut model = DataModel::default();
    let mut watchers = Watchers::default();
    watchers.watch(1, Pointer::root());
    watchers.clear();
    let write = model.set(&p("/foo"), json!("bar")).unwrap().unwrap();
    assert_that!(watchers.notified(&write, &model)).is_empty();
}
