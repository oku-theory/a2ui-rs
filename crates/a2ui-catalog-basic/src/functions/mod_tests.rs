use a2ui_runtime::FunctionRegistry;
use rstest::rstest;
use serde_json::{Map, json};
use speculoos::prelude::*;

use super::test_support::evaluate_with;
use super::*;
use crate::CATALOG_ID;

/// The fourteen functions the basic catalog declares.
const ALL: [&str; 14] = [
    "required",
    "regex",
    "length",
    "numeric",
    "email",
    "formatString",
    "formatNumber",
    "formatCurrency",
    "formatDate",
    "pluralize",
    "openUrl",
    "and",
    "or",
    "not",
];

#[test]
fn functions_registers_all_fourteen() {
    let mut registry = FunctionRegistry::new();

    functions(&mut registry, BasicOptions::default()).unwrap();

    for name in ALL {
        assert_that!(registry.implementation(CATALOG_ID, name).is_some()).is_true();
    }
}

/// Every argument each formatting function declares.
const FORMAT_ARGS: [(&str, &[&str]); 4] = [
    ("formatNumber", &["value", "decimals", "grouping"]),
    (
        "formatCurrency",
        &["value", "currency", "decimals", "grouping"],
    ),
    ("formatDate", &["value", "format"]),
    (
        "pluralize",
        &["value", "zero", "one", "two", "few", "many", "other"],
    ),
];

#[rstest]
#[case::null(json!(null))]
#[case::true_(json!(true))]
#[case::false_(json!(false))]
#[case::max(json!(1e308))]
#[case::min(json!(-1e308))]
#[case::negative_zero(json!(-0.0))]
#[case::tiny(json!(5e-324))]
#[case::huge_integer(json!(u64::MAX))]
#[case::empty_string(json!(""))]
#[case::numeric_string(json!("1e308"))]
#[case::date_string(json!("2026-02-30T25:61:61+99:99"))]
#[case::quote_string(json!("'"))]
#[case::array(json!([1, "2", null]))]
#[case::object(json!({"valid": true}))]
fn format_functions_never_panic_on_any_json_scalar(#[case] scalar: Value) {
    for (name, parameters) in FORMAT_ARGS {
        let every: Map<String, Value> = parameters
            .iter()
            .map(|parameter| ((*parameter).to_owned(), scalar.clone()))
            .collect();
        let value_only = json!({"value": scalar});

        for args in [Value::Object(every), value_only] {
            let evaluation = evaluate_with(BasicOptions::default(), name, args);
            assert_that!(evaluation.errors.len()).is_less_than_or_equal_to(1);
        }
    }
}
