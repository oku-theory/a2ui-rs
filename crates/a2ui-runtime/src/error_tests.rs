use rstest::rstest;
use speculoos::prelude::*;

use super::*;
use crate::detail::{Detail, DetailCode};
use crate::limits::MAX_QUOTED_CHARS;
use crate::quoted::reason;

#[test]
fn error_display_quoted_field_escapes_controls() {
    let error: Error = UnknownComponentSnafu {
        component: "Evil\n[INFO] forged",
    }
    .build()
    .into();
    assert_that!(error.to_string())
        .is_equal_to("Unrecognized component type 'Evil\\n[INFO] forged'".to_owned());
}

#[test]
fn error_display_reason_escapes_controls() {
    let error: Error = MalformedCatalogSnafu {
        reason: "$ref '#/\u{1b}[2J' at /x does not resolve",
    }
    .build()
    .into();
    assert_that!(error.to_string())
        .is_equal_to("Malformed catalog: $ref '#/\\u{1b}[2J' at /x does not resolve".to_owned());
}

#[test]
fn error_display_large_schema_violation_keeps_head_marker_and_tail() {
    let half = MAX_QUOTED_CHARS / 2;
    let value = "v".repeat(100_000);
    let error: Error = SchemaViolationSnafu {
        at: "/value",
        message: format!("\"{value}\" is not of type \"integer\""),
    }
    .build()
    .into();
    let tail = "\" is not of type \"integer\"";
    let elided = value.len() + 2 + tail.len() - 1 - 2 * half;
    let expected = format!(
        "\"{}…[{elided} characters elided]…{}{tail} at '/value'",
        "v".repeat(half - 1),
        "v".repeat(half - tail.len())
    );
    assert_that!(error.to_string()).is_equal_to(expected);
}

#[test]
fn error_debug_large_field_is_capped() {
    let error: Error = UnknownFunctionSnafu {
        function: "f".repeat(100_000),
    }
    .build()
    .into();
    let debug = format!("{error:?}");
    assert_that!(debug.contains(&"f".repeat(MAX_QUOTED_CHARS + 1))).is_false();
    assert_that!(debug.contains("characters elided")).is_true();
}

#[rstest]
#[case::data(ErrorCode::DataError, "DATA_ERROR")]
#[case::expression(ErrorCode::ExpressionError, "EXPRESSION_ERROR")]
#[case::catalog(ErrorCode::CatalogError, "CATALOG_ERROR")]
#[case::validation(ErrorCode::ValidationFailed, "VALIDATION_FAILED")]
#[case::function_call(ErrorCode::InvalidFunctionCall, "INVALID_FUNCTION_CALL")]
fn error_code_as_str_spells_upstream_code(#[case] code: ErrorCode, #[case] expected: &str) {
    assert_that!(code.as_str()).is_equal_to(expected);
}

#[rstest]
#[case::forbidden_segment(
    ForbiddenSegmentSnafu { path: "/__proto__", segment: "__proto__" }.build(),
    Category::Data,
    ErrorCode::DataError
)]
#[case::traverse_primitive(
    TraversePrimitiveSnafu { path: "/a/b", segment: "a" }.build(),
    Category::Data,
    ErrorCode::DataError
)]
#[case::invalid_pointer(
    InvalidPointerSnafu { path: "not_a_pointer", reason: "must start with '/'" }.build(),
    Category::Validation,
    ErrorCode::ValidationFailed
)]
#[case::expression_syntax(
    ExpressionSyntaxSnafu { message: "unterminated", offset: 2_usize }.build(),
    Category::Parse,
    ErrorCode::ExpressionError
)]
#[case::expression_limit(
    ExpressionLimitSnafu { limit: "parts" }.build(),
    Category::Expression,
    ErrorCode::ExpressionError
)]
#[case::catalog_json(
    CatalogJsonSnafu { message: "EOF" }.build(),
    Category::Catalog,
    ErrorCode::CatalogError
)]
#[case::embedded_schema(
    EmbeddedSchemaSnafu { name: "common_types.json", message: "broken" }.build(),
    Category::Catalog,
    ErrorCode::CatalogError
)]
#[case::unknown_component(
    UnknownComponentSnafu { component: "Nope" }.build(),
    Category::Validation,
    ErrorCode::ValidationFailed
)]
#[case::unknown_function(
    UnknownFunctionSnafu { function: "unknownFunc" }.build(),
    Category::Validation,
    ErrorCode::ValidationFailed
)]
#[case::component_limit(
    ComponentLimitSnafu { what: "function calls", limit: 1024_usize }.build(),
    Category::Validation,
    ErrorCode::ValidationFailed
)]
#[case::agent_only_call(
    AgentOnlyCallSnafu { function: "f" }.build(),
    Category::Validation,
    ErrorCode::ValidationFailed
)]
#[case::function_call_shape(
    FunctionCallShapeSnafu { reason: reason!["bad call"] }.build(),
    Category::Validation,
    ErrorCode::InvalidFunctionCall
)]
#[case::message_envelope(
    MessageEnvelopeSnafu { reason: reason!["bad envelope"] }.build(),
    Category::Validation,
    ErrorCode::ValidationFailed
)]
#[case::message_field(
    MessageFieldSnafu { reason: reason!["bad field"] }.build(),
    Category::Validation,
    ErrorCode::ValidationFailed
)]
fn error_class_of_variant_is_its_upstream_category_and_code(
    #[case] inner: Inner,
    #[case] category: Category,
    #[case] code: ErrorCode,
) {
    let error: Error = inner.into();
    assert_that!((error.category(), error.code())).is_equal_to((category, code));
}

fn value_limit() -> Error {
    ValueLimitSnafu { limit: 64_usize }.build().into()
}

#[test]
fn error_without_context_reads_none() {
    let error = value_limit();
    assert_that!(error.path()).is_none();
    assert_that!(error.surface_id()).is_none();
    assert_that!(error.function_call_id()).is_none();
    assert_that!(error.details().is_empty()).is_true();
}

#[test]
fn error_context_reads_back() {
    let detail = Detail::new("a.b".to_owned(), DetailCode::InvalidValue);
    let error = value_limit()
        .with_path("/a".to_owned())
        .with_surface("s1")
        .with_function_call("c1")
        .with_detail(detail.clone());
    assert_that!(error.path()).is_equal_to(Some("/a"));
    assert_that!(error.surface_id()).is_equal_to(Some("s1"));
    assert_that!(error.function_call_id()).is_equal_to(Some("c1"));
    assert_that!(error.details()).is_equal_to(&[detail][..]);
}

#[test]
fn error_display_is_the_inner_message() {
    let error = value_limit().with_path("/a".to_owned());
    assert_that!(error.to_string()).is_equal_to("Value nesting exceeds 64 levels".to_owned());
}

#[test]
fn error_backtrace_delegates_to_inner() {
    let error = value_limit();
    assert_that!(ErrorCompat::backtrace(&error).is_some()).is_true();
}

#[test]
fn error_source_is_none_for_a_leaf_variant() {
    let error = value_limit();
    assert_that!(std::error::Error::source(&error).is_none()).is_true();
}
