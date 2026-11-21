use rstest::rstest;
use serde_json::{Map, Value, json};
use speculoos::prelude::*;

use super::test_support::{catalogs, clean, evaluate, failed, only_error, registry};
use super::*;
use crate::catalog::test_support::on_a_small_stack;
use crate::{Category, DataModel, ErrorCode, Pointer};

/// `levels` arrays, each holding the one below, around `1`.
fn arrays(levels: usize) -> Value {
    (0..levels).fold(json!(1), |inner, _| Value::Array(vec![inner]))
}

/// `levels` `echo` calls, each the `value` argument of the one above, around `1`.
///
/// Built by hand: `json!` would serialize each inner level again, recursively.
fn calls(levels: usize) -> Value {
    (0..levels).fold(json!(1), |inner, _| {
        let mut args = Map::new();
        args.insert("value".to_owned(), inner);
        let mut call = Map::new();
        call.insert("@call".to_owned(), json!("echo"));
        call.insert("args".to_owned(), Value::Object(args));
        Value::Object(call)
    })
}

/// Drops `value` one container at a time.
///
/// `serde_json`'s `Drop` recurses, so a 10 000-level value is taken apart here.
fn dismantle(value: Value) {
    let mut pending = vec![value];
    while let Some(value) = pending.pop() {
        match value {
            Value::Array(items) => pending.extend(items),
            Value::Object(members) => pending.extend(members.into_iter().map(|(_, v)| v)),
            _ => {}
        }
    }
}

/// A 10 000-level array chain, with no data.
fn deep_arrays() -> (Value, Value) {
    (arrays(10_000), json!({}))
}

/// A 10 000-level chain of calls, each an argument of the one above, with no data.
fn deep_calls() -> (Value, Value) {
    (calls(10_000), json!({}))
}

/// A `fmt` call interpolating `/t`, a template that interpolates `/t` again, endlessly.
fn deep_templates() -> (Value, Value) {
    let call = json!({"@call": "fmt", "args": {"value": {"@path": "/t"}}});
    (call, json!({"t": "${fmt(value: /t)}"}))
}

/// How many arrays are nested in `value`, following each array's first item.
fn levels(mut value: &Value) -> usize {
    let mut count = 0;
    while let Value::Array(items) = value {
        count += 1;
        value = items.first().unwrap_or(&Value::Null);
    }
    count
}

#[test]
fn evaluate_literal_is_itself() {
    let data = DataModel::default();
    let literal = json!({"a": [1, "two", true, null], "b": {"c": 3.5}});

    let value = clean(evaluate(&literal, &Scope::new("s", Some("c"), &data)));

    assert_that!(value).is_equal_to(literal);
}

#[test]
fn evaluate_path_reads_data_model() {
    let data = DataModel::new(json!({"user": {"name": "Ada"}}));

    let value = clean(evaluate(
        &json!({"@path": "/user/name"}),
        &Scope::new("s", Some("c"), &data),
    ));

    assert_that!(value).is_equal_to(json!("Ada"));
}

#[test]
fn evaluate_relative_path_uses_template_base() {
    let data = DataModel::new(json!({"items": [{"name": "a"}, {"name": "b"}]}));
    let scope =
        Scope::new("s", Some("c"), &data).in_template(Pointer::parse("/items/1").unwrap(), 1);

    let value = clean(evaluate(&json!({"@path": "name"}), &scope));

    assert_that!(value).is_equal_to(json!("b"));
}

#[test]
fn evaluate_missing_path_is_null_without_error() {
    let data = DataModel::new(json!({"user": {}}));

    let value = clean(evaluate(
        &json!({"@path": "/user/name"}),
        &Scope::new("s", Some("c"), &data),
    ));

    assert_that!(value).is_equal_to(Value::Null);
}

#[test]
fn evaluate_escaped_key_strips_one_at() {
    let data = DataModel::new(json!({"a": 1}));

    let value = clean(evaluate(
        &json!({"@@path": "/a", "@@@call": "x"}),
        &Scope::new("s", Some("c"), &data),
    ));

    assert_that!(value).is_equal_to(json!({"@path": "/a", "@@call": "x"}));
}

#[test]
fn evaluate_escaped_key_beside_its_plain_form_is_a_collision() {
    let data = DataModel::default();

    let evaluation = evaluate(
        &json!({"@@a": 1, "@a": 2, "b": 3}),
        &Scope::new("s", Some("c"), &data),
    );

    assert_that!(evaluation.value).is_equal_to(json!({"@a": null, "b": 3}));
    let error = only_error(evaluation);
    assert_that!(error.code()).is_equal_to(ErrorCode::ValidationFailed);
    assert_that!(error.to_string())
        .is_equal_to("Key '@@a' collides with '@a' after unescaping".to_owned());
}

#[test]
fn evaluate_twice_escaped_key_beside_once_escaped_keeps_both() {
    let data = DataModel::default();

    let value = clean(evaluate(
        &json!({"@@@a": 1, "@@a": 2}),
        &Scope::new("s", Some("c"), &data),
    ));

    assert_that!(value).is_equal_to(json!({"@@a": 1, "@a": 2}));
}

#[test]
fn evaluate_object_with_one_failing_binding_keeps_siblings() {
    let data = DataModel::new(json!({"x": 1}));
    let value = json!({
        "a": {"@path": "/x"},
        "b": {"@call": "nope"},
        "c": {"@call": "echo", "args": {"value": 2}}
    });

    let evaluation = evaluate(&value, &Scope::new("s", Some("c"), &data));

    assert_that!(evaluation.value).is_equal_to(json!({
        "a": 1,
        "b": null,
        "c": {"catalog": "c", "value": 2}
    }));
    assert_that!(only_error(evaluation).code()).is_equal_to(ErrorCode::ExpressionError);
}

