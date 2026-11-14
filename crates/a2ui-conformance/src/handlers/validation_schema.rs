//! The `validation_schema` action: a loaded catalog's validation schema equals the golden.
//!
//! Compared with `schema_eq`: key order and the order of `enum` and `required` are ignored.

use crate::case::Case;
use crate::compare::schema_eq;
use crate::handlers::catalog_input::{document, load};
use crate::keys::assert_case_keys;
use crate::paths::expect_file;

/// Loads the case's catalog and compares `validation_schema()` with `expectFile`.
pub fn run(case: &Case) -> Result<(), String> {
    assert_case_keys(
        case,
        &[
            "catalog",
            "catalogPath",
            "catalogId",
            "protocolVersion",
            "expectFile",
        ],
    )?;
    let at = case.at();
    let (path, expected) = expect_file(case)?;
    let input = document(case)?;
    let catalog = load(case, &input)?.map_err(|err| format!("{at}: {err}"))?;
    if schema_eq(catalog.validation_schema(), &expected) {
        Ok(())
    } else {
        Err(format!("{at}: validation_schema differs from {path}"))
    }
}
