use std::sync::Arc;

use a2ui_runtime::{Category, ErrorCode};
use rstest::rstest;
use serde_json::{Value, json};
use speculoos::prelude::*;

use super::super::test_support::{call, evaluate_with, first_error};
use crate::{BasicOptions, CurrencyFormat, EnUs, Locale, PluralCategory};

#[rstest]
#[case::explicit_zero(json!({"value": 0, "zero": "none", "one": "one", "other": "many"}), "none")]
#[case::zero_without_zero(json!({"value": 0, "one": "one", "other": "many"}), "many")]
#[case::one(json!({"value": 1, "zero": "none", "one": "1 item", "other": "{n} items"}), "1 item")]
#[case::one_from_locale_string(json!({"value": "1", "one": "1 item", "other": "x"}), "1 item")]
#[case::empty_string_kept(json!({"value": 1, "one": "", "other": "default"}), "")]
#[case::explicit_two(json!({"value": 2, "two": "a couple", "other": "items"}), "a couple")]
#[case::two_without_two(json!({"value": 2, "one": "one", "other": "items"}), "items")]
#[case::other(json!({"value": 5, "one": "1 item", "other": "Many items"}), "Many items")]
#[case::placeholder_left_as_is(json!({"value": 7, "other": "{n} items"}), "{n} items")]
#[case::null_category_ignored(json!({"value": 0, "zero": null, "other": "items"}), "items")]
fn pluralize_rows(#[case] args: Value, #[case] expected: &str) {
    assert_that!(call("pluralize", args).unwrap()).is_equal_to(json!(expected));
}

#[rstest]
#[case::no_number(json!({"value": "many", "other": "x"}))]
#[case::no_other(json!({"value": 5, "one": "x"}))]
#[case::no_other_though_one_matches(json!({"value": 1, "one": "x"}))]
fn pluralize_without_number_or_other_is_an_expression_error(#[case] args: Value) {
    let error = call("pluralize", args).unwrap_err();

    assert_that!(error.category()).is_equal_to(Category::Expression);
    assert_that!(error.code()).is_equal_to(ErrorCode::ExpressionError);
}

/// English, except that every count is in the `few` category.
#[derive(Debug)]
struct AllFew;

impl Locale for AllFew {
    fn plural(&self, _n: f64) -> PluralCategory {
        PluralCategory::Few
    }
    fn decimal_separator(&self) -> char {
        EnUs.decimal_separator()
    }
    fn group_separator(&self) -> char {
        EnUs.group_separator()
    }
    fn currency(&self, code: &str) -> CurrencyFormat {
        EnUs.currency(code)
    }
    fn default_decimals(&self, currency: Option<&str>) -> u8 {
        EnUs.default_decimals(currency)
    }
    fn month(&self, month: time::Month, long: bool) -> &str {
        EnUs.month(month, long)
    }
    fn weekday(&self, day: time::Weekday, long: bool) -> &str {
        EnUs.weekday(day, long)
    }
    fn am_pm(&self, pm: bool) -> &str {
        EnUs.am_pm(pm)
    }
}

#[rstest]
#[case::category_given(json!({"value": 5, "few": "a few", "other": "lots"}), "a few")]
#[case::category_absent(json!({"value": 5, "many": "many", "other": "lots"}), "lots")]
#[case::explicit_one_first(json!({"value": 1, "one": "one", "few": "a few", "other": "x"}), "one")]
fn pluralize_follows_locale_category(#[case] args: Value, #[case] expected: &str) {
    let options = BasicOptions {
        locale: Arc::new(AllFew),
        ..BasicOptions::default()
    };

    let result = first_error(evaluate_with(options, "pluralize", args));

    assert_that!(result.unwrap()).is_equal_to(json!(expected));
}
