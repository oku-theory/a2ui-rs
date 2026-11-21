use rstest::rstest;
use speculoos::prelude::*;

use super::*;
use crate::detail::{Detail, DetailCode};
use crate::error::process::ProcessFailure;
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
#[case::recursion(ErrorCode::RecursionLimitExceeded, "RECURSION_LIMIT_EXCEEDED")]
#[case::integrity(ErrorCode::IntegrityError, "INTEGRITY_ERROR")]
#[case::catalog_not_found(ErrorCode::CatalogNotFound, "CATALOG_NOT_FOUND")]
#[case::unallowed_parent(ErrorCode::UnallowedParent, "UNALLOWED_PARENT")]
#[case::unallowed_child(ErrorCode::UnallowedChild, "UNALLOWED_CHILD")]
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
#[case::value_depth(
    ValueDepthSnafu { limit: 50_usize }.build(),
    Category::Recursion,
    ErrorCode::RecursionLimitExceeded
)]
#[case::call_depth(
    CallDepthSnafu {
        depth: 6_usize,
        limit: 5_usize
    }
    .build(),
    Category::Recursion,
    ErrorCode::RecursionLimitExceeded
)]
#[case::surface_exists(
    process(ProcessFailure::SurfaceExists { id: "main".into() }),
    Category::Integrity,
    ErrorCode::IntegrityError
)]
#[case::catalog_not_found(
    process(ProcessFailure::CatalogNotFound { id: "missing".into() }),
    Category::Catalog,
    ErrorCode::CatalogNotFound
)]
fn error_class_of_variant_is_its_upstream_category_and_code(
    #[case] inner: Inner,
    #[case] category: Category,
    #[case] code: ErrorCode,
) {
    let error: Error = inner.into();
    assert_that!((error.category(), error.code())).is_equal_to((category, code));
}

/// `failure` as the inner error the processor raises.
fn process(failure: ProcessFailure) -> Inner {
    ProcessSnafu {
        failure: Box::new(failure),
    }
    .build()
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

#[rstest]
#[case::recursion(Category::Recursion, "RecursionError")]
#[case::integrity(Category::Integrity, "IntegrityError")]
fn category_as_str_spells_the_suite_category(#[case] category: Category, #[case] expected: &str) {
    assert_that!(category.as_str()).is_equal_to(expected);
}

#[test]
fn error_size_stays_within_128_bytes() {
    assert_that!(size_of::<Error>()).is_less_than(128);
}

#[test]
fn at_message_without_context_names_its_index() {
    let error = value_limit().at_message(3);
    assert_that!(error.message_index()).is_equal_to(Some(3));
    assert_that!(error.details().is_empty()).is_true();
}

#[test]
fn at_message_prefixes_detail_paths_and_names_its_index() {
    let detail = Detail::new(
        "deleteSurface.surfaceId".to_owned(),
        DetailCode::MissingField,
    );
    let error = value_limit().with_detail(detail).at_message(1);
    assert_that!(error.message_index()).is_equal_to(Some(1));
    assert_that!(error.details()[0].path()).is_equal_to("messages.1.deleteSurface.surfaceId");
}

#[test]
fn error_without_batch_has_no_message_index() {
    assert_that!(value_limit().with_surface("s1").message_index()).is_none();
}

#[test]
fn with_surface_stamps_details_that_have_none() {
    let mut named = Detail::new("b".to_owned(), DetailCode::InvalidValue);
    named.surface_id = Some("other".to_owned());
    let error = value_limit()
        .with_detail(Detail::new("a".to_owned(), DetailCode::InvalidValue))
        .with_detail(named)
        .with_surface("s1");
    let surfaces: Vec<Option<&str>> = error.details().iter().map(Detail::surface_id).collect();
    assert_that!(surfaces).is_equal_to(vec![Some("s1"), Some("other")]);
}

#[test]
fn prefix_path_prepends_to_the_path() {
    let error = value_limit()
        .with_path("/text".to_owned())
        .prefix_path("/components/0");
    assert_that!(error.path()).is_equal_to(Some("/components/0/text"));
}

#[test]
fn prefix_path_without_path_sets_it() {
    let error = value_limit().prefix_path("/components/0");
    assert_that!(error.path()).is_equal_to(Some("/components/0"));
}
