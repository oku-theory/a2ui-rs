use serde_json::{Value, json};
use speculoos::prelude::*;

use super::*;
use crate::case::parse_suite;

fn case() -> Case {
    parse_suite("core/x.yaml", "- {name: test_a, action: process_messages}")
        .unwrap()
        .remove(0)
}

#[test]
fn aliased_line_is_one_json_object_with_its_newline() {
    let line = aliased_line(&case(), &["c1".to_owned(), "c2".to_owned()]).unwrap();
    assert_that!(line.ends_with('\n')).is_true();
    assert_that!(line.matches('\n').count()).is_equal_to(1);
    let value: Value = serde_json::from_str(&line).unwrap();
    let want = json!({"suite": "core/x.yaml", "case": "test_a", "aliased": ["c1", "c2"]});
    assert_that!(value).is_equal_to(want);
}

#[test]
fn aliased_line_empty_list_is_none() {
    assert_that!(aliased_line(&case(), &[])).is_none();
}
