//! Location of the vendored upstream repository.

use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::case::Case;

/// The vendored upstream root, found by walking up to `conformance_schema.json`.
pub fn upstream_root() -> Result<PathBuf, String> {
    let start = Path::new(env!("CARGO_MANIFEST_DIR"));
    start
        .ancestors()
        .map(|dir| dir.join("vendor/a2ui"))
        .find(|root| root.join("conformance/conformance_schema.json").is_file())
        .ok_or_else(|| format!("no vendor/a2ui above {}", start.display()))
}

/// Reads and parses the JSON file at `path`.
pub fn read_json(path: &Path) -> Result<serde_json::Value, String> {
    let text = std::fs::read_to_string(path).map_err(|err| format!("{}: {err}", path.display()))?;
    serde_json::from_str(&text).map_err(|err| format!("{}: {err}", path.display()))
}

/// The `expectFile` a case names, and that file's JSON.
pub(crate) fn expect_file(case: &Case) -> Result<(String, Value), String> {
    let path = case
        .get("expectFile")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{}: `expectFile` must be a string", case.at()))?;
    Ok((path.to_owned(), read_json(&upstream_root()?.join(path))?))
}
