use a2ui_runtime::{Category, ErrorCode};
use rstest::rstest;
use serde_json::{Value, json};
use speculoos::prelude::*;

use super::super::test_support::call;

#[rstest]
#[case::and("and")]
#[case::or("or")]
fn and_one_value_is_an_expression_error(#[case] name: &str) {
    let error = call(name, json!({"values": [true]})).unwrap_err();

    assert_that!(error.category()).is_equal_to(Category::Expression);
    assert_that!(error.code()).is_equal_to(ErrorCode::ExpressionError);
    assert_that!(error.to_string()).contains("needs at least 2 values");
}

#[test]
fn and_values_not_an_array_is_an_expression_error() {
    let error = call("and", json!({"values": true})).unwrap_err();
    assert_that!(error.code()).is_equal_to(ErrorCode::ExpressionError);
}

#[rstest]
#[case::and_all_true("and", json!([true, true]), true)]
#[case::and_one_false("and", json!([true, false]), false)]
#[case::and_non_true_string("and", json!([1, "non-empty"]), false)]
#[case::and_string_true_any_case("and", json!(["TRUE", 2]), true)]
#[case::and_empty_string("and", json!([1, ""]), false)]
#[case::and_validation_results("and", json!([{"valid": true}, {"valid": false}]), false)]
#[case::and_three_true("and", json!([true, "true", {"valid": true}]), true)]
#[case::or_all_false("or", json!([false, false]), false)]
#[case::or_one_true("or", json!([false, true]), true)]
#[case::or_zero_and_yes("or", json!([0, "yes"]), false)]
#[case::or_null_and_object("or", json!([null, {}]), false)]
#[case::or_validation_results("or", json!([{"valid": false}, {"valid": false}]), false)]
#[case::or_validation_result_true("or", json!([0, {"valid": true}]), true)]
fn and_or_coerce_operands_give_bool(
    #[case] name: &str,
    #[case] values: Value,
    #[case] expected: bool,
) {
    let result = call(name, json!({ "values": values }));
    assert_that!(result.unwrap()).is_equal_to(json!(expected));
}

#[rstest]
#[case::true_(json!(true), false)]
#[case::false_(json!(false), true)]
#[case::empty_string(json!(""), true)]
#[case::other_string(json!("hello"), true)]
#[case::string_true(json!("TRUE"), false)]
#[case::zero(json!(0), true)]
#[case::failed_validation(json!({"valid": false}), true)]
fn not_negates_coerced_value(#[case] value: Value, #[case] expected: bool) {
    let result = call("not", json!({ "value": value }));
    assert_that!(result.unwrap()).is_equal_to(json!(expected));
}
