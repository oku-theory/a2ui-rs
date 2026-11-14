//! Compile-fail tests: `reason!` refuses wording that skips `Quoted`'s cap.

// A rustc release may reword these diagnostics: refresh the `.stderr` files with
// `TRYBUILD=overwrite cargo test -p a2ui-runtime --test reason_compile_fail`, then re-read
// them to check each still fails on the `Piece` bound or a `'static` borrow.

use std::env::{self, VarError};

/// Names the cases to run; `just msrv-compile-fail` points it at a copy of `tests/ui`.
const CASES_VAR: &str = "A2UI_TRYBUILD_CASES";

#[test]
fn reason_non_piece_inputs_fail_to_compile() {
    let cases = match env::var(CASES_VAR) {
        Ok(cases) if !cases.is_empty() => cases,
        Ok(_) => panic!("{CASES_VAR} is empty"),
        Err(VarError::NotPresent) => "tests/ui/*.rs".to_owned(),
        Err(error) => panic!("{CASES_VAR} is not valid Unicode: {error}"),
    };
    trybuild::TestCases::new().compile_fail(cases);
}
