//! The logic functions `and`, `or` and `not`, over values read by `coerce::boolean`.

use a2ui_runtime::{Args, CallContext, Error, coerce};
use serde_json::Value;

use super::arg;

/// The `values` array of `function`'s `args`, holding at least two values.
fn operands<'a>(args: &'a Args, function: &str) -> Result<&'a [Value], Error> {
    match arg(args, "values") {
        Value::Array(values) if values.len() >= 2 => Ok(values),
        _ => Err(Error::function(function, "needs at least 2 values")),
    }
}

/// `and(values)`: whether every value reads as true.
pub(super) fn and(args: &Args, _context: &mut CallContext<'_>) -> Result<Value, Error> {
    Ok(Value::Bool(
        operands(args, "and")?.iter().all(coerce::boolean),
    ))
}

/// `or(values)`: whether some value reads as true.
pub(super) fn or(args: &Args, _context: &mut CallContext<'_>) -> Result<Value, Error> {
    Ok(Value::Bool(
        operands(args, "or")?.iter().any(coerce::boolean),
    ))
}

/// `not(value)`: whether the value reads as false.
pub(super) fn not(args: &Args, _context: &mut CallContext<'_>) -> Result<Value, Error> {
    Ok(Value::Bool(!coerce::boolean(arg(args, "value"))))
}

#[cfg(test)]
#[path = "logic_tests.rs"]
mod tests;
