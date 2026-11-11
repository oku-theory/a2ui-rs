//! The `resolve_path` action: a relative path against a context path.

use a2ui_runtime::{Pointer, resolve_path};
use serde_json::Value;

use crate::case::Case;
use crate::keys::{assert_case_keys, assert_known_keys};

/// Resolves `args.path` against `args.contextPath` and compares with `expect`.
pub fn run(case: &Case) -> Result<(), String> {
    assert_case_keys(case, &["catalog", "protocolVersion", "args", "expect"])?;
    let at = case.at();
    let args = case
        .get("args")
        .and_then(Value::as_object)
        .ok_or_else(|| format!("{at}: a case needs `args`"))?;
    assert_known_keys(args, &["path", "contextPath"], &at)?;
    let path = args
        .get("path")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{at}: `args.path` must be a string"))?;
    // An absent `contextPath` is the documented root scope; a present one must be a string.
    let context = match args.get("contextPath") {
        None => "",
        Some(Value::String(context)) => context,
        Some(other) => return Err(format!("{at}: `contextPath` must be a string, got {other}")),
    };
    let scope = Pointer::parse(context).map_err(|err| format!("{at}: {err}"))?;
    let resolved = resolve_path(path, &scope).map_err(|err| format!("{at}: {err}"))?;
    let expected = case
        .get("expect")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{at}: `expect` must be a string"))?;
    if resolved.to_string() == expected {
        Ok(())
    } else {
        Err(format!("{at}: resolved {resolved}, expected {expected}"))
    }
}
