//! The number functions `formatNumber` and `formatCurrency`.
//!
//! A number is rounded as written: the shortest decimal that reads back as the `f64` (what
//! `f64`'s `Display` prints, never with an exponent) is rounded half away from zero, so `1.005`
//! to two places is `1.01`. A result that rounds to zero carries no minus sign.

use a2ui_runtime::{Args, Error, coerce};
use serde_json::Value;

use super::arg;
use crate::Locale;

/// The most fraction digits a call may ask for; more are clamped to it.
const MAX_DECIMALS: f64 = 20.0;

/// `formatNumber(value, decimals?, grouping = true)`: `value` rounded, grouped by the locale.
pub(super) fn format_number(args: &Args, locale: &dyn Locale) -> Result<Value, Error> {
    let name = "formatNumber";
    let value = amount(args, name)?;
    let decimals = decimals(args, name, locale.default_decimals(None))?;
    let (negative, digits) = written(value, decimals, grouping(args), locale);
    let sign = if negative { "-" } else { "" };
    Ok(Value::String(format!("{sign}{digits}")))
}

/// `formatCurrency(value, currency, decimals?, grouping = true)`: the amount with its symbol.
///
/// A minus sign comes before the symbol: `-€3.00`.
pub(super) fn format_currency(args: &Args, locale: &dyn Locale) -> Result<Value, Error> {
    let name = "formatCurrency";
    let value = amount(args, name)?;
    let Value::String(code) = arg(args, "currency") else {
        return Err(Error::function(name, "'currency' is not a string"));
    };
    let decimals = decimals(args, name, locale.default_decimals(Some(code)))?;
    let (negative, digits) = written(value, decimals, grouping(args), locale);
    let sign = if negative { "-" } else { "" };
    let format = locale.currency(code);
    let text = if format.prefix {
        format!("{sign}{}{digits}", format.symbol)
    } else {
        format!("{sign}{digits}{}", format.symbol)
    };
    Ok(Value::String(text))
}

/// The `value` argument of `function` as a finite number.
fn amount(args: &Args, function: &str) -> Result<f64, Error> {
    coerce::number(arg(args, "value"))
        .ok_or_else(|| Error::function(function, "'value' is not a number"))
}

/// The `decimals` argument, truncated and clamped to 0-20; `default` when absent or `null`.
fn decimals(args: &Args, function: &str, default: u8) -> Result<usize, Error> {
    let given = match arg(args, "decimals") {
        Value::Null => return Ok(usize::from(default)),
        given => coerce::number(given)
            .ok_or_else(|| Error::function(function, "'decimals' is not a number"))?,
    };
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "the value is a whole number clamped to 0-20, so the conversion is exact"
    )]
    let decimals = given.trunc().clamp(0.0, MAX_DECIMALS) as usize;
    Ok(decimals)
}

/// The `grouping` argument by `coerce::boolean`; on when absent or `null`.
fn grouping(args: &Args) -> bool {
    match arg(args, "grouping") {
        Value::Null => true,
        given => coerce::boolean(given),
    }
}

/// `value`'s magnitude with `decimals` fraction digits, and whether it is below zero once
/// rounded.
fn written(value: f64, decimals: usize, grouping: bool, locale: &dyn Locale) -> (bool, String) {
    let shortest = value.abs().to_string();
    let (integer, fraction) = shortest.split_once('.').unwrap_or((&shortest, ""));
    let mut digits: Vec<u8> = integer.bytes().chain(fraction.bytes()).collect();
    let integer_len = integer.len();
    let kept = integer_len + decimals;
    let round_up = digits.get(kept).is_some_and(|digit| *digit >= b'5');
    digits.resize(kept, b'0');
    let carried = round_up && increment(&mut digits);
    let integer_len = if carried {
        digits.insert(0, b'1');
        integer_len + 1
    } else {
        integer_len
    };
    let negative = value < 0.0 && digits.iter().any(|digit| *digit != b'0');
    let (integer, fraction) = digits.split_at(integer_len);
    let mut text = group(integer, grouping.then(|| locale.group_separator()));
    if !fraction.is_empty() {
        text.push(locale.decimal_separator());
        text.extend(fraction.iter().map(|digit| char::from(*digit)));
    }
    (negative, text)
}

/// Adds one to the last of the ASCII `digits`; whether it carried out of the first.
fn increment(digits: &mut [u8]) -> bool {
    for digit in digits.iter_mut().rev() {
        if *digit == b'9' {
            *digit = b'0';
        } else {
            *digit += 1;
            return false;
        }
    }
    true
}

/// The ASCII `digits` with `separator`, if any, between groups of three from the right.
fn group(digits: &[u8], separator: Option<char>) -> String {
    let mut text = String::with_capacity(digits.len() * 2);
    for (index, digit) in digits.iter().enumerate() {
        let left = digits.len() - index;
        if index > 0
            && left.is_multiple_of(3)
            && let Some(separator) = separator
        {
            text.push(separator);
        }
        text.push(char::from(*digit));
    }
    text
}

#[cfg(test)]
#[path = "number_tests.rs"]
mod tests;
