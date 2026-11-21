use std::sync::{Arc, Mutex};

use a2ui_runtime::{
    AgentMessage, Catalog, CatalogSet, ErrorCode, FunctionRegistry, MessageLimits, MessageProcessor,
};
use rstest::rstest;
use serde_json::json;
use speculoos::prelude::*;

use super::super::test_support::{call, evaluate_in, evaluate_with, first_error};
use crate::{BasicOptions, CATALOG_ID, CATALOG_JSON, UrlOpener, functions};

/// An opener that records every URL it is asked to open.
#[derive(Debug, Default)]
struct Recording(Mutex<Vec<String>>);

impl UrlOpener for Recording {
    fn open(&self, url: &str) {
        self.0.lock().unwrap().push(url.to_owned());
    }
}

/// Basic options opening URLs with `opener`.
fn opening_with(opener: &Arc<Recording>) -> BasicOptions {
    BasicOptions {
        url_opener: Arc::clone(opener) as Arc<dyn UrlOpener>,
        ..BasicOptions::default()
    }
}

#[test]
fn format_string_interpolates_through_context() {
    let mut catalogs = CatalogSet::new();
    catalogs
        .insert(Catalog::parse(CATALOG_JSON, None, None).unwrap())
        .unwrap();
    let mut registry = FunctionRegistry::new();
    functions(&mut registry, BasicOptions::default()).unwrap();
    let mut processor = MessageProcessor::new(catalogs).with_functions(registry);
    let create = json!({"version": "1.0", "createSurface": {
        "surfaceId": "main", "catalogId": CATALOG_ID,
        "dataModel": {"name": "Ada", "n": 3}}});
    processor
        .handle(&AgentMessage::from_json(&create, &MessageLimits::default()).unwrap())
        .unwrap();
    let call = json!({"@call": "formatString", "args": {"value": "${/name} has ${/n}"}});

    let evaluation = processor.evaluate("main", &call, false).unwrap();

    assert_that!(evaluation.errors).is_empty();
    assert_that!(evaluation.value).is_equal_to(json!("Ada has 3"));
}

#[test]
fn format_string_template_call_runs_in_the_call_catalog() {
    let args = json!({"value": "valid: ${not(value: false)}"});

    let evaluation = evaluate_in(None, "formatString", Some(CATALOG_ID), args);

    assert_that!(first_error(evaluation).unwrap()).is_equal_to(json!("valid: true"));
}

#[test]
fn format_string_null_is_empty() {
    let result = call("formatString", json!({"value": "Greeting: ${null}"}));
    assert_that!(result.unwrap()).is_equal_to(json!("Greeting: "));
}

#[test]
fn format_string_static_text_is_unchanged() {
    let result = call("formatString", json!({"value": "Hello World"}));
    assert_that!(result.unwrap()).is_equal_to(json!("Hello World"));
}

#[test]
fn format_string_non_string_value_is_its_text() {
    let result = call("formatString", json!({"value": 12}));
    assert_that!(result.unwrap()).is_equal_to(json!("12"));
}

#[test]
fn format_string_unknown_template_call_is_an_expression_error() {
    let error = call("formatString", json!({"value": "${noSuchFn()}"})).unwrap_err();

    assert_that!(error.code()).is_equal_to(ErrorCode::ExpressionError);
    assert_that!(error.to_string()).contains("noSuchFn");
}

#[rstest]
#[case::https("https://example.com")]
#[case::http_upper_case("HTTP://example.com")]
#[case::mailto("mailto:a@b.co")]
#[case::tel("tel:+15551234")]
fn open_url_allowed_scheme_calls_opener(#[case] url: &str) {
    let opener = Arc::new(Recording::default());

    let result = first_error(evaluate_with(
        opening_with(&opener),
        "openUrl",
        json!({"url": url}),
    ));

    assert_that!(result.unwrap()).is_equal_to(json!(null));
    assert_that!(*opener.0.lock().unwrap()).is_equal_to(vec![url.to_owned()]);
}

#[rstest]
#[case::javascript("javascript:alert(1)")]
#[case::javascript_upper_case("JAVASCRIPT:alert(1)")]
#[case::data("data:text/html,<p>")]
#[case::file("file:///etc/passwd")]
#[case::no_scheme("example.com/page")]
#[case::leading_space(" https://example.com")]
fn open_url_refuses_other_schemes(#[case] url: &str) {
    let opener = Arc::new(Recording::default());

    let result = first_error(evaluate_with(
        opening_with(&opener),
        "openUrl",
        json!({"url": url}),
    ));

    assert_that!(result.unwrap_err().code()).is_equal_to(ErrorCode::ExpressionError);
    assert_that!(*opener.0.lock().unwrap()).is_empty();
}
