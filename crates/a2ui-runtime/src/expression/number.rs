//! Number literals inside expressions.

use serde_json::{Number, Value};

use super::syntax;
use crate::error::Error;

const MAX_SAFE_INTEGER: f64 = 9_007_199_254_740_991.0;

/// Whether `token` must be read as a number rather than as a path.
pub(super) fn looks_numeric(token: &str) -> bool {
    let unsigned = token.strip_prefix(['+', '-']).unwrap_or(token);
    let mut chars = unsigned.chars();
    match chars.next() {
        Some(c) if c.is_ascii_digit() => true,
        Some('.') => chars.next().is_some_and(|c| c.is_ascii_digit()),
        _ => false,
    }
}

/// Byte length of the literal at the start of a numeric `token`.
///
/// Scans a sign, digits and dots, then an exponent (`e`, sign, digits) even
/// when malformed, so `1e+` is rejected as a literal while `-1x` stops before `x`.
pub(super) fn literal_len(token: &str) -> usize {
    let bytes = token.as_bytes();
    let at = |end: usize, accept: fn(u8) -> bool| bytes.get(end).copied().is_some_and(accept);
    let mut end = usize::from(at(0, |b| matches!(b, b'+' | b'-')));
    while at(end, |b| b.is_ascii_digit() || b == b'.') {
        end += 1;
    }
    if at(end, |b| matches!(b, b'e' | b'E')) {
        end += 1;
        end += usize::from(at(end, |b| matches!(b, b'+' | b'-')));
        while at(end, |b| b.is_ascii_digit()) {
            end += 1;
        }
    }
    end
}

/// Parses a numeric token; integral values become JSON integers.
pub(super) fn parse(token: &str, offset: usize) -> Result<Value, Error> {
    if !well_formed(token) {
        return Err(syntax(offset, "Invalid number literal"));
    }
    let value: f64 = token
        .parse()
        .map_err(|_| syntax(offset, "Invalid number literal"))?;
    if !value.is_finite() {
        return Err(syntax(offset, "Number literal out of range"));
    }
    let value = value + 0.0; // -0.0 + 0.0 is 0.0
    if value.fract() == 0.0 && value.abs() <= MAX_SAFE_INTEGER {
        // Integral and within ±2^53, so the cast is exact.
        return Ok(Value::from(value as i64));
    }
    Number::from_f64(value)
        .map(Value::Number)
        .ok_or_else(|| syntax(offset, "Number literal out of range"))
}

fn well_formed(token: &str) -> bool {
    let body = token.strip_prefix(['+', '-']).unwrap_or(token);
    let (mantissa, exponent) = match body.find(['e', 'E']) {
        Some(at) => (&body[..at], Some(&body[at + 1..])),
        None => (body, None),
    };
    let digits = |text: &str| text.bytes().all(|byte| byte.is_ascii_digit());
    let (int, frac) = match mantissa.split_once('.') {
        Some((int, frac)) => (int, Some(frac)),
        None => (mantissa, None),
    };
    let mantissa_ok = digits(int)
        && frac.is_none_or(digits)
        && (!int.is_empty() || frac.is_some_and(|frac| !frac.is_empty()));
    let exponent_ok = exponent.is_none_or(|exp| {
        let exp = exp.strip_prefix(['+', '-']).unwrap_or(exp);
        !exp.is_empty() && digits(exp)
    });
    mantissa_ok && exponent_ok
}

#[cfg(test)]
#[path = "number_tests.rs"]
mod tests;
