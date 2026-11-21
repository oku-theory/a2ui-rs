use rstest::rstest;
use serde_json::{Value, json};
use speculoos::prelude::*;

use super::super::test_support::{catalog_a, catalog_set, on_a_small_stack, wrapped};
use super::{CallCatalogs, Located, Report};
use crate::limits::MAX_JSON_DEPTH;
use crate::{CatalogSet, Category, DetailCode, Error, ErrorCode};

/// A `Text` with id `t1` whose `text` is `value`.
fn text(value: Value) -> Value {
    json!({"id": "t1", "component": "Text", "text": value})
}

/// Validates `component` in catalog `a` as the processor does: every violation reported, calls
/// resolved in the set of `a` and `b`, default catalog `a`; `allow` accepts unknown elements
/// and undeclared properties, as permissive rules do.
fn located(component: &Value, allow: bool) -> Result<(), Error> {
    located_with(component, allow, allow)
}

/// [`located`] with each flag set on its own.
fn located_with(
    component: &Value,
    allow_unknown: bool,
    allow_undeclared: bool,
) -> Result<(), Error> {
    let set = catalog_set();
    let id = component["id"].as_str().unwrap();
    let at = Located {
        default_catalog: Some("a"),
        surface_id: "s1",
        catalogs: &set,
        report: Report::All { id },
        calls: CallCatalogs::Resolve,
        allow_unknown,
        allow_undeclared,
    };
    catalog_a().validate_located(component, &at)
}

/// The `(path, code)` of each detail of `error`.
fn details(error: &Error) -> Vec<(String, DetailCode)> {
    let pairs = error.details().iter();
    pairs.map(|d| (d.path().to_owned(), d.code())).collect()
}

#[rstest]
#[case::wrong_type(
    json!({"id": "t1", "component": "Text", "text": 5}),
    "/text",
    Some(DetailCode::TypeMismatch)
)]
#[case::outside_enum(
    json!({"id": "c1", "component": "Chart", "chartType": "pie"}),
    "/chartType",
    Some(DetailCode::InvalidValue)
)]
#[case::missing_children(
    json!({"id": "c1", "component": "Column"}),
    "/children",
    Some(DetailCode::MissingField)
)]
#[case::unknown_nested_function(
    json!({"id": "b1", "component": "Button", "child": "x",
        "action": {"functionCall": {"@call": "nope"}}}),
    "/action/functionCall",
    None
)]
#[case::bad_argument(
    text(json!({"@call": "upper", "args": {"value": 5}})),
    "/text/args/value",
    None
)]
#[case::agent_only(text(json!({"@call": "fetch", "args": {}})), "/text", None)]
#[case::unknown_type(
    json!({"id": "t1", "component": "Nope"}),
    "/component",
    Some(DetailCode::InvalidValue)
)]
fn validate_located_error_has_the_component_path(
    #[case] component: Value,
    #[case] path: &str,
    #[case] code: Option<DetailCode>,
) {
    let error = located(&component, false).unwrap_err();

    assert_that!(error.category()).is_equal_to(Category::Validation);
    assert_that!(error.path()).is_equal_to(Some(path));
    let expected: Vec<(String, DetailCode)> = code
        .into_iter()
        .map(|code| (path.to_owned(), code))
        .collect();
    assert_that!(details(&error)).is_equal_to(expected);
}

#[test]
fn validate_located_non_string_path_binding_is_refused_at_its_property() {
    let error = located(&text(json!({"@path": 5})), false).unwrap_err();

    assert_that!(error.category()).is_equal_to(Category::Validation);
    assert_that!(error.path()).is_equal_to(Some("/text"));
    let expected = vec![("/text".to_owned(), DetailCode::TypeMismatch)];
    assert_that!(details(&error)).is_equal_to(expected);
}

#[test]
fn validate_located_two_violations_point_at_the_component() {
    let component = json!({"id": "b1", "component": "Button", "bogus": 1});

    let error = located(&component, false).unwrap_err();

    assert_that!(error.path()).is_equal_to(Some(""));
    assert_that!(error.to_string()).contains("(Button) has 2 schema violations; first:");
    assert_that!(details(&error)).has_length(2);
    assert_that!(details(&error)).contains(("/bogus".to_owned(), DetailCode::InvalidValue));
    assert_that!(details(&error)).contains(("/child".to_owned(), DetailCode::MissingField));
}

#[test]
fn validate_located_seventeen_violations_keeps_sixteen_details() {
    let error = located(&json!({"id": "m1", "component": "Many"}), false).unwrap_err();

    assert_that!(error.to_string())
        .starts_with("Component 'm1' (Many) has more than 16 schema violations; first:");
    assert_that!(details(&error)).has_length(16);
}

