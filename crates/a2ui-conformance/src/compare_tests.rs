use rstest::rstest;
use serde_json::json;
use speculoos::prelude::*;

use super::*;

#[rstest]
#[case(json!(25000), json!(25000.0))]
#[case(json!({"a": [1, {"b": true}]}), json!({"a": [1.0, {"b": true}]}))]
#[case(json!(null), json!(null))]
fn json_eq_equivalent_values_are_equal(#[case] a: Value, #[case] b: Value) {
    assert_that!(json_eq(&a, &b)).is_true();
}

#[rstest]
#[case(json!(1), json!("1"))]
#[case(json!([1, 2]), json!([2, 1]))]
#[case(json!({"a": 1}), json!({"a": 1, "b": 2}))]
fn json_eq_different_values_are_not_equal(#[case] a: Value, #[case] b: Value) {
    assert_that!(json_eq(&a, &b)).is_false();
}
