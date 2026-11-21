use rstest::rstest;
use speculoos::prelude::*;

use super::*;
use crate::error::Error;

#[rstest]
#[case::undeclared(
    EvalFailure::UndeclaredFunction { catalog: "c".into(), name: "nope".into() },
    "Function 'nope' is not declared by catalog 'c'",
    (Category::Catalog, ErrorCode::CatalogError)
)]
#[case::unknown(
    EvalFailure::UnknownFunction { catalog: "c".into(), name: "f".into() },
    "Function 'f' is not defined in catalog 'c'",
    (Category::Expression, ErrorCode::ExpressionError)
)]
#[case::renderer_forbidden(
    EvalFailure::RendererForbidden { name: "f".into() },
    "Function 'f' cannot be called by renderer",
    (Category::Expression, ErrorCode::ExpressionError)
)]
#[case::activation_required(
    EvalFailure::ActivationRequired { name: "openUrl".into() },
    "Function 'openUrl' requires user activation",
    (Category::Expression, ErrorCode::ExpressionError)
)]
#[case::not_implemented(
    EvalFailure::NotImplemented { catalog: "c".into(), name: "f".into() },
    "Function 'f' of catalog 'c' has no renderer implementation",
    (Category::Expression, ErrorCode::ExpressionError)
)]
#[case::index_outside_template(
    EvalFailure::IndexOutsideTemplate,
    "@index function can only be evaluated inside a collection template iteration scope.",
    (Category::Expression, ErrorCode::ExpressionError)
)]
#[case::depth(
    EvalFailure::Depth { limit: 100 },
    "Evaluation nests deeper than 100 levels",
    (Category::Recursion, ErrorCode::RecursionLimitExceeded)
)]
#[case::unsupported_catalog(
    EvalFailure::UnsupportedCatalog { catalog: "c".into(), surface: "s".into() },
    "Catalog 'c' is not supported by surface 's'.",
    (Category::Catalog, ErrorCode::CatalogError)
)]
#[case::function(
    EvalFailure::Function { name: "f".into(), reason: "bad input".into() },
    "Function 'f': bad input",
    (Category::Expression, ErrorCode::ExpressionError)
)]
#[case::regex(
    EvalFailure::Regex { rule: "NESTED_QUANTIFIER", detail: "(a*)*".into() },
    "NESTED_QUANTIFIER: (a*)*",
    (Category::Expression, ErrorCode::ExpressionError)
)]
fn failure_has_message_category_and_code(
    #[case] failure: EvalFailure,
    #[case] message: &str,
    #[case] class: (Category, ErrorCode),
) {
    let error = Error::from(failure);

    assert_that!(error.to_string()).is_equal_to(message.to_owned());
    assert_that!((error.category(), error.code())).is_equal_to(class);
}

#[test]
fn function_error_quotes_and_caps_reason() {
    let long = "x".repeat(10_000);

    let error = Error::function("f\u{202e}", &long);

    let message = error.to_string();
    assert_that!(message.starts_with("Function 'f\\u{202e}': ")).is_true();
    assert_that!(message.contains("characters elided")).is_true();
    assert_that!(message.len() < 1_000).is_true();
    assert_that!(error.category()).is_equal_to(Category::Expression);
    assert_that!(error.code()).is_equal_to(ErrorCode::ExpressionError);
}