#[test]
fn validate_located_allow_unknown_accepts_unknown_types_and_functions() {
    let component = json!({"id": "n1", "component": "Nope", "x": {"@call": "nope"}});
    let known = text(json!({"@call": "nope"}));

    assert_that!(located(&component, true)).is_ok();
    assert_that!(located(&known, true)).is_ok();
}

#[test]
fn validate_located_allow_unknown_still_refuses_a_bad_type_name() {
    let error = located(&json!({"id": "n1", "component": "😀"}), true).unwrap_err();

    assert_that!(error.path()).is_equal_to(Some("/component"));
    assert_that!(error.to_string())
        .is_equal_to("component type '😀' must be a valid UAX #31 identifier".to_owned());
}

#[test]
fn validate_located_undeclared_property_is_refused_at_its_pointer() {
    let component = json!({"id": "t1", "component": "Text", "text": "x", "extra": 1});

    let error = located_with(&component, false, false).unwrap_err();

    assert_that!(error.code()).is_equal_to(ErrorCode::ValidationFailed);
    assert_that!(error.path()).is_equal_to(Some("/extra"));
    assert_that!(error.to_string()).contains("'extra'");
    assert_that!(details(&error))
        .is_equal_to(vec![("/extra".to_owned(), DetailCode::InvalidValue)]);
}

#[test]
fn validate_located_undeclared_property_allowed_is_accepted() {
    let component = json!({"id": "t1", "component": "Text", "text": "x", "extra": 1});

    assert_that!(located_with(&component, false, true)).is_ok();
}

#[test]
fn validate_located_schema_fault_wins_over_an_undeclared_property() {
    let component = json!({"id": "t1", "component": "Text", "text": 5, "extra": 1});

    let error = located_with(&component, false, false).unwrap_err();

    assert_that!(error.path()).is_equal_to(Some("/text"));
    assert_that!(details(&error)).is_equal_to(vec![("/text".to_owned(), DetailCode::TypeMismatch)]);
}

#[test]
fn validate_located_open_schema_accepts_an_undeclared_property() {
    let component = json!({"id": "a1", "component": "Any", "extra": {"deep": 1}});

    assert_that!(located_with(&component, false, false)).is_ok();
}

#[test]
fn validate_located_envelope_keys_are_not_undeclared() {
    let component = json!({"id": "t1", "component": "Text", "catalogId": "a", "text": "x",
        "metadata": {}});

    assert_that!(located_with(&component, false, false)).is_ok();
}

#[test]
fn validate_component_two_violations_reports_the_first_only() {
    let catalog = catalog_a();
    let component = json!({"id": "b1", "component": "Button", "bogus": 1});

    let error = catalog
        .validate_component(&component, "a", &catalog)
        .unwrap_err();

    assert_that!(error.details().is_empty()).is_true();
    assert_that!(error.to_string()).does_not_contain("schema violations");
    assert_that!(error.to_string()).ends_with(" at ''");
}

#[test]
fn validate_located_deep_component_is_refused_on_a_small_stack() {
    let refused = on_a_small_stack(|| {
        // Deep enough to overflow a recursive walk, small enough to stay under the node cap.
        let call = wrapped(json!({"@call": "nope"}), "args", 30_000);
        let mut component = json!({"id": "a1", "component": "Any"});
        component["x"] = call;
        let set = CatalogSet::new();
        let at = Located {
            default_catalog: None,
            surface_id: "s1",
            catalogs: &set,
            report: Report::All { id: "a1" },
            calls: CallCatalogs::Resolve,
            allow_unknown: true,
            allow_undeclared: true,
        };
        let refused = catalog_a()
            .validate_located(&component, &at)
            .map_err(|error| {
                (
                    error.code(),
                    error.to_string(),
                    error.path().map(str::to_owned),
                )
            });
        // Dropping 30 000 levels recurses past the stack.
        std::mem::forget(component);
        refused
    });
    let text = format!("Value nesting exceeds {MAX_JSON_DEPTH} levels");
    let expected = (ErrorCode::ValidationFailed, text, Some(String::new()));
    assert_that!(refused).is_equal_to(Err(expected));
}

#[test]
fn undeclared_path_is_the_closing_keyword_at_the_root() {
    let keyword = super::super::compile::UNDECLARED_KEYWORD;

    assert_that!(super::UNDECLARED_PATH.to_owned()).is_equal_to(format!("/{keyword}"));
}
