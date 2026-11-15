//! Input framing, oracles and the basic catalog shared by the fuzz targets.
//!
//! A target reads its input as UTF-8 fields split at NUL bytes: JSON text never holds a raw NUL
//! (a string escapes it), so a seed of whole JSON documents and ids keeps each part whole. Small
//! parts come first and a catalog last, so libFuzzer's mutations, spread over the bytes, reach
//! them; scripts/fuzz_targets.py writes seeds in this shape. [`outcome`] makes results
//! comparable, for determinism oracles.

use a2ui_runtime::{Catalog, Category, Error, ErrorCode};

/// Splits `data` at NUL bytes into UTF-8 fields, at least one.
///
/// `None` when a field is not UTF-8. An input without NUL is one field, so `fields[0]` exists.
pub fn fields(data: &[u8]) -> Option<Vec<&str>> {
    data.split(|&byte| byte == 0)
        .map(|field| std::str::from_utf8(field).ok())
        .collect()
}

/// A result as a value: the value, or the error's code, category and message.
///
/// The runtime keeps its results free of randomness, so one input must give one outcome.
pub fn outcome<T>(result: Result<T, Error>) -> Result<T, (ErrorCode, Category, String)> {
    result.map_err(|error| (error.code(), error.category(), error.to_string()))
}

/// The shared basic catalog; its failing to load is a finding.
pub fn basic() -> &'static Catalog {
    #[expect(
        clippy::expect_used,
        reason = "the embedded catalog always loads (its tests guard it): a failure is a finding"
    )]
    a2ui_catalog_basic::catalog().expect("the basic catalog loads")
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
