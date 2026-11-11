use rstest::rstest;
use speculoos::prelude::*;

use super::*;

#[rstest]
#[case("Component_ℓ", true)]
#[case("_private", true)]
#[case("ünïcode", true)]
#[case("format_text_1", true)]
#[case("1st", false)]
#[case("has-dash", false)]
#[case("has space", false)]
#[case("", false)]
#[case("Component–Name", false)]
#[case("prop—name", false)]
#[case("func֊name", false)]
#[case("@index", false)]
fn is_identifier_uax31_names_are_classified(#[case] name: &str, #[case] expected: bool) {
    assert_that!(is_identifier(name)).is_equal_to(expected);
}
