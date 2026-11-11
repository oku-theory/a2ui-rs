//! Location of the vendored upstream repository.

use std::path::{Path, PathBuf};

/// The vendored upstream root, found by walking up to `conformance_schema.json`.
pub fn upstream_root() -> Result<PathBuf, String> {
    let start = Path::new(env!("CARGO_MANIFEST_DIR"));
    start
        .ancestors()
        .map(|dir| dir.join("vendor/a2ui"))
        .find(|root| root.join("conformance/conformance_schema.json").is_file())
        .ok_or_else(|| format!("no vendor/a2ui above {}", start.display()))
}
