use rstest::rstest;
use serde_json::{Map, Value, json};
use speculoos::prelude::*;

use super::*;
use crate::catalog::test_support::catalog_set;
use crate::{AgentMessage, Category, DetailCode, ErrorCode, MessageLimits};

/// A surface `s1` whose default catalog is `catalog`.
fn surface(catalog: Option<&str>) -> Surface {
    let mut body = json!({"surfaceId": "s1"});
    if let Some(catalog) = catalog {
        body["catalogId"] = json!(catalog);
    }
    let message = json!({"version": "1.0", "createSurface": body});
    match AgentMessage::from_json(&message, &MessageLimits::default()).unwrap() {
        AgentMessage::CreateSurface(create) => Surface::new(&create),
        other => panic!("not a createSurface: {other:?}"),
    }
}

/// Checks `components` on `surface` under strict rules and default limits.
fn check_on(surface: &Surface, components: &[Value]) -> Result<Staged, Error> {
    let set = catalog_set();
    let limits = StateLimits::default();
    check(
        surface,
        components,
        ValidationRules::strict(),
        &set,
        &limits,
    )
}

/// An error's class, message and path.
type Refusal = ((Category, ErrorCode), String, Option<String>);

/// A refusal of class `class` with message `text` at `path`.
fn refused(class: (Category, ErrorCode), text: &str, path: &str) -> Option<Refusal> {
    Some((class, text.to_owned(), Some(path.to_owned())))
}

/// The refusal `result` holds, if any.
fn refusal(result: Result<Staged, Error>) -> Option<Refusal> {
    result.err().map(|error| {
        let path = error.path().map(str::to_owned);
        ((error.category(), error.code()), error.to_string(), path)
    })
}

const NOT_FOUND: (Category, ErrorCode) = (Category::Catalog, ErrorCode::CatalogNotFound);
const INVALID: (Category, ErrorCode) = (Category::Validation, ErrorCode::ValidationFailed);
const NO_DEFAULT: &str = "Surface 's1' has no default catalogId for function 'both'";

/// A `Text` in catalog `a` whose `text` is `value`.
fn text_in_a(value: Value) -> Value {
    json!({"id": "t1", "component": "Text", "catalogId": "a", "text": value})
}

/// An `Any` in catalog `a` whose `items` is `value`.
fn items_in_a(value: Value) -> Value {
    json!({"id": "a1", "component": "Any", "catalogId": "a", "items": value})
}

#[rstest]
#[case::explicit_call_id_with_another_default(
    Some("b"),
    text_in_a(json!({"@call": "upper", "catalogId": "a", "args": {"value": "x"}})),
    None
)]
#[case::unknown_call_id(
    Some("a"),
    text_in_a(json!({"@call": "upper", "catalogId": "missing-catalog"})),
    refused(NOT_FOUND, "Catalog not found: missing-catalog", "/components/0/text")
)]
#[case::non_string_call_id(
    Some("a"),
    items_in_a(json!({"@call": "both", "catalogId": 5})),
    refused(
        INVALID,
        "Invalid function call: catalogId of 'both' must be a string",
        "/components/0/items/catalogId"
    )
)]
#[case::call_resolves_in_the_surface_default(
    Some("a"),
    json!({"id": "b1", "component": "Badge", "catalogId": "b",
        "label": {"@call": "upper", "args": {"value": "x"}}}),
    None
)]
#[case::call_never_resolves_in_the_component_catalog(
    Some("a"),
    json!({"id": "b1", "component": "Badge", "catalogId": "b", "label": {"@call": "onlyB"}}),
    refused(INVALID, "Unrecognized function 'onlyB'", "/components/0/label")
)]
#[case::no_default_in_args(
    None,
    text_in_a(json!({"@call": "upper", "catalogId": "a", "args": {"value": {"@call": "both"}}})),
    refused(NOT_FOUND, NO_DEFAULT, "/components/0/text/args/value")
)]
#[case::no_default_in_a_list(
    None,
    items_in_a(json!(["x", {"@call": "both"}])),
    refused(NOT_FOUND, NO_DEFAULT, "/components/0/items/1")
)]
#[case::no_default_in_index_args(
    None,
    text_in_a(json!({"@call": "@index", "args": {"offset": {"@call": "both"}}})),
    refused(NOT_FOUND, NO_DEFAULT, "/components/0/text/args/offset")
)]
#[case::index_in_a_list(None, items_in_a(json!([{"@call": "@index"}])), None)]
fn call_catalog_resolves_or_errors(
    #[case] default: Option<&str>,
    #[case] component: Value,
    #[case] expected: Option<Refusal>,
) {
    let result = check_on(&surface(default), &[component]);
    assert_that!(refusal(result)).is_equal_to(expected);
}

