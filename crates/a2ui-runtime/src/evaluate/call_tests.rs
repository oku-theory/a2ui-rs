use rstest::rstest;
use serde_json::{Value, json};
use speculoos::prelude::*;

use crate::evaluate::test_support::{agent_only_echo, catalog, clean, evaluate, failed, registry};
use crate::evaluate::{Evaluation, Evaluator, Scope};
use crate::{CatalogSet, Category, DataModel, ErrorCode, Pointer};

/// `value` evaluated in the root scope of a surface whose default catalog is `default`.
fn at_root(value: Value, default: Option<&str>) -> Evaluation {
    let data = DataModel::default();
    evaluate(&value, &Scope::new("s1", default, &data))
}

/// `value` evaluated in item `index` of a template over `/items`.
fn in_item(value: Value, index: usize) -> Evaluation {
    let data = DataModel::new(json!({"items": [0, 1, 2, 3]}));
    let base = Pointer::parse(&format!("/items/{index}")).unwrap();
    evaluate(
        &value,
        &Scope::new("s1", Some("c"), &data).in_template(base, index),
    )
}

/// Checks that `error` is an `ExpressionError` reading `text`.
fn assert_expression_error(error: &crate::Error, text: &str) {
    assert_that!(error.category()).is_equal_to(Category::Expression);
    assert_that!(error.code()).is_equal_to(ErrorCode::ExpressionError);
    assert_that!(error.to_string()).is_equal_to(text.to_owned());
}

#[test]
fn call_own_catalog_wins_over_default() {
    let value = clean(at_root(
        json!({"@call": "site", "catalogId": "d"}),
        Some("c"),
    ));
    assert_that!(value).is_equal_to(json!("d"));
}

#[test]
fn call_without_catalog_uses_surface_default() {
    let value = clean(at_root(json!({"@call": "site"}), Some("d")));
    assert_that!(value).is_equal_to(json!("d"));
}

#[test]
fn call_without_catalog_or_default_is_no_default_catalog() {
    let error = failed(at_root(json!({"@call": "site"}), None));

    assert_that!(error.category()).is_equal_to(Category::Catalog);
    assert_that!(error.code()).is_equal_to(ErrorCode::CatalogNotFound);
    assert_that!(error.to_string()).contains("has no default catalogId");
}

#[test]
fn call_unknown_catalog_is_catalog_not_found() {
    let call = json!({"@call": "site", "catalogId": "missing-catalog"});

    let error = failed(at_root(call, Some("c")));

    assert_that!(error.code()).is_equal_to(ErrorCode::CatalogNotFound);
    assert_that!(error.to_string()).is_equal_to("Catalog not found: missing-catalog".to_owned());
}

#[test]
fn nested_call_does_not_inherit_catalog() {
    let call = json!({"@call": "echo", "catalogId": "d", "args": {"value": {"@call": "site"}}});

    let value = clean(at_root(call, Some("c")));

    assert_that!(value).is_equal_to(json!({"catalog": "d", "value": "c"}));
}

#[test]
fn call_unknown_function_is_an_expression_error() {
    let error = failed(at_root(json!({"@call": "nope"}), Some("c")));
    assert_expression_error(&error, "Function 'nope' is not defined in catalog 'c'");
}

#[test]
fn call_agent_only_function_is_an_expression_error() {
    let error = failed(at_root(json!({"@call": "secret"}), Some("c")));

    assert_expression_error(&error, "Function 'secret' cannot be called by renderer");
}

#[test]
fn call_requiring_activation_without_it_is_an_expression_error() {
    let error = failed(at_root(json!({"@call": "open"}), Some("c")));
    assert_expression_error(&error, "Function 'open' requires user activation");
}

#[test]
fn call_requiring_activation_with_it_runs() {
    let data = DataModel::default();
    let scope = Scope::new("s1", Some("c"), &data).with_user_activation(true);

    let value = clean(evaluate(&json!({"@call": "open"}), &scope));

    assert_that!(value).is_equal_to(json!("opened"));
}

#[test]
fn call_schema_only_function_is_not_implemented() {
    let error = failed(at_root(json!({"@call": "ghost"}), Some("c")));
    assert_expression_error(
        &error,
        "Function 'ghost' of catalog 'c' has no renderer implementation",
    );
}

#[test]
fn catalog_reloaded_under_same_id_uses_its_own_policy() {
    // The registry was bound on catalog `c`, where `echo` is renderer-callable; the set holds a
    // catalog `c` loaded again, where `echo` is agentOnly.
    let functions = registry();
    let mut catalogs = CatalogSet::new();
    catalogs.insert(agent_only_echo("c")).unwrap();
    assert_that!(
        functions
            .implementation(catalog("c").id(), "echo")
            .is_some()
    )
    .is_true();
    let data = DataModel::default();

    let evaluation = Evaluator::new(&catalogs, &functions).evaluate(
        &json!({"@call": "echo"}),
        &Scope::new("s1", Some("c"), &data),
    );

    assert_expression_error(
        &failed(evaluation),
        "Function 'echo' cannot be called by renderer",
    );
}

#[test]
fn index_outside_template_is_an_expression_error() {
    let error = failed(at_root(json!({"@call": "@index"}), Some("c")));
    assert_expression_error(
        &error,
        "@index function can only be evaluated inside a collection template iteration scope.",
    );
}

#[test]
fn index_inside_template_adds_offset() {
    let call = json!({"@call": "@index", "args": {"offset": {"@path": "/items/1"}}});

    let value = clean(in_item(call, 2));

    assert_that!(value).is_equal_to(json!(3));
}

#[test]
fn index_ignores_catalog_id() {
    let value = clean(in_item(
        json!({"@call": "@index", "catalogId": "missing-catalog"}),
        2,
    ));
    assert_that!(value).is_equal_to(json!(2));
}

#[rstest]
#[case::number(json!(1), 3)]
#[case::numeric_string(json!(" 2 "), 4)]
#[case::negative(json!(-2), 0)]
#[case::integral_float(json!(1.0), 3)]
fn index_integral_offset_is_added(#[case] offset: Value, #[case] expected: i64) {
    let call = json!({"@call": "@index", "args": {"offset": offset}});

    let value = clean(in_item(call, 2));

    assert_that!(value).is_equal_to(json!(expected));
}

#[rstest]
#[case::fraction(json!(1.5))]
#[case::text(json!("one"))]
#[case::boolean(json!(true))]
#[case::null(Value::Null)]
#[case::missing_path(json!({"@path": "/nothing"}))]
fn index_non_integral_offset_is_an_expression_error(#[case] offset: Value) {
    let call = json!({"@call": "@index", "args": {"offset": offset}});

    let error = failed(in_item(call, 2));

    assert_expression_error(&error, "Function '@index': offset must be an integer");
}
