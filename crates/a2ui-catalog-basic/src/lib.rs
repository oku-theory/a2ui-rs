//! The A2UI v1.0 basic catalog, embedded for `a2ui-runtime`.
//!
//! [`catalog`] loads the published `catalog.json` ([`CATALOG_JSON`], copied by
//! `vendor/sync.sh`, Apache-2.0: see `LICENSE-A2UI`), whose id is [`CATALOG_ID`]. The
//! renderer-side implementations of its functions arrive with the function runtime (Plan 4).

use a2ui_runtime::{Catalog, Error};

/// The basic catalog's id.
pub const CATALOG_ID: &str = "https://a2ui.org/specification/v1_0/catalogs/basic/catalog.json";

/// The published catalog document, as JSON text.
pub const CATALOG_JSON: &str = include_str!("../catalog.json");

/// Loads the basic catalog; each call parses and compiles it anew (about 20 ms).
pub fn catalog() -> Result<Catalog, Error> {
    Catalog::parse(CATALOG_JSON, None, None)
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
