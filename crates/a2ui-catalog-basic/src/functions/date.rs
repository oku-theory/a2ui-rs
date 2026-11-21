//! The date function `formatDate`: `time` parses the timestamp, the TR35 printer is ours.
//!
//! The result depends on the arguments alone: no clock, no host time zone.

use a2ui_runtime::{Args, coerce};
use serde_json::Value;
use time::format_description::well_known::Rfc3339;
use time::macros::format_description;
use time::{Date, OffsetDateTime, PrimitiveDateTime, UtcOffset};

use super::arg;
use crate::Locale;

/// A field a pattern token prints.
#[derive(Debug, Clone, Copy)]
enum Field {
    /// `yyyy`, `yyy` and `y`: the year on four digits.
    Year,
    /// `yy`: the last two digits of the year.
    ShortYear,
    /// `MMMM`: the month's full name.
    MonthName,
    /// `MMM`: the month's abbreviated name.
    ShortMonthName,
    /// `MM`: the month on two digits.
    PaddedMonth,
    /// `M`: the month number.
    Month,
    /// `dd`: the day on two digits.
    PaddedDay,
    /// `d`: the day number.
    Day,
    /// `EEEE`: the weekday's full name.
    WeekdayName,
    /// `EEE`, `EE` and `E`: the weekday's abbreviated name.
    ShortWeekdayName,
    /// `HH`: the 24-hour hour on two digits.
    PaddedHour,
    /// `H`: the 24-hour hour.
    Hour,
    /// `hh`: the 12-hour hour on two digits.
    PaddedHour12,
    /// `h`: the 12-hour hour.
    Hour12,
    /// `mm`: the minute on two digits.
    Minute,
    /// `ss`: the second on two digits.
    Second,
    /// `a`: the AM/PM marker.
    Period,
}

/// The pattern tokens, longest first so that `yyyy` is never read as `yy` `yy`.
const TOKENS: [(&str, Field); 21] = [
    ("yyyy", Field::Year),
    ("yyy", Field::Year),
    ("yy", Field::ShortYear),
    ("y", Field::Year),
    ("MMMM", Field::MonthName),
    ("MMM", Field::ShortMonthName),
    ("MM", Field::PaddedMonth),
    ("M", Field::Month),
    ("dd", Field::PaddedDay),
    ("d", Field::Day),
    ("EEEE", Field::WeekdayName),
    ("EEE", Field::ShortWeekdayName),
    ("EE", Field::ShortWeekdayName),
    ("E", Field::ShortWeekdayName),
    ("HH", Field::PaddedHour),
    ("H", Field::Hour),
    ("hh", Field::PaddedHour12),
    ("h", Field::Hour12),
    ("mm", Field::Minute),
    ("ss", Field::Second),
    ("a", Field::Period),
];

/// `formatDate(value, format)`: the timestamp written with a TR35 pattern, or `"ISO"`.
///
/// A timestamp is RFC 3339, `YYYY-MM-DDTHH:mm:ss[.fff]` without offset (read as written), or
/// `YYYY-MM-DD` (midnight). The pattern prints the wall clock of the written offset; `"ISO"`
/// prints `YYYY-MM-DDTHH:mm:ss.sssZ` in UTC. An unreadable or impossible timestamp, or one
/// whose year, as read or once in UTC, lies outside 0-9999, gives `""`; so does a `value`
/// that is not a string. A `format` that is absent or not a string reads through
/// `coerce::text`: `null` as `""`, which prints nothing, and `5` as the pattern `"5"`.
pub(super) fn format_date(args: &Args, locale: &dyn Locale) -> Value {
    let format = coerce::text(arg(args, "format"));
    let text = match arg(args, "value") {
        Value::String(value) => parse(value).filter(in_range).and_then(|moment| {
            if format == "ISO" {
                iso(moment)
            } else {
                Some(pattern(moment, &format, locale))
            }
        }),
        _ => None,
    };
    Value::String(text.unwrap_or_default())
}

