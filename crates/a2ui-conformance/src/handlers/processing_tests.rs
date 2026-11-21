use rstest::rstest;
use speculoos::prelude::*;

use super::*;
use crate::case::parse_suite;

fn case(yaml: &str) -> Case {
    parse_suite("core/x.yaml", yaml).unwrap().remove(0)
}

/// A case header with one open stub catalog `c`, followed by `rest`.
fn with_stub(rest: &str) -> Case {
    case(&format!(
        "- name: t\n  action: process_messages\n  catalogs:\n    \
         - {{catalogId: c, protocolVersion: \"1.0\"}}\n{rest}"
    ))
}

/// A `createSurface` of `id` on catalog `c` whose `orphan` nothing references.
fn orphan_create(id: &str) -> String {
    format!(
        "{{version: \"1.0\", createSurface: {{surfaceId: {id}, catalogId: c, components: \
         [{{id: root, component: Text}}, {{id: orphan, component: Text}}]}}}}"
    )
}

#[test]
fn step_rules_apply_to_one_call() {
    let rest = format!(
        "  steps:\n    - validationConfig: {{allowOrphanComponents: true}}\n      \
         messages: [{}]\n      expect: {{surfaces: {{s1: {{exists: true}}}}}}\n    \
         - messages: [{}]\n      expectError: {{message: is not reachable}}\n",
        orphan_create("s1"),
        orphan_create("s2"),
    );
    assert_that!(run(&with_stub(&rest))).is_ok();
}

#[test]
fn empty_validation_config_is_strict() {
    let rest = format!(
        "  strictMode: false\n  steps:\n    - validationConfig: {{}}\n      \
         messages: [{}]\n      expectError: {{message: is not reachable}}\n",
        orphan_create("s1"),
    );
    assert_that!(run(&with_stub(&rest))).is_ok();
}

#[test]
fn strict_mode_false_is_permissive() {
    let rest = format!(
        "  strictMode: false\n  messages: [{}]\n  expect: {{surfaces: {{s1: {{exists: true}}}}}}\n",
        orphan_create("s1"),
    );
    assert_that!(run(&with_stub(&rest))).is_ok();
}

const LATER_CREATE: &str = r#"
  messages:
    - {version: "1.0", updateComponents: {surfaceId: s9, components: []}}
    - {version: "1.0", createSurface: {surfaceId: s2}}
  expect: {surfaces: {s2: {exists: false}}}
"#;

#[test]
fn first_error_wins_and_later_messages_do_not_run() {
    let rest = format!("{LATER_CREATE}  expectError: {{surfaceId: s9}}\n");
    assert_that!(run(&with_stub(&rest))).is_ok();
}

#[rstest]
#[case::code("{code: CATALOG_NOT_FOUND}", r#"code expected "CATALOG_NOT_FOUND""#)]
#[case::path("{path: /nowhere}", r#"path expected "/nowhere""#)]
#[case::surface("{surfaceId: s2}", r#"surfaceId expected "s2""#)]
fn wrong_error_code_path_or_surface_fails(#[case] expected: &str, #[case] mismatch: &str) {
    let rest = format!("{LATER_CREATE}  expectError: {expected}\n");
    assert_that!(run(&with_stub(&rest)).unwrap_err()).contains(mismatch);
}

#[test]
fn missing_expected_error_fails() {
    let rest = "  messages: [{version: \"1.0\", createSurface: {surfaceId: s1}}]\n  \
                expectError: {category: ValidationError}\n";
    assert_that!(run(&with_stub(rest)).unwrap_err()).contains("expected an error");
}

#[test]
fn case_without_expectations_fails() {
    let rest = "  messages: [{version: \"1.0\", createSurface: {surfaceId: s1}}]\n";
    assert_that!(run(&with_stub(rest)).unwrap_err()).contains("expect");
}

#[test]
fn unexpected_refusal_fails() {
    let rest = format!(
        "  messages: [{}]\n  expect: {{surfaces: {{s1: {{exists: true}}}}}}\n",
        orphan_create("s1"),
    );
    assert_that!(run(&with_stub(&rest)).unwrap_err()).contains("not reachable");
}

#[test]
fn unknown_step_key_fails() {
    let rest = "  steps:\n    - {messages: [], surprise: 1, expect: {surfaces: {}}}\n";
    assert_that!(run(&with_stub(rest)).unwrap_err()).contains("Unknown key 'surprise'");
}
