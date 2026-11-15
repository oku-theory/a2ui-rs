# a2ui-fuzz

libFuzzer targets for a2ui-runtime's agent-facing entry points, in their own workspace so
`libfuzzer-sys` stays out of the published crates' lock, MSRV and minimal-versions builds. Not
published. Built and run by `just fuzz <target> [seconds]` on the pinned toolchain, without
cargo-fuzz or AddressSanitizer (t-1660); `just fuzz-check` lints it and tests `src/lib.rs`.

## Layout

- `src/lib.rs`: `fields` splits an input into UTF-8 parts at NUL bytes (JSON text holds none
  raw), small parts first and the catalog last; `outcome` makes results comparable (code,
  category, message); `basic` loads the basic catalog once.
- `fuzz_targets/`: `catalog_parse` (overrides, then text; `to_json` reloads to the same
  catalog), `validate_component` (component, then a default id that may be unknown),
  `parse_then_validate` (component, [id,] then an agent catalog parsed twice: same JSON, same
  outcome in its `Catalog` lookup and in the basic one's `[Catalog]`), `template_parse`,
  `pointer_parse` (no forbidden or empty segment; written pointers parse back).
- `a2ui.dict`: JSON and A2UI tokens for libFuzzer's `-dict=`.
- `fuzz-only.lock`: pins for what only this workspace uses; `Cargo.lock` is generated from the
  root lock (plan 2b, Decision 45). `corpus/` and `artifacts/` are local, ignored by git.

## Adding a target

Add the `[[bin]]` in `Cargo.toml`, its limits and seeds in `scripts/fuzz_targets.py` and its name
in `.github/workflows/fuzz.yml`'s matrix (the script's tests check all three). A crash
becomes a regression test in the crate it fails (`tests/regressions/issue_NNNN_*.rs`) first.

## Dependencies

`a2ui-runtime`, `a2ui-catalog-basic` (by path), `libfuzzer-sys`, `serde_json`; `rstest` and
`speculoos` in tests.
