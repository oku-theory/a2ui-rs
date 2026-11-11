use speculoos::prelude::*;

use super::*;
use crate::case::parse_suite;

const SKIPS: &str = r#"
[actions]
validate = "Plan 3"

[cases]
test_named = "known gap"
"#;

fn case(yaml: &str) -> Case {
    parse_suite("core/x.yaml", yaml).unwrap().remove(0)
}

#[test]
fn reason_named_case_wins() {
    let skips = Skips::parse(SKIPS).unwrap();
    let reason = skips.reason(&case("- {name: test_named, action: data_model}"));
    assert_that!(reason).is_equal_to(Some("known gap".to_owned()));
}

#[test]
fn reason_unsupported_action_is_reported() {
    let skips = Skips::parse(SKIPS).unwrap();
    let reason = skips.reason(&case(
        "- {name: test_x, action: validate, protocolVersion: '1.0'}",
    ));
    assert_that!(reason).is_equal_to(Some("Plan 3".to_owned()));
}

#[test]
fn reason_old_version_is_skipped() {
    let skips = Skips::parse(SKIPS).unwrap();
    let reason = skips.reason(&case(
        "- {name: test_x, action: from_json, protocolVersion: v0.9}",
    ));
    assert_that!(reason).is_equal_to(Some("protocol version v0.9 is not supported".to_owned()));
}

#[test]
fn reason_version_neutral_action_runs_on_old_version() {
    let skips = Skips::parse(SKIPS).unwrap();
    let reason = skips.reason(&case(
        "- {name: test_x, action: data_model, catalog: {protocolVersion: v0.9}}",
    ));
    assert_that!(reason).is_none();
}

#[test]
fn unused_entry_without_matching_case_is_listed() {
    let skips = Skips::parse(SKIPS).unwrap();
    let cases = vec![case("- {name: test_x, action: data_model}")];
    assert_that!(skips.unused(&cases)).is_equal_to(vec![
        "actions.validate".to_owned(),
        "cases.test_named".to_owned(),
    ]);
}
