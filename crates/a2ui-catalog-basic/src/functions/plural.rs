//! The plural function `pluralize`.

use a2ui_runtime::{Args, Error, coerce};
use serde_json::Value;

use super::arg;
use crate::{Locale, PluralCategory};

/// `pluralize(value, zero?, one?, two?, few?, many?, other)`: the string for `value`'s count.
///
/// An explicit `zero`, `one` or `two` wins when the count is exactly 0, 1 or 2; else the
/// string of the locale's category, if given; else `other`. A string is returned as given,
/// empty or holding `{n}`. An argument that is absent or `null` is not given; `other`, which
/// the catalog requires, is checked first, whatever the count.
pub(super) fn pluralize(args: &Args, locale: &dyn Locale) -> Result<Value, Error> {
    let name = "pluralize";
    let other = given(args, "other").ok_or_else(|| Error::function(name, "'other' is missing"))?;
    let n = coerce::number(arg(args, "value"))
        .ok_or_else(|| Error::function(name, "'value' is not a number"))?;
    let exact = match n {
        0.0 => Some("zero"),
        1.0 => Some("one"),
        2.0 => Some("two"),
        _ => None,
    };
    let chosen = exact
        .and_then(|key| given(args, key))
        .or_else(|| given(args, category_key(locale.plural(n))))
        .unwrap_or(other);
    Ok(Value::String(chosen))
}

/// Argument `key` as text, unless it is absent or `null`.
fn given(args: &Args, key: &str) -> Option<String> {
    match arg(args, key) {
        Value::Null => None,
        value => Some(coerce::text(value)),
    }
}

/// The argument naming `category`.
fn category_key(category: PluralCategory) -> &'static str {
    match category {
        PluralCategory::Zero => "zero",
        PluralCategory::One => "one",
        PluralCategory::Two => "two",
        PluralCategory::Few => "few",
        PluralCategory::Many => "many",
        PluralCategory::Other => "other",
    }
}

#[cfg(test)]
#[path = "plural_tests.rs"]
mod tests;
