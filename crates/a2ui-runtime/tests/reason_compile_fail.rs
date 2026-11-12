//! Compile-fail tests: `reason!` refuses wording that skips `Quoted`'s cap.

// A rustc release may reword these diagnostics: refresh the `.stderr` files with
// `TRYBUILD=overwrite cargo test -p a2ui-runtime --test reason_compile_fail`, then re-read
// them to check each still fails on the `Piece` bound or a `'static` borrow.

#[test]
fn reason_non_piece_inputs_fail_to_compile() {
    trybuild::TestCases::new().compile_fail("tests/ui/*.rs");
}
