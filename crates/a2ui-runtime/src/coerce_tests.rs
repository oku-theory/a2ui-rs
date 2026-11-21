use rstest::rstest;
use serde_json::{Value, json};
use speculoos::prelude::*;

use super::*;

#[rstest]
#[case::true_(json!(true), true)]
#[case::string_true_any_case(json!("TRUE"), true)]
#[case::string_yes(json!("yes"), false)]
#[case::one(json!(1), true)]
#[case::zero(json!(0), false)]
#[case::valid_true(json!({"valid": true}), true)]
#[case::valid_false(json!({"valid": false}), false)]
#[case::empty_object(json!({}), false)]
#[case::null(json!(null), false)]
#[case::empty_array(json!([]), false)]
#[case::negative_zero(json!(-0.0), false)]
fn boolean_coerces_each_kind(#[case] value: Value, #[case] expected: bool) {
    assert_that!(boolean(&value)).is_equal_to(expected);
}

#[rstest]
#[case::padded_decimal(json!(" 12.5 "), Some(12.5))]
#[case::exponent(json!("1e3"), Some(1000.0))]
#[case::word(json!("abc"), None)]
#[case::infinity(json!("inf"), None)]
#[case::boolean(json!(true), None)]
#[case::number(json!(-3), Some(-3.0))]
#[case::null(json!(null), None)]
#[case::nan(json!("NaN"), None)]
#[case::overflow(json!("1e999"), None)]
#[case::empty(json!(""), None)]
#[case::blank(json!("  "), None)]
fn number_reads_numbers_and_numeric_strings(#[case] value: Value, #[case] expected: Option<f64>) {
    assert_that!(number(&value)).is_equal_to(expected);
}

#[rstest]
#[case::null(json!(null), "")]
#[case::number(json!(1.5), "1.5")]
#[case::boolean(json!(true), "true")]
#[case::string(json!("a b"), "a b")]
#[case::object(json!({"a": 1}), r#"{"a":1}"#)]
#[case::array(json!([1, 2]), "[1,2]")]
fn text_converts_each_kind(#[case] value: Value, #[case] expected: &str) {
    assert_that!(text(&value)).is_equal_to(expected.to_owned());
}
