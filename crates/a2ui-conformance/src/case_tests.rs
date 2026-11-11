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
