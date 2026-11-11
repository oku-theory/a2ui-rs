//! The `round_trip` action: `from_json(c).to_json()` equals `c`, and stays stable.

use crate::case::Case;
use crate::compare::json_eq;
use crate::handlers::catalog_input::{document, load};
use crate::keys::assert_case_keys;

/// Loads the case's catalog, serializes it twice and compares both with the input.
pub fn run(case: &Case) -> Result<(), String> {
    assert_case_keys(
        case,
        &["protocolVersion", "catalogId", "catalog", "catalogPath"],
    )?;
    let at = case.at();
    let input = document(case)?;
    let first = load(case, &input)?
        .map_err(|err| format!("{at}: load failed: {err}"))?
        .to_json();
    if !json_eq(&first, &input) {
        return Err(format!("{at}: to_json differs from the input"));
    }
    let second = load(case, &first)?
        .map_err(|err| format!("{at}: reload failed: {err}"))?
        .to_json();
    if json_eq(&second, &first) {
        Ok(())
    } else {
        Err(format!("{at}: a second round trip changed the document"))
    }
}