#[test]
fn evaluate_depth_101_is_a_recursion_error() {
    // 10 000 levels, evaluated on a wasm-sized stack: the walk must stop at 100.
    let (depth, error) = on_a_small_stack(|| {
        let catalogs = catalogs();
        let functions = registry();
        let data = DataModel::default();
        let deep = arrays(10_000);
        let evaluation =
            Evaluator::new(&catalogs, &functions).evaluate(&deep, &Scope::new("s", None, &data));
        dismantle(deep);
        let depth = levels(&evaluation.value);
        (depth, only_error(evaluation))
    });

    assert_that!(depth).is_equal_to(MAX_EVALUATION_DEPTH);
    assert_that!(error.category()).is_equal_to(Category::Recursion);
    assert_that!(error.code()).is_equal_to(ErrorCode::RecursionLimitExceeded);
    assert_that!(error.to_string())
        .is_equal_to("Evaluation nests deeper than 100 levels".to_owned());
}

#[rstest]
#[case::arrays(deep_arrays)]
#[case::calls(deep_calls)]
#[case::templates(deep_templates)]
fn evaluate_unbounded_nesting_is_one_recursion_error(#[case] shape: fn() -> (Value, Value)) {
    // Built, evaluated and dropped on a wasm-sized stack: the walk must stop at 100.
    let error = on_a_small_stack(move || {
        let catalogs = catalogs();
        let functions = registry();
        let (deep, data) = shape();
        let data = DataModel::new(data);
        let evaluation = Evaluator::new(&catalogs, &functions)
            .evaluate(&deep, &Scope::new("s", Some("c"), &data));
        dismantle(deep);
        let Evaluation { value, mut errors } = evaluation;
        dismantle(value);
        assert_that!(errors).has_length(1);
        errors.remove(0)
    });

    assert_that!(error.code()).is_equal_to(ErrorCode::RecursionLimitExceeded);
    assert_that!(error.to_string())
        .is_equal_to("Evaluation nests deeper than 100 levels".to_owned());
}

#[test]
fn evaluate_depth_100_is_evaluated() {
    let data = DataModel::default();
    let deep = arrays(MAX_EVALUATION_DEPTH);

    let value = clean(evaluate(&deep, &Scope::new("s", None, &data)));

    assert_that!(value).is_equal_to(deep);
}

#[test]
fn evaluate_arguments_before_call() {
    let data = DataModel::new(json!({"name": "Ada"}));
    let call = json!({"@call": "echo", "args": {"value": {"@path": "/name"}}});

    let value = clean(evaluate(&call, &Scope::new("s", Some("c"), &data)));

    assert_that!(value).is_equal_to(json!({"catalog": "c", "value": "Ada"}));
}

#[test]
fn evaluate_malformed_call_is_a_validation_error() {
    let data = DataModel::default();

    let error = failed(evaluate(
        &json!({"@call": "echo", "args": [1]}),
        &Scope::new("s", Some("c"), &data),
    ));

    assert_that!(error.code()).is_equal_to(ErrorCode::ValidationFailed);
    assert_that!(error.to_string())
        .is_equal_to("Invalid function call: args of 'echo' must be an object".to_owned());
}

/// `count` calls to `site`, side by side in one array.
fn sibling_calls(count: usize) -> Value {
    Value::Array(vec![json!({"@call": "site"}); count])
}

#[rstest]
#[case::at_budget(MAX_EVALUATION_STEPS, 0)]
#[case::one_past_budget(MAX_EVALUATION_STEPS + 1, 1)]
fn evaluate_steps_past_budget_raise_one_error(#[case] count: usize, #[case] errors: usize) {
    let data = DataModel::default();

    let evaluation = evaluate(&sibling_calls(count), &Scope::new("s1", Some("c"), &data));

    assert_that!(evaluation.errors).has_length(errors);
    let last = evaluation
        .value
        .as_array()
        .and_then(|items| items.last())
        .cloned();
    let expected = if errors == 0 { json!("c") } else { Value::Null };
    assert_that!(last).is_equal_to(Some(expected));
}

#[test]
fn evaluate_steps_past_budget_is_recursion_limit_exceeded() {
    let data = DataModel::default();
    let scope = Scope::new("s1", Some("c"), &data);

    let error = only_error(evaluate(&sibling_calls(MAX_EVALUATION_STEPS + 1), &scope));

    assert_that!(error.category()).is_equal_to(Category::Recursion);
    assert_that!(error.code()).is_equal_to(ErrorCode::RecursionLimitExceeded);
    assert_that!(error.to_string())
        .is_equal_to("Evaluation takes more than 10000 steps".to_owned());
}

#[test]
fn evaluate_template_parts_count_against_budget() {
    // Each `fmt` call is one step and each of its parts another: 5 000 calls of one part each
    // fit the budget exactly, so the walk stops at the next.
    let data = DataModel::default();
    let call = json!({"@call": "fmt", "args": {"value": "${true}"}});
    let value = Value::Array(vec![call; MAX_EVALUATION_STEPS / 2 + 1]);

    let error = only_error(evaluate(&value, &Scope::new("s1", Some("c"), &data)));

    assert_that!(error.code()).is_equal_to(ErrorCode::RecursionLimitExceeded);
}
