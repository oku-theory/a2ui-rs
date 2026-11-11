# a2ui-rs Plan 2: Catalogs and schemas Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Pass stage C2 of the upstream harness: the `core/catalog.yaml`, `core/common_types.yaml` and `core/agent_to_renderer.yaml` cases that declare protocol 1.0 (or no version). 55 of the 62 runnable cases pass; 7 are skipped by name with a reason (6 need Plan 3 component validation, 1 is implicitly v0.9).

**Architecture:** `a2ui-runtime` gains four pure modules. `spec` embeds the upstream v1.0 documents (`common_types.json`, `agent_to_renderer.json`, the basic catalog) with `include_str!`. `common_types` holds serde models of the common-type `$defs` and `validate_common_type`. `catalog` loads a catalog document (`Catalog::from_json`), writes it back (`to_json`), serves the basic catalog (`Catalog::basic`), builds the self-contained `validation_schema` and narrows a catalog with `derive`. `ident` shares the UAX #31 check between expressions and catalogs. `a2ui-conformance` keys its skip list by trial name and protocol version, matches error messages as the upstream harness does, and gains one handler per C2 action.

**Tech Stack:** Rust 2024 edition (rust-version 1.88), `serde` + `serde_json` (feature `preserve_order`, new), `snafu` 0.9, `unicode-ident`, `libtest-mimic`, `serde_yaml_ng`, `regex`, `toml` 1, `speculoos`, `rstest` 0.28, cargo-nextest, just.

**Spec:** `docs/specs/2026-11-10-a2ui-rs-leptos-design.md`. Upstream references at commit `3295e67b5f4820639aa88c578a64af2196d0b752` of github.com/a2ui-project/a2ui: `conformance/conformance_harness_implementation_guide.md`, the Python SDK `agent_sdks/python/src/a2ui/schema/catalog.py` (`from_json`, `to_json`, `validation_schema`, `derive`) and its harness `agent_sdks/python/tests/conformance/test_conformance.py` (`_exception_matches_expected_message`, `normalize_set_keywords`).

**Oku tasks:** parent t-1544; Tasks 1–8 below are t-1554 … t-1561 in order. Mark each sub-task done with `oku cli task done --project slider --id t-N --result "…"` when its task is committed.

## Plan series

The spec is delivered in six plans that follow the upstream harness stages; each ends with working, tested software. Plans 3–6 are written when the previous plan lands, against the APIs it actually produced.

| Plan | Stage | Suites turned on |
|---|---|---|
| 1 | C1 data model and expressions | `data_model`, `data_context`, `expressions` (parse) |
| 2 (this) | C2 catalogs and schemas | `catalog`, `common_types`, `agent_to_renderer` |
| 3 | C3 validation and message processing | `validator_v1_0`, `error_responses`, `message_processor_v1_0`, `composition_constraints`, `reserved_keys`, `validation_result`, `data_model_pointers`, `data_deletion`, `expressions` (validate) |
| 4 | C4 functions, actions, RPC, regex guard, `a2ui-catalog-basic` | `functions`, `index_function`, `multi_catalog`, `actions`, `rpc_functions`, `agent_function_routing` |
| 5 | C5 node resolution and timing | `node_resolution`, `message_processor_scaling` |
| 6 | `a2ui-leptos`, gallery, release 0.1 | adapter suites run in the browser |

## Decisions

- **D1 Embedded spec documents.** `vendor/sync.sh` also copies `common_types.json`, `agent_to_renderer.json` and `catalogs/basic/v1/catalog.json` (as `basic_catalog.json`) plus the upstream `LICENSE` into `crates/a2ui-runtime/spec/`; `spec.rs` embeds them with `include_str!`. Reason: a published crate cannot `include_str!` from `vendor/`; conformance compares against `vendor/`, so any drift fails CI, and a unit test checks the typed models' field sets against the embedded `$defs`.
- **D2 No JSON Schema validator in Plan 2.** `validate_common_type` round-trips through hand-written serde models in `a2ui-runtime::common_types`. Reason: Plan 3 chooses the validator; the models are reused for message parsing.
- **D3 The builtin basic catalog is `from_json` of the embedded document.** `to_json` returns the authored document minus the entries the unions leave out. Reason: `to_json` and `round_trip` then hold by construction.
- **D4 A catalog without `protocolVersion` loads as v1.0.** `protocol_version()` reports the declared string, or `"1.0"` when absent; the implicitly-v0.9 case is skipped by name. Reason: the runtime is v1.0-only.
- **D5 Error messages match as a case-insensitive substring, else a regex search.** Reason: upstream's Python harness does exactly this (`_exception_matches_expected_message`); `"Unsupported external $ref"` can never match as a regex.
- **D6 Schema comparisons.** `common_types_schema` and `agent_to_renderer_schema` compare exactly (`json_eq`); `to_json`, `round_trip`, `validation_schema` and `derive` compare after `normalize_set_keywords` (sorted `enum` and `required`, recursively), with `schema_eq`. Reason: upstream compares those the same way.
- **D7 Skip list by trial name and version.** `skips.toml` gains `[versions]` (each entry checked by the unused-entry guard; `VERSION_NEUTRAL_ACTIONS` stays in code); `[cases]` keys become trial names `core/<suite>::<case>`; `Case::at()` gives the `suite#name` location used in every message. Reason: carried from the Plan 1 final review; upstream repeats case names across suites.
- **D8 New error categories.** `Category::Catalog` (`"CatalogError"`) and `Category::Validation` (`"ValidationError"`) with the variants listed in Tasks 3, 4, 6, 7; `Error::code()` stays deferred to Plan 3. Reason: the suites assert those categories.
- **D9 One UAX #31 check.** `is_identifier` moves from `expression/parser.rs` to `ident.rs`. Reason: catalogs and expressions share it (one implementation per concept).
- **D10 `$ref` targets are classified, never fetched.** Reason: the spec and the reference loader forbid fetching; the runtime has no I/O.
- **D11 Other protocol versions are refused.** `from_json` with a version other than `1.0`/`v1.0` (declared or passed) fails with `CatalogError` `Unsupported protocol version`. Reason: a v1.0-only runtime must not silently load an older catalog.
- **D12 Deviation from outline: fallible signatures.** `common_types_schema()`, `agent_to_renderer_schema()`, `Catalog::basic()` and `Catalog::validation_schema()` return `Result<_, Error>` instead of `Value`/`Option`, and the builtin catalog is `Catalog::basic()` rather than `builtin_catalog(name)`. Reason: parsing an embedded document is fallible and production code may not `unwrap`; a parse failure is `EmbeddedDocument`/`EmbeddedDefs` (`ParseError`).
- **D13 An unlisted protocol version fails its trial.** `Skips::reason` returns `Result`; a case declaring a version that is neither 1.0 nor in `[versions]` is recorded `failed`. Reason: a new upstream version must be triaged, not silently run or hidden.
- **D14 Document order is preserved.** `serde_json` gets the `preserve_order` feature (indexmap); `data.rs` removes keys with `shift_remove`; `deny.toml` allows `Zlib` again (`foldhash`, via indexmap). Reason: the `anyComponent`/`anyFunction` member order and entry order are compared with upstream goldens.
- **D15 Stricter loading than upstream.** A non-object `$defs`, `theme` or `properties`, a `returnType` outside the eight types, a bad `allowedCallers`, a non-boolean `requiresUserActivation` are `Malformed catalog`; the component name `Surface` is reserved. Reason: no silent fallback hides a malformed catalog.
- **D16 Only `#/$defs/...` references are localized and bundled.** Other local refs (`#/components/X`) occur only in the regenerated unions and are kept. Reason: both upstream goldens are reproduced without inlining.
- **D17 `COMMON_TYPES` lists the 12 modeled definitions.** The harness rejects a `validate_common_type` case naming any other. Reason: an unmodeled name would make every `expectError` step pass vacuously.
- **D18 `selfContained` and `matchesBuiltinCatalog` are harness checks.** They live in `handlers/catalog_checks.rs`; a `matchesBuiltinCatalog` catalog whose id is not the basic catalog fails. Reason: they are test oracles, not runtime API.
- **D19 `derive` semantics.** The source loads with no overrides; the derived document drops `$id`, `title`, `description`, `instructions`, writes `catalogId` only when the source declared one, keeps entries in list order (a later item replaces an earlier one in place), regenerates a union only when its names changed (`{"not": {}}` when empty) and drops a leaf `$defs` entry only when the old entries reached it and the new ones do not. An unknown name is `CatalogError` naming the source catalog, the kind and the name (not the target id, which keeps `Error` within clippy's `result_large_err` limit of 128 bytes). Reason: reproduces the Python `derive` on all 7 cases.

## Global Constraints

- Repository `/Users/matthieu/Projects/a2ui-rs`; license MIT OR Apache-2.0; vendored upstream files and `crates/a2ui-runtime/spec/` are Apache-2.0 and keep their `LICENSE`.
- Protocol v1.0 only. Cases declaring another version are skipped through `skips.toml` `[versions]`, except the version-neutral actions `data_model`, `resolve_path`, `parse_expression_template`; a version neither 1.0 nor listed fails its trial.
- Snafu (0.9) errors with a `backtrace: Backtrace` in every variant; one opaque public `Error` exposing `category()`; `Error` stays within 128 bytes (clippy `result_large_err`).
- Never `unwrap`/`expect` outside tests; no silent fallback hiding a bug; no `#[allow]` (use `#[expect(..., reason = "...")]`); no dead code.
- Never depend on `thiserror` or `chrono`; every dependency version lives in `[workspace.dependencies]`, crates use `.workspace = true`.
- Files aim at 500 lines, never above 1000; split by concept.
- Unit tests in a sibling `x_tests.rs` included by `#[cfg(test)] #[path = "x_tests.rs"] mod tests;`, asserting with speculoos, `rstest` for tables, names `unit_scenario_result`, one behavior per test.
- Every pub item and module documented, first sentence under 15 words; each crate keeps an `ARCHITECTURE.md` of at most 50 lines, committed with the code it describes.
- `a2ui-runtime` stays free of Leptos, wasm-bindgen, async runtimes, randomness and I/O (`include_str!` of `spec/` is the only file access), and builds for `wasm32-unknown-unknown`.
- Conformance handlers are strict: every case key is checked (`assert_case_keys`, `assert_known_keys`), every expectation is compared, a malformed expectation field is an error, never a lenient default.
- Error messages are English and contain the phrases the conformance suites match: `Forbidden path segment`, `Cannot set path`, `non-numeric segment`, `Invalid number literal`, `out of range`, `Unexpected characters`, `after argument name`, `after function arguments`, `Unclosed interpolation`, `Max recursion depth`, `catalog_id must be provided or exist in catalog schema`, `Unsupported external $ref`, `Invalid UAX #31 component identifier`, `requiresUserActivation`, and the unknown entry name in `derive` errors.
- Conventional Commits in English, citing the Oku sub-task: `type(scope): description (t-N)`. No `Co-Authored-By` line.

Build note: every command runs from the repository root. Counts below come from `cargo test -p a2ui-conformance --test core 2>&1 | tail -3`; the baseline before Task 1 is `test result: ok. 160 passed; 0 failed; 685 ignored` (159 cases plus the `skips_toml_entries_all_match_a_case` guard).

---

### Task 1: Harness groundwork: skips by trial name and version, upstream error matching (t-1554)

**Files:**
- Modify: `crates/a2ui-conformance/src/case.rs`, `case_tests.rs`, `skips.rs`, `skips_tests.rs`, `compare.rs`, `compare_tests.rs`, `keys.rs`, `lib.rs`, `handlers/data_model.rs`, `handlers/expression.rs`, `handlers/resolve_path.rs`, `tests/core.rs`, `skips.toml`, `ARCHITECTURE.md`

**Interfaces:**
- Consumes: Plan 1 `Case`, `Skips`, `expect_error`, `json_eq`, `Manifest`.
- Produces:
  - `Case::at(&self) -> String` (`suite#name`) and `Case::trial_name(&self) -> String` (`core/<suite>::<case>`, the `.yaml` dropped).
  - `Skips::reason(&self, case: &Case) -> Result<Option<String>, String>`: `[cases]` by trial name, then `[actions]`, then the version (none, `1.0` or a version-neutral action → `Ok(None)`; listed → its reason; unlisted → `Err`). `Skips::unused()` also reports `versions.V` entries no non-neutral case declares.
  - `pub fn normalize_set_keywords(value: &Value) -> Value` and `pub fn schema_eq(a: &Value, b: &Value) -> bool`.
  - `expect_error`: message is a case-insensitive substring, else a regex search; a bad regex that is not a substring fails with `bad message regex`.

- [ ] **Step 1: Write the failing tests**

Apply to `crates/a2ui-conformance/src/case_tests.rs`:

```diff
--- a/crates/a2ui-conformance/src/case_tests.rs
+++ b/crates/a2ui-conformance/src/case_tests.rs
@@ -53,3 +53,15 @@
 fn parse_suite_case_without_action_is_error() {
     assert_that!(parse_suite("core/x.yaml", "- name: test_x\n")).is_err();
 }
+
+#[test]
+fn at_joins_suite_and_name() {
+    let cases = parse_suite("core/x.yaml", SUITE).unwrap();
+    assert_that!(cases[0].at()).is_equal_to("core/x.yaml#test_top_level_version".to_owned());
+}
+
+#[test]
+fn trial_name_drops_the_yaml_extension() {
+    let cases = parse_suite("core/x.yaml", SUITE).unwrap();
+    assert_that!(cases[0].trial_name()).is_equal_to("core/x::test_top_level_version".to_owned());
+}
```

Replace `crates/a2ui-conformance/src/skips_tests.rs` with:

```rust
use speculoos::prelude::*;

use super::*;
use crate::case::parse_suite;

const SKIPS: &str = r#"
[actions]
validate = "Plan 3"

[versions]
"v0.9" = "protocol v0.9"

[cases]
"core/x::test_named" = "known gap"
"#;

fn case(yaml: &str) -> Case {
    parse_suite("core/x.yaml", yaml).unwrap().remove(0)
}

#[test]
fn reason_named_case_wins() {
    let skips = Skips::parse(SKIPS).unwrap();
    let reason = skips.reason(&case("- {name: test_named, action: data_model}"));
    assert_that!(reason).is_equal_to(Ok(Some("known gap".to_owned())));
}

#[test]
fn reason_case_name_in_another_suite_runs() {
    let skips = Skips::parse(SKIPS).unwrap();
    let other = parse_suite("core/y.yaml", "- {name: test_named, action: data_model}")
        .unwrap()
        .remove(0);
    assert_that!(skips.reason(&other)).is_equal_to(Ok(None));
}

#[test]
fn reason_unsupported_action_is_reported() {
    let skips = Skips::parse(SKIPS).unwrap();
    let reason = skips.reason(&case(
        "- {name: test_x, action: validate, protocolVersion: '1.0'}",
    ));
    assert_that!(reason).is_equal_to(Ok(Some("Plan 3".to_owned())));
}

#[test]
fn reason_listed_old_version_is_skipped() {
    let skips = Skips::parse(SKIPS).unwrap();
    let reason = skips.reason(&case(
        "- {name: test_x, action: from_json, protocolVersion: v0.9}",
    ));
    assert_that!(reason).is_equal_to(Ok(Some("protocol v0.9".to_owned())));
}

#[test]
fn reason_unlisted_old_version_is_error() {
    let skips = Skips::parse(SKIPS).unwrap();
    let reason = skips.reason(&case(
        "- {name: test_x, action: from_json, protocolVersion: v0.8}",
    ));
    assert_that!(reason).is_err();
}

#[test]
fn reason_version_neutral_action_runs_on_old_version() {
    let skips = Skips::parse(SKIPS).unwrap();
    let reason = skips.reason(&case(
        "- {name: test_x, action: data_model, catalog: {protocolVersion: v0.9}}",
    ));
    assert_that!(reason).is_equal_to(Ok(None));
}

#[test]
fn unused_entry_without_matching_case_is_listed() {
    let skips = Skips::parse(SKIPS).unwrap();
    let cases = vec![case("- {name: test_x, action: data_model}")];
    assert_that!(skips.unused(&cases)).is_equal_to(vec![
        "actions.validate".to_owned(),
        "versions.v0.9".to_owned(),
        "cases.core/x::test_named".to_owned(),
    ]);
}

#[test]
fn unused_version_declared_only_by_neutral_action_is_listed() {
    let skips = Skips::parse(SKIPS).unwrap();
    let cases = vec![case(
        "- {name: test_named, action: data_model, catalog: {protocolVersion: v0.9}}",
    )];
    assert_that!(skips.unused(&cases)).contains("versions.v0.9".to_owned());
}
```

Apply to `crates/a2ui-conformance/src/compare_tests.rs`:

```diff
--- a/crates/a2ui-conformance/src/compare_tests.rs
+++ b/crates/a2ui-conformance/src/compare_tests.rs
@@ -1,3 +1,4 @@
+use a2ui_runtime::Pointer;
 use rstest::rstest;
 use serde_json::json;
 use speculoos::prelude::*;
@@ -20,9 +21,34 @@
     assert_that!(json_eq(&a, &b)).is_false();
 }
 
-use a2ui_runtime::Pointer;
+#[test]
+fn normalize_set_keywords_sorts_nested_enum_and_required() {
+    let schema = json!({"properties": {"a": {"enum": ["z", "a"]}}, "required": ["b", "a"]});
+    let expected = json!({"properties": {"a": {"enum": ["a", "z"]}}, "required": ["a", "b"]});
+    assert_that!(normalize_set_keywords(&schema)).is_equal_to(expected);
+}
 
 #[test]
+fn normalize_set_keywords_keeps_other_array_order() {
+    let schema = json!({"oneOf": [{"$ref": "#/b"}, {"$ref": "#/a"}]});
+    assert_that!(normalize_set_keywords(&schema)).is_equal_to(schema.clone());
+}
+
+#[rstest]
+#[case(json!({"required": ["b", "a"]}), json!({"required": ["a", "b"]}))]
+#[case(json!({"enum": [2, 1], "x": 1}), json!({"x": 1.0, "enum": [1, 2]}))]
+fn schema_eq_ignores_set_keyword_order(#[case] a: Value, #[case] b: Value) {
+    assert_that!(schema_eq(&a, &b)).is_true();
+}
+
+#[test]
+fn schema_eq_keeps_one_of_order() {
+    let a = json!({"oneOf": [{"type": "string"}, {"type": "number"}]});
+    let b = json!({"oneOf": [{"type": "number"}, {"type": "string"}]});
+    assert_that!(schema_eq(&a, &b)).is_false();
+}
+
+#[test]
 fn expect_error_matching_category_and_message_passes() {
     let err = Pointer::parse("/__proto__").unwrap_err();
     let expected = json!({"category": "DataError", "message": "Forbidden path segment"});
@@ -30,6 +56,36 @@
 }
 
 #[test]
+fn expect_error_message_matches_as_case_insensitive_substring() {
+    let err = Pointer::parse("/__proto__").unwrap_err();
+    let expected =
+        json!({"category": "DataError", "message": "forbidden PATH segment '__proto__'"});
+    assert_that!(expect_error(&err, &expected, "here")).is_ok();
+}
+
+#[test]
+fn expect_error_message_with_regex_metacharacters_matches_as_substring() {
+    let err = Pointer::parse("/a~2").unwrap_err();
+    let expected = json!({"category": "DataError", "message": "Invalid JSON Pointer '/a~2"});
+    assert_that!(expect_error(&err, &expected, "here")).is_ok();
+}
+
+#[test]
+fn expect_error_message_matches_as_regex() {
+    let err = Pointer::parse("/__proto__").unwrap_err();
+    let expected = json!({"category": "DataError", "message": "^Forbidden .* segment"});
+    assert_that!(expect_error(&err, &expected, "here")).is_ok();
+}
+
+#[test]
+fn expect_error_bad_regex_without_substring_match_fails() {
+    let err = Pointer::parse("/__proto__").unwrap_err();
+    let expected = json!({"category": "DataError", "message": "(unclosed"});
+    let result = expect_error(&err, &expected, "here");
+    assert_that!(result.unwrap_err().contains("bad message regex")).is_true();
+}
+
+#[test]
 fn expect_error_wrong_category_fails() {
     let err = Pointer::parse("/__proto__").unwrap_err();
     assert_that!(expect_error(
@@ -52,7 +108,6 @@
     let err = Pointer::parse("/__proto__").unwrap_err();
     let expected = json!({"category": "DataError", "message": 5});
     let result = expect_error(&err, &expected, "here");
-    assert_that!(result).is_err();
     assert_that!(result.unwrap_err().contains("must be a string")).is_true();
 }
 
```

- [ ] **Step 2: Run the tests to see them fail**

Run: `cargo test -p a2ui-conformance --lib`
Expected: FAIL to compile: `no method named 'at' found for struct 'Case'`, `cannot find function 'normalize_set_keywords'`, and mismatched types on `reason(...)` (still `Option`).

- [ ] **Step 3: Implement**

Apply to `crates/a2ui-conformance/src/case.rs`:

```diff
--- a/crates/a2ui-conformance/src/case.rs
+++ b/crates/a2ui-conformance/src/case.rs
@@ -30,6 +30,16 @@
     pub fn get(&self, key: &str) -> Option<&Value> {
         self.body.get(key)
     }
+
+    /// Where the case lives, as `suite#name`, for failure messages.
+    pub fn at(&self) -> String {
+        format!("{}#{}", self.suite, self.name)
+    }
+
+    /// The test name, such as `core/catalog::test_x`, unique across suites.
+    pub fn trial_name(&self) -> String {
+        format!("{}::{}", self.suite.trim_end_matches(".yaml"), self.name)
+    }
 }
 
 /// Parses a suite's YAML text into its cases; an empty suite is an error.
```

Replace `crates/a2ui-conformance/src/skips.rs` with:

```rust
//! The skip list: unsupported actions, skipped cases and unsupported versions.

use std::collections::BTreeMap;

use serde::Deserialize;

use crate::case::Case;

/// The protocol version this runtime implements.
const SUPPORTED_VERSION: &str = "1.0";

/// Actions whose behavior does not depend on the declared protocol version.
const VERSION_NEUTRAL_ACTIONS: [&str; 3] =
    ["data_model", "resolve_path", "parse_expression_template"];

/// Skip entries read from `skips.toml`, each with its reason.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Skips {
    #[serde(default)]
    actions: BTreeMap<String, String>,
    #[serde(default)]
    versions: BTreeMap<String, String>,
    #[serde(default)]
    cases: BTreeMap<String, String>,
}

impl Skips {
    /// Parses the TOML text of `skips.toml`.
    pub fn parse(text: &str) -> Result<Self, String> {
        toml::from_str(text).map_err(|err| format!("skips.toml: {err}"))
    }

    /// Why `case` is skipped, `None` when it runs, `Err` for an unlisted version.
    ///
    /// `[cases]` is keyed by [`Case::trial_name`]; a version other than `1.0`
    /// must be listed under `[versions]` unless the action is version-neutral.
    pub fn reason(&self, case: &Case) -> Result<Option<String>, String> {
        if let Some(reason) = self.cases.get(&case.trial_name()) {
            return Ok(Some(reason.clone()));
        }
        if let Some(reason) = self.actions.get(&case.action) {
            return Ok(Some(reason.clone()));
        }
        let Some(version) = case.protocol_version() else {
            return Ok(None);
        };
        if version == SUPPORTED_VERSION || is_version_neutral(case) {
            return Ok(None);
        }
        match self.versions.get(version) {
            Some(reason) => Ok(Some(reason.clone())),
            None => Err(format!(
                "{}: protocol version {version} is neither 1.0 nor listed in skips.toml [versions]",
                case.at()
            )),
        }
    }

    /// Entries that match no case, as `actions.<name>`, `versions.<v>` or `cases.<trial>`.
    pub fn unused(&self, cases: &[Case]) -> Vec<String> {
        let actions = self
            .actions
            .keys()
            .filter(|action| !cases.iter().any(|case| &case.action == *action))
            .map(|action| format!("actions.{action}"));
        let versions = self
            .versions
            .keys()
            .filter(|version| {
                !cases.iter().any(|case| {
                    !is_version_neutral(case) && case.protocol_version() == Some(version.as_str())
                })
            })
            .map(|version| format!("versions.{version}"));
        let named = self
            .cases
            .keys()
            .filter(|trial| !cases.iter().any(|case| &case.trial_name() == *trial))
            .map(|trial| format!("cases.{trial}"));
        actions.chain(versions).chain(named).collect()
    }
}

fn is_version_neutral(case: &Case) -> bool {
    VERSION_NEUTRAL_ACTIONS.contains(&case.action.as_str())
}

#[cfg(test)]
#[path = "skips_tests.rs"]
mod tests;
```

Replace `crates/a2ui-conformance/src/compare.rs` with:

```rust
//! JSON comparisons and error matching used by every expectation.

use serde_json::Value;

/// Deep equality where numbers compare as `f64` and object key order is ignored.
pub fn json_eq(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => x.as_f64() == y.as_f64(),
        (Value::Array(xs), Value::Array(ys)) => {
            xs.len() == ys.len() && xs.iter().zip(ys).all(|(x, y)| json_eq(x, y))
        }
        (Value::Object(xs), Value::Object(ys)) => {
            xs.len() == ys.len()
                && xs
                    .iter()
                    .all(|(key, x)| ys.get(key).is_some_and(|y| json_eq(x, y)))
        }
        _ => a == b,
    }
}

/// Sorts every `enum` and `required` list, recursively, by its JSON text.
///
/// Upstream's `normalize_set_keywords`: those keywords are sets, other arrays
/// (`oneOf`, `allOf`, `items`) keep their order.
pub fn normalize_set_keywords(value: &Value) -> Value {
    match value {
        Value::Array(items) => Value::Array(items.iter().map(normalize_set_keywords).collect()),
        Value::Object(fields) => {
            let mut normalized: serde_json::Map<String, Value> = fields
                .iter()
                .map(|(key, value)| (key.clone(), normalize_set_keywords(value)))
                .collect();
            for keyword in ["enum", "required"] {
                if let Some(Value::Array(items)) = normalized.get_mut(keyword) {
                    items.sort_by_key(Value::to_string);
                }
            }
            Value::Object(normalized)
        }
        _ => value.clone(),
    }
}

/// [`json_eq`] after [`normalize_set_keywords`] on both sides.
pub fn schema_eq(a: &Value, b: &Value) -> bool {
    json_eq(&normalize_set_keywords(a), &normalize_set_keywords(b))
}

/// Checks `actual` against an `expectError` / `expect_error` value `{category, message?}`.
///
/// `message` matches as a case-insensitive substring or, failing that, as a
/// regular expression, as upstream's reference harness does
/// (`_exception_matches_expected_message` in `test_conformance.py`):
/// `"Unsupported external $ref"` only matches as a substring.
pub fn expect_error(
    actual: &a2ui_runtime::Error,
    expected: &Value,
    at: &str,
) -> Result<(), String> {
    let Value::Object(fields) = expected else {
        return Err(format!("{at}: expect_error must be a mapping"));
    };
    crate::keys::assert_known_keys(fields, &["category", "message"], at)?;
    let category = fields.get("category").and_then(Value::as_str);
    if category != Some(actual.category().as_str()) {
        return Err(format!(
            "{at}: expected {category:?}, got {}: {actual}",
            actual.category().as_str()
        ));
    }
    let pattern = match fields.get("message") {
        None => return Ok(()),
        Some(Value::String(pattern)) => pattern,
        Some(other) => return Err(format!("{at}: message must be a string, got {other}")),
    };
    let message = actual.to_string();
    if message.to_lowercase().contains(&pattern.to_lowercase()) {
        return Ok(());
    }
    let regex =
        regex::Regex::new(pattern).map_err(|err| format!("{at}: bad message regex: {err}"))?;
    if regex.is_match(&message) {
        Ok(())
    } else {
        Err(format!(
            "{at}: message {message} does not match /{pattern}/"
        ))
    }
}

