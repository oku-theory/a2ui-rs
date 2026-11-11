//! The catalog a case names, and the overrides it passes to `Catalog::from_json`.

use a2ui_runtime::Catalog;
use serde_json::Value;

use crate::case::Case;
use crate::paths::{read_json, upstream_root};

/// The case's catalog document: exactly one of inline `catalog` or `catalogPath`.
pub fn document(case: &Case) -> Result<Value, String> {
    match (case.get("catalog"), case.get("catalogPath")) {
        (Some(catalog), None) => Ok(catalog.clone()),
        (None, Some(Value::String(path))) => read_json(&upstream_root()?.join(path)),
        (None, Some(_)) => Err(format!("{}: `catalogPath` must be a string", case.at())),
        _ => Err(format!(
            "{}: a case needs exactly one of `catalog` and `catalogPath`",
            case.at()
        )),
    }
}

/// Loads `document` with the case's top-level `catalogId` and `protocolVersion` overrides.
///
/// The outer `Err` is a malformed case; the inner one is the runtime's answer.
pub fn load(case: &Case, document: &Value) -> Result<Result<Catalog, a2ui_runtime::Error>, String> {
    let text = |key: &str| match case.get(key) {
        None => Ok(None),
        Some(Value::String(value)) => Ok(Some(value.as_str())),
        Some(_) => Err(format!("{}: `{key}` must be a string", case.at())),
    };
    Ok(Catalog::from_json(
        document,
        text("catalogId")?,
        text("protocolVersion")?,
    ))
}
