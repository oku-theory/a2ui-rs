# a2ui-conformance

Runs `vendor/a2ui/conformance/core/*.yaml` against `a2ui-runtime`, one libtest-mimic trial per case
(`tests/core.rs`). Not published.

## Modules

- `case`: `Case` (suite, name, action, body), `parse_suite`, `load_suite`; `effective_version`
  classifies the version, first match wins: top-level or `catalog.protocolVersion`, first
  message's `version` (`messages`, `payload`, first step), `args.version`, first catalog
  (`catalogs`, `catalogPaths`, `catalogPath` file or inline `catalog`; none declared = `0.9`), a
  v0.8 envelope key, suite suffix `_v1_0`; no leading `v`. A version-dependent action left
  unclassified is `VersionError::Unclassified`, only the version-neutral ones give `None`; a
  read or type error is `VersionError::Failed`. `Skips::reason_for` skips an unclassified case
  named in `[cases]`/`[actions]`, fails every other error; `at()` is `suite#name`.
- `skips`: `skips.toml` `[actions]`, `[cases]`, `[versions]` (each with a reason; the
  version-neutral `data_model`, `resolve_path`, `parse_expression_template` ignore `[versions]`).
  An entry matching no case fails `skips_toml_entries_all_match_a_case`.
- `keys`: strict key checks; an unread key fails the case.
- `compare`: `json_eq`, `schema_eq` (`enum`/`required` as multisets), `expect_error`: a string
  matches the message only; a mapping takes `category` (`ValidationError` also accepts
  `IntegrityError`/`RecursionError`), `message` (case-insensitive literal, then regex), `code`,
  `path`, `surfaceId`, `functionCallId`, `details` (by index: `path`, `code`, `surfaceId`),
  `rendererError`; any other key fails.
- `outbound`: `check_renderer_error` validates `RendererMessage::error(..)` against upstream's
  `renderer_to_agent.json` (`common_types.json` under its `$id`, a stand-in `catalog.json` for
  its `anyFunction` ref), matching `version` and `error.{code,surfaceId,functionCallId,path,
  message}`; a missing error body or message fails.
- `manifest`: upstream's executed-case JSONL, and aliased ids per case in
  `rust-core-<pid>.aliased.txt` (`.txt`: outside the comparer's `*.jsonl` glob), written whenever
  `A2UI_CONFORMANCE_MANIFEST_DIR` is set (`just conformance-manifest` sets it).
- `paths`: `upstream_root` (the vendored `vendor/a2ui`), `read_json`, `expect_file`.
- `handlers`: `handler(action)`, one runner per implemented action (schema_doc: the two
  schema-equality actions; common_type; round_trip; from_json; to_json; derive;
  validation_schema (golden comparison with `schema_eq`); catalog_checks: entries,
  `selfContained`, `matchesBuiltinCatalog` against `a2ui-catalog-basic`);
  `catalog_input` reads the case's catalog (inline or `catalogPath`) and its
  `catalogId`/`protocolVersion` overrides; `processing` runs `process_messages` and `validate`
  (acceptance alone may be its check) steps on one `MessageProcessor`; `catalogs` registers v1.0
  catalogs (an unversioned one takes the case's version; stubs opened, missing ids aliased);
  `surfaces` checks `expect.surfaces`; `reports`: capabilities (lists sorted), data model.

Depends on: `a2ui-runtime`, `a2ui-catalog-basic`, `jsonschema`, `percent-encoding`.

## Rule
A failing case is fixed in `a2ui-runtime`, never by bending the harness.

Accepted advisory: RUSTSEC-2024-0449 (`unsafe-libyaml` unmaintained, via `serde_yaml_ng`) is a
warning only; this unpublished harness parses vendored YAML and it never reaches `a2ui-runtime`.
No audit ignore entry.
