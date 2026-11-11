# a2ui-conformance

Runs `vendor/a2ui/conformance/core/*.yaml` against `a2ui-runtime`, one libtest-mimic trial per case
(`tests/core.rs`). Not published.

## Modules

- `case`: `Case` (suite, name, action, body), `parse_suite`, `load_suite`; the protocol version is
  the top-level `protocolVersion`, else `catalog.protocolVersion`.
- `skips`: `skips.toml` (`[actions]`, `[cases]`, each with a reason) plus the version filter
  (only `1.0`, except the version-neutral `data_model`, `resolve_path`, `parse_expression_template`).
  An entry matching no case fails `skips_toml_entries_all_match_a_case`.
- `keys`: strict key checks; an unread key fails the case.
- `compare`: `json_eq` (numbers as `f64`, key order ignored); `expect_error` (category, optional
  message regex).
- `manifest`: upstream's executed-case JSONL, written whenever `A2UI_CONFORMANCE_MANIFEST_DIR` is set
  (`just conformance-manifest` sets it).
- `paths`: `upstream_root`, the vendored `vendor/a2ui` found from the crate directory.
- `handlers`: `handler(action)`, one runner per implemented action.

## Rule

A failing case is fixed in `a2ui-runtime`, never by bending the harness.

Accepted advisory: RUSTSEC-2024-0449 (`unsafe-libyaml` unmaintained, via `serde_yaml_ng`) is a
warning only; this unpublished harness parses vendored YAML and it never reaches `a2ui-runtime`.
No audit ignore entry.
