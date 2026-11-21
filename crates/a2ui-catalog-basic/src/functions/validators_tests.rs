use a2ui_runtime::{Category, ErrorCode};
use rstest::rstest;
use serde_json::{Value, json};
use speculoos::prelude::*;

use super::super::test_support::call;

/// `{"valid": valid}`, the result every validator returns.
fn valid(valid: bool) -> Value {
    json!({ "valid": valid })
}

#[rstest]
#[case::string(json!("hello"), true)]
#[case::empty_string(json!(""), false)]
#[case::null(json!(null), false)]
#[case::array(json!([1, 2]), true)]
#[case::empty_array(json!([]), false)]
#[case::empty_object(json!({}), true)]
#[case::zero(json!(0), true)]
#[case::false_(json!(false), true)]
fn required_rows_give_validity(#[case] value: Value, #[case] expected: bool) {
    let result = call("required", json!({ "value": value }));
    assert_that!(result.unwrap()).is_equal_to(valid(expected));
}

#[rstest]
#[case::within(json!({"value": "hello", "min": 3, "max": 10}), true)]
#[case::too_short(json!({"value": "hi", "min": 3}), false)]
#[case::too_long(json!({"value": "hello world", "max": 5}), false)]
#[case::at_both_bounds(json!({"value": "abc", "min": 3, "max": 3}), true)]
#[case::array_items(json!({"value": ["a", "b", "c"], "min": 2, "max": 4}), true)]
#[case::array_too_short(json!({"value": ["a"], "min": 2}), false)]
#[case::null_is_zero(json!({"value": null, "max": 10}), true)]
#[case::null_below_min(json!({"value": null, "min": 1}), false)]
#[case::astral_by_code_points(json!({"value": "😀😀", "max": 2}), true)]
#[case::number_as_text(json!({"value": 12345, "max": 4}), false)]
fn length_rows_give_validity(#[case] args: Value, #[case] expected: bool) {
    assert_that!(call("length", args).unwrap()).is_equal_to(valid(expected));
}

#[test]
fn length_non_numeric_bound_is_an_expression_error() {
    let error = call("length", json!({"value": "abc", "min": "few"})).unwrap_err();
    assert_that!(error.code()).is_equal_to(ErrorCode::ExpressionError);
}

#[rstest]
#[case::float_within(json!({"value": 42.5, "min": 0, "max": 100}), true)]
#[case::string_in_range(json!({"value": "12", "min": 10, "max": 20}), true)]
#[case::too_small(json!({"value": -5, "min": 0}), false)]
#[case::too_large(json!({"value": 150, "max": 100}), false)]
#[case::at_both_bounds(json!({"value": 7, "min": 7, "max": 7}), true)]
#[case::unparseable(json!({"value": "abc", "min": 0}), false)]
#[case::null(json!({"value": null, "max": 1}), false)]
fn numeric_rows_give_validity(#[case] args: Value, #[case] expected: bool) {
    assert_that!(call("numeric", args).unwrap()).is_equal_to(valid(expected));
}

#[test]
fn numeric_non_numeric_bound_is_an_error_whatever_the_value() {
    let error = call("numeric", json!({"value": "abc", "min": "few"})).unwrap_err();
    assert_that!(error.code()).is_equal_to(ErrorCode::ExpressionError);
}

#[rstest]
#[case::standard("user@example.com", true)]
#[case::short("a@b.co", true)]
#[case::subdomain("first.last@mail.example.org", true)]
#[case::no_domain("invalid-email", false)]
#[case::one_letter_tld("a@b.c", false)]
#[case::one_letter_tld_longer("test@test.c", false)]
#[case::space_in_local("a b@c.de", false)]
#[case::two_ats("a@b@c.de", false)]
#[case::empty_local("@c.de", false)]
#[case::empty_label("a@c..de", false)]
#[case::digit_tld("a@c.d1", false)]
#[case::non_ascii_local("é@c.de", false)]
#[case::inner_hyphen("a@b-c.de", true)]
#[case::label_starts_with_hyphen("a@-b.de", false)]
#[case::label_ends_with_hyphen("a@b-.de", false)]
fn email_rows_give_validity(#[case] value: &str, #[case] expected: bool) {
    let result = call("email", json!({ "value": value }));
    assert_that!(result.unwrap()).is_equal_to(valid(expected));
}

#[rstest]
#[case::matches("^[0-9]+$", "12345", true)]
#[case::mismatch("^[0-9]+$", "abc", false)]
#[case::searches("b", "abc", true)]
fn regex_rows_give_validity(#[case] pattern: &str, #[case] value: &str, #[case] expected: bool) {
    let result = call("regex", json!({ "pattern": pattern, "value": value }));
    assert_that!(result.unwrap()).is_equal_to(valid(expected));
}

#[rstest]
#[case::backreference("(a)\\1", "UNSUPPORTED_SYNTAX")]
#[case::nested_quantifier("(a+)+", "NESTED_QUANTIFIER")]
#[case::too_long(&"a".repeat(257), "PATTERN_TOO_LONG")]
fn regex_refused_pattern_returns_dialect_error(#[case] pattern: &str, #[case] code: &str) {
    let error = call("regex", json!({ "pattern": pattern, "value": "aa" })).unwrap_err();

    assert_that!(error.category()).is_equal_to(Category::Expression);
    assert_that!(error.to_string()).starts_with(code);
}

#[test]
fn regex_pattern_not_a_string_is_an_expression_error() {
    let error = call("regex", json!({ "pattern": 1, "value": "1" })).unwrap_err();
    assert_that!(error.code()).is_equal_to(ErrorCode::ExpressionError);
}