#[rstest]
#[case::duplicate_before_catalog(
    json!([{"id": "c1", "component": "Text", "text": "x", "catalogId": "missing"},
        {"id": "c1", "component": "Text", "text": "y"}]),
    ErrorCode::IntegrityError,
    None
)]
#[case::bad_id_before_later_catalog(
    json!([{"id": "1x", "component": "Text", "text": "x"},
        {"id": "c2", "component": "Text", "text": "y", "catalogId": "missing"}]),
    ErrorCode::ValidationFailed,
    Some("/components/0/id")
)]
#[case::catalog_before_later_schema(
    json!([{"id": "c1", "component": "Text", "text": "x", "catalogId": "missing"},
        {"id": "c2", "component": "Text"}]),
    ErrorCode::CatalogNotFound,
    Some("/components/0")
)]
#[case::catalog_before_body(
    json!([{"id": "c1", "component": "Text", "catalogId": "missing"}]),
    ErrorCode::CatalogNotFound,
    Some("/components/0")
)]
fn check_reports_the_first_failure_in_order(
    #[case] components: Value,
    #[case] code: ErrorCode,
    #[case] path: Option<&str>,
) {
    let error = check_on(&surface(Some("a")), components.as_array().unwrap()).unwrap_err();

    assert_that!(error.code()).is_equal_to(code);
    assert_that!(error.path()).is_equal_to(path);
}

#[test]
fn check_duplicate_ids_is_an_integrity_error() {
    let component = json!({"id": "c1", "component": "Text", "text": "x"});

    let error = check_on(&surface(Some("a")), &[component.clone(), component]).unwrap_err();

    assert_that!(error.category()).is_equal_to(Category::Integrity);
    assert_that!(error.to_string()).contains("c1");
}

#[test]
fn check_invalid_id_points_at_it() {
    let components = [
        json!({"id": "root", "component": "Text", "text": "x"}),
        json!({"id": "123invalid_id", "component": "Text", "text": "y"}),
    ];

    let error = check_on(&surface(Some("a")), &components).unwrap_err();

    assert_that!(error.category()).is_equal_to(Category::Validation);
    assert_that!(error.path()).is_equal_to(Some("/components/1/id"));
    assert_that!(error.to_string()).contains("Component id '123invalid_id' must be a valid");
}

#[test]
fn check_component_errors_are_prefixed() {
    let component = json!({"id": "t1", "component": "Text", "text": 5});

    let error = check_on(&surface(Some("a")), &[component]).unwrap_err();

    assert_that!(error.path()).is_equal_to(Some("/components/0/text"));
    let detail = &error.details()[0];
    assert_that!(detail.path()).is_equal_to("/text");
    assert_that!(detail.code()).is_equal_to(DetailCode::TypeMismatch);
}

#[test]
fn check_unknown_component_type_has_an_invalid_value_detail() {
    let component = json!({"id": "t1", "component": "Nowhere"});

    let error = check_on(&surface(Some("a")), &[component]).unwrap_err();

    assert_that!(error.path()).is_equal_to(Some("/components/0/component"));
    let detail = &error.details()[0];
    assert_that!(detail.path()).is_equal_to("/component");
    assert_that!(detail.code()).is_equal_to(DetailCode::InvalidValue);
}

