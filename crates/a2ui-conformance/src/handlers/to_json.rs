//! The `to_json` action: the SDK's own basic catalog serializes to the published file.

use serde_json::Value;

use crate::case::Case;
use crate::compare::json_eq;
use crate::keys::assert_case_keys;
use crate::paths::expect_file;

/// Compares `a2ui_catalog_basic::catalog().to_json()` with `expectFile`.
pub fn run(case: &Case) -> Result<(), String> {
    assert_case_keys(case, &["protocolVersion", "builtinCatalog", "expectFile"])?;
    let at = case.at();
    if case.get("builtinCatalog").and_then(Value::as_str) != Some("basic") {
        return Err(format!("{at}: only `builtinCatalog: basic` exists"));
    }
    let (path, expected) = expect_file(case)?;
    let catalog = a2ui_catalog_basic::catalog().map_err(|err| format!("{at}: {err}"))?;
    if json_eq(&catalog.to_json(), &expected) {
        Ok(())
    } else {
        Err(format!(
            "{at}: the basic catalog's to_json differs from {path}"
        ))
    }
}
