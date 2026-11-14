use speculoos::prelude::*;

use super::*;
use crate::limits::MAX_QUOTED_CHARS;

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
