use speculoos::prelude::*;

use super::*;
use crate::case::parse_suite;

const SKIPS: &str = r#"
[actions]
validate = "Plan 3"

[cases]
test_named = "known gap"

[versions]
"0.9" = "protocol v0.9"
"#;

fn case(yaml: &str) -> Case {
    parse_suite("core/x.yaml", yaml).unwrap().remove(0)
}

#[test]
fn reason_named_case_wins_over_action_and_version() {
    let skips = Skips::parse(SKIPS).unwrap();
    let named = case("- {name: test_named, action: validate}");
    let reason = skips.reason(&named, Some("0.9"));
    assert_that!(reason).is_equal_to(Some("known gap".to_owned()));
}

#[test]
fn reason_unsupported_action_is_reported() {
    let skips = Skips::parse(SKIPS).unwrap();
    let reason = skips.reason(&case("- {name: test_x, action: validate}"), Some("1.0"));
    assert_that!(reason).is_equal_to(Some("Plan 3".to_owned()));
}

#[test]
fn reason_listed_version_is_skipped() {
    let skips = Skips::parse(SKIPS).unwrap();
    let reason = skips.reason(&case("- {name: test_x, action: from_json}"), Some("0.9"));
    assert_that!(reason).is_equal_to(Some("protocol v0.9".to_owned()));
}

#[test]
fn reason_supported_version_runs() {
    let skips = Skips::parse(SKIPS).unwrap();
    let reason = skips.reason(&case("- {name: test_x, action: from_json}"), Some("1.0"));
    assert_that!(reason).is_none();
}

#[test]
fn reason_unlisted_version_runs() {
    let skips = Skips::parse(SKIPS).unwrap();
    let reason = skips.reason(&case("- {name: test_x, action: from_json}"), Some("2.0"));
    assert_that!(reason).is_none();
}

#[test]
fn reason_case_without_version_runs() {
    let skips = Skips::parse(SKIPS).unwrap();
    let reason = skips.reason(&case("- {name: test_x, action: from_json}"), None);
    assert_that!(reason).is_none();
}

#[test]
fn reason_version_neutral_action_runs_on_listed_version() {
    let skips = Skips::parse(SKIPS).unwrap();
    let reason = skips.reason(&case("- {name: test_x, action: data_model}"), Some("0.9"));
    assert_that!(reason).is_none();
}

#[test]
fn unused_entries_without_matching_case_are_listed() {
    let skips = Skips::parse(SKIPS).unwrap();
    let neutral = case("- {name: test_x, action: data_model}");
    assert_that!(skips.unused(&[(&neutral, Some("0.9"))])).is_equal_to(vec![
        "actions.validate".to_owned(),
        "cases.test_named".to_owned(),
        "versions.0.9".to_owned(),
    ]);
}

#[test]
fn unused_version_of_a_skipped_case_is_not_listed() {
    let skips = Skips::parse(SKIPS).unwrap();
    let old = case("- {name: test_named, action: validate}");
    assert_that!(skips.unused(&[(&old, Some("0.9"))])).is_empty();
}
