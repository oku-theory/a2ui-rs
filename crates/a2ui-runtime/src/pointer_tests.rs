use rstest::rstest;
use speculoos::prelude::*;

use super::*;
use crate::error::Category;

#[rstest]
#[case("", "/")]
#[case("/", "/")]
#[case("///", "/")]
#[case("/user/name", "/user/name")]
#[case("/foo/", "/foo")]
#[case("//a///b//", "/a/b")]
#[case("/user/a~1b", "/user/a~1b")]
#[case("/user/a~01b", "/user/a~01b")]
fn parse_valid_path_displays_normalized(#[case] input: &str, #[case] shown: &str) {
    assert_that!(Pointer::parse(input).unwrap().to_string()).is_equal_to(shown.to_owned());
}

#[test]
fn parse_escapes_unescape_tilde_after_slash() {
    let pointer = Pointer::parse("/a~01b").unwrap();
    assert_that!(pointer.segments()).is_equal_to(&["a~1b".to_owned()][..]);
}

#[rstest]
#[case("/__proto__/polluted")]
#[case("/constructor/prototype/polluted")]
#[case("/user/prototype/polluted")]
fn parse_forbidden_segment_is_data_error(#[case] input: &str) {
    let err = Pointer::parse(input).unwrap_err();
    assert_that!(err.category()).is_equal_to(Category::Data);
}

#[test]
fn parse_forbidden_segment_message_names_it() {
    let err = Pointer::parse("/__proto__").unwrap_err();
    assert_that!(err.to_string()).contains("Forbidden path segment");
}

#[rstest]
#[case("relative")]
#[case("/bad~2escape")]
#[case("/trailing~")]
fn parse_malformed_pointer_is_data_error(#[case] input: &str) {
    assert_that!(Pointer::parse(input).unwrap_err().category()).is_equal_to(Category::Data);
}

#[test]
fn starts_with_ancestor_is_true() {
    let child = Pointer::parse("/user/name").unwrap();
    assert_that!(child.starts_with(&Pointer::parse("/user").unwrap())).is_true();
}

#[test]
fn starts_with_prefix_sibling_is_false() {
    let foobar = Pointer::parse("/foobar").unwrap();
    assert_that!(foobar.starts_with(&Pointer::parse("/foo").unwrap())).is_false();
}

#[test]
fn join_appends_segments() {
    let joined = Pointer::parse("/user")
        .unwrap()
        .join(&Pointer::parse("/address/city").unwrap());
    assert_that!(joined.to_string()).is_equal_to("/user/address/city".to_owned());
}

#[rstest]
#[case("0", ListIndex::At(0))]
#[case("10000", ListIndex::At(10_000))]
#[case("10001", ListIndex::TooLarge)]
#[case("999999999999999999999999", ListIndex::TooLarge)]
#[case("01", ListIndex::NotIndex)]
#[case("-1", ListIndex::NotIndex)]
#[case("foo", ListIndex::NotIndex)]
#[case("", ListIndex::NotIndex)]
fn list_index_classifies_segment(#[case] segment: &str, #[case] expected: ListIndex) {
    assert_that!(list_index(segment)).is_equal_to(expected);
}

#[rstest]
#[case::plain("Child", "Child")]
#[case::slash("a/b", "a~1b")]
#[case::tilde("a~b", "a~0b")]
#[case::tilde_before_slash("/~", "~1~0")]
#[case::empty("", "")]
fn escape_token_escapes_tilde_then_slash(#[case] token: &str, #[case] expected: &str) {
    assert_that!(escape_token(token).as_ref()).is_equal_to(expected);
}

#[rstest]
#[case::plain("Child", Some("Child"))]
#[case::slash("a~1b", Some("a/b"))]
#[case::tilde("a~0b", Some("a~b"))]
#[case::not_double_unescaped("~01", Some("~1"))]
#[case::bad_escape("a~2b", None)]
#[case::trailing_tilde("a~", None)]
#[case::lone_tilde("~", None)]
#[case::double_tilde("~~", None)]
#[case::empty("", Some(""))]
#[case::both_in_sequence("~1~0", Some("/~"))]
fn unescape_token_reads_only_rfc_6901_escapes(#[case] token: &str, #[case] expected: Option<&str>) {
    assert_that!(unescape_token(token).as_deref()).is_equal_to(expected);
}
