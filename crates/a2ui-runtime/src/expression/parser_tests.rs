use rstest::rstest;
use serde_json::{Value, json};
use speculoos::prelude::*;

use crate::expression::parse_template;

fn parsed(input: &str) -> Value {
    Value::Array(
        parse_template(input)
            .unwrap()
            .iter()
            .map(|part| part.to_json())
            .collect(),
    )
}

#[rstest]
#[case("hello world", json!(["hello world"]))]
#[case("", json!([]))]
#[case("value is ${/user/name}", json!(["value is ", {"path": "/user/name"}]))]
#[case("${x}${y}", json!([{"path": "x"}, {"path": "y"}]))]
#[case("${'it\\'s'}", json!(["it's"]))]
#[case("${true} ${false}", json!([true, " ", false]))]
#[case("a${null}b", json!(["ab"]))]
#[case("${${nested}}", json!([{"path": "nested"}]))]
#[case("\\${literal} ${real}", json!(["${literal} ", {"path": "real"}]))]
#[case("${add(a: 1, b: 2,)}", json!([{"call": "add", "args": {"a": 1, "b": 2}, "returnType": "any"}]))]
#[case("${f(x: null)}", json!([{"call": "f", "args": {"x": ""}, "returnType": "any"}]))]
#[case("${@index(offset: 1)}", json!([{"call": "@index", "args": {"offset": 1}, "returnType": "any"}]))]
#[case("${/użytkownik/größe}", json!([{"path": "/użytkownik/größe"}]))]
fn parse_template_valid_input_gives_parts(#[case] input: &str, #[case] expected: Value) {
    assert_that!(parsed(input)).is_equal_to(expected);
}

#[rstest]
#[case("hello ${world", "Unclosed interpolation")]
#[case("${add(a 1)}", "after argument name")]
#[case("${f(1)}", "after argument name")]
#[case("${add(a: 1, b: 2}", "after function arguments")]
#[case("${a b}", "Unexpected characters")]
#[case("${a½}", "Unexpected characters")]
#[case("${-1x}", "Unexpected characters")]
fn parse_template_malformed_input_is_parse_error(#[case] input: &str, #[case] message: &str) {
    assert_that!(parse_template(input).unwrap_err().to_string()).contains(message);
}

#[test]
fn parse_template_nesting_past_limit_is_rejected() {
    let input = format!("{}x{}", "${".repeat(101), "}".repeat(101));
    assert_that!(parse_template(&input).unwrap_err().to_string()).contains("Max recursion depth");
}

#[test]
fn parse_template_nesting_at_limit_is_accepted() {
    let input = format!("{}x{}", "${".repeat(100), "}".repeat(100));
    assert_that!(parse_template(&input)).is_ok();
}

#[test]
fn parse_template_too_long_is_expression_error() {
    let err = parse_template(&"a".repeat(10_001)).unwrap_err();
    assert_that!(err.category()).is_equal_to(crate::Category::Expression);
}
