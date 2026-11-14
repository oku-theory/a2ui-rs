use speculoos::prelude::*;

use super::*;

const SUITE: &str = r#"
- name: test_top_level_version
  action: validate
  protocolVersion: "1.0"
- name: test_catalog_version
  action: data_model
  catalog:
    protocolVersion: "v0.9"
- name: test_no_version
  action: parse_expression_template
  input: "x"
"#;

#[test]
fn parse_suite_three_cases_keeps_order() {
    let cases = parse_suite("core/x.yaml", SUITE).unwrap();
    let names: Vec<_> = cases.iter().map(|case| case.name.as_str()).collect();
    assert_that!(names).is_equal_to(vec![
        "test_top_level_version",
        "test_catalog_version",
        "test_no_version",
    ]);
}

#[test]
fn protocol_version_top_level_wins() {
    let cases = parse_suite("core/x.yaml", SUITE).unwrap();
    assert_that!(cases[0].protocol_version()).is_equal_to(Some("1.0"));
}

#[test]
fn protocol_version_catalog_block_is_fallback() {
    let cases = parse_suite("core/x.yaml", SUITE).unwrap();
    assert_that!(cases[1].protocol_version()).is_equal_to(Some("v0.9"));
}

#[test]
fn protocol_version_absent_is_none() {
    let cases = parse_suite("core/x.yaml", SUITE).unwrap();
    assert_that!(cases[2].protocol_version()).is_none();
}

#[test]
fn parse_suite_empty_list_is_error() {
    assert_that!(parse_suite("core/x.yaml", "[]")).is_err();
}

#[test]
fn parse_suite_case_without_action_is_error() {
    assert_that!(parse_suite("core/x.yaml", "- name: test_x\n")).is_err();
}

#[test]
fn at_parsed_case_joins_suite_and_name() {
    let cases = parse_suite("core/x.yaml", SUITE).unwrap();
    assert_that!(cases[0].at()).is_equal_to("core/x.yaml#test_top_level_version".to_owned());
}

fn root() -> std::path::PathBuf {
    crate::paths::upstream_root().unwrap()
}

fn one(yaml: &str) -> Case {
    parse_suite("core/x.yaml", yaml).unwrap().remove(0)
}

const DERIVE_SOURCE: &str = "conformance/test_data/catalogs/derive_source_catalog_v1_0.json";
const WITHOUT_VERSION: &str = "conformance/test_data/catalogs/catalog_without_id.json";

#[test]
fn effective_version_top_level_beats_the_catalog_file() {
    let case = one(&format!(
        "- {{name: t, action: derive, protocolVersion: v0.9, catalogPath: '{DERIVE_SOURCE}'}}"
    ));
    assert_that!(case.effective_version(&root()).unwrap()).is_equal_to(Some("0.9".to_owned()));
}

#[test]
fn effective_version_catalog_file_version_is_read() {
    let case = one(&format!(
        "- {{name: t, action: derive, catalogPath: '{DERIVE_SOURCE}'}}"
    ));
    assert_that!(case.effective_version(&root()).unwrap()).is_equal_to(Some("1.0".to_owned()));
}

#[test]
fn effective_version_catalog_file_without_version_is_upstream_default() {
    let case = one(&format!(
        "- {{name: t, action: from_json, catalogPath: '{WITHOUT_VERSION}'}}"
    ));
    assert_that!(case.effective_version(&root()).unwrap())
        .is_equal_to(Some(UPSTREAM_DEFAULT_VERSION.to_owned()));
}

#[test]
fn effective_version_inline_catalog_version_is_used() {
    let case = one("- {name: t, action: from_json, catalog: {protocolVersion: '1.0'}}");
    assert_that!(case.effective_version(&root()).unwrap()).is_equal_to(Some("1.0".to_owned()));
}

#[test]
fn effective_version_inline_catalog_without_version_is_upstream_default() {
    let case = one("- {name: t, action: from_json, catalog: {components: {}}}");
    assert_that!(case.effective_version(&root()).unwrap())
        .is_equal_to(Some(UPSTREAM_DEFAULT_VERSION.to_owned()));
}

#[test]
fn effective_version_leading_v_is_dropped() {
    let case = one("- {name: t, action: validate, protocolVersion: v0.8}");
    assert_that!(case.effective_version(&root()).unwrap()).is_equal_to(Some("0.8".to_owned()));
}

#[test]
fn effective_version_case_without_catalog_is_none() {
    let case = one("- {name: t, action: data_model}");
    assert_that!(case.effective_version(&root()).unwrap()).is_none();
}

#[test]
fn effective_version_missing_catalog_file_is_error() {
    let case = one("- {name: t, action: round_trip, catalogPath: 'nope.json'}");
    let message = case.effective_version(&root()).unwrap_err();
    assert_that!(message).contains("nope.json");
}

#[test]
fn effective_version_non_string_catalog_path_is_error() {
    let case = one("- {name: t, action: round_trip, catalogPath: 3}");
    let message = case.effective_version(&root()).unwrap_err();
    assert_that!(message).contains("`catalogPath` must be a string");
}
