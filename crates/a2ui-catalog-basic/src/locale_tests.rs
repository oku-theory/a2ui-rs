use rstest::rstest;
use speculoos::prelude::*;

use time::{Month, Weekday};

use super::*;

#[rstest]
#[case::one(1.0, PluralCategory::One)]
#[case::zero(0.0, PluralCategory::Other)]
#[case::two(2.0, PluralCategory::Other)]
#[case::fraction(1.5, PluralCategory::Other)]
fn en_us_plural_is_one_only_for_one(#[case] n: f64, #[case] expected: PluralCategory) {
    assert_that!(EnUs.plural(n)).is_equal_to(expected);
}

#[rstest]
#[case::usd("USD", "$")]
#[case::eur("EUR", "€")]
#[case::gbp("GBP", "£")]
#[case::jpy("JPY", "¥")]
#[case::other_code("CHF", "CHF ")]
fn en_us_currency_symbols_are_prefixed(#[case] code: &str, #[case] symbol: &str) {
    let expected = CurrencyFormat {
        symbol: symbol.to_owned(),
        prefix: true,
    };
    assert_that!(EnUs.currency(code)).is_equal_to(expected);
}

#[rstest]
#[case::no_currency(None, 0)]
#[case::yen(Some("JPY"), 0)]
#[case::dollar(Some("USD"), 2)]
fn en_us_default_decimals_follow_currency(#[case] code: Option<&str>, #[case] expected: u8) {
    assert_that!(EnUs.default_decimals(code)).is_equal_to(expected);
}

#[test]
fn en_us_separators_are_dot_and_comma() {
    assert_that!((EnUs.decimal_separator(), EnUs.group_separator())).is_equal_to(('.', ','));
}

#[test]
fn en_us_am_pm_markers_are_upper_case() {
    assert_that!((EnUs.am_pm(false), EnUs.am_pm(true))).is_equal_to(("AM", "PM"));
}

#[rstest]
#[case::january_long(Month::January, true, "January")]
#[case::january_short(Month::January, false, "Jan")]
#[case::may_short(Month::May, false, "May")]
#[case::september_short(Month::September, false, "Sep")]
#[case::december_long(Month::December, true, "December")]
fn en_us_month_names_follow_cldr(#[case] month: Month, #[case] long: bool, #[case] name: &str) {
    assert_that!(EnUs.month(month, long)).is_equal_to(name);
}

#[rstest]
#[case::monday_long(Weekday::Monday, true, "Monday")]
#[case::tuesday_short(Weekday::Tuesday, false, "Tue")]
#[case::sunday_short(Weekday::Sunday, false, "Sun")]
fn en_us_weekday_names_follow_cldr(#[case] day: Weekday, #[case] long: bool, #[case] name: &str) {
    assert_that!(EnUs.weekday(day, long)).is_equal_to(name);
}
