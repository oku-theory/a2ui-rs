use rstest::rstest;
use serde_json::json;
use speculoos::prelude::*;

use super::*;
use crate::case::parse_suite;

fn case(yaml: &str) -> Case {
    parse_suite("core/x.yaml", yaml).unwrap().remove(0)
}

/// Catalog `b`, inline in a case.
const CATALOG_B: &str =
    r#"{catalogId: b, protocolVersion: "1.0", components: {B: {type: object}}}"#;
/// Catalog `a`, inline in a case.
const CATALOG_A: &str =
    r#"{catalogId: a, protocolVersion: "1.0", components: {A: {type: object}}}"#;

/// A capabilities case on catalogs `b` then `a`, expecting `ids` and both catalogs inline.
fn capabilities_case(ids: &str) -> Case {
    case(&format!(
        "- name: t\n  action: get_renderer_capabilities\n  catalogs:\n    - {CATALOG_B}\n    \
         - {CATALOG_A}\n  args: {{includeInlineCatalogs: true, version: \"1.0\"}}\n  \
         expect:\n    \"1.0\":\n      supportedCatalogIds: {ids}\n      inlineCatalogs:\n        \
         - {CATALOG_B}\n        - {CATALOG_A}\n"
    ))
}

/// A data model case: surface `s` sends its model when `sends`; `args.version` is `version`.
fn data_model_case(sends: bool, version: &str, expect: &str) -> Case {
    case(&format!(
        "- name: t\n  action: get_renderer_data_model\n  protocolVersion: \"1.0\"\n  \
         messages:\n    - {{version: \"1.0\", createSurface: {{surfaceId: s, \
         sendDataModel: {sends}}}}}\n  args: {{version: \"{version}\"}}\n  expect: {expect}\n"
    ))
}

#[test]
fn capabilities_compare_sorted() {
    assert_that!(capabilities(&capabilities_case("[b, a]"))).is_ok();
}

#[test]
fn capabilities_missing_id_fails() {
    let err = capabilities(&capabilities_case("[b]")).unwrap_err();
    assert_that!(err).contains("capabilities\n  expected: ");
    assert_that!(err).contains("\n  reported: ");
}

#[test]
fn data_model_null_matches_none() {
    assert_that!(data_model(&data_model_case(false, "1.0", "null"))).is_ok();
}

#[test]
fn data_model_some_against_null_fails() {
    let err = data_model(&data_model_case(true, "1.0", "null")).unwrap_err();
    assert_that!(err).contains("data model\n  expected: null\n  reported: {");
}

#[test]
fn data_model_report_matches_the_expected_surfaces() {
    let expect = "{version: \"1.0\", surfaces: {s: {}}}";
    assert_that!(data_model(&data_model_case(true, "1.0", expect))).is_ok();
}

#[rstest]
#[case::v0_9("v0.9")]
#[case::bare_0_8("0.8")]
#[case::patch("v0.9.1")]
fn data_model_older_version_reports_none(#[case] version: &str) {
    assert_that!(data_model(&data_model_case(true, version, "null"))).is_ok();
}

#[rstest]
#[case::future("2.0")]
#[case::word("latest")]
#[case::bare_v("v")]
#[case::three_dots("0.9.1.2")]
#[case::empty_minor("0.")]
fn data_model_unknown_version_fails(#[case] version: &str) {
    let err = data_model(&data_model_case(true, version, "null")).unwrap_err();
    assert_that!(err).contains("unknown protocol version");
}

#[rstest]
#[case::inline_catalog_without_id(json!({"1.0": {"inlineCatalogs": [{"components": []}]}}),
    "catalogId")]
#[case::component_without_name(
    json!({"1.0": {"inlineCatalogs": [{"catalogId": "a", "components": [{"x": 1}]}]}}),
    "name"
)]
fn sorted_entry_without_its_key_fails(#[case] report: Value, #[case] field: &str) {
    assert_that!(sorted(&report).unwrap_err()).contains(format!("without `{field}`").as_str());
}