#[cfg(test)]
#[path = "compare_tests.rs"]
mod tests;
```

Apply the location and export changes:

```diff
--- a/crates/a2ui-conformance/src/keys.rs
+++ b/crates/a2ui-conformance/src/keys.rs
@@ -21,5 +21,5 @@
 /// Checks a case's top-level keys; `name`, `description`, `features`, `action` are implied.
 pub fn assert_case_keys(case: &Case, allowed: &[&str]) -> Result<(), String> {
     let all: Vec<&str> = CASE_METADATA.iter().chain(allowed).copied().collect();
-    assert_known_keys(&case.body, &all, &format!("{}#{}", case.suite, case.name))
+    assert_known_keys(&case.body, &all, &case.at())
 }
```

```diff
--- a/crates/a2ui-conformance/src/lib.rs
+++ b/crates/a2ui-conformance/src/lib.rs
@@ -12,7 +12,7 @@
 mod skips;
 
 pub use case::{Case, load_suite, parse_suite};
-pub use compare::{expect_error, json_eq};
+pub use compare::{expect_error, json_eq, normalize_set_keywords, schema_eq};
 pub use keys::{assert_case_keys, assert_known_keys};
 pub use manifest::{Manifest, Status};
 pub use paths::upstream_root;
```

```diff
--- a/crates/a2ui-conformance/src/handlers/data_model.rs
+++ b/crates/a2ui-conformance/src/handlers/data_model.rs
@@ -54,7 +54,7 @@
         .and_then(Value::as_array)
         .ok_or("a case needs `steps`")?;
     for (index, step) in steps.iter().enumerate() {
-        let at = format!("{}#{} step {index}", case.suite, case.name);
+        let at = format!("{} step {index}", case.at());
         let step = step
             .as_object()
             .ok_or_else(|| format!("{at}: a step is a mapping"))?;
```

```diff
--- a/crates/a2ui-conformance/src/handlers/expression.rs
+++ b/crates/a2ui-conformance/src/handlers/expression.rs
@@ -10,7 +10,7 @@
 /// Parses `input` and compares the parts with `expect`, or the error with `expect_error`.
 pub fn run(case: &Case) -> Result<(), String> {
     assert_case_keys(case, &["input", "expect", "expect_error"])?;
-    let at = format!("{}#{}", case.suite, case.name);
+    let at = case.at();
     let input = case
         .get("input")
         .and_then(Value::as_str)
```

```diff
--- a/crates/a2ui-conformance/src/handlers/resolve_path.rs
+++ b/crates/a2ui-conformance/src/handlers/resolve_path.rs
@@ -9,7 +9,7 @@
 /// Resolves `args.path` against `args.contextPath` and compares with `expect`.
 pub fn run(case: &Case) -> Result<(), String> {
     assert_case_keys(case, &["catalog", "protocolVersion", "args", "expect"])?;
-    let at = format!("{}#{}", case.suite, case.name);
+    let at = case.at();
     let args = case
         .get("args")
         .and_then(Value::as_object)
```

```diff
--- a/crates/a2ui-conformance/tests/core.rs
+++ b/crates/a2ui-conformance/tests/core.rs
@@ -61,8 +61,20 @@
 }
 
 fn trial(case: Case, skips: &Skips, manifest: Option<Arc<Manifest>>) -> Result<Trial, String> {
-    let name = format!("{}::{}", case.suite.trim_end_matches(".yaml"), case.name);
-    if let Some(reason) = skips.reason(&case) {
+    let name = case.trial_name();
+    let reason = match skips.reason(&case) {
+        Ok(reason) => reason,
+        // An unlisted protocol version fails its trial rather than running or hiding.
+        Err(message) => {
+            if let Some(manifest) = &manifest {
+                manifest.record(&case.suite, &case.name, Status::Failed, None)?;
+            }
+            return Ok(Trial::test(name, move || {
+                Err(Failed::from(message.clone()))
+            }));
+        }
+    };
+    if let Some(reason) = reason {
         if let Some(manifest) = &manifest {
             manifest.record(&case.suite, &case.name, Status::Skipped, Some(&reason))?;
         }
@@ -74,10 +86,7 @@
     Ok(Trial::test(name, move || {
         let outcome = match handlers::handler(&case.action) {
             Some(run) => run(&case),
-            None => Err(format!(
-                "Unknown action '{}' in {}#{}",
-                case.action, case.suite, case.name
-            )),
+            None => Err(format!("Unknown action '{}' in {}", case.action, case.at())),
         };
         if let Some(manifest) = &manifest {
             let status = if outcome.is_ok() {
```

Apply to `crates/a2ui-conformance/skips.toml` (the three versions are every non-1.0 version the suites declare: `grep -ho 'protocolVersion: *"[^"]*"' vendor/a2ui/conformance/core/*.yaml | sort -u`):

```diff
--- a/crates/a2ui-conformance/skips.toml
+++ b/crates/a2ui-conformance/skips.toml
@@ -22,5 +22,11 @@
 render_surface = "framework adapter suite: run by a2ui-leptos in the browser (Plan 6)"
 accessibility_check = "framework adapter suite: run by a2ui-leptos in the browser (Plan 6)"
 
-# Individual cases skipped on purpose.
+# Protocol versions other than 1.0; version-neutral actions run whatever they declare.
+[versions]
+"v0.8" = "protocol version v0.8 is not supported: a2ui-rs implements v1.0 only"
+"v0.9" = "protocol version v0.9 is not supported: a2ui-rs implements v1.0 only"
+"v0.9.1" = "protocol version v0.9.1 is not supported: a2ui-rs implements v1.0 only"
+
+# Individual cases skipped on purpose, keyed by trial name (`core/<suite>::<case>`).
 [cases]
```

- [ ] **Step 4: Run the unit tests**

Run: `cargo test -p a2ui-conformance --lib`
Expected: PASS, 0 failed.

- [ ] **Step 5: Run the conformance suites**

Run: `cargo test -p a2ui-conformance --test core 2>&1 | tail -3`
Expected: `test result: ok. 160 passed; 0 failed; 685 ignored` (unchanged: every former version skip now comes from `[versions]`, and the guard passes, so each `[versions]` entry matches a case).

- [ ] **Step 6: Update `crates/a2ui-conformance/ARCHITECTURE.md`**

Replace the `case`, `skips` and `compare` bullets with:

```markdown
- `case`: `Case` (suite, name, action, body), `parse_suite`, `load_suite`; `at()` (`suite#name`,
  for messages) and `trial_name()` (`core/<suite>::<case>`); the protocol version is the
  top-level `protocolVersion`, else `catalog.protocolVersion`.
- `skips`: `skips.toml`: `[actions]`, `[versions]` and `[cases]` (keyed by trial name), each with a
  reason. Version 1.0 and the version-neutral `data_model`, `resolve_path`,
  `parse_expression_template` run; a version neither 1.0 nor listed fails its trial. An entry
  matching no case fails `skips_toml_entries_all_match_a_case`.
- `compare`: `json_eq` (numbers as `f64`, key order ignored); `schema_eq` after
  `normalize_set_keywords` (sorted `enum`, `required`); `expect_error` (category, message as a
  case-insensitive substring, else a regex, as upstream's Python harness).
```

- [ ] **Step 7: Run the gate**

Run: `just check && just test`
Expected: success.

- [ ] **Step 8: Commit**

```bash
git add -A
git commit -m "test(conformance): key skips by trial name and version, match errors as upstream (t-1554)"
oku cli task done --project slider --id t-1554 --result "skips.toml [versions] and trial-name [cases]; substring-or-regex messages; schema_eq; 160 passed"
```

---

### Task 2: Vendoring, embedded spec documents and schema export actions (t-1555)

**Files:**
- Modify: `vendor/sync.sh`; generated: `vendor/a2ui/catalogs/mcp/v1/**`, `vendor/a2ui/catalogs/vegachart/v1/**`
- Create (generated by `vendor/sync.sh`): `crates/a2ui-runtime/spec/common_types.json`, `spec/agent_to_renderer.json`, `spec/basic_catalog.json`, `spec/LICENSE`
- Create: `crates/a2ui-runtime/src/ident.rs`, `ident_tests.rs`, `spec.rs`, `spec_tests.rs`
- Modify: `crates/a2ui-runtime/src/lib.rs`, `error.rs`, `expression/parser.rs`, `ARCHITECTURE.md`
- Create: `crates/a2ui-conformance/src/handlers/schema_export.rs`
- Modify: `crates/a2ui-conformance/src/paths.rs`, `src/lib.rs`, `src/handlers/mod.rs`, `skips.toml`, `ARCHITECTURE.md`

**Interfaces:**
- Consumes: Task 1 `Case::at`, `json_eq`.
- Produces:
  - `pub fn common_types_schema() -> Result<Value, Error>`, `pub fn agent_to_renderer_schema() -> Result<Value, Error>`; `pub(crate) fn spec::parse(name: &'static str, text: &str) -> Result<Value, Error>`.
  - `pub(crate) fn ident::is_identifier(name: &str) -> bool`.
  - Variant `EmbeddedDocument { name: &'static str, source: serde_json::Error }` → `Category::Parse`, display `Embedded document {name} is not valid JSON: {source}`.
  - Harness: `pub fn read_json(relative: &str) -> Result<Value, String>` (a file under the upstream root); actions `common_types_schema`, `agent_to_renderer_schema`.

- [ ] **Step 1: Vendor the extra catalogs and the spec documents**

Replace `vendor/sync.sh` with:

```bash
#!/usr/bin/env bash
# Copies upstream A2UI v1.0 specification, catalogs and conformance suites at a pinned commit,
# and embeds the documents a2ui-runtime serves into crates/a2ui-runtime/spec/.
set -euo pipefail
commit="${1:?usage: vendor/sync.sh <upstream-commit>}"
root="$(cd "$(dirname "$0")" && pwd)"
spec="$root/../crates/a2ui-runtime/spec"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
git clone --quiet --filter=blob:none --no-checkout https://github.com/a2ui-project/a2ui.git "$work/a2ui"
git -C "$work/a2ui" sparse-checkout set --no-cone \
  /LICENSE /specification/v1_0 /catalogs/basic/v1 /catalogs/mcp/v1 /catalogs/vegachart/v1 \
  /conformance/core /conformance/test_data /conformance/README.md \
  /conformance/conformance_schema.json /conformance/executed_case_manifest.schema.json \
  /conformance/manifest_corpora.json /conformance/conformance_harness_implementation_guide.md
git -C "$work/a2ui" checkout --quiet "$commit"
rm -rf "$root/a2ui"
mkdir -p "$root/a2ui/specification" "$root/a2ui/catalogs/basic" "$root/a2ui/catalogs/mcp" \
  "$root/a2ui/catalogs/vegachart"
cp "$work/a2ui/LICENSE" "$root/a2ui/LICENSE"
cp -R "$work/a2ui/specification/v1_0" "$root/a2ui/specification/v1_0"
cp -R "$work/a2ui/catalogs/basic/v1" "$root/a2ui/catalogs/basic/v1"
cp -R "$work/a2ui/catalogs/mcp/v1" "$root/a2ui/catalogs/mcp/v1"
cp -R "$work/a2ui/catalogs/vegachart/v1" "$root/a2ui/catalogs/vegachart/v1"
cp -R "$work/a2ui/conformance" "$root/a2ui/conformance"
mkdir -p "$spec"
cp "$work/a2ui/LICENSE" "$spec/LICENSE"
cp "$work/a2ui/specification/v1_0/json/common_types.json" "$spec/common_types.json"
cp "$work/a2ui/specification/v1_0/json/agent_to_renderer.json" "$spec/agent_to_renderer.json"
cp "$work/a2ui/catalogs/basic/v1/catalog.json" "$spec/basic_catalog.json"
printf '%s\n' "$commit" > "$root/UPSTREAM"
```

Run (needs network; on macOS run it outside any sandbox that blocks `mktemp -d`):

```bash
just vendor-sync 3295e67b5f4820639aa88c578a64af2196d0b752
git status --short vendor crates/a2ui-runtime/spec
cmp vendor/a2ui/specification/v1_0/json/common_types.json crates/a2ui-runtime/spec/common_types.json
cmp vendor/a2ui/catalogs/basic/v1/catalog.json crates/a2ui-runtime/spec/basic_catalog.json
```

Expected: `vendor/UPSTREAM` unchanged; the only changes are the new `vendor/a2ui/catalogs/mcp/v1/` (`catalog.json`, `examples/`, `mcp_app_specification.md`), `vendor/a2ui/catalogs/vegachart/v1/catalog.json` and the four files of `crates/a2ui-runtime/spec/`; both `cmp` print nothing.

- [ ] **Step 2: Write the failing tests**

`crates/a2ui-runtime/src/ident_tests.rs`:

```rust
use rstest::rstest;
use speculoos::prelude::*;

use super::*;

#[rstest]
#[case("Text")]
#[case("_private")]
#[case("format_text_1")]
#[case("Component_ℓ")]
#[case("ünïcode")]
fn is_identifier_unicode_identifier_is_accepted(#[case] name: &str) {
    assert_that!(is_identifier(name)).is_true();
}

#[rstest]
#[case("")]
#[case("1st")]
#[case("has-dash")]
#[case("has space")]
#[case("@index")]
#[case("Component\u{2013}Name")]
#[case("prop\u{2014}name")]
#[case("func\u{058A}name")]
fn is_identifier_non_identifier_is_rejected(#[case] name: &str) {
    assert_that!(is_identifier(name)).is_false();
}
```

`crates/a2ui-runtime/src/spec_tests.rs` (`common_types.json` has 21 `$defs`):

```rust
use serde_json::Value;
use speculoos::prelude::*;

use super::*;

#[test]
fn common_types_schema_holds_the_published_defs() {
    let schema = common_types_schema().unwrap();
    let defs = schema["$defs"].as_object().unwrap();
    assert_that!(defs.len()).is_equal_to(21);
}

#[test]
fn agent_to_renderer_schema_is_an_object() {
    assert_that!(agent_to_renderer_schema().unwrap().is_object()).is_true();
}

#[test]
fn parse_malformed_document_is_parse_error() {
    let err = parse("broken.json", "{").unwrap_err();
    assert_that!(err.category().as_str()).is_equal_to("ParseError");
}

#[test]
fn parse_valid_document_returns_value() {
    assert_that!(parse("x.json", "[1]").unwrap()).is_equal_to(Value::from(vec![1]));
}
```

Declare the modules in `crates/a2ui-runtime/src/lib.rs`:

```diff
--- a/crates/a2ui-runtime/src/lib.rs
+++ b/crates/a2ui-runtime/src/lib.rs
@@ -13,7 +13,9 @@
 mod data;
 mod error;
 mod expression;
+mod ident;
 mod pointer;
+mod spec;
 mod subscribe;
 pub use subscribe::Watchers;
 
@@ -22,3 +24,4 @@
 pub use error::{Category, Error};
 pub use expression::{Expr, MAX_DEPTH, MAX_TEMPLATE_CHARS, MAX_TEMPLATE_PARTS, parse_template};
 pub use pointer::{MAX_LIST_INDEX, Pointer};
+pub use spec::{agent_to_renderer_schema, common_types_schema};
```

- [ ] **Step 3: Run the tests to see them fail**

Run: `cargo test -p a2ui-runtime ident spec`
Expected: FAIL to compile: `file not found for module 'ident'` and `'spec'`.

- [ ] **Step 4: Implement**

`crates/a2ui-runtime/src/ident.rs`:

```rust
//! UAX #31 identifiers, as catalog names, arguments and extension keys use them.

use unicode_ident::{is_xid_continue, is_xid_start};

/// Whether `name` is `_` or `XID_Start`, then `XID_Continue` characters.
pub(crate) fn is_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|c| c == '_' || is_xid_start(c)) && chars.all(is_xid_continue)
}

#[cfg(test)]
#[path = "ident_tests.rs"]
mod tests;
```

Move the parser onto it (delete its private `is_identifier`):

```diff
--- a/crates/a2ui-runtime/src/expression/parser.rs
+++ b/crates/a2ui-runtime/src/expression/parser.rs
@@ -1,10 +1,11 @@
 //! Recursive-descent parser for templates, interpolations and calls.
 
 use serde_json::Value;
-use unicode_ident::{is_xid_continue, is_xid_start};
+use unicode_ident::is_xid_continue;
 
 use super::{Expr, MAX_DEPTH, number, syntax};
 use crate::error::Error;
+use crate::ident::is_identifier;
 
 const DELIMITERS: [char; 7] = ['(', ')', ',', ':', '\'', '"', '}'];
 
@@ -215,11 +216,6 @@
     }
 }
 
-fn is_identifier(name: &str) -> bool {
-    let mut chars = name.chars();
-    chars.next().is_some_and(|c| c == '_' || is_xid_start(c)) && chars.all(is_xid_continue)
-}
-
 fn flush(parts: &mut Vec<Expr>, text: &mut String) {
     if !text.is_empty() {
         parts.push(Expr::Literal(Value::String(std::mem::take(text))));
```

`crates/a2ui-runtime/src/spec.rs`:

```rust
//! The published v1.0 specification documents the runtime embeds.
//!
//! `vendor/sync.sh` copies them from upstream into `spec/`; the conformance
//! suites compare them with `vendor/a2ui`, so a stale copy fails CI.

use serde_json::Value;
use snafu::ResultExt;

use crate::error::{EmbeddedDocumentSnafu, Error};

const COMMON_TYPES: &str = include_str!("../spec/common_types.json");
const AGENT_TO_RENDERER: &str = include_str!("../spec/agent_to_renderer.json");

/// The v1.0 `common_types.json` schema, exactly as published.
pub fn common_types_schema() -> Result<Value, Error> {
    parse("common_types.json", COMMON_TYPES)
}

/// The v1.0 `agent_to_renderer.json` schema, exactly as published.
pub fn agent_to_renderer_schema() -> Result<Value, Error> {
    parse("agent_to_renderer.json", AGENT_TO_RENDERER)
}

/// Parses the embedded document `name`.
pub(crate) fn parse(name: &'static str, text: &str) -> Result<Value, Error> {
    Ok(serde_json::from_str(text).context(EmbeddedDocumentSnafu { name })?)
}

#[cfg(test)]
#[path = "spec_tests.rs"]
mod tests;
```

Add the parse error to `crates/a2ui-runtime/src/error.rs`:

```diff
--- a/crates/a2ui-runtime/src/error.rs
+++ b/crates/a2ui-runtime/src/error.rs
@@ -86,6 +86,13 @@
         limit: &'static str,
         backtrace: Backtrace,
     },
+
+    #[snafu(display("Embedded document {name} is not valid JSON: {source}"))]
+    EmbeddedDocument {
+        name: &'static str,
+        source: serde_json::Error,
+        backtrace: Backtrace,
+    },
 }
 
 impl Inner {
@@ -96,7 +103,7 @@
             | Self::TraversePrimitive { .. }
             | Self::NonNumericSegment { .. }
             | Self::IndexOutOfRange { .. } => Category::Data,
-            Self::ExpressionSyntax { .. } => Category::Parse,
+            Self::ExpressionSyntax { .. } | Self::EmbeddedDocument { .. } => Category::Parse,
             Self::ExpressionLimit { .. } => Category::Expression,
         }
     }
```

- [ ] **Step 5: Run the unit tests**

Run: `cargo test -p a2ui-runtime`
Expected: PASS, 0 failed (the expression tests still pass on the shared `is_identifier`).

- [ ] **Step 6: Add the schema export handlers**

Apply to `crates/a2ui-conformance/src/paths.rs`:

```diff
--- a/crates/a2ui-conformance/src/paths.rs
+++ b/crates/a2ui-conformance/src/paths.rs
@@ -1,7 +1,9 @@
-//! Location of the vendored upstream repository.
+//! Location of the vendored upstream repository and the files under it.
 
 use std::path::{Path, PathBuf};
 
+use serde_json::Value;
+
 /// The vendored upstream root, found by walking up to `conformance_schema.json`.
 pub fn upstream_root() -> Result<PathBuf, String> {
     let start = Path::new(env!("CARGO_MANIFEST_DIR"));
@@ -11,3 +13,11 @@
         .find(|root| root.join("conformance/conformance_schema.json").is_file())
         .ok_or_else(|| format!("no vendor/a2ui above {}", start.display()))
 }
+
+/// Parses the JSON file at `relative`, a path under the upstream root.
+pub fn read_json(relative: &str) -> Result<Value, String> {
+    let path = upstream_root()?.join(relative);
+    let text =
+        std::fs::read_to_string(&path).map_err(|err| format!("{}: {err}", path.display()))?;
+    serde_json::from_str(&text).map_err(|err| format!("{}: {err}", path.display()))
+}
```

```diff
--- a/crates/a2ui-conformance/src/lib.rs
+++ b/crates/a2ui-conformance/src/lib.rs
@@ -15,5 +15,5 @@
 pub use compare::{expect_error, json_eq, normalize_set_keywords, schema_eq};
 pub use keys::{assert_case_keys, assert_known_keys};
 pub use manifest::{Manifest, Status};
-pub use paths::upstream_root;
+pub use paths::{read_json, upstream_root};
 pub use skips::Skips;
```

`crates/a2ui-conformance/src/handlers/schema_export.rs`:

```rust
//! The `common_types_schema` and `agent_to_renderer_schema` actions.

use a2ui_runtime::{Error, agent_to_renderer_schema, common_types_schema};
use serde_json::Value;

use crate::case::Case;
use crate::compare::json_eq;
use crate::keys::assert_case_keys;
use crate::paths::read_json;

/// Compares the runtime's common types schema with `expectFile`, exactly.
pub fn run_common_types(case: &Case) -> Result<(), String> {
    run(case, common_types_schema)
}

/// Compares the runtime's agent-to-renderer schema with `expectFile`, exactly.
pub fn run_agent_to_renderer(case: &Case) -> Result<(), String> {
    run(case, agent_to_renderer_schema)
}

/// Key order is ignored; array order, `required` and `enum` included, is not.
fn run(case: &Case, schema: fn() -> Result<Value, Error>) -> Result<(), String> {
    assert_case_keys(case, &["protocolVersion", "expectFile"])?;
    let at = case.at();
    if case.protocol_version() != Some("1.0") {
        return Err(format!("{at}: a2ui-rs serves only the v1.0 schema"));
    }
    let file = case
        .get("expectFile")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{at}: `expectFile` must be a string"))?;
    let expected = read_json(file)?;
    let actual = schema().map_err(|err| format!("{at}: {err}"))?;
    if json_eq(&actual, &expected) {
        Ok(())
    } else {
        Err(format!("{at}: the schema differs from {file}"))
    }
}
```

```diff
--- a/crates/a2ui-conformance/src/handlers/mod.rs
+++ b/crates/a2ui-conformance/src/handlers/mod.rs
@@ -3,6 +3,7 @@
 mod data_model;
 mod expression;
 mod resolve_path;
+mod schema_export;
 
 use crate::case::Case;
 
@@ -15,6 +16,8 @@
         "data_model" => Some(data_model::run),
         "resolve_path" => Some(resolve_path::run),
         "parse_expression_template" => Some(expression::run),
+        "common_types_schema" => Some(schema_export::run_common_types),
+        "agent_to_renderer_schema" => Some(schema_export::run_agent_to_renderer),
         _ => None,
     }
 }
```

```diff
--- a/crates/a2ui-conformance/skips.toml
+++ b/crates/a2ui-conformance/skips.toml
@@ -5,9 +5,7 @@
 round_trip = "Plan 2: catalogs and schemas"
 validation_schema = "Plan 2: catalogs and schemas"
 derive = "Plan 2: catalogs and schemas"
-common_types_schema = "Plan 2: catalogs and schemas"
 validate_common_type = "Plan 2: catalogs and schemas"
-agent_to_renderer_schema = "Plan 2: catalogs and schemas"
 validate = "Plan 3: validation and message processing"
 process_messages = "Plan 3: validation and message processing"
 get_renderer_capabilities = "Plan 3: validation and message processing"
```

- [ ] **Step 7: Run the conformance suites**

Run: `cargo test -p a2ui-conformance --test core 2>&1 | tail -3`
Expected: `test result: ok. 162 passed; 0 failed; 683 ignored`.

- [ ] **Step 8: Update the ARCHITECTURE.md files**

`crates/a2ui-runtime/ARCHITECTURE.md`, add after the `expression` bullet:

```markdown
- `ident`: `is_identifier`, the UAX #31 check shared by expressions and catalogs.
- `spec`: the v1.0 documents in `spec/` (copied by `vendor/sync.sh`, Apache-2.0 per
  `spec/LICENSE`), embedded with `include_str!`: `common_types_schema()`,
  `agent_to_renderer_schema()`, and the basic catalog behind `Catalog::basic()` (Task 5).
```

`crates/a2ui-conformance/ARCHITECTURE.md`, replace the `paths` bullet with:

```markdown
- `paths`: `upstream_root`, the vendored `vendor/a2ui` found from the crate directory; `read_json`
  for files named relative to it.
```

- [ ] **Step 9: Run the gate**

Run: `just check && just test`
Expected: success (`just check` includes the wasm build; `include_str!` is fine there).

- [ ] **Step 10: Commit**

```bash
git add -A
git commit -m "feat(runtime): embed the v1.0 spec documents and export their schemas (t-1555)"
oku cli task done --project slider --id t-1555 --result "spec/ embedded via sync.sh; ident module; schema export actions; 162 passed"
```

---

### Task 3: Error categories and common types (t-1556)

**Files:**
- Modify: `crates/a2ui-runtime/Cargo.toml`, `src/error.rs`, `src/lib.rs`, `ARCHITECTURE.md`
- Create: `crates/a2ui-runtime/src/common_types/mod.rs`, `common_types/dynamic.rs`, `common_types/component.rs`, `common_types/function.rs`, `common_types/common_types_tests.rs`
- Create: `crates/a2ui-conformance/src/handlers/common_type.rs`
- Modify: `crates/a2ui-conformance/src/handlers/mod.rs`, `skips.toml`

**Interfaces:**
- Consumes: Task 2 `ident::is_identifier`, `spec::common_types_schema`.
- Produces:
  - `Category::Catalog` (`"CatalogError"`), `Category::Validation` (`"ValidationError"`).
  - Variants (both `Validation`): `InvalidCommonType { definition: String, source: serde_json::Error }` (`Invalid {definition}: {source}`), `UnknownCommonType { definition: String }` (`Unknown common type '{definition}'`).
  - `pub mod common_types` with `pub const COMMON_TYPES: [&str; 12]` (AccessibilityAttributes, ComponentCommon, DataBinding, DynamicBoolean, DynamicNumber, DynamicString, DynamicValue, Extensions, FunctionCall, FunctionResponse, IndexSystemFunction, Surface) and `pub fn validate_common_type(definition: &str, value: &Value) -> Result<Value, Error>` (the value as the model writes it back).
  - Models: `DataBinding`, `FunctionCall`, `Dynamic<T>` (`DynamicString`, `DynamicNumber`, `DynamicBoolean`), `DynamicValue`, `Extensions`, `ComponentMetadata`, `Live`, `AccessibilityAttributes`, `ComponentCommon`, `SurfaceName`, `SurfaceChild`, `Surface`, `IndexCall`, `IndexArgs`, `IndexSystemFunction`, `FunctionError`, `FunctionOutcome`, `FunctionResponse`. Every struct refuses unknown keys except `ComponentCommon` (component properties flatten into `rest`); an explicit `null` is refused where a value is required; a `Dynamic` with both `@path` and `@call` is refused; a literal object in `DynamicValue` refuses keys starting with `@` (but not `@@`); `Extensions` keys must be UAX #31 identifiers.

- [ ] **Step 1: Add serde**

`crates/a2ui-runtime/Cargo.toml` `[dependencies]`: add `serde.workspace = true` after `snafu.workspace = true`.

- [ ] **Step 2: Write the failing tests**

`crates/a2ui-runtime/src/common_types/common_types_tests.rs`:

```rust
use std::collections::BTreeSet;

use rstest::rstest;
use serde_json::{Value, json};
use speculoos::prelude::*;

use super::*;
use crate::common_types_schema;

#[rstest]
#[case("Extensions", json!({"vendor": {"any": "value"}, "ünïcode": true}))]
#[case("ComponentCommon", json!({"id": "c1", "metadata": {}}))]
#[case("ComponentCommon", json!({"id": "c1", "custom": [1, 2]}))]
#[case("FunctionResponse", json!({"functionCallId": "call_1", "value": null}))]
#[case("FunctionResponse", json!({"functionCallId": "call_1", "error": {"code": "E", "message": "m"}}))]
#[case("IndexSystemFunction", json!({"@call": "@index", "args": {"offset": 1}}))]
#[case("Surface", json!({}))]
#[case("DynamicValue", json!({"@@path": "/a"}))]
#[case("DynamicValue", json!([1, "two"]))]
#[case("DynamicNumber", json!({"@call": "now", "args": {"a": 1}}))]
#[case("DynamicString", json!({"@path": "/name"}))]
#[case("FunctionCall", json!({"@call": "x", "catalogId": "c", "args": {"a": 1}}))]
fn validate_common_type_valid_value_round_trips(#[case] definition: &str, #[case] value: Value) {
    assert_that!(validate_common_type(definition, &value).unwrap()).is_equal_to(value);
}

#[rstest]
#[case("Extensions", json!({"has-dash": 1}))]
#[case("Extensions", json!({"": 1}))]
#[case("ComponentCommon", json!({"id": "c1", "metadata": {"owner": "me"}}))]
#[case("ComponentCommon", json!({"id": "c1", "catalogId": null}))]
#[case("FunctionResponse", json!({"functionCallId": "call_1"}))]
#[case("FunctionResponse", json!({"functionCallId": "call_1", "value": 3, "error": {"code": "E", "message": "m"}}))]
#[case("FunctionResponse", json!({"functionCallId": "call_1", "error": null}))]
#[case("IndexSystemFunction", json!({"@call": "now"}))]
#[case("IndexSystemFunction", json!({"@call": "@index", "args": null}))]
#[case("Surface", json!({"child": "main"}))]
#[case("Surface", json!({"component": null}))]
#[case("DynamicValue", json!({"@call": "now", "@path": "/a"}))]
#[case("DynamicValue", json!({"@unknown": 1}))]
#[case("DynamicValue", json!(null))]
#[case("DynamicBoolean", json!("true"))]
#[case("FunctionCall", json!({"@call": "x", "bogus": 1}))]
fn validate_common_type_invalid_value_is_validation_error(
    #[case] definition: &str,
    #[case] value: Value,
) {
    let err = validate_common_type(definition, &value).unwrap_err();
    assert_that!(err.category().as_str()).is_equal_to("ValidationError");
}

#[test]
fn validate_common_type_unknown_definition_is_error() {
    let err = validate_common_type("Nope", &json!({})).unwrap_err();
    assert_that!(err.to_string()).is_equal_to("Unknown common type 'Nope'".to_owned());
}

#[test]
fn common_types_every_name_is_a_published_def() {
    let schema = common_types_schema().unwrap();
    for name in COMMON_TYPES {
        assert_that!(schema["$defs"].get(name)).is_some();
    }
}

fn declared_properties(definition: &str) -> BTreeSet<String> {
    let schema = common_types_schema().unwrap();
    schema["$defs"][definition]["properties"]
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect()
}

fn written_keys(definition: &str, value: &Value) -> BTreeSet<String> {
    let written = validate_common_type(definition, value).unwrap();
    written.as_object().unwrap().keys().cloned().collect()
}

/// The typed models name exactly the properties the embedded schema declares.
#[rstest]
#[case(
    "AccessibilityAttributes",
    json!({"label": "l", "description": "d", "live": "polite", "hidden": true})
)]
#[case(
    "ComponentCommon",
    json!({"id": "c1", "catalogId": "c", "accessibility": {}, "metadata": {"extensions": {}}})
)]
#[case("IndexSystemFunction", json!({"@call": "@index", "args": {"offset": 1}}))]
#[case("Surface", json!({"component": "Surface", "child": "root"}))]
#[case("DataBinding", json!({"@path": "/a"}))]
fn validate_common_type_model_fields_match_the_spec(#[case] definition: &str, #[case] full: Value) {
    assert_that!(written_keys(definition, &full)).is_equal_to(declared_properties(definition));
}

#[test]
fn validate_common_type_function_response_outcomes_cover_the_spec_fields() {
    let mut modeled = written_keys(
        "FunctionResponse",
        &json!({"functionCallId": "c", "value": 1}),
    );
    modeled.extend(written_keys(
        "FunctionResponse",
        &json!({"functionCallId": "c", "error": {"code": "E", "message": "m"}}),
    ));
    assert_that!(modeled).is_equal_to(declared_properties("FunctionResponse"));
}
```

Declare and export the module in `crates/a2ui-runtime/src/lib.rs`:

```diff
--- a/crates/a2ui-runtime/src/lib.rs
+++ b/crates/a2ui-runtime/src/lib.rs
@@ -6,9 +6,12 @@
 //! Key items: [`Pointer`] (normalized RFC 6901 pointers), [`DataModel`]
 //! (pointer-addressed JSON), [`Write`] (one committed change), [`Error`]
 //! (classified by [`Category`]), [`Watchers`] (observers a write notifies),
-//! [`resolve_path`] (scoped relative paths) and [`parse_template`] (`${...}`
-//! expression templates).
+//! [`resolve_path`] (scoped relative paths), [`parse_template`] (`${...}`
+//! expression templates), the embedded v1.0 schemas ([`common_types_schema`],
+//! [`agent_to_renderer_schema`]) and the typed [`common_types`] with
+//! [`validate_common_type`].
 
+pub mod common_types;
 mod context;
 mod data;
 mod error;
@@ -19,6 +22,7 @@
 mod subscribe;
 pub use subscribe::Watchers;
 
+pub use common_types::{COMMON_TYPES, validate_common_type};
 pub use context::resolve_path;
 pub use data::{DataModel, Write};
 pub use error::{Category, Error};
```

- [ ] **Step 3: Run the tests to see them fail**

Run: `cargo test -p a2ui-runtime common_types`
Expected: FAIL to compile: `file not found for module 'common_types'`.

- [ ] **Step 4: Implement**

Add the categories and variants to `crates/a2ui-runtime/src/error.rs`:

```diff
--- a/crates/a2ui-runtime/src/error.rs
+++ b/crates/a2ui-runtime/src/error.rs
@@ -12,6 +12,10 @@
     Parse,
     /// An expression template past the length or part limits.
     Expression,
+    /// A catalog document that cannot be loaded or derived.
+    Catalog,
+    /// A value that does not match its schema, such as a common type.
+    Validation,
 }
 
 impl Category {
@@ -21,6 +25,8 @@
             Self::Data => "DataError",
             Self::Parse => "ParseError",
             Self::Expression => "ExpressionError",
+            Self::Catalog => "CatalogError",
+            Self::Validation => "ValidationError",
         }
     }
 }
@@ -87,6 +93,19 @@
         backtrace: Backtrace,
     },
 
+    #[snafu(display("Invalid {definition}: {source}"))]
+    InvalidCommonType {
+        definition: String,
+        source: serde_json::Error,
+        backtrace: Backtrace,
+    },
+
+    #[snafu(display("Unknown common type '{definition}'"))]
+    UnknownCommonType {
+        definition: String,
+        backtrace: Backtrace,
+    },
+
     #[snafu(display("Embedded document {name} is not valid JSON: {source}"))]
     EmbeddedDocument {
         name: &'static str,
@@ -105,6 +124,7 @@
             | Self::IndexOutOfRange { .. } => Category::Data,
             Self::ExpressionSyntax { .. } | Self::EmbeddedDocument { .. } => Category::Parse,
             Self::ExpressionLimit { .. } => Category::Expression,
+            Self::InvalidCommonType { .. } | Self::UnknownCommonType { .. } => Category::Validation,
         }
     }
 }
```

`crates/a2ui-runtime/src/common_types/mod.rs`:

```rust
//! Typed models of the v1.0 common types, as messages carry them.
//!
//! Each model accepts exactly the values its `$defs` entry in
//! `common_types.json` accepts (without consulting a catalog) and serializes
//! back to the fields that were set. Optional fields refuse an explicit `null`.

mod component;
mod dynamic;
mod function;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use snafu::ResultExt;

pub use component::{
    AccessibilityAttributes, ComponentCommon, ComponentMetadata, Extensions, Live, Surface,
    SurfaceChild, SurfaceName,
};
pub use dynamic::{
    DataBinding, Dynamic, DynamicBoolean, DynamicNumber, DynamicString, DynamicValue, FunctionCall,
};
pub use function::{
    FunctionError, FunctionOutcome, FunctionResponse, IndexArgs, IndexCall, IndexSystemFunction,
};

use crate::error::{Error, InvalidCommonTypeSnafu, UnknownCommonTypeSnafu};

/// The `$defs` names [`validate_common_type`] has a model for.
pub const COMMON_TYPES: [&str; 12] = [
    "AccessibilityAttributes",
    "ComponentCommon",
    "DataBinding",
    "DynamicBoolean",
    "DynamicNumber",
    "DynamicString",
    "DynamicValue",
    "Extensions",
    "FunctionCall",
    "FunctionResponse",
    "IndexSystemFunction",
    "Surface",
];

/// Validates `value` as the common type `definition` and serializes it back.
///
/// The result holds the wire names of the fields that were set, so a valid
/// value comes back unchanged.
pub fn validate_common_type(definition: &str, value: &Value) -> Result<Value, Error> {
    match definition {
        "AccessibilityAttributes" => round_trip::<AccessibilityAttributes>(definition, value),
        "ComponentCommon" => round_trip::<ComponentCommon>(definition, value),
        "DataBinding" => round_trip::<DataBinding>(definition, value),
        "DynamicBoolean" => round_trip::<DynamicBoolean>(definition, value),
        "DynamicNumber" => round_trip::<DynamicNumber>(definition, value),
        "DynamicString" => round_trip::<DynamicString>(definition, value),
        "DynamicValue" => round_trip::<DynamicValue>(definition, value),
        "Extensions" => round_trip::<Extensions>(definition, value),
        "FunctionCall" => round_trip::<FunctionCall>(definition, value),
        "FunctionResponse" => round_trip::<FunctionResponse>(definition, value),
        "IndexSystemFunction" => round_trip::<IndexSystemFunction>(definition, value),
        "Surface" => round_trip::<Surface>(definition, value),
        _ => Err(UnknownCommonTypeSnafu { definition }.build().into()),
    }
}

fn round_trip<T: DeserializeOwned + Serialize>(
    definition: &str,
    value: &Value,
) -> Result<Value, Error> {
    let typed: T =
        serde_json::from_value(value.clone()).context(InvalidCommonTypeSnafu { definition })?;
    Ok(serde_json::to_value(typed).context(InvalidCommonTypeSnafu { definition })?)
}

/// Deserializes a field that is present as `Some`, so an explicit `null` is refused.
///
/// Used with `#[serde(default)]`: a missing field stays `None`.
fn present<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}

#[cfg(test)]
#[path = "common_types_tests.rs"]
mod tests;
```

`crates/a2ui-runtime/src/common_types/dynamic.rs`:

```rust
//! Values that are literals, data bindings or function calls.

use serde::de::{DeserializeOwned, Error as _};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Map, Number, Value};

use super::present;

/// A JSON Pointer into the data model: `{"@path": "/a/b"}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DataBinding {
    /// The pointer, absolute or relative to the template scope.
    #[serde(rename = "@path")]
    pub path: String,
}

/// A function call: `{"@call": name, "catalogId"?, "args"?}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FunctionCall {
    /// The function name.
    #[serde(rename = "@call")]
    pub call: String,
    /// The catalog that defines the function, when not the surface default.
    #[serde(
        rename = "catalogId",
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub catalog_id: Option<String>,
    /// The named arguments.
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub args: Option<Map<String, Value>>,
}

/// A literal `T`, a [`DataBinding`] or a [`FunctionCall`].
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
pub enum Dynamic<T> {
    /// A literal value.
    Literal(T),
    /// A value read from the data model.
    Binding(DataBinding),
    /// A value a function call returns.
    Call(FunctionCall),
}

/// `DynamicString`: a string, a binding or a call.
pub type DynamicString = Dynamic<String>;
/// `DynamicNumber`: a number, a binding or a call.
pub type DynamicNumber = Dynamic<Number>;
/// `DynamicBoolean`: a boolean, a binding or a call.
pub type DynamicBoolean = Dynamic<bool>;

impl<'de, T: DeserializeOwned> Deserialize<'de> for Dynamic<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = Value::deserialize(deserializer)?;
        match shape(&value).map_err(D::Error::custom)? {
            Shape::Binding => from_value(value).map(Self::Binding),
            Shape::Call => from_value(value).map(Self::Call),
            Shape::Literal => from_value(value).map(Self::Literal),
        }
    }
}

/// `DynamicValue`: any literal but `null`, a binding or a call.
///
/// A literal object may not hold a key starting with a single `@`; `@@` escapes it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
pub enum DynamicValue {
    /// A literal string.
    String(String),
    /// A literal number.
    Number(Number),
    /// A literal boolean.
    Boolean(bool),
    /// A literal list.
    Array(Vec<Value>),
    /// A literal object.
    Object(Map<String, Value>),
    /// A value read from the data model.
    Binding(DataBinding),
    /// A value a function call returns.
    Call(FunctionCall),
}

impl<'de> Deserialize<'de> for DynamicValue {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match Value::deserialize(deserializer)? {
            Value::Null => Err(D::Error::custom("a dynamic value is never null")),
            Value::String(text) => Ok(Self::String(text)),
            Value::Number(number) => Ok(Self::Number(number)),
            Value::Bool(flag) => Ok(Self::Boolean(flag)),
            Value::Array(items) => Ok(Self::Array(items)),
            Value::Object(fields) => {
                let value = Value::Object(fields);
                match shape(&value).map_err(D::Error::custom)? {
                    Shape::Binding => from_value(value).map(Self::Binding),
                    Shape::Call => from_value(value).map(Self::Call),
                    Shape::Literal => literal_object(value).map_err(D::Error::custom),
                }
            }
        }
    }
}

/// Which branch of a dynamic value an object takes.
enum Shape {
    Binding,
    Call,
    Literal,
}

fn shape(value: &Value) -> Result<Shape, &'static str> {
    let Some(fields) = value.as_object() else {
        return Ok(Shape::Literal);
    };
    match (fields.contains_key("@path"), fields.contains_key("@call")) {
        (true, true) => Err("a dynamic value holds both `@path` and `@call`"),
        (true, false) => Ok(Shape::Binding),
        (false, true) => Ok(Shape::Call),
        (false, false) => Ok(Shape::Literal),
    }
}

fn literal_object(value: Value) -> Result<DynamicValue, String> {
    let Value::Object(fields) = value else {
        return Err("a literal object must be an object".to_owned());
    };
    match fields
        .keys()
        .find(|key| key.starts_with('@') && !key.starts_with("@@"))
    {
        Some(key) => Err(format!(
            "literal object key '{key}' uses the reserved '@' prefix"
        )),
        None => Ok(DynamicValue::Object(fields)),
    }
}

fn from_value<T: DeserializeOwned, E: serde::de::Error>(value: Value) -> Result<T, E> {
    serde_json::from_value(value).map_err(E::custom)
}
```

`crates/a2ui-runtime/src/common_types/component.rs`:

```rust
//! Component envelopes: common fields, metadata, accessibility and `Surface`.

use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Map, Value};

use super::present;
use super::{DynamicBoolean, DynamicString};
use crate::ident::is_identifier;

/// Vendor extension metadata; every key is a UAX #31 identifier.
#[derive(Debug, Clone, PartialEq, Default, Serialize)]
#[serde(transparent)]
pub struct Extensions(pub Map<String, Value>);

impl<'de> Deserialize<'de> for Extensions {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let fields = Map::<String, Value>::deserialize(deserializer)?;
        match fields.keys().find(|key| !is_identifier(key)) {
            Some(key) => Err(D::Error::custom(format!(
                "Invalid UAX #31 extension identifier: \"{key}\". Keys MUST be Unicode identifiers (UAX #31)."
            ))),
            None => Ok(Self(fields)),
        }
    }
}

/// Component-level metadata, which holds only `extensions`.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ComponentMetadata {
    /// Vendor extensions.
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub extensions: Option<Extensions>,
}

/// How a screen reader announces updates (`aria-live`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Live {
    /// No announcement.
    Off,
    /// Announce when the user pauses.
    Polite,
    /// Announce immediately.
    Assertive,
}

/// Attributes for assistive technologies.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AccessibilityAttributes {
    /// A short label.
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub label: Option<DynamicString>,
    /// A longer description.
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub description: Option<DynamicString>,
    /// How updates are announced.
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub live: Option<Live>,
    /// Whether the element is hidden from assistive technologies.
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub hidden: Option<DynamicBoolean>,
}

/// The fields every component carries; other keys are kept in `rest`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComponentCommon {
    /// The component id, unique in its surface.
    pub id: String,
    /// The catalog of the component, when not the surface default.
    #[serde(
        rename = "catalogId",
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub catalog_id: Option<String>,
    /// Accessibility attributes.
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub accessibility: Option<AccessibilityAttributes>,
    /// Vendor metadata.
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub metadata: Option<ComponentMetadata>,
    /// The component's other properties, as written.
    #[serde(flatten)]
    pub rest: Map<String, Value>,
}

/// The only value `Surface.component` takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SurfaceName {
    /// `"Surface"`.
    Surface,
}

/// The only value `Surface.child` takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SurfaceChild {
    /// `"root"`.
    #[serde(rename = "root")]
    Root,
}

/// The reserved container component of a surface.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Surface {
    /// Always `"Surface"` when present.
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub component: Option<SurfaceName>,
    /// Always `"root"` when present.
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub child: Option<SurfaceChild>,
}
```

`crates/a2ui-runtime/src/common_types/function.rs`:

```rust
//! Function responses and the `@index` system function.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::DynamicNumber;
use super::present;

/// The only value `IndexSystemFunction["@call"]` takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IndexCall {
    /// `"@index"`.
    #[serde(rename = "@index")]
    Index,
}

/// The arguments of `@index`.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IndexArgs {
    /// Added to the 0-based index.
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub offset: Option<DynamicNumber>,
}

/// A call of `@index`, the current item's index in a list template.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IndexSystemFunction {
    /// Always `"@index"`.
    #[serde(rename = "@call")]
    pub call: IndexCall,
    /// The optional offset.
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub args: Option<IndexArgs>,
}

/// The error a failed function call reports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FunctionError {
    /// A stable error code.
    pub code: String,
    /// A message for logs.
    pub message: String,
}

/// The result of a function call: a value (possibly `null`) or an error.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum FunctionOutcome {
    /// The returned value.
    Value(Value),
    /// The failure.
    Error(FunctionError),
}

/// The answer to a function call, holding exactly one outcome.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "ResponseFields", rename_all = "camelCase")]
pub struct FunctionResponse {
    /// The id of the call answered.
    pub function_call_id: String,
    /// The value or the error.
    #[serde(flatten)]
    pub outcome: FunctionOutcome,
}

/// The wire fields of a [`FunctionResponse`], before the one-outcome check.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct ResponseFields {
    function_call_id: String,
    #[serde(default, deserialize_with = "present")]
    value: Option<Value>,
    #[serde(default, deserialize_with = "present")]
    error: Option<FunctionError>,
}

impl TryFrom<ResponseFields> for FunctionResponse {
    type Error = &'static str;

    fn try_from(fields: ResponseFields) -> Result<Self, Self::Error> {
        let outcome = match (fields.value, fields.error) {
            (Some(value), None) => FunctionOutcome::Value(value),
            (None, Some(error)) => FunctionOutcome::Error(error),
            _ => return Err("a function response holds exactly one of `value` and `error`"),
        };
        Ok(Self {
            function_call_id: fields.function_call_id,
            outcome,
        })
    }
}
```

- [ ] **Step 5: Run the unit tests**

Run: `cargo test -p a2ui-runtime common_types`
Expected: PASS, 0 failed (including the test comparing each model's fields with the embedded `$defs` properties).

- [ ] **Step 6: Add the handler**

`crates/a2ui-conformance/src/handlers/common_type.rs`:

```rust
//! The `validate_common_type` action: values against one common type model.

use a2ui_runtime::{COMMON_TYPES, validate_common_type};
use serde_json::Value;

use crate::case::Case;
use crate::compare::{expect_error, json_eq};
use crate::keys::{assert_case_keys, assert_known_keys};

/// Validates each step's `value` as `definition`; the result must equal `value`.
pub fn run(case: &Case) -> Result<(), String> {
    assert_case_keys(case, &["protocolVersion", "definition", "steps"])?;
    let at = case.at();
    if case.protocol_version() != Some("1.0") {
        return Err(format!("{at}: a2ui-rs models only the v1.0 common types"));
    }
    let definition = case
        .get("definition")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{at}: `definition` must be a string"))?;
    // An unmodeled name would fail every step, so `expectError` steps would pass vacuously.
    if !COMMON_TYPES.contains(&definition) {
        return Err(format!("{at}: no model for common type '{definition}'"));
    }
    let steps = match case.get("steps") {
        Some(Value::Array(steps)) if !steps.is_empty() => steps,
        _ => return Err(format!("{at}: `steps` must be a non-empty list")),
    };
    for (index, step) in steps.iter().enumerate() {
        let at = format!("{at} step {index}");
        let step = step
            .as_object()
            .ok_or_else(|| format!("{at}: a step is a mapping"))?;
        assert_known_keys(step, &["value", "expectError"], &at)?;
        let value = step
            .get("value")
            .ok_or_else(|| format!("{at}: a step needs a `value`"))?;
        match (
            validate_common_type(definition, value),
            step.get("expectError"),
        ) {
            (Err(err), Some(expected)) => expect_error(&err, expected, &at)?,
            (Err(err), None) => return Err(format!("{at}: {err}")),
            (Ok(written), Some(_)) => {
                return Err(format!("{at}: expected an error, accepted as {written}"));
            }
            (Ok(written), None) if json_eq(&written, value) => {}
            (Ok(written), None) => {
                return Err(format!("{at}: serialized as {written}, expected {value}"));
            }
        }
    }
    Ok(())
}
```

```diff
--- a/crates/a2ui-conformance/src/handlers/mod.rs
+++ b/crates/a2ui-conformance/src/handlers/mod.rs
@@ -1,5 +1,6 @@
 //! The runner for each conformance action.
 
+mod common_type;
 mod data_model;
 mod expression;
 mod resolve_path;
@@ -17,6 +18,7 @@
         "resolve_path" => Some(resolve_path::run),
         "parse_expression_template" => Some(expression::run),
         "common_types_schema" => Some(schema_export::run_common_types),
+        "validate_common_type" => Some(common_type::run),
         "agent_to_renderer_schema" => Some(schema_export::run_agent_to_renderer),
         _ => None,
     }
```

```diff
--- a/crates/a2ui-conformance/skips.toml
+++ b/crates/a2ui-conformance/skips.toml
@@ -5,7 +5,6 @@
 round_trip = "Plan 2: catalogs and schemas"
 validation_schema = "Plan 2: catalogs and schemas"
 derive = "Plan 2: catalogs and schemas"
-validate_common_type = "Plan 2: catalogs and schemas"
 validate = "Plan 3: validation and message processing"
 process_messages = "Plan 3: validation and message processing"
 get_renderer_capabilities = "Plan 3: validation and message processing"
```

- [ ] **Step 7: Run the conformance suites**

Run: `cargo test -p a2ui-conformance --test core 2>&1 | tail -3`
Expected: `test result: ok. 173 passed; 0 failed; 672 ignored`.

- [ ] **Step 8: Update `crates/a2ui-runtime/ARCHITECTURE.md`**

In the `error` bullet, replace `(`DataError`, …)` with `(`DataError`, `CatalogError`, `ValidationError`, …)`. Add after the `spec` bullet:

```markdown
- `common_types`: serde models of the `common_types.json` `$defs` listed in `COMMON_TYPES`;
  `validate_common_type` round-trips a value through its model (Plan 3 parses messages with them).
```

In `## Dependencies`, add `serde` (common-type models).

- [ ] **Step 9: Run the gate**

Run: `just check && just test`
Expected: success.

- [ ] **Step 10: Commit**

```bash
git add -A
git commit -m "feat(runtime): model and validate the v1.0 common types (t-1556)"
oku cli task done --project slider --id t-1556 --result "CatalogError/ValidationError categories; common_types models; validate_common_type; 173 passed"
```

---

### Task 4: Catalog loading with `from_json` (t-1557)

**Files:**
- Modify: `Cargo.toml` (workspace), `deny.toml`, `crates/a2ui-runtime/src/data.rs`, `src/error.rs`, `src/lib.rs`, `ARCHITECTURE.md`
- Create: `crates/a2ui-runtime/src/catalog/mod.rs`, `catalog/refs.rs`, `catalog/refs_tests.rs`, `catalog/entries.rs`, `catalog/entries_tests.rs`, `catalog/load.rs`, `catalog/load_tests.rs`
- Create: `crates/a2ui-conformance/src/handlers/catalog_input.rs`, `handlers/from_json.rs`
- Modify: `crates/a2ui-conformance/src/handlers/mod.rs`, `skips.toml`, `ARCHITECTURE.md`

**Interfaces:**
- Consumes: Task 2 `ident::is_identifier`; Task 3 `ComponentMetadata`, `Category::{Catalog, Validation}`.
- Produces:
  - `pub struct Catalog` (`Debug`, `Clone`, `PartialEq`) with `Catalog::from_json(document: &Value, protocol_version: Option<&str>, catalog_id: Option<&str>) -> Result<Catalog, Error>` and accessors `id() -> &str`, `protocol_version() -> &str`, `components() -> &[ComponentDef]`, `component(name) -> Option<&ComponentDef>`, `functions() -> &[FunctionDef]`, `function(name) -> Option<&FunctionDef>`, `theme() -> Option<&Value>`.
  - `pub struct ComponentDef` (`name()`, `schema()`, `allowed_parents() -> Option<&[String]>`, `allowed_children()`, `metadata() -> Option<&ComponentMetadata>`), `pub struct FunctionDef` (`name()`, `schema()`, `return_type() -> Option<&str>`, `allowed_callers() -> AllowedCallers`, `requires_user_activation() -> bool`), `pub enum AllowedCallers { RendererOnly, AgentOnly, RendererOrAgent }` with `as_str()`.
  - Variants, all `Catalog` unless noted:

    | Variant | Display |
    |---|---|
    | `MissingCatalogId` | `catalog_id must be provided or exist in catalog schema.` |
    | `UnsupportedVersion { version }` | `Unsupported protocol version '{version}': a2ui-rs implements A2UI v1.0 only` |
    | `UnsupportedRef { reference, at }` | `Unsupported external $ref '{reference}' at {at}/$ref: a v1.0 catalog may only reference its own components and functions (#/...) or common_types.json.` |
    | `InvalidIdentifier { kind: &'static str, name, owner }` | `Invalid UAX #31 {kind} identifier: '{name}'{owner}` |
    | `ReservedComponent` | `Component name 'Surface' is reserved for the surface container` |
    | `UserActivationCallers { function, callers: &'static str }` | `Function '{function}' declares requiresUserActivation=true with allowedCallers='{callers}'; requiresUserActivation=true requires allowedCallers='rendererOnly'.` |
    | `MalformedCatalog { reason }` | `Malformed catalog: {reason}` |
    | `InvalidMetadata { component, source }` (Validation) | `Invalid metadata of component '{component}': {source}` |

  - Harness: `catalog_input::{document, catalog_id, check_entries}`, action `from_json`.

Load order (reference `from_json`, survey §3.2): the document is an object → catalog id (argument, else `catalogId`, else `MissingCatalogId`) → version (argument, else `protocolVersion`, else `"1.0"`; its bare form must be `1.0`) → every `$ref` classified (`#...`, `catalog.json#...` or `…/common_types.json#...`, else `UnsupportedRef`) → `$defs` is an object → `anyComponent`/`anyFunction` member names → `theme` is an object → each entry checked (component: object, identifier, not `Surface`, property identifiers, metadata, allowed parents/children; function: object, identifier, argument identifiers, `allowedCallers`, `requiresUserActivation`, the activation rule, `returnType`) → entries the unions leave out are removed from the stored document.

- [ ] **Step 1: Turn on document order**

Apply to the workspace `Cargo.toml`:

```diff
-serde_json = "1"
+serde_json = { version = "1", features = ["preserve_order"] }
```

`deny.toml` (indexmap brings `foldhash`, Zlib):

```toml
[licenses]
allow = ["MIT", "Apache-2.0", "Unicode-3.0", "Zlib"]
```

With `preserve_order`, `Map::remove` swap-removes (it moves the last key into the hole); keep the order with `shift_remove` in `crates/a2ui-runtime/src/data.rs`:

```diff
--- a/crates/a2ui-runtime/src/data.rs
+++ b/crates/a2ui-runtime/src/data.rs
@@ -89,7 +89,7 @@
         }
         match node {
             Value::Object(map) => {
-                map.remove(last);
+                map.shift_remove(last);
             }
             other => *slot(other, last, path)? = Value::Null,
         }
```

Run: `cargo test -p a2ui-runtime data && cargo deny check licenses`
Expected: PASS, `licenses ok`.

- [ ] **Step 2: Write the failing tests**

`crates/a2ui-runtime/src/catalog/refs_tests.rs`:

```rust
use rstest::rstest;
use serde_json::json;
use speculoos::prelude::*;

use super::*;

#[rstest]
#[case("#/$defs/Weight")]
#[case("catalog.json#/$defs/anyFunction")]
#[case("common_types.json#/$defs/DynamicString")]
#[case("https://a2ui.org/specification/v1_0/common_types.json#/$defs/DynamicString")]
fn is_supported_local_or_common_types_ref_is_accepted(#[case] reference: &str) {
    assert_that!(is_supported(reference)).is_true();
}

#[rstest]
#[case("basic_catalog.json#/components/Text")]
#[case("https://a2ui.org/specification/v1_0/catalogs/basic/catalog.json#/components/Button")]
#[case("other_catalog.json")]
fn is_supported_other_document_is_refused(#[case] reference: &str) {
    assert_that!(is_supported(reference)).is_false();
}

#[test]
fn check_external_names_the_pointer_of_the_ref() {
    let document = json!({"components": {"a/b": {"allOf": [{"$ref": "x.json#/y"}]}}});
    let err = check_external(&document, "").unwrap_err();
    assert_that!(err.to_string())
        .starts_with("Unsupported external $ref 'x.json#/y' at /components/a~1b/allOf/0/$ref");
}

#[test]
fn check_external_supported_refs_pass() {
    let document = json!({"a": [{"$ref": "#/$defs/A"}, {"$ref": "common_types.json#/$defs/B"}]});
    assert_that!(check_external(&document, "")).is_ok();
}
```

`crates/a2ui-runtime/src/catalog/entries_tests.rs`:

```rust
use rstest::rstest;
use serde_json::{Value, json};
use speculoos::prelude::*;

use super::*;

#[rstest]
#[case("My-Component", json!({}), "Invalid UAX #31 component identifier: 'My-Component'")]
#[case(
    "A",
    json!({"properties": {"bad-name": {}}}),
    "Invalid UAX #31 property identifier: 'bad-name' in component 'A'"
)]
#[case("Surface", json!({}), "Component name 'Surface' is reserved")]
#[case("A", json!({"allowedParents": "Row"}), "Malformed catalog")]
#[case("A", json!([]), "Malformed catalog")]
fn component_invalid_entry_is_catalog_error(
    #[case] name: &str,
    #[case] schema: Value,
    #[case] message: &str,
) {
    let err = component(name, &schema).unwrap_err();
    assert_that!(err.category().as_str()).is_equal_to("CatalogError");
    assert_that!(err.to_string()).starts_with(message);
}

#[rstest]
#[case(json!({"owner": "me"}))]
#[case(json!({"extensions": {"com.example.policy": 1}}))]
fn component_invalid_metadata_is_validation_error(#[case] metadata: Value) {
    let err = component("A", &json!({"metadata": metadata})).unwrap_err();
    assert_that!(err.category().as_str()).is_equal_to("ValidationError");
}

#[test]
fn component_reads_composition_and_metadata() {
    let schema = json!({
        "allowedParents": ["Surface", "Row"],
        "metadata": {"extensions": {"a2ui_experimental": "preview"}}
    });
    let loaded = component("Card", &schema).unwrap();
    assert_that!(loaded.allowed_parents())
        .is_equal_to(Some(&["Surface".to_owned(), "Row".to_owned()][..]));
    assert_that!(loaded.metadata().is_some()).is_true();
}

#[rstest]
#[case("@index", json!({"returnType": "number"}), "Invalid UAX #31 function identifier: '@index'")]
#[case(
    "f",
    json!({"returnType": "string", "parameters": {"123invalid_arg": {"type": "string"}}}),
    "Invalid UAX #31 argument identifier: '123invalid_arg' in function 'f'"
)]
#[case(
    "f",
    json!({"properties": {"args": {"properties": {"bad-arg": {}}}}}),
    "Invalid UAX #31 argument identifier: 'bad-arg'"
)]
#[case(
    "f",
    json!({"args": {"$ref": "#/$defs/A", "$defs": {"A": {"properties": {"bad arg": {}}}}}}),
    "Invalid UAX #31 argument identifier: 'bad arg'"
)]
#[case(
    "openUrl",
    json!({"returnType": "void", "allowedCallers": "rendererOrAgent", "requiresUserActivation": true}),
    "Function 'openUrl' declares requiresUserActivation=true with allowedCallers='rendererOrAgent'"
)]
#[case("f", json!({"returnType": "text"}), "Malformed catalog")]
#[case("f", json!({"allowedCallers": "everyone"}), "Malformed catalog")]
#[case("f", json!({"requiresUserActivation": "yes"}), "Malformed catalog")]
fn function_invalid_entry_is_catalog_error(
    #[case] name: &str,
    #[case] schema: Value,
    #[case] message: &str,
) {
    let err = function(name, &schema).unwrap_err();
    assert_that!(err.category().as_str()).is_equal_to("CatalogError");
    assert_that!(err.to_string()).starts_with(message);
}

#[test]
fn function_activation_without_callers_is_accepted() {
    let loaded = function(
        "copy",
        &json!({"returnType": "void", "requiresUserActivation": true}),
    );
    assert_that!(loaded.unwrap().allowed_callers()).is_equal_to(AllowedCallers::RendererOnly);
}

#[test]
fn function_call_shaped_entry_reads_return_type_const() {
    let schema =
        json!({"properties": {"call": {"const": "upper"}, "returnType": {"const": "string"}}});
    assert_that!(function("upper", &schema).unwrap().return_type()).is_equal_to(Some("string"));
}

#[test]
fn function_call_shaped_entry_does_not_name_call_as_argument() {
    let schema = json!({"properties": {"call": {"const": "f"}, "bad-name": {}}});
    assert_that!(function("f", &schema)).is_ok();
}
```

`crates/a2ui-runtime/src/catalog/load_tests.rs`:

```rust
use serde_json::{Value, json};
use speculoos::prelude::*;

use super::*;
use crate::catalog::ComponentDef;

fn catalog(document: Value) -> Catalog {
    Catalog::from_json(&document, None, None).unwrap()
}

fn error_text(document: Value) -> String {
    Catalog::from_json(&document, None, None)
        .unwrap_err()
        .to_string()
}

#[test]
fn from_json_override_id_wins() {
    let document = json!({"catalogId": "original"});
    let loaded = Catalog::from_json(&document, None, Some("overridden")).unwrap();
    assert_that!(loaded.id()).is_equal_to("overridden");
}

#[test]
fn from_json_missing_id_is_catalog_error() {
    let err = Catalog::from_json(&json!({"components": {}}), None, None).unwrap_err();
    assert_that!(err.to_string())
        .is_equal_to("catalog_id must be provided or exist in catalog schema.".to_owned());
}

#[test]
fn from_json_undeclared_version_is_one_point_zero() {
    assert_that!(catalog(json!({"catalogId": "c"})).protocol_version()).is_equal_to("1.0");
}

#[test]
fn from_json_declared_version_is_reported_verbatim() {
    let loaded = catalog(json!({"catalogId": "c", "protocolVersion": "v1.0"}));
    assert_that!(loaded.protocol_version()).is_equal_to("v1.0");
}

#[test]
fn from_json_other_version_is_catalog_error() {
    let err = Catalog::from_json(&json!({"catalogId": "c"}), Some("v0.9"), None).unwrap_err();
    assert_that!(err.category().as_str()).is_equal_to("CatalogError");
}

#[test]
fn from_json_external_ref_is_refused() {
    let document = json!({"catalogId": "c", "components": {"A": {"$ref": "other.json#/A"}}});
    assert_that!(error_text(document)).starts_with("Unsupported external $ref 'other.json#/A'");
}

#[test]
fn from_json_non_object_components_is_malformed() {
    assert_that!(error_text(json!({"catalogId": "c", "components": []})))
        .starts_with("Malformed catalog");
}

#[test]
fn from_json_keeps_document_order() {
    let loaded = catalog(json!({"catalogId": "c", "components": {"Z": {}, "A": {}, "M": {}}}));
    let names: Vec<&str> = loaded.components().iter().map(ComponentDef::name).collect();
    assert_that!(names).is_equal_to(vec!["Z", "A", "M"]);
}

#[test]
fn from_json_union_leaves_unlisted_entries_out() {
    let loaded = catalog(json!({
        "catalogId": "c",
        "components": {"Listed": {}, "Unlisted": {}},
        "functions": {"listed": {}, "unlisted": {}},
        "$defs": {
            "anyComponent": {"oneOf": [{"$ref": "#/components/Listed"}]},
            "anyFunction": {"oneOf": [{"$ref": "#/functions/listed"}]}
        }
    }));
    assert_that!(loaded.component("Unlisted")).is_none();
    assert_that!(loaded.function("unlisted")).is_none();
}

#[test]
fn from_json_checks_entries_a_union_leaves_out() {
    let document = json!({
        "catalogId": "c",
        "functions": {
            "good": {"returnType": "void"},
            "bad": {"returnType": "void", "allowedCallers": "agentOnly", "requiresUserActivation": true}
        },
        "$defs": {"anyFunction": {"oneOf": [{"$ref": "#/functions/good"}]}}
    });
    assert_that!(error_text(document)).contains("requiresUserActivation");
}

#[test]
fn from_json_non_object_theme_is_malformed() {
    assert_that!(error_text(json!({"catalogId": "c", "theme": "dark"})))
        .starts_with("Malformed catalog");
}

#[test]
fn bare_version_drops_a_leading_v() {
    assert_that!(bare_version("v1.0")).is_equal_to("1.0");
}

#[test]
fn bare_version_keeps_other_text() {
    assert_that!(bare_version("version")).is_equal_to("version");
}
```

Declare and export the module in `crates/a2ui-runtime/src/lib.rs`:

```diff
--- a/crates/a2ui-runtime/src/lib.rs
+++ b/crates/a2ui-runtime/src/lib.rs
@@ -11,6 +11,7 @@
 //! [`agent_to_renderer_schema`]) and the typed [`common_types`] with
 //! [`validate_common_type`].
 
+mod catalog;
 pub mod common_types;
 mod context;
 mod data;
@@ -22,6 +23,7 @@
 mod subscribe;
 pub use subscribe::Watchers;
 
+pub use catalog::{AllowedCallers, Catalog, ComponentDef, FunctionDef};
 pub use common_types::{COMMON_TYPES, validate_common_type};
 pub use context::resolve_path;
 pub use data::{DataModel, Write};
```

- [ ] **Step 3: Run the tests to see them fail**

Run: `cargo test -p a2ui-runtime catalog`
Expected: FAIL to compile: `file not found for module 'catalog'`.

- [ ] **Step 4: Implement**

Add the variants to `crates/a2ui-runtime/src/error.rs`:

```diff
--- a/crates/a2ui-runtime/src/error.rs
+++ b/crates/a2ui-runtime/src/error.rs
@@ -93,6 +93,62 @@
         backtrace: Backtrace,
     },
 
+    #[snafu(display("catalog_id must be provided or exist in catalog schema."))]
+    MissingCatalogId { backtrace: Backtrace },
+
+    #[snafu(display(
+        "Unsupported protocol version '{version}': a2ui-rs implements A2UI v1.0 only"
+    ))]
+    UnsupportedVersion {
+        version: String,
+        backtrace: Backtrace,
+    },
+
+    #[snafu(display(
+        "Unsupported external $ref '{reference}' at {at}/$ref: a v1.0 catalog may only reference \
+         its own components and functions (#/...) or common_types.json."
+    ))]
+    UnsupportedRef {
+        reference: String,
+        at: String,
+        backtrace: Backtrace,
+    },
+
+    #[snafu(display("Invalid UAX #31 {kind} identifier: '{name}'{owner}"))]
+    InvalidIdentifier {
+        kind: &'static str,
+        name: String,
+        owner: String,
+        backtrace: Backtrace,
+    },
+
+    #[snafu(display("Component name 'Surface' is reserved for the surface container"))]
+    ReservedComponent { backtrace: Backtrace },
+
+    #[snafu(display(
+        "Function '{function}' declares requiresUserActivation=true with \
+         allowedCallers='{callers}'; requiresUserActivation=true requires \
+         allowedCallers='rendererOnly'."
+    ))]
+    UserActivationCallers {
+        function: String,
+        callers: &'static str,
+        backtrace: Backtrace,
+    },
+
+    #[snafu(display("Malformed catalog: {reason}"))]
+    MalformedCatalog {
+        reason: String,
+        backtrace: Backtrace,
+    },
+
+    #[snafu(display("Invalid metadata of component '{component}': {source}"))]
+    InvalidMetadata {
+        component: String,
+        source: serde_json::Error,
+        backtrace: Backtrace,
+    },
+
     #[snafu(display("Invalid {definition}: {source}"))]
     InvalidCommonType {
         definition: String,
@@ -124,7 +180,16 @@
             | Self::IndexOutOfRange { .. } => Category::Data,
             Self::ExpressionSyntax { .. } | Self::EmbeddedDocument { .. } => Category::Parse,
             Self::ExpressionLimit { .. } => Category::Expression,
-            Self::InvalidCommonType { .. } | Self::UnknownCommonType { .. } => Category::Validation,
+            Self::MissingCatalogId { .. }
+            | Self::UnsupportedVersion { .. }
+            | Self::UnsupportedRef { .. }
+            | Self::InvalidIdentifier { .. }
+            | Self::ReservedComponent { .. }
+            | Self::UserActivationCallers { .. }
+            | Self::MalformedCatalog { .. } => Category::Catalog,
+            Self::InvalidMetadata { .. }
+            | Self::InvalidCommonType { .. }
+            | Self::UnknownCommonType { .. } => Category::Validation,
         }
     }
 }
```

`crates/a2ui-runtime/src/catalog/mod.rs`:

```rust
//! v1.0 catalogs: loading, serialization, validation schemas and derivation.
//!
//! A [`Catalog`] keeps the document it was loaded from (minus the entries its
//! unions leave out), so [`Catalog::to_json`] writes it back unchanged.
//! `$ref` targets are classified, never fetched.

mod entries;
mod load;
mod refs;

use serde_json::{Map, Value};

use crate::common_types::ComponentMetadata;
use crate::error::{Error, MalformedCatalogSnafu};

/// The protocol version every catalog of this runtime targets.
const PROTOCOL_VERSION: &str = "1.0";

/// A loaded v1.0 catalog of components and functions.
#[derive(Debug, Clone, PartialEq)]
pub struct Catalog {
    id: String,
    protocol_version: String,
    document: Map<String, Value>,
    components: Vec<ComponentDef>,
    functions: Vec<FunctionDef>,
}

impl Catalog {
    /// The catalog id, the override given to `from_json` or the document's.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// The protocol version as given or declared; `"1.0"` when neither.
    pub fn protocol_version(&self) -> &str {
        &self.protocol_version
    }

    /// The components, in document order.
    pub fn components(&self) -> &[ComponentDef] {
        &self.components
    }

    /// The component named `name`.
    pub fn component(&self, name: &str) -> Option<&ComponentDef> {
        self.components
            .iter()
            .find(|component| component.name == name)
    }

    /// The functions, in document order.
    pub fn functions(&self) -> &[FunctionDef] {
        &self.functions
    }

    /// The function named `name`.
    pub fn function(&self, name: &str) -> Option<&FunctionDef> {
        self.functions.iter().find(|function| function.name == name)
    }

    /// The theme schema, the document's top-level `theme`.
    pub fn theme(&self) -> Option<&Value> {
        self.document.get("theme")
    }
}

/// One component of a catalog: its schema and composition rules.
#[derive(Debug, Clone, PartialEq)]
pub struct ComponentDef {
    name: String,
    schema: Value,
    allowed_parents: Option<Vec<String>>,
    allowed_children: Option<Vec<String>>,
    metadata: Option<ComponentMetadata>,
}

impl ComponentDef {
    /// The component name, such as `Text`.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The schema as the document writes it.
    pub fn schema(&self) -> &Value {
        &self.schema
    }

    /// The component names that may hold this one, when restricted.
    pub fn allowed_parents(&self) -> Option<&[String]> {
        self.allowed_parents.as_deref()
    }

    /// The component names this one may hold, when restricted.
    pub fn allowed_children(&self) -> Option<&[String]> {
        self.allowed_children.as_deref()
    }

    /// The component's vendor metadata, when it declares any.
    pub fn metadata(&self) -> Option<&ComponentMetadata> {
        self.metadata.as_ref()
    }
}

/// Who may call a catalog function.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AllowedCallers {
    /// Only the renderer (the default).
    #[default]
    RendererOnly,
    /// Only the agent.
    AgentOnly,
    /// The renderer and the agent.
    RendererOrAgent,
}

impl AllowedCallers {
    /// The wire name, such as `"rendererOnly"`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::RendererOnly => "rendererOnly",
            Self::AgentOnly => "agentOnly",
            Self::RendererOrAgent => "rendererOrAgent",
        }
    }
}

/// One function of a catalog: its schema and call metadata.
#[derive(Debug, Clone, PartialEq)]
pub struct FunctionDef {
    name: String,
    schema: Value,
    return_type: Option<String>,
    allowed_callers: AllowedCallers,
    requires_user_activation: bool,
}

impl FunctionDef {
    /// The function name, such as `formatString`.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The schema as the document writes it.
    pub fn schema(&self) -> &Value {
        &self.schema
    }

    /// The declared return type, such as `"string"`, when the entry has one.
    pub fn return_type(&self) -> Option<&str> {
        self.return_type.as_deref()
    }

    /// Who may call the function.
    pub fn allowed_callers(&self) -> AllowedCallers {
        self.allowed_callers
    }

    /// Whether a call needs a user gesture.
    pub fn requires_user_activation(&self) -> bool {
        self.requires_user_activation
    }
}

/// A `CatalogError` for a document that is not shaped like a catalog.
fn malformed(reason: String) -> Error {
    MalformedCatalogSnafu { reason }.build().into()
}
```

`crates/a2ui-runtime/src/catalog/refs.rs`:

```rust
//! `$ref` targets in catalog documents: classified, never fetched.

use serde_json::Value;

use crate::error::{Error, UnsupportedRefSnafu};

/// Whether a v1.0 catalog may use `reference`: itself or `common_types.json`.
///
/// The document part before `#` is empty (local), `catalog.json` (the
/// catalog itself) or ends in `common_types.json`, relative or absolute.
pub(super) fn is_supported(reference: &str) -> bool {
    let document = reference
        .split_once('#')
        .map_or(reference, |(document, _)| document);
    document.is_empty()
        || document == "catalog.json"
        || document.rsplit('/').next() == Some("common_types.json")
}

/// Fails on the first `$ref` under `node` that [`is_supported`] refuses.
///
/// `at` is the JSON Pointer of `node`; the error names the pointer of the `$ref`.
pub(super) fn check_external(node: &Value, at: &str) -> Result<(), Error> {
    match node {
        Value::Object(fields) => {
            if let Some(Value::String(reference)) = fields.get("$ref")
                && !is_supported(reference)
            {
                return Err(UnsupportedRefSnafu { reference, at }.build().into());
            }
            for (key, value) in fields {
                check_external(value, &format!("{at}/{}", escape(key)))?;
            }
            Ok(())
        }
        Value::Array(items) => {
            for (index, item) in items.iter().enumerate() {
                check_external(item, &format!("{at}/{index}"))?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

/// Escapes one JSON Pointer token (RFC 6901).
fn escape(token: &str) -> String {
    token.replace('~', "~0").replace('/', "~1")
}

#[cfg(test)]
#[path = "refs_tests.rs"]
mod tests;
```

`crates/a2ui-runtime/src/catalog/entries.rs`:

```rust
//! Component and function entries: identifiers, metadata and call rules.

use std::collections::BTreeSet;

use serde_json::{Map, Value};
use snafu::ResultExt;

use super::{AllowedCallers, ComponentDef, FunctionDef, malformed};
use crate::common_types::ComponentMetadata;
use crate::error::{
    Error, InvalidIdentifierSnafu, InvalidMetadataSnafu, ReservedComponentSnafu,
    UserActivationCallersSnafu,
};
use crate::ident::is_identifier;

/// The return types a v1.0 function may declare.
const RETURN_TYPES: [&str; 8] = [
    "string",
    "number",
    "boolean",
    "array",
    "object",
    "validationResult",
    "any",
    "void",
];

/// Checks and reads the component `name` of a catalog document.
pub(super) fn component(name: &str, schema: &Value) -> Result<ComponentDef, Error> {
    let Value::Object(fields) = schema else {
        return Err(malformed(format!(
            "component '{name}' must be a schema object"
        )));
    };
    identifier("component", name, "")?;
    if name == "Surface" {
        return Err(ReservedComponentSnafu.build().into());
    }
    match fields.get("properties") {
        None => {}
        Some(Value::Object(properties)) => {
            for property in properties.keys() {
                identifier("property", property, &format!(" in component '{name}'"))?;
            }
        }
        Some(_) => {
            return Err(malformed(format!(
                "component '{name}' properties must be an object"
            )));
        }
    }
    let metadata = match fields.get("metadata") {
        None => None,
        Some(metadata) => Some(
            serde_json::from_value::<ComponentMetadata>(metadata.clone())
                .context(InvalidMetadataSnafu { component: name })?,
        ),
    };
    Ok(ComponentDef {
        name: name.to_owned(),
        schema: schema.clone(),
        allowed_parents: component_names(fields, "allowedParents", name)?,
        allowed_children: component_names(fields, "allowedChildren", name)?,
        metadata,
    })
}

/// Checks and reads the function `name` of a catalog document.
pub(super) fn function(name: &str, schema: &Value) -> Result<FunctionDef, Error> {
    let Value::Object(fields) = schema else {
        return Err(malformed(format!(
            "function '{name}' must be a schema object"
        )));
    };
    identifier("function", name, "")?;
    for argument in argument_names(fields) {
        identifier("argument", &argument, &format!(" in function '{name}'"))?;
    }
    let allowed_callers = match fields.get("allowedCallers") {
        None => None,
        Some(Value::String(callers)) => Some(callers_named(callers, name)?),
        Some(_) => {
            return Err(malformed(format!(
                "function '{name}' allowedCallers must be a string"
            )));
        }
    };
    let requires_user_activation = match fields.get("requiresUserActivation") {
        None => false,
        Some(Value::Bool(requires)) => *requires,
        Some(_) => {
            return Err(malformed(format!(
                "function '{name}' requiresUserActivation must be a boolean"
            )));
        }
    };
    if let Some(callers) = allowed_callers
        && requires_user_activation
        && callers != AllowedCallers::RendererOnly
    {
        let callers = callers.as_str();
        return Err(UserActivationCallersSnafu {
            function: name,
            callers,
        }
        .build()
        .into());
    }
    Ok(FunctionDef {
        name: name.to_owned(),
        schema: schema.clone(),
        return_type: return_type(fields, name)?,
        allowed_callers: allowed_callers.unwrap_or_default(),
        requires_user_activation,
    })
}

fn identifier(kind: &'static str, name: &str, owner: &str) -> Result<(), Error> {
    if is_identifier(name) {
        Ok(())
    } else {
        Err(InvalidIdentifierSnafu { kind, name, owner }.build().into())
    }
}

fn component_names(
    fields: &Map<String, Value>,
    key: &str,
    component: &str,
) -> Result<Option<Vec<String>>, Error> {
    let Some(value) = fields.get(key) else {
        return Ok(None);
    };
    let names = value.as_array().and_then(|items| {
        items
            .iter()
            .map(|item| item.as_str().map(str::to_owned))
            .collect::<Option<Vec<_>>>()
    });
    names
        .map(Some)
        .ok_or_else(|| malformed(format!("component '{component}' {key} must list names")))
}

fn callers_named(callers: &str, function: &str) -> Result<AllowedCallers, Error> {
    match callers {
        "rendererOnly" => Ok(AllowedCallers::RendererOnly),
        "agentOnly" => Ok(AllowedCallers::AgentOnly),
        "rendererOrAgent" => Ok(AllowedCallers::RendererOrAgent),
        _ => Err(malformed(format!(
            "function '{function}' allowedCallers '{callers}' is not a caller role"
        ))),
    }
}

/// The top-level `returnType`, else the v0.9-style `properties.returnType.const`.
fn return_type(fields: &Map<String, Value>, function: &str) -> Result<Option<String>, Error> {
    let declared = match fields.get("returnType") {
        Some(declared) => Some(declared),
        None => fields
            .get("properties")
            .and_then(|properties| properties.get("returnType"))
            .and_then(|return_type| return_type.get("const")),
    };
    match declared {
        None => Ok(None),
        Some(Value::String(name)) if RETURN_TYPES.contains(&name.as_str()) => {
            Ok(Some(name.clone()))
        }
        Some(other) => Err(malformed(format!(
            "function '{function}' returnType {other} is not a return type"
        ))),
    }
}

/// The argument names a function entry declares, as upstream's loader reads them.
///
/// The arguments schema is `properties.args`, else `args`, else `parameters`;
/// an entry with none that is not call-shaped is its own arguments schema.
fn argument_names(fields: &Map<String, Value>) -> Vec<String> {
    let properties = fields.get("properties").and_then(Value::as_object);
    let arguments = properties
        .and_then(|properties| properties.get("args"))
        .and_then(Value::as_object)
        .or_else(|| fields.get("args").and_then(Value::as_object))
        .or_else(|| fields.get("parameters").and_then(Value::as_object));
    match (arguments, properties) {
        (Some(arguments), _) => parameter_names(arguments),
        (None, Some(properties))
            if !["call", "@call", "function"]
                .iter()
                .any(|key| properties.contains_key(*key)) =>
        {
            parameter_names(fields)
        }
        _ => Vec::new(),
    }
}

/// Property names of an arguments schema, or the keys of a shorthand map.
fn parameter_names(schema: &Map<String, Value>) -> Vec<String> {
    let structured = schema.get("properties").is_some_and(Value::is_object)
        || schema.get("$ref").is_some_and(Value::is_string)
        || ["allOf", "anyOf", "oneOf"]
            .iter()
            .any(|key| schema.get(*key).is_some_and(Value::is_array));
    if structured {
        let mut names = Vec::new();
        let mut visited = BTreeSet::new();
        collect_properties(schema, schema.get("$defs"), &mut visited, &mut names);
        return names;
    }
    let shorthand = !schema.is_empty()
        && !schema.get("type").is_some_and(Value::is_string)
        && !["$ref", "allOf", "oneOf", "anyOf", "not"]
            .iter()
            .any(|key| schema.contains_key(*key))
        && schema
            .values()
            .all(|value| value.is_object() || value.is_boolean());
    if shorthand {
        schema.keys().cloned().collect()
    } else {
        Vec::new()
    }
}

/// Collects `properties` keys through local `#/$defs/` refs and `allOf`/`anyOf`/`oneOf`.
fn collect_properties(
    node: &Map<String, Value>,
    defs: Option<&Value>,
    visited: &mut BTreeSet<String>,
    names: &mut Vec<String>,
) {
    if let Some(Value::String(reference)) = node.get("$ref")
        && let Some(path) = reference.strip_prefix("#/$defs/")
        && visited.insert(reference.clone())
        && let Some(Value::Object(target)) = defs.and_then(|defs| defs.pointer(&format!("/{path}")))
    {
        collect_properties(target, defs, visited, names);
    }
    if let Some(Value::Object(properties)) = node.get("properties") {
        names.extend(properties.keys().cloned());
    }
    for keyword in ["allOf", "anyOf", "oneOf"] {
        if let Some(Value::Array(members)) = node.get(keyword) {
            for member in members.iter().filter_map(Value::as_object) {
                collect_properties(member, defs, visited, names);
            }
        }
    }
}

#[cfg(test)]
#[path = "entries_tests.rs"]
mod tests;
```

`crates/a2ui-runtime/src/catalog/load.rs`:

```rust
//! `Catalog::from_json`: the load checks, in upstream's order.

use serde_json::{Map, Value};

use super::{Catalog, PROTOCOL_VERSION, entries, malformed, refs};
use crate::error::{Error, MissingCatalogIdSnafu, UnsupportedVersionSnafu};

impl Catalog {
    /// Loads a v1.0 catalog document.
    ///
    /// `catalog_id` and `protocol_version` take precedence over the document's
    /// `catalogId` and `protocolVersion`; without either version the catalog
    /// is v1.0. The checks run in upstream's order: the id, the version, every
    /// `$ref` (never fetched), then each component and each function, all of
    /// them before `anyComponent`/`anyFunction` leave entries out.
    pub fn from_json(
        document: &Value,
        protocol_version: Option<&str>,
        catalog_id: Option<&str>,
    ) -> Result<Self, Error> {
        let Value::Object(fields) = document else {
            return Err(malformed(
                "a catalog document must be a JSON object".to_owned(),
            ));
        };
        let id = catalog_id_of(fields, catalog_id)?;
        let protocol_version = version_of(fields, protocol_version)?;
        refs::check_external(document, "")?;
        let defs = match fields.get("$defs") {
            None => None,
            Some(Value::Object(defs)) => Some(defs),
            Some(_) => return Err(malformed("$defs must be an object".to_owned())),
        };
        let listed_components = union_members(defs, "anyComponent", "#/components/");
        let listed_functions = union_members(defs, "anyFunction", "#/functions/");
        if fields.get("theme").is_some_and(|theme| !theme.is_object()) {
            return Err(malformed("theme must be a schema object".to_owned()));
        }
        let mut document = fields.clone();
        let components = load_entries(
            &mut document,
            "components",
            &listed_components,
            entries::component,
        )?;
        let functions = load_entries(
            &mut document,
            "functions",
            &listed_functions,
            entries::function,
        )?;
        Ok(Self {
            id,
            protocol_version,
            document,
            components,
            functions,
        })
    }
}

/// `v1.0` becomes `1.0`: a leading `v` before a digit is dropped.
pub(super) fn bare_version(version: &str) -> &str {
    match version.strip_prefix(['v', 'V']) {
        Some(rest) if rest.starts_with(|c: char| c.is_ascii_digit()) => rest,
        _ => version,
    }
}

fn catalog_id_of(fields: &Map<String, Value>, catalog_id: Option<&str>) -> Result<String, Error> {
    if let Some(id) = catalog_id.filter(|id| !id.is_empty()) {
        return Ok(id.to_owned());
    }
    match fields.get("catalogId") {
        Some(Value::String(id)) if !id.is_empty() => Ok(id.clone()),
        None | Some(Value::String(_)) => Err(MissingCatalogIdSnafu.build().into()),
        Some(_) => Err(malformed("catalogId must be a string".to_owned())),
    }
}

fn version_of(
    fields: &Map<String, Value>,
    protocol_version: Option<&str>,
) -> Result<String, Error> {
    let declared = match fields.get("protocolVersion") {
        None => None,
        Some(Value::String(version)) => Some(version.as_str()),
        Some(_) => return Err(malformed("protocolVersion must be a string".to_owned())),
    };
    let version = protocol_version.or(declared).unwrap_or(PROTOCOL_VERSION);
    if bare_version(version) == PROTOCOL_VERSION {
        Ok(version.to_owned())
    } else {
        Err(UnsupportedVersionSnafu { version }.build().into())
    }
}

/// The entry names a `$defs` union lists as `<prefix><name>` refs.
fn union_members(defs: Option<&Map<String, Value>>, union: &str, prefix: &str) -> Vec<String> {
    let members = defs
        .and_then(|defs| defs.get(union))
        .and_then(|union| union.get("oneOf"))
        .and_then(Value::as_array);
    members
        .into_iter()
        .flatten()
        .filter_map(|member| member.get("$ref")?.as_str())
        .filter(|reference| reference.starts_with(prefix))
        .filter_map(|reference| reference.rsplit('/').next())
        .map(str::to_owned)
        .collect()
}

/// Checks every entry of `kind`; keeps those the union lists (all when it lists none).
///
/// Entries left out are removed from `document`, so `to_json` omits them.
fn load_entries<T>(
    document: &mut Map<String, Value>,
    kind: &str,
    listed: &[String],
    load: fn(&str, &Value) -> Result<T, Error>,
) -> Result<Vec<T>, Error> {
    let Some(entries) = document.get_mut(kind) else {
        return Ok(Vec::new());
    };
    let Value::Object(entries) = entries else {
        return Err(malformed(format!(
            "{kind} must be an object mapping names to schemas"
        )));
    };
    let mut loaded = Vec::new();
    let mut left_out = Vec::new();
    for (name, schema) in entries.iter() {
        let entry = load(name, schema)?;
        if listed.is_empty() || listed.contains(name) {
            loaded.push(entry);
        } else {
            left_out.push(name.clone());
        }
    }
    for name in left_out {
        entries.shift_remove(&name);
    }
    Ok(loaded)
}

#[cfg(test)]
#[path = "load_tests.rs"]
mod tests;
```

- [ ] **Step 5: Run the unit tests**

Run: `cargo test -p a2ui-runtime`
Expected: PASS, 0 failed.

- [ ] **Step 6: Add the handler**

`crates/a2ui-conformance/src/handlers/catalog_input.rs`:

```rust
//! The catalog document a case names and the checks catalog actions share.

use serde_json::Value;

use crate::case::Case;
use crate::compare::schema_eq;
use crate::paths::read_json;

/// The case's catalog document: the inline `catalog` or the file at `catalogPath`.
pub fn document(case: &Case) -> Result<Value, String> {
    let at = case.at();
    match (case.get("catalog"), case.get("catalogPath")) {
        (Some(inline), None) => Ok(inline.clone()),
        (None, Some(Value::String(path))) => read_json(path),
        (None, Some(other)) => Err(format!("{at}: `catalogPath` must be a string, got {other}")),
        (Some(_), Some(_)) => Err(format!("{at}: `catalog` and `catalogPath` are exclusive")),
        (None, None) => Err(format!("{at}: a case needs `catalog` or `catalogPath`")),
    }
}

/// The case-level `catalogId`, which overrides the document's.
pub fn catalog_id(case: &Case) -> Result<Option<&str>, String> {
    match case.get("catalogId") {
        None => Ok(None),
        Some(Value::String(id)) => Ok(Some(id)),
        Some(other) => Err(format!(
            "{}: `catalogId` must be a string, got {other}",
            case.at()
        )),
    }
}

/// Checks an expected `components` or `functions` value against loaded entries.
///
/// A list names the exact set of entries; a mapping also gives each entry's
/// schema, compared after set-keyword normalization.
pub fn check_entries(
    kind: &str,
    expected: &Value,
    actual: &[(&str, &Value)],
    at: &str,
) -> Result<(), String> {
    let mut wanted: Vec<&str> = match expected {
        Value::Array(items) => items
            .iter()
            .map(|item| {
                item.as_str()
                    .ok_or_else(|| format!("{at}: `{kind}` must list names, got {item}"))
            })
            .collect::<Result<_, _>>()?,
        Value::Object(entries) => entries.keys().map(String::as_str).collect(),
        other => {
            return Err(format!(
                "{at}: `{kind}` must be a list or a mapping, got {other}"
            ));
        }
    };
    let mut names: Vec<&str> = actual.iter().map(|(name, _)| *name).collect();
    wanted.sort_unstable();
    names.sort_unstable();
    if names != wanted {
        return Err(format!("{at}: {kind} {names:?}, expected {wanted:?}"));
    }
    let Value::Object(entries) = expected else {
        return Ok(());
    };
    for (name, schema) in entries {
        let loaded = actual
            .iter()
            .find(|(loaded, _)| loaded == name)
            .map(|(_, loaded)| *loaded)
            .ok_or_else(|| format!("{at}: no {kind} entry '{name}'"))?;
        if !schema_eq(loaded, schema) {
            return Err(format!(
                "{at}: {kind} '{name}' is {loaded}, expected {schema}"
            ));
        }
    }
    Ok(())
}
```

`crates/a2ui-conformance/src/handlers/from_json.rs` (`selfContained` and `matchesBuiltinCatalog` fail until Task 6, whose three cases are skipped meanwhile):

```rust
//! The `from_json` action: load a catalog document and check what it holds.

use a2ui_runtime::Catalog;
use serde_json::{Map, Value};

use crate::case::Case;
use crate::compare::{expect_error, json_eq, schema_eq};
use crate::handlers::catalog_input::{catalog_id, check_entries, document};
use crate::keys::{assert_case_keys, assert_known_keys};

const EXPECT_KEYS: [&str; 10] = [
    "catalogId",
    "protocolVersion",
    "components",
    "functions",
    "theme",
    "matchesBuiltinCatalog",
    "selfContained",
    "validComponents",
    "invalidComponents",
    "componentMetadata",
];

/// Loads the case's catalog with its overrides; checks `expect` or `expectError`.
pub fn run(case: &Case) -> Result<(), String> {
    assert_case_keys(
        case,
        &[
            "catalog",
            "catalogPath",
            "catalogId",
            "protocolVersion",
            "expect",
            "expectError",
        ],
    )?;
    let at = case.at();
    let loaded = Catalog::from_json(&document(case)?, case.protocol_version(), catalog_id(case)?);
    match (loaded, case.get("expectError"), case.get("expect")) {
        (_, Some(_), Some(_)) => Err(format!("{at}: `expect` and `expectError` are exclusive")),
        (Err(err), Some(expected), None) => expect_error(&err, expected, &at),
        (Err(err), None, _) => Err(format!("{at}: {err}")),
        (Ok(_), Some(_), None) => Err(format!("{at}: expected an error, the catalog loaded")),
        (Ok(_), None, None) => Ok(()),
        (Ok(catalog), None, Some(Value::Object(expect))) => check(&catalog, expect, &at),
        (Ok(_), None, Some(other)) => Err(format!("{at}: `expect` must be a mapping, got {other}")),
    }
}

fn check(catalog: &Catalog, expect: &Map<String, Value>, at: &str) -> Result<(), String> {
    assert_known_keys(expect, &EXPECT_KEYS, at)?;
    for (key, expected) in expect {
        match key.as_str() {
            "catalogId" => check_text("catalogId", catalog.id(), expected, at)?,
            "protocolVersion" => {
                check_text("protocolVersion", catalog.protocol_version(), expected, at)?;
            }
            "components" => {
                let actual: Vec<(&str, &Value)> = catalog
                    .components()
                    .iter()
                    .map(|component| (component.name(), component.schema()))
                    .collect();
                check_entries("components", expected, &actual, at)?;
            }
            "functions" => {
                let actual: Vec<(&str, &Value)> = catalog
                    .functions()
                    .iter()
                    .map(|function| (function.name(), function.schema()))
                    .collect();
                check_entries("functions", expected, &actual, at)?;
            }
            "theme" => check_theme(catalog, expected, at)?,
            "componentMetadata" => check_metadata(catalog, expected, at)?,
            "selfContained" | "matchesBuiltinCatalog" => {
                return Err(format!(
                    "{at}: `{key}` needs validation_schema (Plan 2, Task 6)"
                ));
            }
            _ => {
                return Err(format!("{at}: `{key}` needs component validation (Plan 3)"));
            }
        }
    }
    Ok(())
}

fn check_text(key: &str, actual: &str, expected: &Value, at: &str) -> Result<(), String> {
    match expected {
        Value::String(expected) if expected == actual => Ok(()),
        Value::String(expected) => Err(format!("{at}: {key} {actual:?}, expected {expected:?}")),
        other => Err(format!("{at}: `{key}` must be a string, got {other}")),
    }
}

/// An empty mapping expects no theme; any other mapping is the theme schema.
fn check_theme(catalog: &Catalog, expected: &Value, at: &str) -> Result<(), String> {
    let Value::Object(fields) = expected else {
        return Err(format!("{at}: `theme` must be a mapping, got {expected}"));
    };
    match (catalog.theme(), fields.is_empty()) {
        (None, true) => Ok(()),
        (Some(theme), false) if schema_eq(theme, expected) => Ok(()),
        (theme, _) => Err(format!("{at}: theme {theme:?}, expected {expected}")),
    }
}

/// Maps each component name to its metadata object, or `null` for none.
fn check_metadata(catalog: &Catalog, expected: &Value, at: &str) -> Result<(), String> {
    let Value::Object(components) = expected else {
        return Err(format!(
            "{at}: `componentMetadata` must be a mapping, got {expected}"
        ));
    };
    for (name, metadata) in components {
        let component = catalog
            .component(name)
            .ok_or_else(|| format!("{at}: no component '{name}'"))?;
        let actual = match component.metadata() {
            None => Value::Null,
            Some(actual) => serde_json::to_value(actual).map_err(|err| format!("{at}: {err}"))?,
        };
        if !json_eq(&actual, metadata) {
            return Err(format!(
                "{at}: '{name}' metadata {actual}, expected {metadata}"
            ));
        }
    }
    Ok(())
}
```

```diff
--- a/crates/a2ui-conformance/src/handlers/mod.rs
+++ b/crates/a2ui-conformance/src/handlers/mod.rs
@@ -1,8 +1,10 @@
 //! The runner for each conformance action.
 
+mod catalog_input;
 mod common_type;
 mod data_model;
 mod expression;
+mod from_json;
 mod resolve_path;
 mod schema_export;
 
@@ -18,6 +20,7 @@
         "resolve_path" => Some(resolve_path::run),
         "parse_expression_template" => Some(expression::run),
         "common_types_schema" => Some(schema_export::run_common_types),
+        "from_json" => Some(from_json::run),
         "validate_common_type" => Some(common_type::run),
         "agent_to_renderer_schema" => Some(schema_export::run_agent_to_renderer),
         _ => None,
```

Add the 7 permanent and 3 temporary named skips, and remove `from_json` from `[actions]`:

```diff
--- a/crates/a2ui-conformance/skips.toml
+++ b/crates/a2ui-conformance/skips.toml
@@ -1,6 +1,5 @@
 # Actions not implemented yet, each with the plan that implements it.
 [actions]
-from_json = "Plan 2: catalogs and schemas"
 to_json = "Plan 2: catalogs and schemas"
 round_trip = "Plan 2: catalogs and schemas"
 validation_schema = "Plan 2: catalogs and schemas"
@@ -27,3 +26,13 @@
 
 # Individual cases skipped on purpose, keyed by trial name (`core/<suite>::<case>`).
 [cases]
+"core/catalog::test_catalog_from_json_override_catalog_id" = "implicitly protocol v0.9 (catalog_definition.json default): a2ui-rs implements v1.0 only"
+"core/catalog::test_v10_published_basic_catalog_validates_function_calls" = "Plan 3: component validation (validComponents/invalidComponents)"
+"core/catalog::test_v10_published_basic_catalog_rejects_function_call_extra_key" = "Plan 3: component validation (validComponents/invalidComponents)"
+"core/catalog::test_v10_published_basic_catalog_requires_text_field_value" = "Plan 3: component validation (validComponents/invalidComponents)"
+"core/catalog::test_v10_published_basic_catalog_validates_nested_calls" = "Plan 3: component validation (validComponents/invalidComponents)"
+"core/catalog::test_v10_published_basic_catalog_rejects_nested_call_bad_identifiers" = "Plan 3: component validation (validComponents/invalidComponents)"
+"core/catalog::test_v10_catalog_from_json_leaf_defs" = "Plan 3: component validation (validComponents/invalidComponents)"
+"core/catalog::test_v10_basic_catalog_from_json" = "Plan 2, Task 6: selfContained and matchesBuiltinCatalog need validation_schema"
+"core/catalog::test_v10_published_mcp_catalog_is_self_contained" = "Plan 2, Task 6: selfContained needs validation_schema"
+"core/catalog::test_v10_published_vegachart_catalog_is_self_contained" = "Plan 2, Task 6: selfContained needs validation_schema"
```

- [ ] **Step 7: Run the conformance suites**

Run: `cargo test -p a2ui-conformance --test core 2>&1 | tail -3`
Expected: `test result: ok. 197 passed; 0 failed; 648 ignored` (24 `from_json` cases pass; 10 are skipped by name).

- [ ] **Step 8: Update the ARCHITECTURE.md files**

`crates/a2ui-runtime/ARCHITECTURE.md`, add after the `common_types` bullet:

```markdown
- `catalog`: `Catalog::from_json` checks a catalog in the reference order (object, id, version
  1.0, external `$ref` classified never fetched, `$defs`, unions, theme, entries) and keeps the
  entries the `anyComponent`/`anyFunction` unions list.
```

In `## Dependencies`, replace `serde_json` with `serde_json` with `preserve_order` (document order is output: union members, entries).

`crates/a2ui-conformance/ARCHITECTURE.md`, replace the `handlers` bullet with:

```markdown
- `handlers`: `handler(action)`, one runner per implemented action. Catalog actions share
  `catalog_input` (case document, id override, entry checks).
```

- [ ] **Step 9: Run the gate**

Run: `just check && just test`
Expected: success.

- [ ] **Step 10: Commit**

```bash
git add -A
git commit -m "feat(runtime): load v1.0 catalogs with from_json (t-1557)"
oku cli task done --project slider --id t-1557 --result "Catalog::from_json with reference load order; from_json handler; 197 passed, 7 permanent + 3 temporary named skips"
```

---

### Task 5: `to_json`, `round_trip` and the builtin basic catalog (t-1558)

**Files:**
- Create: `crates/a2ui-runtime/src/catalog/document.rs`, `catalog/document_tests.rs`
- Modify: `crates/a2ui-runtime/src/catalog/mod.rs`, `src/spec.rs`, `ARCHITECTURE.md`
- Create: `crates/a2ui-conformance/src/handlers/to_json.rs`
- Modify: `crates/a2ui-conformance/src/handlers/mod.rs`, `skips.toml`

**Interfaces:**
- Consumes: Task 2 `spec::parse`; Task 4 `Catalog::from_json`.
- Produces: `Catalog::to_json(&self) -> Value` (the authored document minus the entries the unions leave out; an id passed to `from_json` is not written into it), `Catalog::basic() -> Result<Catalog, Error>`, `pub(crate) const spec::BASIC_CATALOG: &str`; actions `to_json` (`builtinCatalog: basic` or a catalog) and `round_trip`.

- [ ] **Step 1: Write the failing tests**

`crates/a2ui-runtime/src/catalog/document_tests.rs`:

```rust
use serde_json::json;
use speculoos::prelude::*;

use super::*;

#[test]
fn to_json_returns_the_document_unchanged() {
    let document = json!({
        "catalogId": "c",
        "components": {"Label": {"allOf": [{"$ref": "common_types.json#/$defs/Checkable"}]}},
        "$defs": {"UnusedToken": {"type": "string"}},
        "metadata": {"displayName": "x"}
    });
    let loaded = Catalog::from_json(&document, None, None).unwrap();
    assert_that!(loaded.to_json()).is_equal_to(document);
}

#[test]
fn to_json_omits_entries_the_union_leaves_out() {
    let document = json!({
        "catalogId": "c",
        "components": {"Listed": {}, "Unlisted": {}},
        "$defs": {"anyComponent": {"oneOf": [{"$ref": "#/components/Listed"}]}}
    });
    let loaded = Catalog::from_json(&document, None, None).unwrap();
    assert_that!(loaded.to_json()["components"]).is_equal_to(json!({"Listed": {}}));
}

#[test]
fn to_json_keeps_an_overridden_id_out_of_the_document() {
    let document = json!({"components": {}});
    let loaded = Catalog::from_json(&document, None, Some("override")).unwrap();
    assert_that!(loaded.to_json()).is_equal_to(document);
}

#[test]
fn basic_loads_the_published_basic_catalog() {
    let basic = Catalog::basic().unwrap();
    assert_that!(basic.id())
        .is_equal_to("https://a2ui.org/specification/v1_0/catalogs/basic/catalog.json");
}

#[test]
fn basic_has_eighteen_components_and_fourteen_functions() {
    let basic = Catalog::basic().unwrap();
    assert_that!((basic.components().len(), basic.functions().len())).is_equal_to((18, 14));
}
```

Declare the module in `crates/a2ui-runtime/src/catalog/mod.rs`:

```diff
--- a/crates/a2ui-runtime/src/catalog/mod.rs
+++ b/crates/a2ui-runtime/src/catalog/mod.rs
@@ -4,6 +4,7 @@
 //! unions leave out), so [`Catalog::to_json`] writes it back unchanged.
 //! `$ref` targets are classified, never fetched.
 
+mod document;
 mod entries;
 mod load;
 mod refs;
```

- [ ] **Step 2: Run the tests to see them fail**

Run: `cargo test -p a2ui-runtime document`
Expected: FAIL to compile: `file not found for module 'document'`.

- [ ] **Step 3: Implement**

```diff
--- a/crates/a2ui-runtime/src/spec.rs
+++ b/crates/a2ui-runtime/src/spec.rs
@@ -10,6 +10,8 @@
 
 const COMMON_TYPES: &str = include_str!("../spec/common_types.json");
 const AGENT_TO_RENDERER: &str = include_str!("../spec/agent_to_renderer.json");
+/// The published v1.0 basic catalog document.
+pub(crate) const BASIC_CATALOG: &str = include_str!("../spec/basic_catalog.json");
 
 /// The v1.0 `common_types.json` schema, exactly as published.
 pub fn common_types_schema() -> Result<Value, Error> {
```

`crates/a2ui-runtime/src/catalog/document.rs`:

```rust
//! The catalog as a document: `to_json` and the embedded basic catalog.

use serde_json::Value;

use super::Catalog;
use crate::error::Error;
use crate::spec;

impl Catalog {
    /// The catalog document, as loaded: `$ref`s unbundled and written as authored.
    ///
    /// Entries that `anyComponent`/`anyFunction` left out are absent, so
    /// `from_json(doc).to_json()` equals `doc` for any document whose unions
    /// list every entry.
    pub fn to_json(&self) -> Value {
        Value::Object(self.document.clone())
    }

    /// The published v1.0 basic catalog, embedded in the runtime.
    pub fn basic() -> Result<Self, Error> {
        let document = spec::parse("basic_catalog.json", spec::BASIC_CATALOG)?;
        Self::from_json(&document, None, None)
    }
}

#[cfg(test)]
#[path = "document_tests.rs"]
mod tests;
```

- [ ] **Step 4: Run the unit tests**

Run: `cargo test -p a2ui-runtime`
Expected: PASS, 0 failed (the basic catalog has 18 components and 14 functions).

- [ ] **Step 5: Add the handlers**

`crates/a2ui-conformance/src/handlers/to_json.rs`:

```rust
//! The `to_json` and `round_trip` actions: catalogs written back as documents.

use a2ui_runtime::Catalog;
use serde_json::Value;

use crate::case::Case;
use crate::compare::schema_eq;
use crate::handlers::catalog_input::{catalog_id, document};
use crate::keys::assert_case_keys;
use crate::paths::read_json;

/// Compares a catalog's `to_json()` with `expectFile`, after set-keyword normalization.
///
/// `builtinCatalog: basic` names the runtime's own basic catalog; otherwise
/// `catalog` or `catalogPath` is loaded with `from_json`.
pub fn run(case: &Case) -> Result<(), String> {
    assert_case_keys(
        case,
        &[
            "protocolVersion",
            "builtinCatalog",
            "catalog",
            "catalogPath",
            "catalogId",
            "expectFile",
        ],
    )?;
    let at = case.at();
    let catalog = match case.get("builtinCatalog") {
        None => Catalog::from_json(&document(case)?, case.protocol_version(), catalog_id(case)?)
            .map_err(|err| format!("{at}: {err}"))?,
        Some(_) if case.get("catalog").is_some() || case.get("catalogPath").is_some() => {
            return Err(format!(
                "{at}: `builtinCatalog` excludes `catalog` and `catalogPath`"
            ));
        }
        Some(Value::String(name)) if name == "basic" => {
            if case.protocol_version() != Some("1.0") {
                return Err(format!("{at}: the builtin basic catalog is v1.0"));
            }
            Catalog::basic().map_err(|err| format!("{at}: {err}"))?
        }
        Some(other) => return Err(format!("{at}: unknown builtinCatalog {other}")),
    };
    let file = case
        .get("expectFile")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{at}: `expectFile` must be a string"))?;
    let expected = read_json(file)?;
    if schema_eq(&catalog.to_json(), &expected) {
        Ok(())
    } else {
        Err(format!("{at}: to_json differs from {file}"))
    }
}

/// `from_json(doc).to_json()` equals `doc`, and loading that output again changes nothing.
pub fn run_round_trip(case: &Case) -> Result<(), String> {
    assert_case_keys(
        case,
        &["protocolVersion", "catalog", "catalogPath", "catalogId"],
    )?;
    let at = case.at();
    let original = document(case)?;
    let version = case.protocol_version();
    let id = catalog_id(case)?;
    let written = Catalog::from_json(&original, version, id)
        .map_err(|err| format!("{at}: {err}"))?
        .to_json();
    if !schema_eq(&written, &original) {
        return Err(format!(
            "{at}: to_json {written} differs from the document {original}"
        ));
    }
    let again = Catalog::from_json(&written, version, id)
        .map_err(|err| format!("{at}: reloading: {err}"))?
        .to_json();
    if schema_eq(&again, &written) {
        Ok(())
    } else {
        Err(format!("{at}: a second round trip gave {again}"))
    }
}
```

```diff
--- a/crates/a2ui-conformance/src/handlers/mod.rs
+++ b/crates/a2ui-conformance/src/handlers/mod.rs
@@ -7,6 +7,7 @@
 mod from_json;
 mod resolve_path;
 mod schema_export;
+mod to_json;
 
 use crate::case::Case;
 
@@ -21,6 +22,8 @@
         "parse_expression_template" => Some(expression::run),
         "common_types_schema" => Some(schema_export::run_common_types),
         "from_json" => Some(from_json::run),
+        "to_json" => Some(to_json::run),
+        "round_trip" => Some(to_json::run_round_trip),
         "validate_common_type" => Some(common_type::run),
         "agent_to_renderer_schema" => Some(schema_export::run_agent_to_renderer),
         _ => None,
```

```diff
--- a/crates/a2ui-conformance/skips.toml
+++ b/crates/a2ui-conformance/skips.toml
@@ -1,7 +1,5 @@
 # Actions not implemented yet, each with the plan that implements it.
 [actions]
-to_json = "Plan 2: catalogs and schemas"
-round_trip = "Plan 2: catalogs and schemas"
 validation_schema = "Plan 2: catalogs and schemas"
 derive = "Plan 2: catalogs and schemas"
 validate = "Plan 3: validation and message processing"
```

- [ ] **Step 6: Run the conformance suites**

Run: `cargo test -p a2ui-conformance --test core 2>&1 | tail -3`
Expected: `test result: ok. 203 passed; 0 failed; 642 ignored` (1 `to_json` and 5 `round_trip` cases).

- [ ] **Step 7: Update `crates/a2ui-runtime/ARCHITECTURE.md`**

Append to the `catalog` bullet: `` `to_json` writes that document back; `basic()` is the embedded basic catalog. `` and drop ` (Task 5)` from the `spec` bullet.

- [ ] **Step 8: Run the gate**

Run: `just check && just test`
Expected: success.

- [ ] **Step 9: Commit**

```bash
git add -A
git commit -m "feat(runtime): write catalogs back with to_json and add the basic catalog (t-1558)"
oku cli task done --project slider --id t-1558 --result "Catalog::to_json, Catalog::basic; to_json and round_trip handlers; 203 passed"
```

---

### Task 6: `validation_schema`, `selfContained` and `matchesBuiltinCatalog` (t-1559)

**Files:**
- Create: `crates/a2ui-runtime/src/catalog/validation_schema/mod.rs`, `validation_schema/flatten.rs`, `validation_schema/propagate.rs`, `validation_schema/bundle.rs`, `validation_schema/validation_schema_tests.rs`
- Modify: `crates/a2ui-runtime/src/catalog/mod.rs`, `catalog/refs.rs`, `catalog/refs_tests.rs`, `src/error.rs`, `ARCHITECTURE.md`
- Create: `crates/a2ui-conformance/src/handlers/validation_schema.rs`, `handlers/catalog_checks.rs`, `handlers/catalog_checks_tests.rs`
- Modify: `crates/a2ui-conformance/src/handlers/from_json.rs`, `handlers/mod.rs`, `skips.toml`, `ARCHITECTURE.md`

**Interfaces:**
- Consumes: Task 2 `spec::common_types_schema`; Task 4 `refs`, `load::bare_version`; Task 5 `Catalog::basic`.
- Produces:
  - `Catalog::validation_schema(&self) -> Result<Value, Error>`.
  - `refs::{localize, localized, local_def_name, def_names, reachable_defs(roots: &[&Value], defs: &Map<String, Value>) -> BTreeSet<String>}` (`reachable_defs` is reused by Task 7).
  - Variant `EmbeddedDefs` → `Parse`, display `Embedded document common_types.json has no $defs object`.
  - Harness: `catalog_checks::{self_contained(schema: &Value, at: &str) -> Result<(), String>, matches_builtin(loaded: &Catalog, at: &str) -> Result<(), String>}`; action `validation_schema`.

Algorithm (Python `Catalog.validation_schema`, survey §3.5): assemble `$schema`, `catalogId`, non-empty `instructions`, the leaf `$defs` (localized: `common_types.json#/$defs/X` and `catalog.json#/...` become `#/$defs/X`/`#/...`; nested `$defs` hoisted), a non-empty theme as `$defs.theme`, the entries, and regenerated unions (`anyComponent` with `discriminator.propertyName = "component"`) → flatten each component into one object schema (envelope `id`/`component`, mixins `ComponentCommon`, `Checkable`, … merged, unreachable mixins dropped) → copy common type `description`s onto bare references → bundle every common type reached, transitively (a missing union becomes `{"not": {}}`) → write `protocolVersion` in bare form when declared → close every function's `args`.

- [ ] **Step 1: Write the failing tests**

Replace `crates/a2ui-runtime/src/catalog/refs_tests.rs` with:

```rust
use rstest::rstest;
use serde_json::{Map, json};
use speculoos::prelude::*;

use super::*;

#[rstest]
#[case("#/$defs/Weight")]
#[case("catalog.json#/$defs/anyFunction")]
#[case("common_types.json#/$defs/DynamicString")]
#[case("https://a2ui.org/specification/v1_0/common_types.json#/$defs/DynamicString")]
fn is_supported_local_or_common_types_ref_is_accepted(#[case] reference: &str) {
    assert_that!(is_supported(reference)).is_true();
}

#[rstest]
#[case("basic_catalog.json#/components/Text")]
#[case("https://a2ui.org/specification/v1_0/catalogs/basic/catalog.json#/components/Button")]
#[case("other_catalog.json")]
fn is_supported_other_document_is_refused(#[case] reference: &str) {
    assert_that!(is_supported(reference)).is_false();
}

#[test]
fn check_external_names_the_pointer_of_the_ref() {
    let document = json!({"components": {"a/b": {"allOf": [{"$ref": "x.json#/y"}]}}});
    let err = check_external(&document, "").unwrap_err();
    assert_that!(err.to_string())
        .starts_with("Unsupported external $ref 'x.json#/y' at /components/a~1b/allOf/0/$ref");
}

#[test]
fn check_external_supported_refs_pass() {
    let document = json!({"a": [{"$ref": "#/$defs/A"}, {"$ref": "common_types.json#/$defs/B"}]});
    assert_that!(check_external(&document, "")).is_ok();
}

#[rstest]
#[case("common_types.json#/$defs/DynamicString", "#/$defs/DynamicString")]
#[case(
    "https://a2ui.org/specification/v1_0/common_types.json#/$defs/DynamicString",
    "#/$defs/DynamicString"
)]
#[case("catalog.json#/$defs/anyFunction", "#/$defs/anyFunction")]
#[case("#/$defs/Weight", "#/$defs/Weight")]
#[case("#/components/Text", "#/components/Text")]
fn localize_rewrites_common_types_and_catalog_refs(#[case] reference: &str, #[case] local: &str) {
    let localized = localize(&json!({"items": [{"$ref": reference}]}));
    assert_that!(localized).is_equal_to(json!({"items": [{"$ref": local}]}));
}

#[test]
fn def_names_lists_local_defs_targets_only() {
    let node = json!({
        "a": {"$ref": "#/$defs/Weight/properties/x"},
        "b": [{"$ref": "catalog.json#/$defs/anyFunction"}],
        "c": {"$ref": "common_types.json#/$defs/Action"},
        "d": {"$ref": "#/components/Text"}
    });
    let names: Vec<String> = def_names(&node).into_iter().collect();
    assert_that!(names).is_equal_to(vec!["Weight".to_owned(), "anyFunction".to_owned()]);
}

#[test]
fn reachable_defs_follows_defs_transitively() {
    let defs: Map<String, serde_json::Value> = serde_json::from_value(json!({
        "Primary": {"$ref": "#/$defs/Weight"},
        "Weight": {"type": "string"},
        "Unused": {"type": "string"}
    }))
    .unwrap();
    let root = json!({"$ref": "#/$defs/Primary"});
    let reachable: Vec<String> = reachable_defs(&[&root], &defs).into_iter().collect();
    assert_that!(reachable).is_equal_to(vec!["Primary".to_owned(), "Weight".to_owned()]);
}
```

`crates/a2ui-runtime/src/catalog/validation_schema/validation_schema_tests.rs`:

```rust
use serde_json::{Value, json};
use speculoos::prelude::*;

use crate::catalog::Catalog;

fn schema_of(document: Value) -> Value {
    Catalog::from_json(&document, None, None)
        .unwrap()
        .validation_schema()
        .unwrap()
}

fn select_box() -> Value {
    json!({
        "catalogId": "c",
        "protocolVersion": "v1.0",
        "components": {
            "SelectBox": {
                "allOf": [
                    {"$ref": "common_types.json#/$defs/ComponentCommon"},
                    {"properties": {"component": {"const": "SelectBox"}, "weight": {"$ref": "#/$defs/Weight"}}, "required": ["weight"]}
                ],
                "unevaluatedProperties": false
            }
        },
        "$defs": {"Weight": {"type": "number", "description": "A weight."}}
    })
}

#[test]
fn validation_schema_flattens_mixins_into_one_object() {
    let schema = schema_of(select_box());
    let properties = schema["components"]["SelectBox"]["properties"]
        .as_object()
        .unwrap();
    let names: Vec<&str> = properties.keys().map(String::as_str).collect();
    assert_that!(names).is_equal_to(vec![
        "id",
        "accessibility",
        "metadata",
        "weight",
        "component",
    ]);
}

#[test]
fn validation_schema_requires_the_envelope_around_merged_names() {
    let schema = schema_of(select_box());
    assert_that!(schema["components"]["SelectBox"]["required"]).is_equal_to(json!([
        "id",
        "weight",
        "component"
    ]));
}

#[test]
fn validation_schema_keeps_the_component_closure() {
    let schema = schema_of(select_box());
    assert_that!(schema["components"]["SelectBox"]["unevaluatedProperties"])
        .is_equal_to(json!(false));
}

#[test]
fn validation_schema_copies_common_type_descriptions_onto_refs() {
    let schema = schema_of(select_box());
    let id = &schema["components"]["SelectBox"]["properties"]["id"];
    assert_that!(id["description"].is_string()).is_true();
}

#[test]
fn validation_schema_leaves_leaf_def_refs_undecorated() {
    let schema = schema_of(select_box());
    let weight = &schema["components"]["SelectBox"]["properties"]["weight"];
    assert_that!(weight).is_equal_to(&json!({"$ref": "#/$defs/Weight"}));
}

#[test]
fn validation_schema_bundles_reached_common_types_and_empty_unions() {
    let schema = schema_of(select_box());
    let defs = schema["$defs"].as_object().unwrap();
    let mut names: Vec<&str> = defs.keys().map(String::as_str).collect();
    names.sort_unstable();
    assert_that!(names).is_equal_to(vec![
        "AccessibilityAttributes",
        "ComponentId",
        "DataBinding",
        "DynamicBoolean",
        "DynamicNumber",
        "DynamicString",
        "Extensions",
        "FunctionCall",
        "FunctionCommon",
        "IndexSystemFunction",
        "Weight",
        "anyComponent",
        "anyFunction",
    ]);
}

#[test]
fn validation_schema_missing_function_union_is_empty() {
    let schema = schema_of(select_box());
    assert_that!(schema["$defs"]["anyFunction"]).is_equal_to(json!({"not": {}}));
}

#[test]
fn validation_schema_writes_the_bare_declared_version() {
    assert_that!(schema_of(select_box())["protocolVersion"]).is_equal_to(json!("1.0"));
}

fn boxed() -> Value {
    schema_of(json!({
        "catalogId": "c",
        "components": {"Box": {"allOf": [{"$ref": "#/$defs/Sized"}]}},
        "$defs": {"Sized": {"properties": {"size": {"type": "number"}}, "required": ["size"]}}
    }))
}

#[test]
fn validation_schema_merges_a_local_mixin() {
    assert_that!(boxed()["components"]["Box"]["required"]).is_equal_to(json!([
        "id",
        "size",
        "component"
    ]));
}

#[test]
fn validation_schema_drops_a_merged_local_mixin_from_defs() {
    assert_that!(boxed()["$defs"].get("Sized")).is_none();
}

#[test]
fn validation_schema_closes_call_shaped_function_arguments() {
    let schema = schema_of(json!({
        "catalogId": "c",
        "functions": {
            "upper": {"properties": {"@call": {"const": "upper"}, "args": {"type": "object"}}, "returnType": "string"}
        }
    }));
    let arguments = &schema["functions"]["upper"]["properties"]["args"];
    assert_that!(arguments["unevaluatedProperties"]).is_equal_to(json!(false));
}

#[test]
fn validation_schema_of_the_basic_catalog_references_only_its_own_defs() {
    let schema = Catalog::basic().unwrap().validation_schema().unwrap();
    let text = schema.to_string();
    assert_that!(text.contains("common_types.json")).is_false();
}
```

Declare the module in `crates/a2ui-runtime/src/catalog/mod.rs`:

```diff
--- a/crates/a2ui-runtime/src/catalog/mod.rs
+++ b/crates/a2ui-runtime/src/catalog/mod.rs
@@ -8,6 +8,7 @@
 mod entries;
 mod load;
 mod refs;
+mod validation_schema;
 
 use serde_json::{Map, Value};
 
```

- [ ] **Step 2: Run the tests to see them fail**

Run: `cargo test -p a2ui-runtime catalog`
Expected: FAIL to compile: `file not found for module 'validation_schema'` and unresolved `refs::localize`, `refs::reachable_defs`.

- [ ] **Step 3: Implement**

Replace `crates/a2ui-runtime/src/catalog/refs.rs` with:

```rust
//! `$ref` targets in catalog documents: classified, never fetched.

use std::collections::{BTreeSet, VecDeque};

use serde_json::{Map, Value};

use crate::error::{Error, UnsupportedRefSnafu};

/// Whether a v1.0 catalog may use `reference`: itself or `common_types.json`.
///
/// The document part before `#` is empty (local), `catalog.json` (the
/// catalog itself) or ends in `common_types.json`, relative or absolute.
pub(super) fn is_supported(reference: &str) -> bool {
    let document = reference
        .split_once('#')
        .map_or(reference, |(document, _)| document);
    document.is_empty()
        || document == "catalog.json"
        || document.rsplit('/').next() == Some("common_types.json")
}

/// Fails on the first `$ref` under `node` that [`is_supported`] refuses.
///
/// `at` is the JSON Pointer of `node`; the error names the pointer of the `$ref`.
pub(super) fn check_external(node: &Value, at: &str) -> Result<(), Error> {
    match node {
        Value::Object(fields) => {
            if let Some(Value::String(reference)) = fields.get("$ref")
                && !is_supported(reference)
            {
                return Err(UnsupportedRefSnafu { reference, at }.build().into());
            }
            for (key, value) in fields {
                check_external(value, &format!("{at}/{}", escape(key)))?;
            }
            Ok(())
        }
        Value::Array(items) => {
            for (index, item) in items.iter().enumerate() {
                check_external(item, &format!("{at}/{index}"))?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

/// Rewrites references into `common_types.json` or `catalog.json` `$defs` as local ones.
///
/// `common_types.json#/$defs/Action` becomes `#/$defs/Action`, so the schema
/// resolves once the common types are bundled into its own `$defs`.
pub(super) fn localize(node: &Value) -> Value {
    match node {
        Value::Object(fields) => Value::Object(
            fields
                .iter()
                .map(|(key, value)| match (key.as_str(), value) {
                    ("$ref", Value::String(reference)) => {
                        (key.clone(), Value::String(localized(reference)))
                    }
                    _ => (key.clone(), localize(value)),
                })
                .collect(),
        ),
        Value::Array(items) => Value::Array(items.iter().map(localize).collect()),
        _ => node.clone(),
    }
}

fn localized(reference: &str) -> String {
    match reference.split_once('#') {
        Some((document, fragment))
            if !document.is_empty()
                && fragment.starts_with("/$defs/")
                && (document.ends_with("common_types.json")
                    || document.ends_with("catalog.json")) =>
        {
            format!("#{fragment}")
        }
        _ => reference.to_owned(),
    }
}

/// The `$defs` name a reference into the same catalog targets.
///
/// Accepts `#/$defs/X/...` and `catalog.json#/$defs/X/...`.
fn local_def_name(reference: &str) -> Option<&str> {
    let pointer = match reference.split_once('#') {
        Some(("", pointer)) => pointer,
        Some((document, pointer)) if document.ends_with("catalog.json") => pointer,
        _ => return None,
    };
    pointer.strip_prefix("/$defs/")?.split('/').next()
}

/// The `$defs` names the references under `node` target.
pub(super) fn def_names(node: &Value) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    collect_def_names(node, &mut names);
    names
}

fn collect_def_names(node: &Value, names: &mut BTreeSet<String>) {
    match node {
        Value::Object(fields) => {
            if let Some(name) = fields
                .get("$ref")
                .and_then(Value::as_str)
                .and_then(local_def_name)
            {
                names.insert(name.to_owned());
            }
            fields
                .values()
                .for_each(|value| collect_def_names(value, names));
        }
        Value::Array(items) => items.iter().for_each(|item| collect_def_names(item, names)),
        _ => {}
    }
}

/// The names in `defs` that `roots` reference, transitively.
pub(super) fn reachable_defs(roots: &[&Value], defs: &Map<String, Value>) -> BTreeSet<String> {
    let mut reachable = BTreeSet::new();
    let mut queue: VecDeque<String> = roots.iter().flat_map(|root| def_names(root)).collect();
    while let Some(name) = queue.pop_front() {
        if let Some(def) = defs.get(&name)
            && reachable.insert(name)
        {
            queue.extend(def_names(def));
        }
    }
    reachable
}

/// Escapes one JSON Pointer token (RFC 6901).
fn escape(token: &str) -> String {
    token.replace('~', "~0").replace('/', "~1")
}

#[cfg(test)]
#[path = "refs_tests.rs"]
mod tests;
```

```diff
--- a/crates/a2ui-runtime/src/error.rs
+++ b/crates/a2ui-runtime/src/error.rs
@@ -162,6 +162,9 @@
         backtrace: Backtrace,
     },
 
+    #[snafu(display("Embedded document common_types.json has no $defs object"))]
+    EmbeddedDefs { backtrace: Backtrace },
+
     #[snafu(display("Embedded document {name} is not valid JSON: {source}"))]
     EmbeddedDocument {
         name: &'static str,
@@ -178,7 +181,9 @@
             | Self::TraversePrimitive { .. }
             | Self::NonNumericSegment { .. }
             | Self::IndexOutOfRange { .. } => Category::Data,
-            Self::ExpressionSyntax { .. } | Self::EmbeddedDocument { .. } => Category::Parse,
+            Self::ExpressionSyntax { .. }
+            | Self::EmbeddedDocument { .. }
+            | Self::EmbeddedDefs { .. } => Category::Parse,
             Self::ExpressionLimit { .. } => Category::Expression,
             Self::MissingCatalogId { .. }
             | Self::UnsupportedVersion { .. }
```

`crates/a2ui-runtime/src/catalog/validation_schema/mod.rs`:

```rust
//! `Catalog::validation_schema`: the self-contained schema payloads validate against.
//!
//! The algorithm is upstream's (`Catalog.validation_schema` in the Python SDK)
//! for documents loaded with `from_json`: localize references, flatten
//! components, copy common type descriptions onto references, bundle the
//! common types reached, and write the version in bare form.

mod bundle;
mod flatten;
mod propagate;

use serde_json::{Map, Value, json};

use super::load::bare_version;
use super::{Catalog, refs};
use crate::error::{EmbeddedDefsSnafu, Error};
use crate::spec;

/// The JSON Schema dialect of every validation schema.
const DIALECT: &str = "https://json-schema.org/draft/2020-12/schema";
/// The `$defs` a catalog derives from its entries.
const UNIONS: [&str; 2] = ["anyComponent", "anyFunction"];

impl Catalog {
    /// The self-contained JSON Schema that payloads validate against.
    ///
    /// Every `$ref` is local: the common types the catalog references,
    /// transitively, are bundled into `$defs`, each component is one flattened
    /// object schema, and `anyComponent`/`anyFunction` list the entries.
    pub fn validation_schema(&self) -> Result<Value, Error> {
        let common = common_defs()?;
        let mut schema = self.assemble();
        flatten::flatten_components(&mut schema, &common);
        for key in ["components", "functions", "$defs"] {
            if let Some(node) = schema.get_mut(key) {
                propagate::descriptions(node, &common);
            }
        }
        bundle::common_types(&mut schema, &common);
        if let Some(Value::String(declared)) = self.document.get("protocolVersion") {
            let bare = bare_version(declared.trim());
            schema.insert("protocolVersion".to_owned(), Value::from(bare));
        }
        if let Some(Value::Object(functions)) = schema.get_mut("functions") {
            functions.values_mut().for_each(propagate::close_arguments);
        }
        Ok(Value::Object(schema))
    }

    /// The schema before flattening: entries and leaf defs localized, unions listed.
    fn assemble(&self) -> Map<String, Value> {
        let mut schema = Map::new();
        schema.insert("$schema".to_owned(), Value::from(DIALECT));
        schema.insert("catalogId".to_owned(), Value::from(self.id.as_str()));
        if let Some(Value::String(instructions)) = self.document.get("instructions")
            && !instructions.is_empty()
        {
            schema.insert(
                "instructions".to_owned(),
                Value::from(instructions.as_str()),
            );
        }
        let mut defs: Map<String, Value> = match self.document.get("$defs") {
            Some(Value::Object(defs)) => defs
                .iter()
                .filter(|(name, _)| !UNIONS.contains(&name.as_str()))
                .map(|(name, def)| (name.clone(), refs::localize(def)))
                .collect(),
            _ => Map::new(),
        };
        if let Some(theme) = self
            .theme()
            .filter(|theme| theme.as_object().is_some_and(|t| !t.is_empty()))
        {
            defs.insert("theme".to_owned(), open_theme(refs::localize(theme)));
        }
        let components = entries(
            self.components.iter().map(|c| (c.name(), c.schema())),
            &mut defs,
        );
        let functions = entries(
            self.functions.iter().map(|f| (f.name(), f.schema())),
            &mut defs,
        );
        if !components.is_empty() {
            let members: Vec<Value> = components
                .keys()
                .map(|name| json!({"$ref": format!("#/components/{name}")}))
                .collect();
            let union = json!({"oneOf": members, "discriminator": {"propertyName": "component"}});
            defs.insert("anyComponent".to_owned(), union);
            schema.insert("components".to_owned(), Value::Object(components));
        }
        if !functions.is_empty() {
            let members: Vec<Value> = functions
                .keys()
                .map(|name| json!({"$ref": format!("#/functions/{name}")}))
                .collect();
            defs.insert("anyFunction".to_owned(), json!({"oneOf": members}));
            schema.insert("functions".to_owned(), Value::Object(functions));
        }
        if !defs.is_empty() {
            schema.insert("$defs".to_owned(), Value::Object(defs));
        }
        schema
    }
}

/// Localized entry schemas; `$defs` an entry carries move to the catalog's.
fn entries<'a>(
    entries: impl Iterator<Item = (&'a str, &'a Value)>,
    defs: &mut Map<String, Value>,
) -> Map<String, Value> {
    entries
        .map(|(name, schema)| {
            let mut schema = refs::localize(schema);
            if let Value::Object(fields) = &mut schema
                && let Some(Value::Object(own)) = fields.shift_remove("$defs")
            {
                for (def, body) in own {
                    if !defs.contains_key(&def) {
                        defs.insert(def, body);
                    }
                }
            }
            (name.to_owned(), schema)
        })
        .collect()
}

/// An object theme that does not close itself is declared open.
fn open_theme(mut theme: Value) -> Value {
    if let Value::Object(fields) = &mut theme
        && (fields.get("type") == Some(&Value::from("object")) || fields.contains_key("properties"))
        && !["additionalProperties", "unevaluatedProperties", "$ref"]
            .iter()
            .any(|key| fields.contains_key(*key))
    {
        fields.insert("additionalProperties".to_owned(), Value::Bool(true));
    }
    theme
}

/// The published common types' `$defs`, references localized.
fn common_defs() -> Result<Map<String, Value>, Error> {
    match refs::localize(&spec::common_types_schema()?) {
        Value::Object(mut root) => match root.shift_remove("$defs") {
            Some(Value::Object(defs)) => Ok(defs),
            _ => Err(EmbeddedDefsSnafu.build().into()),
        },
        _ => Err(EmbeddedDefsSnafu.build().into()),
    }
}

#[cfg(test)]
#[path = "validation_schema_tests.rs"]
mod tests;
```

`crates/a2ui-runtime/src/catalog/validation_schema/flatten.rs`:

```rust
//! Flattens each component's `allOf` mixins into one object schema.

use std::collections::BTreeSet;

use serde_json::{Map, Value, json};

use super::UNIONS;
use crate::catalog::refs;

/// Properties every component has; the flattened schema writes them itself.
const ENVELOPE: [&str; 3] = ["id", "component", "catalogId"];
/// Common types merged as mixins, never as local defs.
const MIXINS: [&str; 2] = ["ComponentCommon", "Checkable"];
/// Keywords the flattened schema computes; a component's other keywords are copied.
const MERGED: [&str; 9] = [
    "type",
    "description",
    "allOf",
    "properties",
    "required",
    "unevaluatedProperties",
    "additionalProperties",
    "allowedParents",
    "allowedChildren",
];

/// The sub-schemas a component is merged from, by precedence (lowest first).
#[derive(Default)]
struct Pieces {
    mixins: Vec<Map<String, Value>>,
    local: Vec<Map<String, Value>>,
    common_inline: Vec<Map<String, Value>>,
    specific: Vec<Map<String, Value>>,
}

/// What a walk over one component's `allOf` tree shares between levels.
struct Walk<'a> {
    defs: &'a Map<String, Value>,
    common: &'a Map<String, Value>,
    visited: BTreeSet<String>,
    used: &'a mut BTreeSet<String>,
    unmerged: Vec<Value>,
}

/// Flattens every component of `schema`, then drops local mixins nothing reaches.
pub(super) fn flatten_components(schema: &mut Map<String, Value>, common: &Map<String, Value>) {
    let defs = match schema.get("$defs") {
        Some(Value::Object(defs)) => defs.clone(),
        _ => Map::new(),
    };
    let mut used = BTreeSet::new();
    if let Some(Value::Object(components)) = schema.get_mut("components") {
        for (name, component) in components.iter_mut() {
            if let Value::Object(fields) = component {
                let mut walk = Walk {
                    defs: &defs,
                    common,
                    visited: BTreeSet::new(),
                    used: &mut used,
                    unmerged: Vec::new(),
                };
                let pieces = walk.collect(fields);
                let unmerged = walk.unmerged;
                *component = Value::Object(flattened(name, fields, &pieces, unmerged));
            }
        }
    }
    drop_unreachable_mixins(schema, &used);
}

impl Walk<'_> {
    /// Gathers the pieces of `schema`: its `allOf` members, then itself.
    fn collect(&mut self, schema: &Map<String, Value>) -> Pieces {
        let mut pieces = Pieces::default();
        if let Some(Value::Array(members)) = schema.get("allOf") {
            for member in members.iter().filter_map(Value::as_object) {
                self.member(member, &mut pieces);
            }
        }
        if ["properties", "required", "description"]
            .iter()
            .any(|key| schema.contains_key(*key))
        {
            let specific = schema
                .get("properties")
                .and_then(Value::as_object)
                .is_some_and(|properties| properties.contains_key("component"));
            if specific {
                pieces.specific.push(schema.clone());
            } else {
                pieces.common_inline.push(schema.clone());
            }
        }
        pieces
    }

    /// One `allOf` member: inline, a local mixin, a common mixin, or kept as is.
    fn member(&mut self, member: &Map<String, Value>, pieces: &mut Pieces) {
        let Some(Value::String(reference)) = member.get("$ref") else {
            let inner = self.collect(member);
            pieces.mixins.extend(inner.mixins);
            pieces.local.extend(inner.local);
            pieces.common_inline.extend(inner.common_inline);
            pieces.specific.extend(inner.specific);
            let extra: Map<String, Value> = member
                .iter()
                .filter(|(key, _)| !MERGED.contains(&key.as_str()))
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect();
            if !extra.is_empty() {
                self.unmerged.push(Value::Object(extra));
            }
            return;
        };
        let name = reference
            .split_once("#/$defs/")
            .and_then(|(_, rest)| rest.split('/').next());
        if let Some(name) = name
            && !MIXINS.contains(&name)
            && self.defs.contains_key(name)
        {
            self.used.insert(name.to_owned());
            if self.visited.insert(reference.clone())
                && let Some(Value::Object(target)) = self.defs.get(name)
            {
                let inner = self.collect(target);
                pieces.mixins.extend(inner.mixins);
                pieces.local.extend(inner.local);
                pieces.local.extend(inner.common_inline);
                pieces.local.extend(inner.specific);
            }
            return;
        }
        let mixin = name.and_then(|name| self.common.get(name));
        match mixin.and_then(|mixin| Some((mixin, mixin.get("properties")?.as_object()?))) {
            Some((mixin, properties)) => {
                let mut piece = Map::new();
                piece.insert("properties".to_owned(), Value::Object(properties.clone()));
                if let Some(required @ Value::Array(_)) = mixin.get("required") {
                    piece.insert("required".to_owned(), required.clone());
                }
                pieces.mixins.push(piece);
            }
            None => self.unmerged.push(Value::Object(member.clone())),
        }
    }
}

/// One object schema: the envelope, the merged properties and required names.
fn flattened(
    name: &str,
    component: &Map<String, Value>,
    pieces: &Pieces,
    unmerged: Vec<Value>,
) -> Map<String, Value> {
    let mut description = None;
    let mut properties = Map::new();
    let mut required = BTreeSet::new();
    let mut constant = Value::String(name.to_owned());
    let ordered = pieces
        .mixins
        .iter()
        .chain(&pieces.local)
        .chain(&pieces.common_inline)
        .chain(&pieces.specific);
    for piece in ordered {
        if let Some(Value::String(text)) = piece.get("description")
            && !text.is_empty()
        {
            description = Some(text.clone());
        }
        if let Some(Value::Object(fields)) = piece.get("properties") {
            if let Some(Value::String(component)) = fields
                .get("component")
                .and_then(|component| component.get("const"))
            {
                constant = Value::String(component.clone());
            }
            for (property, schema) in fields {
                if !ENVELOPE.contains(&property.as_str()) {
                    properties.insert(property.clone(), schema.clone());
                }
            }
        }
        if let Some(Value::Array(names)) = piece.get("required") {
            let names = names.iter().filter_map(Value::as_str);
            required.extend(
                names
                    .filter(|name| !ENVELOPE.contains(name))
                    .map(str::to_owned),
            );
        }
    }
    let mut flat = Map::new();
    flat.insert("type".to_owned(), json!("object"));
    for (key, value) in component {
        if !MERGED.contains(&key.as_str()) {
            flat.insert(key.clone(), value.clone());
        }
    }
    if let Some(description) = description {
        flat.insert("description".to_owned(), Value::String(description));
    }
    let mut all = Map::new();
    all.insert("id".to_owned(), json!({"$ref": "#/$defs/ComponentId"}));
    all.extend(properties);
    all.insert("component".to_owned(), json!({ "const": constant }));
    flat.insert("properties".to_owned(), Value::Object(all));
    let mut names = vec![Value::from("id")];
    names.extend(required.into_iter().map(Value::String));
    names.push(Value::from("component"));
    flat.insert("required".to_owned(), Value::Array(names));
    if !unmerged.is_empty() {
        flat.insert("allOf".to_owned(), Value::Array(unmerged));
    }
    let closure = component
        .get("unevaluatedProperties")
        .or_else(|| component.get("additionalProperties"))
        .filter(|closure| !closure.is_null());
    if let Some(closure) = closure {
        flat.insert("unevaluatedProperties".to_owned(), closure.clone());
    }
    for key in ["allowedParents", "allowedChildren"] {
        if let Some(Value::Array(names)) = component.get(key)
            && !names.is_empty()
        {
            flat.insert(key.to_owned(), Value::Array(names.clone()));
        }
    }
    flat
}

/// Removes local mixins that flattening merged and nothing references any more.
fn drop_unreachable_mixins(schema: &mut Map<String, Value>, used: &BTreeSet<String>) {
    if used.is_empty() {
        return;
    }
    let reachable = {
        let Some(Value::Object(defs)) = schema.get("$defs") else {
            return;
        };
        let mut roots: Vec<&Value> = ["components", "functions"]
            .iter()
            .filter_map(|key| schema.get(*key))
            .collect();
        roots.extend(defs.get("theme"));
        refs::reachable_defs(&roots, defs)
    };
    if let Some(Value::Object(defs)) = schema.get_mut("$defs") {
        for name in used {
            if !reachable.contains(name) && !UNIONS.contains(&name.as_str()) && name != "theme" {
                defs.shift_remove(name);
            }
        }
    }
}
```

`crates/a2ui-runtime/src/catalog/validation_schema/propagate.rs`:

```rust
//! Common type descriptions copied onto the references that use them.

use serde_json::{Map, Value};

/// Gives each `{"$ref": "#/$defs/D"}` without a description the common type D's.
///
/// Only common types are consulted: a reference to a catalog's own leaf def
/// keeps no description, and a description written at the reference wins.
pub(super) fn descriptions(node: &mut Value, common: &Map<String, Value>) {
    match node {
        Value::Array(items) => items.iter_mut().for_each(|item| descriptions(item, common)),
        Value::Object(fields) => {
            if !fields.contains_key("description")
                && let Some(description) = fields
                    .get("$ref")
                    .and_then(Value::as_str)
                    .and_then(|reference| reference.strip_prefix("#/$defs/"))
                    .and_then(|path| path.split('/').next())
                    .and_then(|name| common.get(name)?.get("description"))
                    .filter(|description| description.is_string())
            {
                fields.insert("description".to_owned(), description.clone());
            }
            fields
                .values_mut()
                .for_each(|value| descriptions(value, common));
        }
        _ => {}
    }
}

/// Closes a call-shaped function's `args` with `unevaluatedProperties` (default `false`).
pub(super) fn close_arguments(function: &mut Value) {
    let Some(Value::Object(properties)) = function.get_mut("properties") else {
        return;
    };
    if !properties.contains_key("call") && !properties.contains_key("@call") {
        return;
    }
    let Some(Value::Object(arguments)) = properties.get_mut("args") else {
        return;
    };
    if arguments.get("type") != Some(&Value::from("object")) || arguments.contains_key("$ref") {
        return;
    }
    let additional = arguments
        .shift_remove("additionalProperties")
        .filter(|additional| !additional.is_null());
    if !arguments.contains_key("unevaluatedProperties") {
        let closure = additional.unwrap_or(Value::Bool(false));
        arguments.insert("unevaluatedProperties".to_owned(), closure);
    }
}
```

`crates/a2ui-runtime/src/catalog/validation_schema/bundle.rs`:

```rust
//! Bundles the common types a validation schema references into its `$defs`.

use std::collections::{BTreeMap, VecDeque};

use serde_json::{Map, Value, json};

use super::UNIONS;
use crate::catalog::refs;

/// Adds every common type the schema references, transitively, to `$defs`.
///
/// A name the schema defines already stays as it is; a common type is copied
/// verbatim (references localized); a missing union becomes `{"not": {}}`.
pub(super) fn common_types(schema: &mut Map<String, Value>, common: &Map<String, Value>) {
    let seeds: Vec<String> = schema.values().flat_map(refs::def_names).collect();
    let mut queue: VecDeque<String> = seeds.into_iter().collect();
    let mut chosen: BTreeMap<String, Value> = BTreeMap::new();
    let defs = schema
        .entry("$defs")
        .or_insert_with(|| Value::Object(Map::new()));
    let Value::Object(defs) = defs else {
        return;
    };
    while let Some(name) = queue.pop_front() {
        if defs.contains_key(&name) || chosen.contains_key(&name) {
            continue;
        }
        let def = match common.get(&name) {
            Some(def) => def.clone(),
            None if UNIONS.contains(&name.as_str()) => json!({"not": {}}),
            None => continue,
        };
        queue.extend(refs::def_names(&def));
        chosen.insert(name, def);
    }
    defs.extend(chosen);
}
```

- [ ] **Step 4: Run the unit tests**

Run: `cargo test -p a2ui-runtime`
Expected: PASS, 0 failed.

- [ ] **Step 5: Add the harness checks and handler**

`crates/a2ui-conformance/src/handlers/catalog_checks_tests.rs`:

```rust
use rstest::rstest;
use serde_json::json;
use speculoos::prelude::*;

use super::*;

#[test]
fn self_contained_local_refs_pass() {
    let schema = json!({"a": {"$ref": "#/$defs/A~1B"}, "$defs": {"A/B": {"type": "string"}}});
    assert_that!(self_contained(&schema, "here")).is_ok();
}

#[rstest]
#[case(json!({"type": "object"}))]
#[case(json!({"a": {"$ref": "common_types.json#/$defs/A"}}))]
#[case(json!({"a": {"$ref": "#/$defs/Missing"}, "$defs": {}}))]
#[case(json!({"a": {"$ref": "#/list/0"}, "list": [{}]}))]
fn self_contained_schema_without_local_resolution_fails(#[case] schema: Value) {
    assert_that!(self_contained(&schema, "here")).is_err();
}

#[test]
fn matches_builtin_basic_catalog_matches_itself() {
    let basic = Catalog::basic().unwrap();
    assert_that!(matches_builtin(&basic, "here")).is_ok();
}

#[test]
fn matches_builtin_unknown_id_fails() {
    let other = Catalog::from_json(&json!({"catalogId": "other"}), None, None).unwrap();
    assert_that!(matches_builtin(&other, "here")).is_err();
}

#[test]
fn matches_builtin_changed_required_names_fail() {
    let mut document = Catalog::basic().unwrap().to_json();
    document["components"]["Text"]["required"] = json!([]);
    let changed = Catalog::from_json(&document, None, None).unwrap();
    assert_that!(matches_builtin(&changed, "here")).is_err();
}
```

`crates/a2ui-conformance/src/handlers/catalog_checks.rs`:

```rust
//! The `selfContained` and `matchesBuiltinCatalog` expectations of `from_json`.

use std::collections::{BTreeMap, BTreeSet};

use a2ui_runtime::Catalog;
use serde_json::Value;

/// Component fields every component carries, left out of shape comparisons.
const ENVELOPE: [&str; 2] = ["id", "component"];

/// The schema has a `$ref`, and every `$ref` is a `#` pointer resolving inside it.
pub fn self_contained(schema: &Value, at: &str) -> Result<(), String> {
    let mut references = Vec::new();
    collect_refs(schema, &mut references);
    if references.is_empty() {
        return Err(format!("{at}: the validation schema has no $ref at all"));
    }
    for reference in references {
        let pointer = reference
            .strip_prefix('#')
            .ok_or_else(|| format!("{at}: $ref '{reference}' leaves the schema"))?;
        let mut target = schema;
        for token in pointer.split('/').skip(1) {
            let token = token.replace("~1", "/").replace("~0", "~");
            target = target
                .as_object()
                .and_then(|fields| fields.get(&token))
                .ok_or_else(|| format!("{at}: $ref '{reference}' does not resolve"))?;
        }
    }
    Ok(())
}

/// The loaded catalog is equivalent to the runtime's own catalog of the same id.
///
/// Same id and version, the same property and required names per component
/// (read from the validation schemas, without the envelope), and the same
/// return type, callers and activation per function.
pub fn matches_builtin(loaded: &Catalog, at: &str) -> Result<(), String> {
    let builtin = Catalog::basic().map_err(|err| format!("{at}: {err}"))?;
    if loaded.id() != builtin.id() {
        return Err(format!(
            "{at}: a2ui-rs has no builtin catalog '{}'",
            loaded.id()
        ));
    }
    if bare(loaded.protocol_version()) != bare(builtin.protocol_version()) {
        return Err(format!(
            "{at}: protocolVersion differs from the builtin catalog"
        ));
    }
    if component_shapes(loaded, at)? != component_shapes(&builtin, at)? {
        return Err(format!("{at}: components differ from the builtin catalog"));
    }
    if signatures(loaded) != signatures(&builtin) {
        return Err(format!("{at}: functions differ from the builtin catalog"));
    }
    Ok(())
}

type Shape = (BTreeSet<String>, BTreeSet<String>);

fn component_shapes(catalog: &Catalog, at: &str) -> Result<BTreeMap<String, Shape>, String> {
    let schema = catalog
        .validation_schema()
        .map_err(|err| format!("{at}: {err}"))?;
    let mut shapes = BTreeMap::new();
    if let Some(Value::Object(components)) = schema.get("components") {
        for (name, component) in components {
            let (mut properties, mut required) = shape(&schema, component, &BTreeSet::new(), at)?;
            for envelope in ENVELOPE {
                properties.remove(envelope);
                required.remove(envelope);
            }
            shapes.insert(name.clone(), (properties, required));
        }
    }
    Ok(shapes)
}

/// Property and required names, merged across `allOf` and local `$ref`s.
fn shape(
    document: &Value,
    schema: &Value,
    seen: &BTreeSet<String>,
    at: &str,
) -> Result<Shape, String> {
    let mut properties = BTreeSet::new();
    let mut required = BTreeSet::new();
    let Value::Object(fields) = schema else {
        return Ok((properties, required));
    };
    if let Some(Value::Object(names)) = fields.get("properties") {
        properties.extend(names.keys().cloned());
    }
    if let Some(Value::Array(names)) = fields.get("required") {
        required.extend(names.iter().filter_map(Value::as_str).map(str::to_owned));
    }
    let mut members: Vec<(&Value, BTreeSet<String>)> = Vec::new();
    if let Some(Value::String(reference)) = fields.get("$ref")
        && let Some(pointer) = reference.strip_prefix('#')
        && !seen.contains(reference)
    {
        let target = document
            .pointer(pointer)
            .ok_or_else(|| format!("{at}: $ref '{reference}' does not resolve"))?;
        let mut deeper = seen.clone();
        deeper.insert(reference.clone());
        members.push((target, deeper));
    }
    if let Some(Value::Array(all_of)) = fields.get("allOf") {
        members.extend(all_of.iter().map(|member| (member, seen.clone())));
    }
    for (member, seen) in members {
        let (more_properties, more_required) = shape(document, member, &seen, at)?;
        properties.extend(more_properties);
        required.extend(more_required);
    }
    Ok((properties, required))
}

type Signature = (Option<String>, &'static str, bool);

fn signatures(catalog: &Catalog) -> BTreeMap<String, Signature> {
    catalog
        .functions()
        .iter()
        .filter(|function| !function.name().starts_with('@'))
        .map(|function| {
            let signature = (
                function.return_type().map(str::to_owned),
                function.allowed_callers().as_str(),
                function.requires_user_activation(),
            );
            (function.name().to_owned(), signature)
        })
        .collect()
}

fn bare(version: &str) -> &str {
    version
        .strip_prefix('v')
        .filter(|rest| rest.starts_with(|c: char| c.is_ascii_digit()))
        .unwrap_or(version)
}

fn collect_refs<'a>(node: &'a Value, references: &mut Vec<&'a str>) {
    match node {
        Value::Object(fields) => {
            if let Some(Value::String(reference)) = fields.get("$ref") {
                references.push(reference);
            }
            fields
                .values()
                .for_each(|value| collect_refs(value, references));
        }
        Value::Array(items) => items.iter().for_each(|item| collect_refs(item, references)),
        _ => {}
    }
}

#[cfg(test)]
#[path = "catalog_checks_tests.rs"]
mod tests;
```

`crates/a2ui-conformance/src/handlers/validation_schema.rs`:

```rust
//! The `validation_schema` action: a loaded catalog's schema against a golden file.

use a2ui_runtime::Catalog;
use serde_json::Value;

use crate::case::Case;
use crate::compare::{expect_error, schema_eq};
use crate::handlers::catalog_input::{catalog_id, document};
use crate::keys::assert_case_keys;
use crate::paths::read_json;

/// Loads the catalog with `from_json`; compares `validation_schema()` with `expectFile`.
///
/// The comparison ignores key order and the order of `enum` and `required`.
pub fn run(case: &Case) -> Result<(), String> {
    assert_case_keys(
        case,
        &[
            "protocolVersion",
            "catalog",
            "catalogPath",
            "catalogId",
            "expectFile",
            "expectError",
        ],
    )?;
    let at = case.at();
    let schema = Catalog::from_json(&document(case)?, case.protocol_version(), catalog_id(case)?)
        .and_then(|catalog| catalog.validation_schema());
    match (schema, case.get("expectError"), case.get("expectFile")) {
        (_, Some(_), Some(_)) => Err(format!(
            "{at}: `expectFile` and `expectError` are exclusive"
        )),
        (Err(err), Some(expected), None) => expect_error(&err, expected, &at),
        (Err(err), None, _) => Err(format!("{at}: {err}")),
        (Ok(_), Some(_), None) => Err(format!("{at}: expected an error, got a schema")),
        (Ok(_), None, None) => Err(format!("{at}: a case needs `expectFile` or `expectError`")),
        (Ok(schema), None, Some(Value::String(file))) => {
            let expected = read_json(file)?;
            if schema_eq(&schema, &expected) {
                Ok(())
            } else {
                Err(format!(
                    "{at}: validation_schema {schema} differs from {file}"
                ))
            }
        }
        (Ok(_), None, Some(other)) => {
            Err(format!("{at}: `expectFile` must be a string, got {other}"))
        }
    }
}
```

Implement `selfContained` and `matchesBuiltinCatalog` in `from_json`:

```diff
--- a/crates/a2ui-conformance/src/handlers/from_json.rs
+++ b/crates/a2ui-conformance/src/handlers/from_json.rs
@@ -5,6 +5,7 @@
 
 use crate::case::Case;
 use crate::compare::{expect_error, json_eq, schema_eq};
+use crate::handlers::catalog_checks::{matches_builtin, self_contained};
 use crate::handlers::catalog_input::{catalog_id, check_entries, document};
 use crate::keys::{assert_case_keys, assert_known_keys};
 
@@ -73,11 +74,17 @@
             }
             "theme" => check_theme(catalog, expected, at)?,
             "componentMetadata" => check_metadata(catalog, expected, at)?,
-            "selfContained" | "matchesBuiltinCatalog" => {
-                return Err(format!(
-                    "{at}: `{key}` needs validation_schema (Plan 2, Task 6)"
-                ));
+            "selfContained" => {
+                expect_true(key, expected, at)?;
+                let schema = catalog
+                    .validation_schema()
+                    .map_err(|err| format!("{at}: {err}"))?;
+                self_contained(&schema, at)?;
             }
+            "matchesBuiltinCatalog" => {
+                expect_true(key, expected, at)?;
+                matches_builtin(catalog, at)?;
+            }
             _ => {
                 return Err(format!("{at}: `{key}` needs component validation (Plan 3)"));
             }
@@ -86,6 +93,14 @@
     Ok(())
 }
 
+/// `selfContained` and `matchesBuiltinCatalog` only ever assert `true`.
+fn expect_true(key: &str, expected: &Value, at: &str) -> Result<(), String> {
+    match expected {
+        Value::Bool(true) => Ok(()),
+        other => Err(format!("{at}: `{key}` must be true, got {other}")),
+    }
+}
+
 fn check_text(key: &str, actual: &str, expected: &Value, at: &str) -> Result<(), String> {
     match expected {
         Value::String(expected) if expected == actual => Ok(()),
```

```diff
--- a/crates/a2ui-conformance/src/handlers/mod.rs
+++ b/crates/a2ui-conformance/src/handlers/mod.rs
@@ -1,5 +1,6 @@
 //! The runner for each conformance action.
 
+mod catalog_checks;
 mod catalog_input;
 mod common_type;
 mod data_model;
@@ -8,6 +9,7 @@
 mod resolve_path;
 mod schema_export;
 mod to_json;
+mod validation_schema;
 
 use crate::case::Case;
 
@@ -24,6 +26,7 @@
         "from_json" => Some(from_json::run),
         "to_json" => Some(to_json::run),
         "round_trip" => Some(to_json::run_round_trip),
+        "validation_schema" => Some(validation_schema::run),
         "validate_common_type" => Some(common_type::run),
         "agent_to_renderer_schema" => Some(schema_export::run_agent_to_renderer),
         _ => None,
```

Remove the three temporary skips and the `validation_schema` action:

```diff
--- a/crates/a2ui-conformance/skips.toml
+++ b/crates/a2ui-conformance/skips.toml
@@ -1,6 +1,5 @@
 # Actions not implemented yet, each with the plan that implements it.
 [actions]
-validation_schema = "Plan 2: catalogs and schemas"
 derive = "Plan 2: catalogs and schemas"
 validate = "Plan 3: validation and message processing"
 process_messages = "Plan 3: validation and message processing"
@@ -31,6 +30,3 @@
 "core/catalog::test_v10_published_basic_catalog_validates_nested_calls" = "Plan 3: component validation (validComponents/invalidComponents)"
 "core/catalog::test_v10_published_basic_catalog_rejects_nested_call_bad_identifiers" = "Plan 3: component validation (validComponents/invalidComponents)"
 "core/catalog::test_v10_catalog_from_json_leaf_defs" = "Plan 3: component validation (validComponents/invalidComponents)"
-"core/catalog::test_v10_basic_catalog_from_json" = "Plan 2, Task 6: selfContained and matchesBuiltinCatalog need validation_schema"
-"core/catalog::test_v10_published_mcp_catalog_is_self_contained" = "Plan 2, Task 6: selfContained needs validation_schema"
-"core/catalog::test_v10_published_vegachart_catalog_is_self_contained" = "Plan 2, Task 6: selfContained needs validation_schema"
```

- [ ] **Step 6: Run the tests**

Run: `cargo test -p a2ui-conformance --lib && cargo test -p a2ui-conformance --test core 2>&1 | tail -3`
Expected: PASS; `test result: ok. 208 passed; 0 failed; 637 ignored` (2 `validation_schema` goldens and the 3 formerly skipped `from_json` cases).

- [ ] **Step 7: Update the ARCHITECTURE.md files**

`crates/a2ui-runtime/ARCHITECTURE.md`, add after the `catalog` bullet:

```markdown
- `catalog::validation_schema`: the self-contained schema agents and validators use: flatten
  component mixins, propagate descriptions, bundle the common types it references.
```

`crates/a2ui-conformance/ARCHITECTURE.md`, append to the `handlers` bullet: `` and `catalog_checks` (`selfContained`, `matchesBuiltinCatalog`). `` (replacing the final period after `entry checks)`).

- [ ] **Step 8: Run the gate**

Run: `just check && just test`
Expected: success.

- [ ] **Step 9: Commit**

```bash
git add -A
git commit -m "feat(runtime): build catalog validation schemas (t-1559)"
oku cli task done --project slider --id t-1559 --result "Catalog::validation_schema reproduces both goldens; selfContained/matchesBuiltinCatalog; 208 passed, temporary skips removed"
```

---

### Task 7: `derive` (t-1560)

**Files:**
- Create: `crates/a2ui-runtime/src/catalog/derive.rs`, `catalog/derive_tests.rs`
- Modify: `crates/a2ui-runtime/src/catalog/mod.rs`, `src/error.rs`, `src/lib.rs`, `ARCHITECTURE.md`
- Create: `crates/a2ui-conformance/src/handlers/derive.rs`
- Modify: `crates/a2ui-conformance/src/handlers/mod.rs`, `skips.toml`

**Interfaces:**
- Consumes: Task 4 `Catalog::from_json`; Task 6 `refs::reachable_defs`.
- Produces:
  - `Catalog::derive(&self, args: DeriveArgs) -> Result<Catalog, Error>`.
  - `pub struct DeriveArgs { pub catalog_id: String, pub components: Option<Vec<DeriveEntry>>, pub functions: Option<Vec<DeriveEntry>>, pub instructions: Option<String> }` (`Default`), `pub enum DeriveEntry { Keep(String), New(String, Value) }`, both re-exported at the crate root.
  - Variant `UnknownDeriveEntry { catalog: String, kind: &'static str, name: String }` → `Catalog`, display `Catalog '{catalog}' has no {kind} '{name}' to derive a catalog from.`
  - Harness action `derive` (case keys `catalog`, `catalogPath`, `args`, exactly one of `expect`/`expectError`; `args` keys `catalogId` (required), `components`, `functions`, `instructions`, entries are names or one-key mappings; `expect` keys `catalogId`, `protocolVersion`, `components`, `functions`, `theme`, `instructions`, `absentKeys`).

- [ ] **Step 1: Write the failing tests**

`crates/a2ui-runtime/src/catalog/derive_tests.rs`:

```rust
use serde_json::{Value, json};
use speculoos::prelude::*;

use super::*;
use crate::Category;
use crate::catalog::ComponentDef;

fn source() -> Catalog {
    let document = json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": "https://example.com/source.json",
        "title": "Source",
        "description": "The source catalog.",
        "catalogId": "source",
        "protocolVersion": "1.0",
        "instructions": "Use source.",
        "components": {
            "Text": {"type": "object", "properties": {"text": {"$ref": "#/$defs/Label"}}},
            "Image": {"type": "object", "properties": {"url": {"$ref": "#/$defs/Url"}}}
        },
        "functions": {
            "upper": {"type": "object", "properties": {"call": {"const": "upper"}}}
        },
        "theme": {"type": "object"},
        "$defs": {
            "Label": {"type": "string"},
            "Url": {"type": "string", "format": "uri"},
            "Spare": {"type": "number"},
            "anyComponent": {
                "oneOf": [{"$ref": "#/components/Text"}, {"$ref": "#/components/Image"}],
                "discriminator": {"propertyName": "component"}
            }
        }
    });
    Catalog::from_json(&document, None, None).unwrap()
}

fn args(components: Option<Vec<DeriveEntry>>) -> DeriveArgs {
    DeriveArgs {
        catalog_id: "derived".to_owned(),
        components,
        ..DeriveArgs::default()
    }
}

fn keep(name: &str) -> DeriveEntry {
    DeriveEntry::Keep(name.to_owned())
}

#[test]
fn derive_named_entries_keeps_only_those() {
    let derived = source().derive(args(Some(vec![keep("Text")]))).unwrap();
    let names: Vec<&str> = derived
        .components()
        .iter()
        .map(ComponentDef::name)
        .collect();
    assert_that!(names).is_equal_to(vec!["Text"]);
}

#[test]
fn derive_omitted_list_keeps_every_entry() {
    let derived = source().derive(args(Some(vec![keep("Text")]))).unwrap();
    assert_that!(derived.functions().len()).is_equal_to(1);
}

#[test]
fn derive_sets_the_new_id_and_keeps_the_version() {
    let derived = source().derive(args(None)).unwrap();
    assert_that!((derived.id(), derived.protocol_version())).is_equal_to(("derived", "1.0"));
}

#[test]
fn derive_drops_the_describing_keys() {
    let written = source().derive(args(None)).unwrap().to_json();
    for key in ["$id", "title", "description", "instructions"] {
        assert_that!(written.get(key)).is_none();
    }
}

#[test]
fn derive_keeps_schema_and_theme() {
    let written = source().derive(args(None)).unwrap().to_json();
    assert_that!((
        written.get("$schema").is_some(),
        written.get("theme").is_some()
    ))
    .is_equal_to((true, true));
}

#[test]
fn derive_instructions_replace_the_source_ones() {
    let derived = source()
        .derive(DeriveArgs {
            instructions: Some("Use derived.".to_owned()),
            ..args(None)
        })
        .unwrap();
    assert_that!(derived.to_json()["instructions"]).is_equal_to(json!("Use derived."));
}

#[test]
fn derive_new_entry_replaces_the_source_entry_in_place() {
    let card = json!({"type": "object"});
    let derived = source()
        .derive(args(Some(vec![
            DeriveEntry::New("Image".to_owned(), card.clone()),
            keep("Text"),
        ])))
        .unwrap();
    let entries: Vec<(&str, &Value)> = derived
        .components()
        .iter()
        .map(|component| (component.name(), component.schema()))
        .collect();
    assert_that!(entries[0]).is_equal_to(("Image", &card));
}

#[test]
fn derive_later_entry_replaces_an_earlier_one() {
    let card = json!({"type": "object", "title": "later"});
    let derived = source()
        .derive(args(Some(vec![
            keep("Text"),
            DeriveEntry::New("Text".to_owned(), card.clone()),
        ])))
        .unwrap();
    assert_that!(derived.component("Text").map(ComponentDef::schema)).is_equal_to(Some(&card));
}

#[test]
fn derive_unknown_component_is_catalog_error() {
    let err = source()
        .derive(args(Some(vec![keep("Slider")])))
        .unwrap_err();
    assert_that!(err.category()).is_equal_to(Category::Catalog);
    assert_that!(err.to_string()).contains("'Slider'");
}

#[test]
fn derive_unknown_function_is_catalog_error() {
    let err = source()
        .derive(DeriveArgs {
            functions: Some(vec![keep("capitalize")]),
            ..args(None)
        })
        .unwrap_err();
    assert_that!(err.to_string()).contains("function 'capitalize'");
}

#[test]
fn derive_new_entry_is_checked_like_from_json() {
    let err = source()
        .derive(args(Some(vec![DeriveEntry::New(
            "Surface".to_owned(),
            json!({}),
        )])))
        .unwrap_err();
    assert_that!(err.category()).is_equal_to(Category::Catalog);
}

#[test]
fn derive_changed_list_rebuilds_the_union() {
    let written = source()
        .derive(args(Some(vec![keep("Text")])))
        .unwrap()
        .to_json();
    assert_that!(written["$defs"]["anyComponent"]["oneOf"])
        .is_equal_to(json!([{"$ref": "#/components/Text"}]));
}

#[test]
fn derive_empty_list_writes_an_empty_union() {
    let written = source().derive(args(Some(Vec::new()))).unwrap().to_json();
    assert_that!(written["$defs"]["anyComponent"]).is_equal_to(json!({"not": {}}));
}

#[test]
fn derive_unchanged_list_keeps_the_written_union() {
    let original = source().to_json()["$defs"]["anyComponent"].clone();
    let written = source()
        .derive(args(Some(vec![keep("Text"), keep("Image")])))
        .unwrap()
        .to_json();
    assert_that!(written["$defs"]["anyComponent"]).is_equal_to(original);
}

#[test]
fn derive_drops_defs_only_dropped_entries_used() {
    let written = source()
        .derive(args(Some(vec![keep("Text")])))
        .unwrap()
        .to_json();
    let defs = written["$defs"].as_object().unwrap();
    assert_that!((defs.contains_key("Label"), defs.contains_key("Url"))).is_equal_to((true, false));
}

#[test]
fn derive_keeps_defs_no_entry_used() {
    let written = source()
        .derive(args(Some(vec![keep("Text")])))
        .unwrap()
        .to_json();
    assert_that!(written["$defs"].get("Spare")).is_some();
}

#[test]
fn derive_without_source_id_key_writes_none() {
    let source = Catalog::from_json(&json!({"components": {}}), None, Some("source")).unwrap();
    let derived = source.derive(args(None)).unwrap();
    assert_that!((derived.id(), derived.to_json().get("catalogId").is_none()))
        .is_equal_to(("derived", true));
}
```

Declare and export the module:

```diff
--- a/crates/a2ui-runtime/src/catalog/mod.rs
+++ b/crates/a2ui-runtime/src/catalog/mod.rs
@@ -4,6 +4,7 @@
 //! unions leave out), so [`Catalog::to_json`] writes it back unchanged.
 //! `$ref` targets are classified, never fetched.
 
+mod derive;
 mod document;
 mod entries;
 mod load;
@@ -12,6 +13,8 @@
 
 use serde_json::{Map, Value};
 
+pub use derive::{DeriveArgs, DeriveEntry};
+
 use crate::common_types::ComponentMetadata;
 use crate::error::{Error, MalformedCatalogSnafu};
 
```

```diff
--- a/crates/a2ui-runtime/src/lib.rs
+++ b/crates/a2ui-runtime/src/lib.rs
@@ -23,7 +23,7 @@
 mod subscribe;
 pub use subscribe::Watchers;
 
-pub use catalog::{AllowedCallers, Catalog, ComponentDef, FunctionDef};
+pub use catalog::{AllowedCallers, Catalog, ComponentDef, DeriveArgs, DeriveEntry, FunctionDef};
 pub use common_types::{COMMON_TYPES, validate_common_type};
 pub use context::resolve_path;
 pub use data::{DataModel, Write};
```

- [ ] **Step 2: Run the tests to see them fail**

Run: `cargo test -p a2ui-runtime derive`
Expected: FAIL to compile: `file not found for module 'derive'`.

- [ ] **Step 3: Implement**

```diff
--- a/crates/a2ui-runtime/src/error.rs
+++ b/crates/a2ui-runtime/src/error.rs
@@ -136,6 +136,14 @@
         backtrace: Backtrace,
     },
 
+    #[snafu(display("Catalog '{catalog}' has no {kind} '{name}' to derive a catalog from."))]
+    UnknownDeriveEntry {
+        catalog: String,
+        kind: &'static str,
+        name: String,
+        backtrace: Backtrace,
+    },
+
     #[snafu(display("Malformed catalog: {reason}"))]
     MalformedCatalog {
         reason: String,
@@ -191,6 +199,7 @@
             | Self::InvalidIdentifier { .. }
             | Self::ReservedComponent { .. }
             | Self::UserActivationCallers { .. }
+            | Self::UnknownDeriveEntry { .. }
             | Self::MalformedCatalog { .. } => Category::Catalog,
             Self::InvalidMetadata { .. }
             | Self::InvalidCommonType { .. }
```

`crates/a2ui-runtime/src/catalog/derive.rs`:

```rust
//! `Catalog::derive`: a new catalog, under its own id, from entries of this one.

use serde_json::{Map, Value, json};

use super::{Catalog, refs};
use crate::error::{Error, UnknownDeriveEntrySnafu};

/// Top-level document keys a catalog models; any other key is kept as written.
const MODELED_KEYS: [&str; 11] = [
    "$schema",
    "$id",
    "title",
    "description",
    "catalogId",
    "protocolVersion",
    "instructions",
    "components",
    "functions",
    "theme",
    "$defs",
];

/// Keys that describe the source catalog, which a derived catalog drops.
const DESCRIBING_KEYS: [&str; 4] = ["$id", "title", "description", "instructions"];

/// What [`Catalog::derive`] builds: the new id, entries and instructions.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DeriveArgs {
    /// The id of the derived catalog.
    pub catalog_id: String,
    /// The components, in order; `None` keeps all of the source's.
    pub components: Option<Vec<DeriveEntry>>,
    /// The functions, in order; `None` keeps all of the source's.
    pub functions: Option<Vec<DeriveEntry>>,
    /// Instructions for agents that use the derived catalog.
    pub instructions: Option<String>,
}

/// One entry of a derived catalog.
#[derive(Debug, Clone, PartialEq)]
pub enum DeriveEntry {
    /// The source's entry of that name.
    Keep(String),
    /// A new entry (name, schema), replacing any source entry of that name.
    New(String, Value),
}

/// The two entry kinds of a catalog document.
#[derive(Clone, Copy)]
struct Kind {
    /// The singular name used in errors, such as `component`.
    singular: &'static str,
    /// The document key, such as `components`.
    key: &'static str,
    /// The union def that lists the entries.
    union: &'static str,
}

const COMPONENTS: Kind = Kind {
    singular: "component",
    key: "components",
    union: "anyComponent",
};
const FUNCTIONS: Kind = Kind {
    singular: "function",
    key: "functions",
    union: "anyFunction",
};

impl Catalog {
    /// Builds a catalog from this one's entries and new ones, under `args.catalog_id`.
    ///
    /// The derived catalog keeps the protocol version, `$schema`, theme and
    /// unknown top-level keys; it drops `$id`, `title`, `description` and
    /// `instructions` (replaced by `args.instructions`), and the leaf defs only
    /// dropped entries used. New entries go through the same checks as
    /// [`Catalog::from_json`].
    pub fn derive(&self, args: DeriveArgs) -> Result<Self, Error> {
        let mut document = self.document.clone();
        for key in DESCRIBING_KEYS {
            document.shift_remove(key);
        }
        if document.contains_key("catalogId") {
            document.insert(
                "catalogId".to_owned(),
                Value::from(args.catalog_id.as_str()),
            );
        }
        if let Some(instructions) = &args.instructions {
            document.insert(
                "instructions".to_owned(),
                Value::from(instructions.as_str()),
            );
        }
        let mut changed = Vec::new();
        for (kind, wanted) in [(COMPONENTS, &args.components), (FUNCTIONS, &args.functions)] {
            if let Some(wanted) = wanted {
                let entries = self.picked(kind, wanted)?;
                if names(Some(&entries))
                    != names(self.document.get(kind.key).and_then(Value::as_object))
                {
                    changed.push(kind);
                }
                document.insert(kind.key.to_owned(), Value::Object(entries));
            }
        }
        if let Some(Value::Object(defs)) = self.document.get("$defs") {
            let defs = self.derived_defs(defs, &document, &changed);
            document.insert("$defs".to_owned(), Value::Object(defs));
        }
        Self::from_json(
            &Value::Object(document),
            Some(&self.protocol_version),
            Some(&args.catalog_id),
        )
    }

    /// The entries `wanted` names, in order; a later entry replaces an earlier one.
    fn picked(&self, kind: Kind, wanted: &[DeriveEntry]) -> Result<Map<String, Value>, Error> {
        let own = self.document.get(kind.key).and_then(Value::as_object);
        let mut picked = Map::new();
        for entry in wanted {
            match entry {
                DeriveEntry::Keep(name) => {
                    let schema = own.and_then(|own| own.get(name)).ok_or_else(|| {
                        UnknownDeriveEntrySnafu {
                            catalog: self.id.as_str(),
                            kind: kind.singular,
                            name,
                        }
                        .build()
                    })?;
                    picked.insert(name.clone(), schema.clone());
                }
                DeriveEntry::New(name, schema) => {
                    picked.insert(name.clone(), schema.clone());
                }
            }
        }
        Ok(picked)
    }

    /// The source's `$defs` for the derived `document`.
    ///
    /// A union whose entries changed is listed from the new entries; a leaf
    /// def stays unless the source's entries used it and the new ones do not.
    fn derived_defs(
        &self,
        defs: &Map<String, Value>,
        document: &Map<String, Value>,
        changed: &[Kind],
    ) -> Map<String, Value> {
        let leaves: Map<String, Value> = defs
            .iter()
            .filter(|(name, _)| ![COMPONENTS.union, FUNCTIONS.union].contains(&name.as_str()))
            .map(|(name, def)| (name.clone(), def.clone()))
            .collect();
        let mut unions = Map::new();
        for kind in [COMPONENTS, FUNCTIONS] {
            if let Some(authored) = defs.get(kind.union) {
                let union = if changed.iter().any(|changed| changed.key == kind.key) {
                    union_of(kind, document.get(kind.key).and_then(Value::as_object))
                } else {
                    authored.clone()
                };
                unions.insert(kind.union.to_owned(), union);
            }
        }
        let old_roots = roots(&self.document, defs.values());
        let old = refs::reachable_defs(&old_roots, &leaves);
        let orphans = leaves
            .iter()
            .filter(|(name, _)| !old.contains(*name))
            .map(|(_, def)| def);
        let new_roots = roots(document, unions.values().chain(orphans));
        let new = refs::reachable_defs(&new_roots, &leaves);
        let mut derived: Map<String, Value> = leaves
            .into_iter()
            .filter(|(name, _)| !old.contains(name) || new.contains(name))
            .collect();
        derived.extend(unions);
        derived
    }
}

/// The parts of a document that reference its `$defs`, plus `more`.
fn roots<'a>(
    document: &'a Map<String, Value>,
    more: impl Iterator<Item = &'a Value>,
) -> Vec<&'a Value> {
    let mut roots: Vec<&Value> = ["components", "functions", "theme"]
        .iter()
        .filter_map(|key| document.get(*key))
        .collect();
    roots.extend(
        document
            .iter()
            .filter(|(key, _)| !MODELED_KEYS.contains(&key.as_str()))
            .map(|(_, value)| value),
    );
    roots.extend(more);
    roots
}

fn names(entries: Option<&Map<String, Value>>) -> Vec<&str> {
    entries
        .into_iter()
        .flat_map(|entries| entries.keys().map(String::as_str))
        .collect()
}

/// The union listing `entries`, or `{"not": {}}` when there are none.
fn union_of(kind: Kind, entries: Option<&Map<String, Value>>) -> Value {
    let members: Vec<Value> = names(entries)
        .into_iter()
        .map(|name| json!({"$ref": format!("#/{}/{name}", kind.key)}))
        .collect();
    match (members.is_empty(), kind.key) {
        (true, _) => json!({"not": {}}),
        (false, "components") => {
            json!({"oneOf": members, "discriminator": {"propertyName": "component"}})
        }
        (false, _) => json!({"oneOf": members}),
    }
}

#[cfg(test)]
#[path = "derive_tests.rs"]
mod tests;
```

- [ ] **Step 4: Run the unit tests**

Run: `cargo test -p a2ui-runtime derive`
Expected: PASS, 17 passed, 0 failed.

- [ ] **Step 5: Add the handler**

`crates/a2ui-conformance/src/handlers/derive.rs`:

```rust
//! The `derive` action: build a catalog from a source catalog's entries.

use a2ui_runtime::{Catalog, DeriveArgs, DeriveEntry};
use serde_json::{Map, Value};

use crate::case::Case;
use crate::compare::{expect_error, schema_eq};
use crate::handlers::catalog_input::{check_entries, document};
use crate::keys::{assert_case_keys, assert_known_keys};

const EXPECT_KEYS: [&str; 7] = [
    "catalogId",
    "protocolVersion",
    "components",
    "functions",
    "theme",
    "instructions",
    "absentKeys",
];

/// Loads the source catalog, derives with `args`; checks `expect` or `expectError`.
pub fn run(case: &Case) -> Result<(), String> {
    assert_case_keys(
        case,
        &["catalog", "catalogPath", "args", "expect", "expectError"],
    )?;
    let at = case.at();
    let source =
        Catalog::from_json(&document(case)?, None, None).map_err(|err| format!("{at}: {err}"))?;
    let args = derive_args(case.get("args"), &at)?;
    match (
        source.derive(args),
        case.get("expectError"),
        case.get("expect"),
    ) {
        (_, Some(_), Some(_)) | (_, None, None) => Err(format!(
            "{at}: a derive case needs exactly one of `expect` and `expectError`"
        )),
        (Err(err), Some(expected), None) => expect_error(&err, expected, &at),
        (Err(err), None, Some(_)) => Err(format!("{at}: {err}")),
        (Ok(_), Some(_), None) => Err(format!("{at}: expected an error, derive succeeded")),
        (Ok(derived), None, Some(Value::Object(expect))) => check(&derived, expect, &at),
        (Ok(_), None, Some(other)) => Err(format!("{at}: `expect` must be a mapping, got {other}")),
    }
}

/// The case's `args` mapping as [`DeriveArgs`].
fn derive_args(args: Option<&Value>, at: &str) -> Result<DeriveArgs, String> {
    let Some(Value::Object(args)) = args else {
        return Err(format!("{at}: `args` must be a mapping"));
    };
    assert_known_keys(
        args,
        &["catalogId", "components", "functions", "instructions"],
        at,
    )?;
    let catalog_id = match args.get("catalogId") {
        Some(Value::String(id)) => id.clone(),
        _ => return Err(format!("{at}: `args.catalogId` must be a string")),
    };
    let instructions = match args.get("instructions") {
        None => None,
        Some(Value::String(text)) => Some(text.clone()),
        Some(other) => {
            return Err(format!(
                "{at}: `args.instructions` must be a string, got {other}"
            ));
        }
    };
    Ok(DeriveArgs {
        catalog_id,
        components: entries(args.get("components"), "components", at)?,
        functions: entries(args.get("functions"), "functions", at)?,
        instructions,
    })
}

/// A list of names (kept entries) and single-key mappings (new entries).
fn entries(list: Option<&Value>, kind: &str, at: &str) -> Result<Option<Vec<DeriveEntry>>, String> {
    let Some(list) = list else {
        return Ok(None);
    };
    let Value::Array(items) = list else {
        return Err(format!("{at}: `args.{kind}` must be a list, got {list}"));
    };
    items
        .iter()
        .map(|item| match item {
            Value::String(name) => Ok(DeriveEntry::Keep(name.clone())),
            Value::Object(entry) if entry.len() == 1 => {
                let (name, schema) = entry
                    .iter()
                    .next()
                    .ok_or_else(|| format!("{at}: empty entry in `args.{kind}`"))?;
                Ok(DeriveEntry::New(name.clone(), schema.clone()))
            }
            other => Err(format!(
                "{at}: an `args.{kind}` item must be a name or a one-key mapping, got {other}"
            )),
        })
        .collect::<Result<_, _>>()
        .map(Some)
}

fn check(derived: &Catalog, expect: &Map<String, Value>, at: &str) -> Result<(), String> {
    assert_known_keys(expect, &EXPECT_KEYS, at)?;
    let written = derived.to_json();
    for (key, expected) in expect {
        match key.as_str() {
            "catalogId" => check_text(key, Some(derived.id()), expected, at)?,
            "protocolVersion" => check_text(key, Some(derived.protocol_version()), expected, at)?,
            "instructions" => {
                check_text(key, written.get(key).and_then(Value::as_str), expected, at)?;
            }
            "components" | "functions" => {
                let actual: Vec<(&str, &Value)> = written
                    .get(key)
                    .and_then(Value::as_object)
                    .into_iter()
                    .flatten()
                    .map(|(name, schema)| (name.as_str(), schema))
                    .collect();
                check_entries(key, expected, &actual, at)?;
            }
            "theme" => match written.get("theme") {
                Some(theme) if schema_eq(theme, expected) => {}
                theme => return Err(format!("{at}: theme {theme:?}, expected {expected}")),
            },
            _ => check_absent(&written, expected, at)?,
        }
    }
    Ok(())
}

fn check_text(key: &str, actual: Option<&str>, expected: &Value, at: &str) -> Result<(), String> {
    match expected {
        Value::String(expected) if Some(expected.as_str()) == actual => Ok(()),
        Value::String(expected) => Err(format!("{at}: {key} {actual:?}, expected {expected:?}")),
        other => Err(format!("{at}: `{key}` must be a string, got {other}")),
    }
}

/// `absentKeys` lists top-level keys the derived document must not have.
fn check_absent(written: &Value, expected: &Value, at: &str) -> Result<(), String> {
    let Value::Array(keys) = expected else {
        return Err(format!("{at}: `absentKeys` must be a list, got {expected}"));
    };
    for key in keys {
        let key = key
            .as_str()
            .ok_or_else(|| format!("{at}: `absentKeys` must list strings, got {key}"))?;
        if written.get(key).is_some() {
            return Err(format!("{at}: the derived catalog still has `{key}`"));
        }
    }
    Ok(())
}
```

```diff
--- a/crates/a2ui-conformance/src/handlers/mod.rs
+++ b/crates/a2ui-conformance/src/handlers/mod.rs
@@ -4,6 +4,7 @@
 mod catalog_input;
 mod common_type;
 mod data_model;
+mod derive;
 mod expression;
 mod from_json;
 mod resolve_path;
@@ -29,6 +30,7 @@
         "validation_schema" => Some(validation_schema::run),
         "validate_common_type" => Some(common_type::run),
         "agent_to_renderer_schema" => Some(schema_export::run_agent_to_renderer),
+        "derive" => Some(derive::run),
         _ => None,
     }
 }
```

```diff
--- a/crates/a2ui-conformance/skips.toml
+++ b/crates/a2ui-conformance/skips.toml
@@ -1,6 +1,5 @@
 # Actions not implemented yet, each with the plan that implements it.
 [actions]
-derive = "Plan 2: catalogs and schemas"
 validate = "Plan 3: validation and message processing"
 process_messages = "Plan 3: validation and message processing"
 get_renderer_capabilities = "Plan 3: validation and message processing"
```

- [ ] **Step 6: Run the conformance suites**

Run: `cargo test -p a2ui-conformance --test core 2>&1 | tail -3`
Expected: `test result: ok. 215 passed; 0 failed; 630 ignored` (7 v1.0 `derive` cases; the v0.9 one is skipped by `[versions]`).

- [ ] **Step 7: Update `crates/a2ui-runtime/ARCHITECTURE.md`**

Append to the `catalog` bullet: `` `derive` builds a narrower catalog under a new id. ``

- [ ] **Step 8: Run the gate**

Run: `just check && just test`
Expected: success.

- [ ] **Step 9: Commit**

```bash
git add -A
git commit -m "feat(runtime): derive catalogs from a source catalog (t-1560)"
oku cli task done --project slider --id t-1560 --result "Catalog::derive with DeriveArgs/DeriveEntry; derive handler; 215 passed"
```

---

### Task 8: Consolidation and Plan 2 gate (t-1561)

Deferred: `Error::code()` (A2UI error codes) and component validation (`validComponents`/`invalidComponents`, 6 named skips) are Plan 3.

**Files:**
- Modify: any file the checks below flag; `crates/a2ui-runtime/src/lib.rs`, `crates/a2ui-runtime/ARCHITECTURE.md`, `crates/a2ui-conformance/ARCHITECTURE.md`, `README.md`

- [ ] **Step 1: Consolidation pass (G10)**

```bash
rg -n 'TODO|FIXME|unimplemented!|todo!|unwrap\(|expect\(' crates --glob '!*_tests.rs'
rg -n 'fn (is_identifier|json_eq|schema_eq|normalize_set_keywords|bare_version|read_json|reachable_defs|check_entries)\b' crates
rg -n 'case\.suite, case\.name' crates
wc -l crates/*/src/**/*.rs crates/*/src/*.rs crates/*/tests/*.rs | sort -n | tail -5
```

Expected: no matches in production code for the first command (`expect(` inside `#[expect(` attributes is fine); each helper defined once; no `suite#name` built by hand (all use `Case::at`); no file above 500 lines (the largest are about 256). Fix anything found.

Make the crate root list the new key items (G4):

```diff
--- a/crates/a2ui-runtime/src/lib.rs
+++ b/crates/a2ui-runtime/src/lib.rs
@@ -8,8 +8,9 @@
 //! (classified by [`Category`]), [`Watchers`] (observers a write notifies),
 //! [`resolve_path`] (scoped relative paths), [`parse_template`] (`${...}`
 //! expression templates), the embedded v1.0 schemas ([`common_types_schema`],
-//! [`agent_to_renderer_schema`]) and the typed [`common_types`] with
-//! [`validate_common_type`].
+//! [`agent_to_renderer_schema`]), the typed [`common_types`] with
+//! [`validate_common_type`], and [`Catalog`] (loaded with `from_json`, written
+//! back with `to_json`, turned into a `validation_schema`, narrowed with `derive`).
 
 mod catalog;
 pub mod common_types;
```

- [ ] **Step 2: Check the skip list is the expected one**

```bash
cargo test -p a2ui-conformance --test core 2>&1 | tail -3
awk '/^\[/{s=$0} /^[^#\[].*=/{c[s]++} END{for(k in c) print k, c[k]}' crates/a2ui-conformance/skips.toml
```

Expected: `test result: ok. 215 passed; 0 failed; 630 ignored`; `[actions] 13` (Plans 3–6 only), `[versions] 3`, `[cases] 7` (the 6 Plan 3 component-validation cases and `core/catalog::test_catalog_from_json_override_catalog_id`).

- [ ] **Step 3: Write the manifest once and verify it**

```bash
dir=$(mktemp -d) && just conformance-manifest "$dir" && wc -l "$dir"/*.jsonl
```

Expected: 844 lines (one record per case of `core/*.yaml`), every `skipped` record with a reason.

- [ ] **Step 4: Run the full gate**

Run: `just ci`
Expected: success (`licenses ok`; `cargo audit` reports only the accepted RUSTSEC-2024-0449 warning).

- [ ] **Step 5: Final docs and commit**

Make both `ARCHITECTURE.md` files match the code (each ≤ 50 lines: `wc -l crates/*/ARCHITECTURE.md`) and replace the `README.md` status line with: `Conformance: stages C1 (data model, paths, expression parsing) and C2 (catalogs, common types, schemas) pass; later stages are listed in crates/a2ui-conformance/skips.toml.`

```bash
git add -A
git commit -m "docs: record the Plan 2 conformance status (t-1561)"
oku cli task done --project slider --id t-1561 --result "Plan 2 gate green: just ci, 215 passed / 630 ignored, manifest complete"
oku cli task update --project slider --id t-1544 --description "Plan 2 (catalogs and schemas) done; next: Plan 3 validation and message processing"
```

Then hand back: report the commit list, the conformance counts (passed / skipped by reason), and the 7 named skips, so Plan 3 can be written against the real APIs.

