//! The validators `required`, `length`, `numeric`, `email` and `regex`.
//!
//! Each returns `{"valid": bool}`. A value that cannot be checked (an unparseable number, say)
//! is invalid, not an error; a malformed bound or pattern is an error.

use a2ui_runtime::{Args, CallContext, DialectRegex, Error, coerce};
use serde_json::{Value, json};

use super::arg;

/// `{"valid": valid}`, a validation result.
fn result(valid: bool) -> Value {
    json!({ "valid": valid })
}

/// Bound `name` of validator `function`: `None` when absent or `null`.
fn bound(args: &Args, name: &str, function: &str) -> Result<Option<f64>, Error> {
    match arg(args, name) {
        Value::Null => Ok(None),
        given => coerce::number(given)
            .map(Some)
            .ok_or_else(|| Error::function(function, &format!("'{name}' is not a number"))),
    }
}

/// The inclusive `min` and `max` bounds of `function`'s `args`, checked before any value.
fn bounds(args: &Args, function: &str) -> Result<(Option<f64>, Option<f64>), Error> {
    Ok((bound(args, "min", function)?, bound(args, "max", function)?))
}

/// Whether `n` lies within the inclusive `(min, max)` bounds.
fn within(n: f64, (min, max): (Option<f64>, Option<f64>)) -> bool {
    min.is_none_or(|min| n >= min) && max.is_none_or(|max| n <= max)
}

/// `required(value)`: invalid for `null`, `""` and `[]`, valid otherwise (`{}` included).
pub(super) fn required(args: &Args, _context: &mut CallContext<'_>) -> Result<Value, Error> {
    let empty = match arg(args, "value") {
        Value::Null => true,
        Value::String(text) => text.is_empty(),
        Value::Array(items) => items.is_empty(),
        Value::Bool(_) | Value::Number(_) | Value::Object(_) => false,
    };
    Ok(result(!empty))
}

/// `length(value, min?, max?)`: arrays count items, anything else the code points of its text.
///
/// `null` reads as `""`, so its length is 0.
pub(super) fn length(args: &Args, _context: &mut CallContext<'_>) -> Result<Value, Error> {
    let bounds = bounds(args, "length")?;
    let count = match arg(args, "value") {
        Value::Array(items) => items.len(),
        other => coerce::text(other).chars().count(),
    };
    #[expect(
        clippy::cast_precision_loss,
        reason = "a length within memory is far below 2^53, so the conversion is exact"
    )]
    let count = count as f64;
    Ok(result(within(count, bounds)))
}

/// `numeric(value, min?, max?)`: the value as a finite number, within the inclusive bounds.
pub(super) fn numeric(args: &Args, _context: &mut CallContext<'_>) -> Result<Value, Error> {
    let bounds = bounds(args, "numeric")?;
    let valid = coerce::number(arg(args, "value")).is_some_and(|n| within(n, bounds));
    Ok(result(valid))
}

/// `email(value)`: one `@`, a printable ASCII local part, and a domain whose last label has at
/// least two letters.
pub(super) fn email(args: &Args, _context: &mut CallContext<'_>) -> Result<Value, Error> {
    Ok(result(is_email(&coerce::text(arg(args, "value")))))
}

/// Whether `text` is `local@domain` as [`email`] reads it.
///
/// The domain is dot-separated labels of `[A-Za-z0-9-]`, none empty or starting or ending with
/// `-`; the local part is printable ASCII.
fn is_email(text: &str) -> bool {
    let Some((local, domain)) = text.split_once('@') else {
        return false;
    };
    let Some((_, top)) = domain.rsplit_once('.') else {
        return false;
    };
    let label = |label: &str| {
        !label.is_empty()
            && !label.starts_with('-')
            && !label.ends_with('-')
            && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
    };
    !local.is_empty()
        && local.chars().all(|c| c.is_ascii_graphic())
        && domain.split('.').all(label)
        && top.len() >= 2
        && top.chars().all(|c| c.is_ascii_alphabetic())
}

/// `regex(value, pattern)`: whether `pattern`, in the A2UI dialect, matches somewhere in `value`.
///
/// A pattern the dialect refuses returns its error (`UNSUPPORTED_SYNTAX`, `NESTED_QUANTIFIER`, …).
pub(super) fn regex(args: &Args, _context: &mut CallContext<'_>) -> Result<Value, Error> {
    let Value::String(pattern) = arg(args, "pattern") else {
        return Err(Error::function("regex", "'pattern' is not a string"));
    };
    let pattern = DialectRegex::new(pattern)?;
    Ok(result(pattern.is_match(&coerce::text(arg(args, "value")))))
}

#[cfg(test)]
#[path = "validators_tests.rs"]
mod tests;
