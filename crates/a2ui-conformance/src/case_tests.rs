use rstest::rstest;
use speculoos::prelude::*;

use super::*;

const SUITE: &str = r#"
- name: test_top_level_version
  action: validate
  protocolVersion: "1.0"
- name: test_data_model_no_version
  action: data_model
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
        "test_data_model_no_version",
        "test_no_version",
    ]);
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
fn effective_version_evaluate_function_without_catalog_is_upstream_default() {
    let case = one("- {name: t, action: evaluate_function, function: required}");
    assert_that!(case.effective_version(&root()).unwrap())
        .is_equal_to(Some(UPSTREAM_DEFAULT_VERSION.to_owned()));
}

#[test]
fn effective_version_missing_catalog_file_is_error() {
    let case = one("- {name: t, action: round_trip, catalogPath: 'nope.json'}");
    let message = case.effective_version(&root()).unwrap_err().to_string();
    assert_that!(message).contains("nope.json");
}

#[test]
fn effective_version_non_string_catalog_path_is_error() {
    let case = one("- {name: t, action: round_trip, catalogPath: 3}");
    let message = case.effective_version(&root()).unwrap_err().to_string();
    assert_that!(message).contains("`catalogPath` must be a string");
}

/// A `validate` case in `suite` with `body` merged over its name and action.
fn in_suite(suite: &str, body: &str) -> Case {
    let Value::Object(mut map) = serde_json::from_str::<Value>(body).unwrap() else {
        panic!("a case body is an object");
    };
    map.insert("name".to_owned(), Value::from("t"));
    map.insert("action".to_owned(), Value::from("validate"));
    Case {
        suite: suite.to_owned(),
        name: "t".to_owned(),
        action: "validate".to_owned(),
        body: map,
    }
}

#[rstest]
#[case::top_level("core/x.yaml", r#"{"protocolVersion": "v0.9"}"#, "0.9")]
#[case::catalog("core/x.yaml", r#"{"catalog": {"protocolVersion": "1.0"}}"#, "1.0")]
#[case::messages("core/x.yaml", r#"{"messages": [{"version": "1.0"}]}"#, "1.0")]
#[case::payload_in_messages(
    "core/x.yaml",
    r#"{"payload": {"messages": [{"version": "1.0"}]}}"#,
    "1.0"
)]
#[case::steps(
    "core/x.yaml",
    r#"{"steps": [{"messages": [{"version": "1.0"}]}]}"#,
    "1.0"
)]
#[case::args("core/x.yaml", r#"{"args": {"version": "v1.0"}}"#, "1.0")]
#[case::catalogs("core/x.yaml", r#"{"catalogs": [{"protocolVersion": "1.0"}]}"#, "1.0")]
#[case::catalog_paths(
    "core/x.yaml",
    r#"{"catalogPaths": ["catalogs/basic/v1/catalog.json"]}"#,
    "1.0"
)]
#[case::v0_8_envelope("core/x.yaml", r#"{"payload": [{"beginRendering": {}}]}"#, "0.8")]
#[case::suite_suffix("core/validator_v1_0.yaml", "{}", "1.0")]
fn effective_version_follows_the_rules(
    #[case] suite: &str,
    #[case] body: &str,
    #[case] expected: &str,
) {
    let case = in_suite(suite, body);
    assert_that!(case.effective_version(&root()).unwrap()).is_equal_to(Some(expected.to_owned()));
}

#[test]
fn effective_version_refuses_an_unclassifiable_case() {
    let case = in_suite("core/reserved_keys.yaml", r#"{"payload": []}"#);
    let error = case.effective_version(&root()).unwrap_err();
    assert_that!(error).is_equal_to(VersionError::Unclassified(
        "core/reserved_keys.yaml#t".to_owned(),
    ));
    assert_that!(error.to_string()).contains("core/reserved_keys.yaml#t");
}

#[test]
fn effective_version_non_string_catalog_paths_entry_names_catalog_paths() {
    let case = in_suite("core/x.yaml", r#"{"catalogPaths": [3]}"#);
    let error = case.effective_version(&root()).unwrap_err();
    assert_that!(error.to_string()).contains("`catalogPaths` must be a string");
}

#[test]
fn effective_version_non_object_first_catalog_names_the_case_and_catalogs() {
    let case = in_suite("core/x.yaml", r#"{"catalogs": ["c.json"]}"#);
    let error = case.effective_version(&root()).unwrap_err();
    assert_that!(error.to_string()).contains("core/x.yaml#t");
    assert_that!(error.to_string()).contains("`catalogs`");
}
