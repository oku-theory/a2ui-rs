use serde_json::json;
use speculoos::prelude::*;

use crate::evaluate::test_support::{clean, evaluate, failed};
use crate::evaluate::{Evaluation, Scope};
use crate::{DataModel, ErrorCode, Pointer};

/// `template` interpolated by `fmt` of catalog `d` on a surface whose default is `c`, in item 2
/// of a template over `/items`.
fn interpolated(template: &str) -> Evaluation {
    let data = DataModel::new(json!({"n": 3, "o": {"a": 1}, "items": [0, 1, 2]}));
    let scope =
        Scope::new("s1", Some("c"), &data).in_template(Pointer::parse("/items/2").unwrap(), 2);
    let call = json!({"@call": "fmt", "catalogId": "d", "args": {"value": template}});
    evaluate(&call, &scope)
}

#[test]
fn interpolate_joins_text_of_each_part() {
    let value = clean(interpolated("n=${/n} o=${/o} b=${true} x=${/missing}."));
    assert_that!(value).is_equal_to(json!(r#"n=3 o={"a":1} b=true x=."#));
}

#[test]
fn interpolate_call_inherits_current_catalog() {
    let value = clean(interpolated("${site()}"));
    assert_that!(value).is_equal_to(json!("d"));
}

#[test]
fn interpolate_index_stays_catalogless() {
    let value = clean(interpolated("#${@index(offset: 1)}"));
    assert_that!(value).is_equal_to(json!("#3"));
}

#[test]
fn interpolate_unknown_function_names_it() {
    let error = failed(interpolated("a ${noSuchFn()} b"));

    assert_that!(error.code()).is_equal_to(ErrorCode::ExpressionError);
    assert_that!(error.to_string())
        .is_equal_to("Function 'noSuchFn' is not defined in catalog 'd'".to_owned());
}

#[test]
fn interpolate_syntax_error_is_returned() {
    let error = failed(interpolated("${a b}"));

    assert_that!(error.to_string()).contains("Unexpected characters");
}
