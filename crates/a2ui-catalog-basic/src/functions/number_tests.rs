use a2ui_runtime::{Category, ErrorCode};
use rstest::rstest;
use serde_json::{Value, json};
use speculoos::prelude::*;

use super::super::test_support::call;

/// `1e21` written out, as `formatNumber` must print it: digits, no exponent.
const TEN_TO_21: &str = "1,000,000,000,000,000,000,000";

#[rstest]
#[case::decimals_and_grouping(
    json!({"value": 1234.567, "decimals": 2, "grouping": true}),
    "1,234.57"
)]
#[case::no_decimals_no_grouping(
    json!({"value": 1234.567, "decimals": 0, "grouping": false}),
    "1235"
)]
#[case::half_rounds_up(json!({"value": 0.5, "decimals": 0}), "1")]
#[case::negative_half_rounds_away(json!({"value": -0.5, "decimals": 0}), "-1")]
#[case::negative_zero(json!({"value": -0.0, "decimals": 0}), "0")]
#[case::rounds_to_zero_unsigned(json!({"value": -0.4, "decimals": 0}), "0")]
#[case::huge_without_exponent(json!({"value": 1e21, "decimals": 0}), TEN_TO_21)]
#[case::decimal_tie_as_written(json!({"value": 1.005, "decimals": 2}), "1.01")]
#[case::carry_into_new_digit(json!({"value": 999.996, "decimals": 2}), "1,000.00")]
#[case::pads_fraction(json!({"value": 3, "decimals": 3}), "3.000")]
#[case::default_decimals_zero(json!({"value": 42, "grouping": false}), "42")]
#[case::grouping_defaults_on(json!({"value": 1234567}), "1,234,567")]
#[case::numeric_string(json!({"value": " 1234.5 ", "decimals": 1}), "1,234.5")]
#[case::decimals_clamped_high(json!({"value": 0.1, "decimals": 99}), "0.10000000000000000000")]
#[case::decimals_clamped_low(json!({"value": 2.5, "decimals": -3}), "3")]
#[case::decimals_truncated(json!({"value": 1.25, "decimals": 1.9}), "1.3")]
#[case::small_fraction(json!({"value": 1e-7, "decimals": 7}), "0.0000001")]
#[case::failing_validation_turns_grouping_off(
    json!({"value": 1234, "decimals": 0,
        "grouping": {"@call": "required", "args": {"value": ""}}}),
    "1234"
)]
#[case::passing_validation_keeps_grouping(
    json!({"value": 1234, "decimals": 0,
        "grouping": {"@call": "required", "args": {"value": "x"}}}),
    "1,234"
)]
fn format_number_rows(#[case] args: Value, #[case] expected: &str) {
    assert_that!(call("formatNumber", args).unwrap()).is_equal_to(json!(expected));
}

#[rstest]
#[case::text(json!({"value": "abc"}))]
#[case::null(json!({}))]
fn format_number_without_number_is_an_expression_error(#[case] args: Value) {
    let error = call("formatNumber", args).unwrap_err();

    assert_that!(error.category()).is_equal_to(Category::Expression);
    assert_that!(error.code()).is_equal_to(ErrorCode::ExpressionError);
}

#[rstest]
#[case::text(json!({"value": 1, "decimals": "two"}))]
#[case::boolean(json!({"value": 1, "decimals": true}))]
fn format_number_decimals_not_a_number_is_an_expression_error(#[case] args: Value) {
    let error = call("formatNumber", args).unwrap_err();

    assert_that!(error.category()).is_equal_to(Category::Expression);
    assert_that!(error.code()).is_equal_to(ErrorCode::ExpressionError);
}

#[rstest]
#[case::usd(json!({"value": 1234.5, "currency": "USD"}), "$1,234.50")]
#[case::euro_negative(json!({"value": -3, "currency": "EUR"}), "-€3.00")]
#[case::other_code(json!({"value": 5, "currency": "CHF"}), "CHF 5.00")]
#[case::explicit_decimals(json!({"value": 99.99, "currency": "USD", "decimals": 2}), "$99.99")]
#[case::yen_default_decimals(json!({"value": 1234.5, "currency": "JPY"}), "¥1,235")]
#[case::without_grouping(
    json!({"value": 1234.5, "currency": "GBP", "grouping": false}),
    "£1234.50"
)]
#[case::rounds_to_zero_unsigned(json!({"value": -0.001, "currency": "USD"}), "$0.00")]
fn format_currency_rows(#[case] args: Value, #[case] expected: &str) {
    assert_that!(call("formatCurrency", args).unwrap()).is_equal_to(json!(expected));
}

#[rstest]
#[case::no_currency(json!({"value": 1}))]
#[case::currency_not_text(json!({"value": 1, "currency": 840}))]
#[case::no_value(json!({"currency": "USD"}))]
fn format_currency_without_amount_or_code_is_an_expression_error(#[case] args: Value) {
    let error = call("formatCurrency", args).unwrap_err();

    assert_that!(error.category()).is_equal_to(Category::Expression);
    assert_that!(error.code()).is_equal_to(ErrorCode::ExpressionError);
}
