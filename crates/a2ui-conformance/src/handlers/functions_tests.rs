use rstest::rstest;
use speculoos::prelude::*;

use super::*;
use crate::case::parse_suite;

fn case(yaml: &str) -> Case {
    parse_suite("core/x.yaml", yaml).unwrap().remove(0)
}

const BASIC: &str = "https://a2ui.org/specification/v1_0/catalogs/basic/catalog.json";

#[rstest]
#[case::matching("{valid: true}", true)]
#[case::different("{valid: false}", false)]
fn run_direct_case_compares_expect(#[case] expect: &str, #[case] passes: bool) {
    let direct = case(&format!(
        "- {{name: t, action: evaluate_function, protocolVersion: \"1.0\", function: required, \
         args: {{value: x}}, expect: {expect}}}"
    ));
    assert_that!(run(&direct).is_ok()).is_equal_to(passes);
}

#[test]
fn run_surface_case_uses_processor() {
    // Called directly, `required` would see the `@path` object itself and answer true.
    let bound = case(&format!(
        "- name: t\n  action: evaluate_function\n  protocolVersion: \"1.0\"\n  \
         catalogPaths: [catalogs/basic/v1/catalog.json]\n  surface: {{catalogId: \"{BASIC}\"}}\n  \
         dataModel: {{name: \"\"}}\n  function: required\n  \
         args: {{value: {{\"@path\": /name}}}}\n  expect: {{valid: false}}\n"
    ));
    assert_that!(run(&bound)).is_ok();
}

#[test]
fn run_reports_missing_expected_error() {
    let lenient = case(
        "- {name: t, action: evaluate_function, protocolVersion: \"1.0\", function: required, \
         args: {value: x}, expectError: {category: ExpressionError}}",
    );
    let outcome = run(&lenient);
    assert_that!(outcome)
        .is_err()
        .matches(|message| message.contains("expected an error"));
}

#[rstest]
#[case::last(r#""chart""#, true)]
#[case::first(r#""basic""#, false)]
fn select_last_component_selection_is_compared(#[case] expected: &str, #[case] passes: bool) {
    let two = case(&format!(
        "- name: t\n  action: select_catalog\n  catalog: {{protocolVersion: \"1.0\"}}\n  \
         args:\n    \
         surface: {{id: s, defaultCatalogId: basic, supportedCatalogIds: [basic, chart]}}\n    \
         components:\n      c1: {{component: Text, catalogId: basic}}\n      \
         c2: {{component: LineChart, catalogId: chart}}\n  expectSelected: {expected}\n"
    ));
    assert_that!(select(&two).is_ok()).is_equal_to(passes);
}

#[test]
fn select_unsupported_catalog_matches_upstream_message() {
    let unsupported = case(
        "- name: t\n  action: select_catalog\n  catalog: {protocolVersion: \"1.0\"}\n  args:\n    \
         surface: {id: main_surface, defaultCatalogId: basic, supportedCatalogIds: [basic]}\n    \
         components:\n      c1: {component: CustomWidget, catalogId: vendor}\n  \
         expectError:\n    category: CatalogError\n    \
         message: \"Catalog 'vendor' is not supported by surface 'main_surface'.\"\n",
    );
    assert_that!(select(&unsupported)).is_ok();
}
