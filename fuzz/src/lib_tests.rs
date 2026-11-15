use rstest::rstest;
use speculoos::prelude::*;

use super::*;

#[rstest]
#[case::empty(b"", vec![""])]
#[case::one(b"{\"a\": 1}", vec!["{\"a\": 1}"])]
#[case::several(b"{}\0id\0", vec!["{}", "id", ""])]
#[case::leading_nul(b"\0x", vec!["", "x"])]
fn fields_input_splits_at_each_nul(#[case] data: &[u8], #[case] expected: Vec<&str>) {
    assert_that!(fields(data)).is_equal_to(Some(expected));
}

#[test]
fn fields_invalid_utf8_is_none() {
    assert_that!(fields(b"ok\0\xff")).is_none();
}

#[test]
fn outcome_ok_passes_value_through() {
    assert_that!(outcome(Ok::<u8, Error>(7))).is_equal_to(Ok(7));
}

#[test]
fn outcome_error_keeps_code_and_category() {
    let error = outcome(a2ui_runtime::Pointer::parse("/__proto__")).unwrap_err();
    assert_that!((error.0, error.1)).is_equal_to((ErrorCode::DataError, Category::Data));
}

#[test]
fn basic_catalog_loads_once() {
    assert_that!(std::ptr::eq(basic(), basic())).is_true();
}