/// `value` read as one of the three accepted forms; `None` if it is none of them.
fn parse(value: &str) -> Option<OffsetDateTime> {
    if !value.starts_with(|first: char| first.is_ascii_digit()) {
        return None;
    }
    if let Ok(moment) = OffsetDateTime::parse(value, &Rfc3339) {
        return Some(moment);
    }
    let local = format_description!(
        version = 2,
        "[year]-[month]-[day]T[hour]:[minute]:[second][optional [.[subsecond]]]"
    );
    if let Ok(moment) = PrimitiveDateTime::parse(value, local) {
        return Some(moment.assume_utc());
    }
    let day = format_description!(version = 2, "[year]-[month]-[day]");
    Date::parse(value, day)
        .ok()
        .map(|date| date.midnight().assume_utc())
}

/// Whether `moment`'s year has four digits, 0-9999, whatever range `time` was built with.
fn in_range(moment: &OffsetDateTime) -> bool {
    four_digit_year(moment.year())
}

/// Whether `year` lies within 0-9999, the years `formatDate` writes.
fn four_digit_year(year: i32) -> bool {
    (0..=9999).contains(&year)
}

/// `moment` in UTC as `YYYY-MM-DDTHH:mm:ss.sssZ`, milliseconds truncated.
///
/// `None` when the UTC year falls outside 0-9999.
fn iso(moment: OffsetDateTime) -> Option<String> {
    let utc = moment.checked_to_offset(UtcOffset::UTC).filter(in_range)?;
    Some(format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",
        utc.year(),
        u8::from(utc.month()),
        utc.day(),
        utc.hour(),
        utc.minute(),
        utc.second(),
        utc.millisecond()
    ))
}

/// `moment`'s wall clock written with the TR35 `format`.
///
/// `'…'` quotes literal text (an unterminated quote runs to the end) and `''` is a quote;
/// any character outside a token is copied.
fn pattern(moment: OffsetDateTime, format: &str, locale: &dyn Locale) -> String {
    let mut out = String::with_capacity(format.len() * 2);
    let mut rest = format;
    while let Some(first) = rest.chars().next() {
        if let Some(after) = rest.strip_prefix("''") {
            out.push('\'');
            rest = after;
        } else if let Some(quoted) = rest.strip_prefix('\'') {
            rest = literal(quoted, &mut out);
        } else if let Some((token, field)) = TOKENS
            .into_iter()
            .find(|(token, _)| rest.starts_with(token))
        {
            out.push_str(&write(moment, field, locale));
            rest = &rest[token.len()..];
        } else {
            out.push(first);
            rest = &rest[first.len_utf8()..];
        }
    }
    out
}

/// Copies the quoted text at the start of `quoted` into `out`; the text after its closing quote.
fn literal<'a>(quoted: &'a str, out: &mut String) -> &'a str {
    let mut rest = quoted;
    loop {
        match rest.split_once('\'') {
            None => {
                out.push_str(rest);
                return "";
            }
            Some((text, after)) => {
                out.push_str(text);
                match after.strip_prefix('\'') {
                    Some(after_doubled) => {
                        out.push('\'');
                        rest = after_doubled;
                    }
                    None => return after,
                }
            }
        }
    }
}

/// `field` of `moment`, as its token prints it.
fn write(moment: OffsetDateTime, field: Field, locale: &dyn Locale) -> String {
    let hour12 = match moment.hour() % 12 {
        0 => 12,
        hour => hour,
    };
    match field {
        Field::Year => format!("{:04}", moment.year()),
        Field::ShortYear => format!("{:02}", moment.year().rem_euclid(100)),
        Field::MonthName => locale.month(moment.month(), true).to_owned(),
        Field::ShortMonthName => locale.month(moment.month(), false).to_owned(),
        Field::PaddedMonth => format!("{:02}", u8::from(moment.month())),
        Field::Month => u8::from(moment.month()).to_string(),
        Field::PaddedDay => format!("{:02}", moment.day()),
        Field::Day => moment.day().to_string(),
        Field::WeekdayName => locale.weekday(moment.weekday(), true).to_owned(),
        Field::ShortWeekdayName => locale.weekday(moment.weekday(), false).to_owned(),
        Field::PaddedHour => format!("{:02}", moment.hour()),
        Field::Hour => moment.hour().to_string(),
        Field::PaddedHour12 => format!("{hour12:02}"),
        Field::Hour12 => hour12.to_string(),
        Field::Minute => format!("{:02}", moment.minute()),
        Field::Second => format!("{:02}", moment.second()),
        Field::Period => locale.am_pm(moment.hour() >= 12).to_owned(),
    }
}

#[cfg(test)]
#[path = "date_tests.rs"]
mod tests;
