use rstest::rstest;
use speculoos::prelude::*;

use super::*;

#[rstest]
#[case("name", "/user", "/user/name")]
#[case("address/city", "/user", "/user/address/city")]
#[case("/list/0", "/user", "/list/0")]
#[case("user/name", "/", "/user/name")]
#[case("foo", "/bar/", "/bar/foo")]
#[case("", "/user/", "/user")]
#[case(".", "/user", "/user")]
#[case(".", "/", "/")]
fn resolve_path_against_scope_gives_absolute(
    #[case] path: &str,
    #[case] scope: &str,
    #[case] expected: &str,
) {
    let scope = Pointer::parse(scope).unwrap();
    assert_that!(resolve_path(path, &scope).unwrap().to_string()).is_equal_to(expected.to_owned());
}

#[test]
fn resolve_path_forbidden_relative_segment_is_error() {
    assert_that!(resolve_path("__proto__/x", &Pointer::root())).is_err();
}
