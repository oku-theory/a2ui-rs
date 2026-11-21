use rstest::rstest;
use serde_json::{Value, json};
use speculoos::prelude::*;

use super::super::test_support::call;
use super::four_digit_year;

/// `formatDate(value, format)`, which never fails.
fn format_date(value: Value, format: &str) -> Value {
    call("formatDate", json!({"value": value, "format": format})).unwrap()
}

#[rstest]
#[case::date_pattern("2026-09-04T12:00:00Z", "yyyy-MM-dd", "2026-09-04")]
#[case::catalog_month_day_year("2026-01-16T14:30:00Z", "MMM dd, yyyy", "Jan 16, 2026")]
#[case::catalog_military("2026-01-16T14:30:00Z", "HH:mm", "14:30")]
#[case::catalog_twelve_hour("2026-01-16T14:30:00Z", "h:mm a", "2:30 PM")]
#[case::catalog_weekday_month("2026-01-16T14:30:00Z", "EEEE, d MMMM", "Friday, 16 January")]
#[case::two_digit_year("2005-03-07T09:05:08Z", "yy", "05")]
#[case::short_numbers("2005-03-07T09:05:08Z", "M/d H", "3/7 9")]
#[case::padded_numbers("2005-03-07T09:05:08Z", "MM/dd HH:mm:ss", "03/07 09:05:08")]
#[case::short_weekday("2005-03-07T09:05:08Z", "E", "Mon")]
#[case::padded_twelve_hour("2005-03-07T21:05:08Z", "hh a", "09 PM")]
#[case::midnight_is_twelve_am("2026-09-04T00:15:00Z", "h:mm a", "12:15 AM")]
#[case::noon_is_twelve_pm("2026-09-04T12:00:00Z", "h a", "12 PM")]
#[case::utc_time_as_written("2026-09-04T23:30:00Z", "yyyy-MM-dd HH:mm", "2026-09-04 23:30")]
#[case::offset_keeps_wall_clock(
    "2026-09-04T23:30:00+05:45",
    "yyyy-MM-dd HH:mm",
    "2026-09-04 23:30"
)]
#[case::negative_offset_kept("2026-09-04T23:30:00-05:00", "yyyy-MM-dd HH:mm", "2026-09-04 23:30")]
#[case::offset_less_as_written("2026-09-04T23:30:00", "yyyy-MM-dd HH:mm", "2026-09-04 23:30")]
#[case::offset_less_fraction("2026-09-04T23:30:00.250", "HH:mm:ss", "23:30:00")]
#[case::date_only_midnight("2026-09-04", "yyyy-MM-dd HH:mm", "2026-09-04 00:00")]
#[case::quoted_literal("2026-09-04T23:30:00Z", "yyyy-MM-dd 'at' HH:mm", "2026-09-04 at 23:30")]
#[case::doubled_quote("2026-09-04T09:00:00Z", "h 'o''clock'", "9 o'clock")]
#[case::lone_doubled_quote("2026-09-04T09:00:00Z", "H''", "9'")]
#[case::unterminated_quote("2026-09-04T09:00:00Z", "H 'h", "9 h")]
#[case::other_letters_copied("2026-09-04T09:00:00Z", "yyyy T Q m s", "2026 T Q m s")]
#[case::one_y_is_full_year("2005-03-07T09:05:08Z", "y", "2005")]
#[case::three_y_is_full_year("2005-03-07T09:05:08Z", "yyy", "2005")]
#[case::two_e_is_short_weekday("2005-03-07T09:05:08Z", "EE", "Mon")]
#[case::three_e_is_short_weekday("2005-03-07T09:05:08Z", "EEE", "Mon")]
#[case::longest_token_first("2026-09-04T09:00:00Z", "yyyyyy", "202626")]
#[case::iso_in_utc("2026-08-26T12:00:00.000Z", "ISO", "2026-08-26T12:00:00.000Z")]
#[case::iso_converts_offset("2026-09-04T23:30:00+05:45", "ISO", "2026-09-04T17:45:00.000Z")]
#[case::iso_crosses_midnight("2026-09-04T23:30:00-05:00", "ISO", "2026-09-05T04:30:00.000Z")]
#[case::iso_milliseconds_truncated("2026-09-04T23:30:00.1239Z", "ISO", "2026-09-04T23:30:00.123Z")]
#[case::iso_offset_less_as_utc("2026-09-04T23:30:00", "ISO", "2026-09-04T23:30:00.000Z")]
#[case::iso_date_only("2026-09-04", "ISO", "2026-09-04T00:00:00.000Z")]
#[case::iso_year_before_zero("0000-01-01T00:30:00+01:00", "ISO", "")]
#[case::pattern_year_zero_kept("0000-01-01T00:30:00+01:00", "yyyy-MM-dd", "0000-01-01")]
#[case::iso_year_past_9999("9999-12-31T23:30:00-01:00", "ISO", "")]
#[case::impossible_day("2026-02-30", "yyyy-MM-dd", "")]
#[case::impossible_month("2026-13-01", "yyyy-MM-dd", "")]
#[case::impossible_hour("2026-09-04T24:00:00Z", "yyyy-MM-dd", "")]
#[case::two_digit_year_input("26-09-04", "yyyy-MM-dd", "")]
#[case::signed_year_input("+2026-09-04", "yyyy-MM-dd", "")]
#[case::trailing_text("2026-09-04 and more", "yyyy-MM-dd", "")]
#[case::not_a_date("tomorrow", "yyyy-MM-dd", "")]
#[case::empty("", "yyyy-MM-dd", "")]
fn format_date_rows(#[case] value: &str, #[case] format: &str, #[case] expected: &str) {
    assert_that!(format_date(json!(value), format)).is_equal_to(json!(expected));
}

#[rstest]
#[case::number(json!(1_788_000_000_000_u64))]
#[case::null(json!(null))]
#[case::object(json!({"year": 2026}))]
fn format_date_of_non_text_is_empty(#[case] value: Value) {
    assert_that!(format_date(value, "yyyy")).is_equal_to(json!(""));
}

#[rstest]
#[case::zero(0, true)]
#[case::last(9999, true)]
#[case::past_last(10_000, false)]
#[case::before_zero(-1, false)]
#[case::far_past(-999_999, false)]
fn four_digit_year_accepts_only_zero_to_9999(#[case] year: i32, #[case] expected: bool) {
    assert_that!(four_digit_year(year)).is_equal_to(expected);
}
