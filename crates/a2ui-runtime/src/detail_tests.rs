use rstest::rstest;
use speculoos::prelude::*;

use super::*;

#[rstest]
#[case::missing(DetailCode::MissingField, "missing_field")]
#[case::mismatch(DetailCode::TypeMismatch, "type_mismatch")]
#[case::invalid(DetailCode::InvalidValue, "invalid_value")]
fn detail_code_as_str_spells_the_suite_code(#[case] code: DetailCode, #[case] expected: &str) {
    assert_that!(code.as_str()).is_equal_to(expected);
}

#[test]
fn detail_new_reads_back_path_and_code() {
    let detail = Detail::new(
        "createSurface.surfaceId".to_owned(),
        DetailCode::TypeMismatch,
    );
    assert_that!(detail.path()).is_equal_to("createSurface.surfaceId");
    assert_that!(detail.code()).is_equal_to(DetailCode::TypeMismatch);
}

#[test]
fn detail_new_has_no_surface_id() {
    let detail = Detail::new("a".to_owned(), DetailCode::MissingField);
    assert_that!(detail.surface_id()).is_none();
}