#[rstest]
#[case::plain_beside_escaped(
    json!({"@path": "/a", "@@path": 1}),
    Some("Key '@@path' collides with '@path' after unescaping")
)]
#[case::two_escape_levels(json!({"@@@x": 1, "@@x": 2}), None)]
#[case::escaped_alone(json!({"@@path": 1}), None)]
fn check_escaped_keys_collide_only_when_equal_once_unescaped(
    #[case] object: Value,
    #[case] collision: Option<&str>,
) {
    let component = items_in_a(json!([object]));

    let result = check_on(&surface(Some("a")), &[component]);

    let expected = collision.map(|text| {
        (
            INVALID,
            text.to_owned(),
            Some("/components/0/items/0".to_owned()),
        )
    });
    assert_that!(refusal(result)).is_equal_to(expected);
}

#[test]
fn check_component_count_past_the_message_limit_is_refused() {
    let set = catalog_set();
    let limits = StateLimits::default().with_max_components_per_message(1);
    let components = [
        json!({"id": "t1", "component": "Text", "text": "x"}),
        json!({"id": "t2", "component": "Text", "text": "y"}),
    ];

    let rules = ValidationRules::strict();
    let error = check(&surface(Some("a")), &components, rules, &set, &limits).unwrap_err();

    let text = "Components per message limit of 1 exceeded";
    assert_that!(error.to_string()).is_equal_to(text.to_owned());
}

#[test]
fn check_component_count_past_the_surface_limit_is_refused() {
    let set = catalog_set();
    let limits = StateLimits::default().with_max_components_per_surface(2);
    let mut target = surface(Some("a"));
    let first = [json!({"id": "t1", "component": "Text", "text": "x"})];
    target.commit(check_on(&target, &first).unwrap());
    let components = [
        json!({"id": "t1", "component": "Text", "text": "replaced"}),
        json!({"id": "t2", "component": "Text", "text": "y"}),
        json!({"id": "t3", "component": "Text", "text": "z"}),
    ];

    let rules = ValidationRules::strict();
    let error = check(&target, &components, rules, &set, &limits).unwrap_err();

    let text = "Components per surface limit of 2 exceeded";
    assert_that!(error.to_string()).is_equal_to(text.to_owned());
}

#[rstest]
#[case::top_level(json!({"@@x": 1}), json!({"@x": 1}))]
#[case::in_args(
    json!({"x": {"@call": "both", "args": {"v": {"@@k": 1}}}}),
    json!({"x": {"@call": "both", "args": {"v": {"@k": 1}}}})
)]
#[case::in_an_array_element(json!({"items": [1, {"@@k": 1}]}), json!({"items": [1, {"@k": 1}]}))]
#[case::one_level_only(json!({"@@@x": 1}), json!({"@@x": 1}))]
#[case::single_at_untouched(json!({"x": {"@path": "/a"}}), json!({"x": {"@path": "/a"}}))]
#[case::value_untouched(json!({"x": "@@x"}), json!({"x": "@@x"}))]
fn unescaped_component_unescapes_one_level(#[case] extra: Value, #[case] expected: Value) {
    let mut component = json!({"id": "a1", "component": "Any"});
    let mut unescaped = component.clone();
    for (key, value) in extra.as_object().unwrap() {
        component[key] = value.clone();
    }
    for (key, value) in expected.as_object().unwrap() {
        unescaped[key] = value.clone();
    }
    let mut target = surface(Some("a"));

    target.commit(check_on(&target, &[component.clone()]).unwrap());

    let stored: Option<&Map<String, Value>> = target.component("a1");
    assert_that!(stored).is_equal_to(component.as_object());
    assert_that!(target.unescaped_component("a1")).is_equal_to(unescaped.as_object().cloned());
}

#[rstest]
#[case::strict(
    ValidationRules::strict(),
    Some(("/components/0/extra".to_owned(), ErrorCode::ValidationFailed))
)]
#[case::allowed(ValidationRules::strict().allow_undeclared_properties(true), None)]
fn check_undeclared_property_follows_the_rules(
    #[case] rules: ValidationRules,
    #[case] expected: Option<(String, ErrorCode)>,
) {
    let set = catalog_set();
    let limits = StateLimits::default();
    let component = json!({"id": "t1", "component": "Text", "text": "x", "extra": 1});

    let result = check(&surface(Some("a")), &[component], rules, &set, &limits);

    let refusal = result
        .err()
        .map(|error| (error.path().unwrap().to_owned(), error.code()));
    assert_that!(refusal).is_equal_to(expected);
}
