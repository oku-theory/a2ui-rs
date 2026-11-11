use rstest::rstest;
use serde_json::{Map, Value, json};
use speculoos::prelude::*;

use super::*;

fn nested(levels: usize) -> Value {
    // `json!({"a": inner})` clones `inner` recursively, so deep values are built by moving.
    (0..levels).fold(json!(1), |inner, _| {
        Value::Object(Map::from_iter([("a".to_owned(), inner)]))
    })
}

#[rstest]
#[case(json!(1), 0, false)]
#[case(json!({"a": 1}), 1, false)]
#[case(json!({"a": [1]}), 1, true)]
#[case(json!([[], {}]), 2, false)]
fn deeper_than_small_values_are_measured(
    #[case] value: Value,
    #[case] limit: usize,
    #[case] expected: bool,
) {
    assert_that!(deeper_than(&value, limit)).is_equal_to(expected);
}

#[test]
fn deeper_than_value_at_the_cap_is_accepted() {
    assert_that!(deeper_than(&nested(MAX_JSON_DEPTH), MAX_JSON_DEPTH)).is_false();
}

#[test]
fn deeper_than_value_past_the_cap_is_rejected() {
    assert_that!(deeper_than(&nested(MAX_JSON_DEPTH + 1), MAX_JSON_DEPTH)).is_true();
}

#[test]
fn deeper_than_hundred_thousand_levels_in_a_small_stack_is_rejected() {
    let rejected = std::thread::Builder::new()
        .stack_size(256 * 1024)
        .spawn(|| {
            let value = nested(100_000);
            let rejected = deeper_than(&value, MAX_JSON_DEPTH);
            // Dropping 100 000 levels would recurse through serde_json's `Drop` and
            // overflow this thread's stack: the test leaks the value on purpose.
            std::mem::forget(value);
            rejected
        })
        .unwrap()
        .join()
        .unwrap();
    assert_that!(rejected).is_true();
}
