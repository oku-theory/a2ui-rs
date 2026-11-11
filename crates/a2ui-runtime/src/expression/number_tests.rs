use rstest::rstest;
use serde_json::{Value, json};
use speculoos::prelude::*;

use super::*;

#[rstest]
#[case("42", json!(42))]
#[case("-3.25", json!(-3.25))]
#[case("1e5", json!(100000))]
#[case("1.", json!(1))]
#[case(".5", json!(0.5))]
#[case("+.5", json!(0.5))]
#[case("007", json!(7))]
#[case("-0", json!(0))]
#[case("1e-999", json!(0))]
fn parse_valid_literal_gives_number(#[case] token: &str, #[case] expected: Value) {
    assert_that!(parse(token, 0).unwrap().as_f64()).is_equal_to(expected.as_f64());
}

#[rstest]
#[case("1.2.3")]
#[case(".5.5")]
#[case("1e")]
#[case("-.5e")]
#[case("-1x")]
fn parse_malformed_literal_is_invalid(#[case] token: &str) {
    assert_that!(parse(token, 0).unwrap_err().to_string()).contains("Invalid number literal");
}

#[test]
fn parse_huge_literal_is_out_of_range() {
    assert_that!(parse("2e308", 0).unwrap_err().to_string()).contains("out of range");
}

#[rstest]
#[case("-a", false)]
#[case("-.", false)]
#[case(".e5", false)]
#[case("+1", true)]
#[case("-.5", true)]
fn looks_numeric_classifies_token(#[case] token: &str, #[case] numeric: bool) {
    assert_that!(looks_numeric(token)).is_equal_to(numeric);
}

#[rstest]
#[case("-1x", 2)]
#[case("1.2.3", 5)]
#[case("1e+", 3)]
#[case("-.5E-1}", 6)]
#[case("1e5x", 3)]
fn literal_len_numeric_token_stops_after_literal(#[case] token: &str, #[case] len: usize) {
    assert_that!(literal_len(token)).is_equal_to(len);
}
