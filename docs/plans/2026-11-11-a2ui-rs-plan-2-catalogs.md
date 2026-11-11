# a2ui-rs Plan 2: Catalogs and Schemas (C2) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Parent task:** Oku t-1544 (sub-tasks t-1554 to t-1561, one per task below; Plan 2b is t-1564).

**Goal:** Load, check, validate against, derive and serialize v1.0 catalogs, and expose the v1.0
common types and agent-to-renderer schemas, so every v1.0 case of `core/catalog.yaml`,
`core/common_types.yaml` and `core/agent_to_renderer.yaml` passes except the two
`validation_schema` cases (Plan 2b).

**Architecture:** The runtime treats the published v1.0 JSON Schemas as its models: it embeds
`common_types.json` and `agent_to_renderer.json` and validates with the `jsonschema` crate on the
linear-time `regex` engine, offline (no `$ref` is ever fetched). `Catalog::from_json` bounds and
checks a catalog document, keeps the authored document for `to_json`, bundles a self-contained
schema (catalog plus the common types it reaches, every `$ref` local and percent-encoded), and
compiles one validator per component and per function from it. A new `a2ui-catalog-basic` crate
embeds the published basic catalog.

**Tech Stack:** Rust 2024 (MSRV 1.88), `serde_json`, `snafu`, `unicode-ident`, `jsonschema` 0.61.1
(default features off), `percent-encoding`, `libtest-mimic`, `speculoos`, `rstest`.

**Spec:** `docs/specs/2026-11-10-a2ui-rs-leptos-design.md` (Task 1 amends it). Read it and
`AGENTS.md` before starting. Upstream rules: `vendor/a2ui/conformance/README.md` (catalog.yaml
paragraph) and `vendor/a2ui/conformance/conformance_harness_implementation_guide.md` (stage C2).

## Global Constraints

- Repository: `/Users/matthieu/Projects/a2ui-rs`, branch `main`. Commands run from its root.
- Every commit: Conventional Commits in English, `type(scope): description (t-N)`, the task's
  Oku ID from the task header. Never add a `Co-Authored-By` line. Stage with `git add <files>` in
  a separate command, then commit with `git commit -F <message file>`. A PreToolUse hook asks for
  a `Reviewed-by:` trailer naming the task review (Plan 1 ruling): add it truthfully.
- No `unwrap`/`expect` outside tests (clippy denies them), no `#[allow]` (use
  `#[expect(..., reason = "...")]`), no dead code, no silent fallback hiding a bug.
- Every `pub` item and module is documented; first sentence under 15 words.
- Errors: the one opaque `a2ui_runtime::Error` (Snafu), a `backtrace: Backtrace` in every variant,
  constructed as `XxxSnafu { .. }.build().into()` or with `ensure!`/`OptionExt::context`; a
  context whose fields need `format!` uses `with_context(|| ...)` so the success path does not
  allocate. `Error` stays at most 128 bytes (`clippy::result_large_err`).
- Unit tests in a sibling `x_tests.rs` included with `#[cfg(test)] #[path = "x_tests.rs"] mod tests;`,
  speculoos assertions, names `unit_scenario_result`, one behavior per test, rstest for tables;
  a table of failures carries a message-fragment column so a case cannot pass for another reason.
- Source files target 500 lines, never above 1000. `ARCHITECTURE.md` files stay at most 50 lines.
- `a2ui-runtime` has no direct Leptos, wasm-bindgen, async-runtime or I/O dependency, and builds
  for `wasm32-unknown-unknown` (`just wasm`); no result depends on randomness.
- Dependencies are declared in `[workspace.dependencies]` and used with `.workspace = true`.
- Run `cargo fmt --all` before each commit, so `just check` (`fmt --check`) passes and lines
  stay within 100 columns.
- A failing conformance case is fixed in `a2ui-runtime`, never by bending the harness; skips go
  through `crates/a2ui-conformance/skips.toml` with a reason.
- `vendor/a2ui` changes only through `vendor/sync.sh`.
- Gate per task: `just check` and `just test` green before the commit. Long runs: `just test`
  builds cold the first time; run it in the background with a log if it exceeds two minutes.

## Decisions (approved by the user, 2026-11-11)

- JSON Schema validation uses `jsonschema` pinned exactly to 0.61.1 with default features off
  (no HTTP or file resolution, hence no `idna` dependency), `PatternOptions::regex()` with 64 KiB
  `size_limit`/`dfa_size_limit`, and `.offline()` (the crate's own retriever that refuses every
  fetch) on every build. The `format` keyword stays an annotation (format assertion off); no C2
  case needs it.
- One `build` per catalog entry, not `ValidationOptions::build_map`: `build_map` compiles every
  object node independently and silently drops the nodes whose compilation fails, so a broken
  entry would just be missing; it is also slower (basic catalog: 51 ms against 21 ms).
- The `validation_schema` golden shape (upstream's flattening, about 900 lines of Python) is a
  separate Plan 2b (Oku t-1564). Plan 2's `Catalog::validation_schema` returns the unflattened
  bundle; the `validation_schema` action stays skipped with reason "Plan 2b".
- `a2ui-catalog-basic` is created now with the catalog document only; its 14 functions arrive in
  Plan 4 as the spec says.
- Catalogs have no allowed top-level key list: upstream accepts `theme`, catalog-level `metadata`
  and vendor keys (Task 1 amends spec §2 accordingly).
- Rejecting a component's call of an `agentOnly` function is C3 behaviour (`payload_validator.py`)
  pulled forward into `validate_component` by choice; no C2 case strictly needs it.
- The call envelope of a catalog's `anyFunction` stays separate from `any_function_envelope`
  (the no-catalog one): the common types form a cycle `FunctionCall` -> `IndexSystemFunction`
  -> `DynamicNumber` -> `FunctionCall`, so one shared envelope cannot serve both. The review
  suggestion to unify them is declined.
- `Error::code()` (stable A2UI codes) is deferred to Plan 3 (t-1565): Plan 2 reports categories
  only, as Plan 1 did, since codes need the message processor that reports them.
- `schema.rs` stays one module: it is under 200 lines even with the common types and the
  metadata helpers, so splitting it would add files without a concept to name.
- The harness writes protocol versions without the leading `v` (`0.9`, as catalogs spell it);
  the runtime's default for a catalog without `protocolVersion` is `0.9`
  (`catalog_definition.json`), which it refuses, and the harness skips such cases through
  `skips.toml` `[versions]` (Plan 1 final review item M1: no hardcoded version filter).

## Verified behaviours (jsonschema 0.61.1 probes and upstream at 3295e67b)

- Builds for `wasm32-unknown-unknown`; supports `unevaluatedProperties` and `\p{XID_Start}`;
  refuses look-around patterns at build time; with `.offline()` any `$ref` to another document
  fails the build, even one no root reaches, without fetching it.
- A `$ref` fragment must be a valid URI: `#/components/Component_ℓ` fails to build,
  `#/components/Component_%E2%84%93` works. Every `$ref` the runtime writes is percent-encoded
  (`bundle::local_ref`), and its own resolvers decode (`bundle::resolve`).
- A nested `$id` rebases relative refs and a nested absolute `$id` or `$schema` compiles silently;
  `$anchor` and `$dynamicAnchor` make `#Name` refs that bypass the expansion count, and
  `$dynamicRef`/`$recursiveRef` resolve dynamically. The runtime rejects nested `$id`/`$schema`,
  `$anchor`, `$dynamicAnchor`, `$dynamicRef`, `$recursiveRef` and `$recursiveAnchor` itself, with
  a clear error, and accepts only `$ref` fragments that are empty or start with `/`.
- A same-instance `$ref` cycle (X→Y→X through `$ref`/`allOf`/`anyOf`) builds and validates
  without overflowing, but with fan-out (two branches per level) validation of a 64-deep
  instance is exponential. The runtime therefore rejects every `$ref` cycle in a catalog entry
  (`Cycle`), and its own walkers stay cycle-safe.
- A schema nested 5000 levels overflows the stack at build: documents are capped at 64 levels,
  and the depth reached through followed `$ref`s at `MAX_SCHEMA_DEPTH` (128).
- Pattern cost is bounded by the 64 KiB compile cap per pattern and `MAX_CATALOG_PATTERNS` (256
  `pattern` values and `patternProperties` keys per catalog); a catalog document is capped at
  `MAX_CATALOG_NODES` (200 000 JSON nodes). The published basic catalog has 1011 JSON nodes, a
  `$ref`-expanded depth of 23, no `$ref` cycle, no anchor and 2 patterns, so every cap sits far
  above real data.
- An `allOf` fan-out of two `$ref`s per level with `unevaluatedProperties` compiles in
  exponential time (27 ms at 10 levels, 330 ms at 12, 290 s at 16), and validation visits every
  branch: each entry is capped at 16 384 schema nodes once its `$ref`s are expanded, a catalog at
  262 144 (the published basic catalog's largest entry is 4 493).
- `jsonschema` enables `getrandom`'s `wasm_js` backend on wasm32, so `wasm-bindgen` is a
  transitive dependency of `a2ui-runtime` there (through `getrandom` only, to seed `ahash`); no
  result depends on it. `just deps` checks it stays the only path. Plan 6 must run a validator in
  a browser (backlog task from this plan).
- `cargo deny` needs `MIT-0` (`fluent-uri`) and `Zlib` (a `hashbrown` dependency) allowed.
- Upstream's catalog default protocol version is `0.9`; a catalog case that declares no version
  anywhere is a v0.9 case (`test_catalog_from_json_missing_catalog_id_error`,
  `test_catalog_from_json_override_catalog_id`).
- Upstream's reference harness matches `expectError.message` as a case-insensitive literal
  substring or a regex (`_exception_matches_expected_message`); `"Unsupported external $ref"`
  only matches literally.
- `selfContained` is checked on the validation schema: at least one `$ref`, every `$ref` starts
  with `#` and resolves in that document.
- `matchesBuiltinCatalog` compares id, protocol version, per-component property and required
  names (through `allOf` and local `$ref`, ignoring `id` and `component`), and per-function
  `returnType`, `allowedCallers`, `requiresUserActivation`. Its only case compares the published
  basic catalog with itself (`a2ui-catalog-basic` embeds the same file), so it is tautological
  here; the real check, a catalog built by code against the published one, belongs to Plan 4.
- A nested call runs in the catalog it names, or in the surface default catalog when it names
  none; validating a component against one catalog fully checks only the calls that run there
  and checks only the identifiers of the others (`payload_validator.py`,
  `_validate_nested_functions`).
- Effective versions of the non-version-neutral cases: `0.9` (72), `0.8` (37), `0.9.1` (12);
  the v1.0 C2 cases are from_json 32, to_json 1, round_trip 5, validation_schema 2, derive 7,
  common_types_schema 1, validate_common_type 11, agent_to_renderer_schema 1 (60).

## File Structure

```
Cargo.toml                                   # + jsonschema 0.61.1, percent-encoding (T2), a2ui-catalog-basic (T6)
deny.toml                                    # allow: MIT, MIT-0, Apache-2.0, Unicode-3.0, Zlib (T2)
justfile                                     # deps recipe in check (T2); wasm builds a2ui-catalog-basic (T6)
AGENTS.md                                    # runtime dependency invariant reworded (T2)
vendor/sync.sh                               # + mcp/v1, vegachart/v1, NOTICE, embedded copies (T1);
                                             # basic catalog copy (T6)
docs/specs/2026-11-10-a2ui-rs-leptos-design.md   # amended (T1)
crates/a2ui-runtime/
  schemas/v1_0/{common_types.json,agent_to_renderer.json,LICENSE}   # copied by sync.sh (T1)
  src/schema.rs (+ _tests)        # embedded documents (T1), common types (T2), metadata (T4),
                                  # stays one file (under 200 lines)
  src/ident.rs (+ _tests)         # is_identifier, moved from expression/parser.rs (T2)
  src/limits.rs (+ _tests)        # depth, pattern cap (T2); catalog caps, `expand` (T4)
  src/validator.rs (+ _tests)     # jsonschema compile/check (T2)
  src/bundle.rs (+ _tests)        # $ref rules and encoding (T2), reference checks, closure (T4)
  src/catalog/mod.rs              # Catalog, ComponentDef, FunctionDef, AllowedCallers (T4)
  src/catalog/load.rs (+ _tests)  # from_json, parse, to_json (T4)
  src/catalog/entries.rs (+ _tests)   # component and function definitions (T4)
  src/catalog/compile.rs (+ _tests)   # bundling, size checks, validators (T4)
  src/catalog/document.rs (+ _tests)  # argument names, return type, unions (T4)
  src/catalog/shape.rs (+ _tests) # ComponentShape (T4)
  src/catalog/test_support.rs     # cfg(test) catalog builders shared by the catalog tests (T4)
  src/catalog/validate.rs (+ _tests)  # validate_component, nested calls (T5)
  src/catalog/derive.rs (+ _tests)    # Derivation, DeriveEntry, derive (T7)
crates/a2ui-catalog-basic/        # new crate (T6)
  Cargo.toml, ARCHITECTURE.md, catalog.json, LICENSE-A2UI, src/lib.rs, src/lib_tests.rs
crates/a2ui-conformance/
  src/paths.rs                    # + read_json (T1)
  src/case.rs, src/skips.rs, tests/core.rs, skips.toml   # effective version, [versions] (T3)
  src/compare.rs                  # + schema_eq, substring-or-regex message match (T3)
  src/handlers/schema_doc.rs      # common_types_schema, agent_to_renderer_schema (T1)
  src/handlers/common_type.rs     # validate_common_type (T2)
  src/handlers/catalog_input.rs   # the case's catalog document and overrides (T4)
  src/handlers/round_trip.rs      # (T4)
  src/handlers/catalog_checks.rs (+ _tests)  # entries, selfContained, matchesBuiltinCatalog (T6)
  src/handlers/from_json.rs, src/handlers/to_json.rs  # (T6)
  src/handlers/derive.rs          # (T7)
```

---

### Task 1: Vendored catalogs and embedded schema documents

Oku: t-1554.

**Files:**
- Modify: `vendor/sync.sh`, `docs/specs/2026-11-10-a2ui-rs-leptos-design.md`
- Create (by sync.sh): `vendor/a2ui/catalogs/mcp/v1/`, `vendor/a2ui/catalogs/vegachart/v1/`,
  `crates/a2ui-runtime/schemas/v1_0/{common_types.json,agent_to_renderer.json,LICENSE}` (plus
  `NOTICE-A2UI` when upstream has a `NOTICE`)
- Create: `crates/a2ui-runtime/src/schema.rs`, `crates/a2ui-runtime/src/schema_tests.rs`,
  `crates/a2ui-conformance/src/handlers/schema_doc.rs`
- Modify: `crates/a2ui-runtime/src/{lib.rs,error.rs}`, `crates/a2ui-runtime/ARCHITECTURE.md`,
  `crates/a2ui-conformance/src/{lib.rs,paths.rs,case.rs,case_tests.rs,keys.rs,handlers/mod.rs,handlers/expression.rs,handlers/resolve_path.rs}`,
  `crates/a2ui-conformance/tests/core.rs`,
  `crates/a2ui-conformance/skips.toml`, `crates/a2ui-conformance/ARCHITECTURE.md`

**Interfaces:**
- Produces: `a2ui_runtime::common_types_schema() -> Result<&'static Value, Error>`,
  `a2ui_runtime::agent_to_renderer_schema() -> Result<&'static Value, Error>`,
  `Category::Catalog` (`"CatalogError"`),
  `a2ui_conformance::read_json(path: &Path) -> Result<Value, String>`,
  `Case::at(&self) -> String` (`"<suite>#<name>"`, Plan 1 final review item M3).

- [ ] **Step 1: Extend `vendor/sync.sh`**

Replace the whole script with:

```bash
#!/usr/bin/env bash
# Copies upstream A2UI v1.0 specification, catalogs and conformance suites at a pinned commit,
# plus the documents the crates embed (a crate cannot include files outside its directory).
set -euo pipefail
commit="${1:?usage: vendor/sync.sh <upstream-commit>}"
root="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$root/.." && pwd)"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
git clone --quiet --filter=blob:none --no-checkout https://github.com/a2ui-project/a2ui.git "$work/a2ui"
git -C "$work/a2ui" sparse-checkout set --no-cone \
  /LICENSE /NOTICE /specification/v1_0 \
  /catalogs/basic/v1 /catalogs/mcp/v1 /catalogs/vegachart/v1 \
  /conformance/core /conformance/test_data /conformance/README.md \
  /conformance/conformance_schema.json /conformance/executed_case_manifest.schema.json \
  /conformance/manifest_corpora.json /conformance/conformance_harness_implementation_guide.md
git -C "$work/a2ui" checkout --quiet "$commit"
rm -rf "$root/a2ui"
mkdir -p "$root/a2ui/specification" "$root/a2ui/catalogs/basic" "$root/a2ui/catalogs/mcp" \
  "$root/a2ui/catalogs/vegachart"
cp "$work/a2ui/LICENSE" "$root/a2ui/LICENSE"
if [[ -f "$work/a2ui/NOTICE" ]]; then
  cp "$work/a2ui/NOTICE" "$root/a2ui/NOTICE"
fi
cp -R "$work/a2ui/specification/v1_0" "$root/a2ui/specification/v1_0"
cp -R "$work/a2ui/catalogs/basic/v1" "$root/a2ui/catalogs/basic/v1"
cp -R "$work/a2ui/catalogs/mcp/v1" "$root/a2ui/catalogs/mcp/v1"
cp -R "$work/a2ui/catalogs/vegachart/v1" "$root/a2ui/catalogs/vegachart/v1"
cp -R "$work/a2ui/conformance" "$root/a2ui/conformance"
schemas="$repo/crates/a2ui-runtime/schemas/v1_0"
mkdir -p "$schemas"
cp "$work/a2ui/specification/v1_0/json/common_types.json" \
  "$work/a2ui/specification/v1_0/json/agent_to_renderer.json" "$schemas/"
cp "$work/a2ui/LICENSE" "$schemas/LICENSE"
rm -f "$schemas/NOTICE-A2UI"
if [[ -f "$work/a2ui/NOTICE" ]]; then
  cp "$work/a2ui/NOTICE" "$schemas/NOTICE-A2UI"
fi
printf '%s\n' "$commit" > "$root/UPSTREAM"
```

- [ ] **Step 2: Re-vendor at the same commit**

Run: `just vendor-sync 3295e67b5f4820639aa88c578a64af2196d0b752` (needs network to github.com).
Then: `git status --short vendor crates/a2ui-runtime/schemas`
Expected: only new paths (`vendor/a2ui/catalogs/mcp/`, `vendor/a2ui/catalogs/vegachart/`,
`vendor/a2ui/NOTICE` when upstream has one, `crates/a2ui-runtime/schemas/`);
`git diff --stat vendor/a2ui/conformance vendor/a2ui/specification` prints nothing.

- [ ] **Step 3: Amend the spec**

In `docs/specs/2026-11-10-a2ui-rs-leptos-design.md`:

(a) Replace the two lines of the `catalog` bullet of §2 (starting ``- `catalog`: catalog loading and
validation (allowed top-level keys``) with:

```markdown
- `catalog`: catalog loading and validation (no allowed top-level key list, since upstream
  accepts `theme`, catalog-level `metadata` and vendor keys; restricted `$ref` targets never
  fetched; reserved `Surface` name; size and depth caps; parent/child rules); a surface's
  `CatalogSet`.
```

(b) Replace the paragraph starting `Dependencies: \`serde\`, \`serde_json\`, \`snafu\`, \`regex\`, \`time\`.`
(two sentences) with:

```markdown
Dependencies: `serde`, `serde_json`, `snafu`, `regex`, `time`, `unicode-ident`,
`percent-encoding`, and `jsonschema` (default features off: no HTTP or file `$ref` resolution;
patterns on the linear `regex` engine; builds offline). The published v1.0 schemas are the
runtime's models: it embeds `common_types.json` and `agent_to_renderer.json` and validates
against them. No `tokio`, no randomness in any result; the clock is injected (action
timestamps). On wasm32, `jsonschema` brings `wasm-bindgen` through `getrandom`, which seeds its
hash maps; that is the only path (`just deps` checks it) and no outcome depends on it.
```

(c) In §4, replace `the only skips left are cases declaring another
  protocol version` with `the only skips left are cases declaring another
  protocol version or whose catalog declares none (upstream's default is v0.9)` (keep the
  parenthesis that follows).

(d) Replace the bullet starting `- Delivery: six plans` with:

```markdown
- Delivery: seven plans following the upstream harness stages (C1 data model and expressions; C2
  catalogs and schemas, with `a2ui-catalog-basic` holding the catalog document; C2b the
  `validationSchema` flattening matching upstream's goldens; C3 validation and processing; C4
  functions/actions/RPC, adding the basic functions to `a2ui-catalog-basic`; C5 node resolution
  and timing; then `a2ui-leptos`); each plan is written when the previous lands.
```

(e) In §5, replace ``- `a2ui-runtime` stays free of Leptos, wasm-bindgen, async runtimes and I/O.``
with ``- `a2ui-runtime` has no direct Leptos, wasm-bindgen, async-runtime or I/O dependency
  (`wasm-bindgen` reaches it only through `jsonschema`'s `getrandom`, see §2).``

(f) In §1, after the sentence ending `recorded in \`vendor/UPSTREAM\`.`, add: "The directories
copied also include `catalogs/mcp/v1` and `catalogs/vegachart/v1`, which the catalog suite
names. `vendor/sync.sh` further copies upstream's `LICENSE` (and its `NOTICE`, as `NOTICE-A2UI`,
when it exists) beside the schemas the runtime embeds, since a crate cannot include files outside
its directory."

(g) In §2, replace the `regex_guard` bullet (``- `regex_guard`: static analyzer for the ECMAScript
subset (256/4096 code-point caps, no nested or overlapping quantifiers, no backreferences or
lookbehind) in front of the `regex` crate.``) with:

```markdown
- `validator`: catalog `pattern`s compile with `jsonschema`'s `regex` engine (linear time, 64 KiB
  size and DFA limits, look-around refused, a per-catalog pattern cap); the runtime never runs a
  backtracking regex.
```

- [ ] **Step 4: Write the failing runtime test**

Create `crates/a2ui-runtime/src/schema_tests.rs`:

```rust
use speculoos::prelude::*;

use super::*;

#[test]
fn common_types_schema_embedded_document_is_v1_0() {
    let schema = common_types_schema().unwrap();
    assert_that!(schema["$id"].as_str())
        .is_equal_to(Some("https://a2ui.org/specification/v1_0/common_types.json"));
}

#[test]
fn agent_to_renderer_schema_embedded_document_is_v1_0() {
    let schema = agent_to_renderer_schema().unwrap();
    assert_that!(schema["$id"].as_str())
        .is_equal_to(Some("https://a2ui.org/specification/v1_0/agent_to_renderer.json"));
}
```

- [ ] **Step 5: Run it to see it fail**

Run: `cargo nextest run -p a2ui-runtime schema`
Expected: fails to compile: unresolved items.

- [ ] **Step 6: Add the `Catalog` category and the embedded-schema error**

In `crates/a2ui-runtime/src/error.rs`, add to `Category` (after `Expression`):

```rust
    /// A catalog that cannot be loaded, or an embedded schema that is broken.
    Catalog,
```

and to `Category::as_str`: `Self::Catalog => "CatalogError",`.

Add to `Inner`:

```rust
    #[snafu(display("Embedded schema {name} is broken: {message}"))]
    EmbeddedSchema {
        name: &'static str,
        message: String,
        backtrace: Backtrace,
    },
```

and to `Inner::category`: `Self::EmbeddedSchema { .. } => Category::Catalog,` (its own arm;
later tasks add their catalog variants to it).

- [ ] **Step 7: Write `schema.rs`**

Create `crates/a2ui-runtime/src/schema.rs`:

```rust
//! The v1.0 specification schemas the runtime embeds and validates with.
//!
//! The runtime's models are the published schemas themselves, so the schema it generates from
//! its models is the embedded document. `vendor/sync.sh` refreshes the copies in `schemas/`.

use std::sync::LazyLock;

use serde_json::Value;

use crate::error::{EmbeddedSchemaSnafu, Error};

const COMMON_TYPES: &str = include_str!("../schemas/v1_0/common_types.json");
const AGENT_TO_RENDERER: &str = include_str!("../schemas/v1_0/agent_to_renderer.json");

static COMMON_TYPES_DOC: LazyLock<Result<Value, String>> = LazyLock::new(|| parse(COMMON_TYPES));
static AGENT_TO_RENDERER_DOC: LazyLock<Result<Value, String>> =
    LazyLock::new(|| parse(AGENT_TO_RENDERER));

fn parse(text: &str) -> Result<Value, String> {
    serde_json::from_str(text).map_err(|err| err.to_string())
}

fn embedded(
    name: &'static str,
    document: &'static Result<Value, String>,
) -> Result<&'static Value, Error> {
    document.as_ref().map_err(|message| {
        EmbeddedSchemaSnafu {
            name,
            message: message.clone(),
        }
        .build()
        .into()
    })
}

/// The v1.0 `common_types.json` schema, as the runtime validates with it.
pub fn common_types_schema() -> Result<&'static Value, Error> {
    embedded("common_types.json", &COMMON_TYPES_DOC)
}

/// The v1.0 `agent_to_renderer.json` message schema.
pub fn agent_to_renderer_schema() -> Result<&'static Value, Error> {
    embedded("agent_to_renderer.json", &AGENT_TO_RENDERER_DOC)
}

#[cfg(test)]
#[path = "schema_tests.rs"]
mod tests;
```

(`&COMMON_TYPES_DOC` derefs the `LazyLock` to `&'static Result<..>`, forcing it.)

In `crates/a2ui-runtime/src/lib.rs` add `mod schema;` and
`pub use schema::{agent_to_renderer_schema, common_types_schema};`, and append to the crate doc's
key items: ``[`common_types_schema`] and [`agent_to_renderer_schema`] (the embedded v1.0
schemas)``.

- [ ] **Step 8: Run the runtime tests**

Run: `cargo nextest run -p a2ui-runtime schema`
Expected: 2 passed.

- [ ] **Step 9: Add `read_json` and `Case::at` to the harness**

In `crates/a2ui-conformance/src/paths.rs` add:

```rust
/// Reads and parses the JSON file at `path`.
pub fn read_json(path: &Path) -> Result<serde_json::Value, String> {
    let text =
        std::fs::read_to_string(path).map_err(|err| format!("{}: {err}", path.display()))?;
    serde_json::from_str(&text).map_err(|err| format!("{}: {err}", path.display()))
}
```

and in `lib.rs` change the re-export to `pub use paths::{read_json, upstream_root};`.

In `case.rs`, add to `impl Case`:

```rust
    /// The case's location for messages, such as `core/catalog.yaml#test_x`.
    pub fn at(&self) -> String {
        format!("{}#{}", self.suite, self.name)
    }
```

and append to `case_tests.rs`:

```rust
#[test]
fn at_parsed_case_joins_suite_and_name() {
    let cases = parse_suite("core/x.yaml", SUITE).unwrap();
    assert_that!(cases[0].at()).is_equal_to("core/x.yaml#test_top_level_version".to_owned());
}
```

Replace `&format!("{}#{}", case.suite, case.name)` in `keys.rs` with `&case.at()`, and
`let at = format!("{}#{}", case.suite, case.name);` in `handlers/expression.rs` and
`handlers/resolve_path.rs` with `let at = case.at();`. In `tests/core.rs`, the
`Unknown action` message becomes `format!("Unknown action '{}' in {}", case.action, case.at())`.

- [ ] **Step 10: Write the two schema handlers**

Create `crates/a2ui-conformance/src/handlers/schema_doc.rs`:

```rust
//! The `common_types_schema` and `agent_to_renderer_schema` actions.
//!
//! The runtime's models are the embedded published schemas, so the generated schema must equal
//! the specification document exactly (compared as parsed JSON: key order is ignored, array
//! order is not).

use serde_json::Value;

use crate::case::Case;
use crate::compare::json_eq;
use crate::keys::assert_case_keys;
use crate::paths::{read_json, upstream_root};

/// Compares the runtime's common types schema with `expectFile`.
pub fn run_common_types(case: &Case) -> Result<(), String> {
    compare(case, a2ui_runtime::common_types_schema())
}

/// Compares the runtime's agent-to-renderer schema with `expectFile`.
pub fn run_agent_to_renderer(case: &Case) -> Result<(), String> {
    compare(case, a2ui_runtime::agent_to_renderer_schema())
}

fn compare(
    case: &Case,
    generated: Result<&Value, a2ui_runtime::Error>,
) -> Result<(), String> {
    assert_case_keys(case, &["protocolVersion", "expectFile"])?;
    let at = case.at();
    let generated = generated.map_err(|err| format!("{at}: {err}"))?;
    let path = case
        .get("expectFile")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{at}: `expectFile` must be a string"))?;
    let expected = read_json(&upstream_root()?.join(path))?;
    if json_eq(generated, &expected) {
        Ok(())
    } else {
        Err(format!("{at}: the runtime's schema differs from {path}"))
    }
}
```

In `handlers/mod.rs` add `mod schema_doc;` and the match arms:

```rust
        "common_types_schema" => Some(schema_doc::run_common_types),
        "agent_to_renderer_schema" => Some(schema_doc::run_agent_to_renderer),
```

Remove the `common_types_schema` and `agent_to_renderer_schema` lines from `skips.toml`.

- [ ] **Step 11: Run the conformance cases**

Run: `cargo nextest run -p a2ui-conformance --test core schema_matches_specification`
Expected: `core/common_types::test_v10_common_types_schema_matches_specification` and
`core/agent_to_renderer::test_v10_agent_to_renderer_schema_matches_specification` pass; the
v0.8/v0.9/v0.9.1 ones are reported ignored (protocol version).

- [ ] **Step 12: Update the architecture notes**

In `crates/a2ui-runtime/ARCHITECTURE.md`, add under Modules:

```markdown
- `schema`: the embedded v1.0 `common_types.json` and `agent_to_renderer.json` (copied into
  `schemas/v1_0/` by `vendor/sync.sh`, Apache-2.0 `LICENSE` beside them), returned as is.
```

In `crates/a2ui-conformance/ARCHITECTURE.md`, extend the `paths` line with `; read_json`, the
`case` line with ``; `at()` is the `suite#name` location``, and the `handlers` line with
`(schema_doc: the two schema-equality actions)`.

- [ ] **Step 13: Gate and commit**

Run: `just check` then `just test`. Expected: green.

```bash
git add vendor crates/a2ui-runtime crates/a2ui-conformance docs/specs/2026-11-10-a2ui-rs-leptos-design.md
```

Message file content:

```
feat(runtime): embed the v1.0 common types and agent_to_renderer schemas (t-1554)

vendor/sync.sh also vendors the v1 MCP and VegaChart catalogs the catalog suite names and copies
the documents the runtime embeds into crates/a2ui-runtime/schemas. The spec records the
jsonschema dependency, the catalog rules and the Plan 2b split.
```

Then `oku cli task done --project slider --id t-1554 --result "<one line: what landed>"`.

---
### Task 2: Schema validation and common types

Oku: t-1555.

**Files:**
- Modify: `Cargo.toml`, `deny.toml`, `justfile`, `AGENTS.md`, `crates/a2ui-runtime/Cargo.toml`,
  `crates/a2ui-runtime/src/{lib.rs,error.rs,schema.rs,schema_tests.rs,expression/parser.rs}`,
  `crates/a2ui-runtime/ARCHITECTURE.md`
- Create: `crates/a2ui-runtime/src/{ident.rs,ident_tests.rs,limits.rs,limits_tests.rs,validator.rs,validator_tests.rs,bundle.rs,bundle_tests.rs}`,
  `crates/a2ui-conformance/src/handlers/common_type.rs`
- Modify: `crates/a2ui-conformance/src/handlers/mod.rs`, `crates/a2ui-conformance/skips.toml`,
  `crates/a2ui-conformance/ARCHITECTURE.md`

**Interfaces:**
- Produces:
  - `Category::Validation` (`"ValidationError"`).
  - `pub(crate) fn ident::is_identifier(name: &str) -> bool`.
  - `pub(crate) const limits::{MAX_JSON_DEPTH, MAX_PATTERN_BYTES}` (64 and 64 KiB),
    `pub(crate) fn limits::deeper_than(value: &Value, limit: usize) -> bool`.
  - `pub(crate) fn validator::compile(document: &Value) -> Result<jsonschema::Validator, Error>`,
    `pub(crate) fn validator::rooted(top: &Map<String, Value>, reference: &str) -> Value`,
    `pub(crate) fn validator::check(validator: &jsonschema::Validator, instance: &Value) -> Result<(), Error>`,
    `pub(crate) const validator::DRAFT_2020_12: &str`.
  - `pub(crate) fn bundle::is_supported_ref(reference: &str) -> bool`,
    `bundle::local_ref(pointer: &str) -> String`, `bundle::localize_ref(reference: &str) -> String`,
    `bundle::localize_refs(node: &mut Value)`, `bundle::any_function_envelope() -> Value`.
  - `pub fn a2ui_runtime::validate_common_type(definition: &str, value: &Value) -> Result<(), Error>`,
    `pub(crate) fn schema::common_type_defs() -> Result<&'static Map<String, Value>, Error>`,
    `pub(crate) fn schema::common_document() -> Result<Map<String, Value>, Error>`.

Every item this task adds has a caller in this task: the catalog-only helpers (reference checks,
common type closure, metadata validation) arrive with their caller in Task 4, so no
`dead_code` expectation is needed.

- [ ] **Step 1: Add the dependencies, their licenses and the dependency check**

Root `Cargo.toml`, in `[workspace.dependencies]` (alphabetical):

```toml
jsonschema = { version = "=0.61.1", default-features = false }
percent-encoding = "2"
```

`crates/a2ui-runtime/Cargo.toml` `[dependencies]`: `jsonschema.workspace = true` and
`percent-encoding.workspace = true`.

`deny.toml` becomes exactly:

```toml
[licenses]
allow = ["MIT", "MIT-0", "Apache-2.0", "Unicode-3.0", "Zlib"]
```

(`MIT-0`: `fluent-uri`; `Zlib`: a `hashbrown` dependency; both seen in the probe.)

`AGENTS.md`, replace the invariant
`- Keep a2ui-runtime free of Leptos, wasm-bindgen, async runtimes, randomness and I/O. (spec:2026-11-10-a2ui-rs-leptos-design)` with
`- Keep a2ui-runtime free of direct Leptos, wasm-bindgen, async-runtime and I/O dependencies, and its results free of randomness. (spec:2026-11-10-a2ui-rs-leptos-design)`.

`justfile`: replace the `check` recipe and add `deps` after it:

```make
# Formatting, lints, dependency rules and the wasm build.
check:
    cargo fmt --all --check
    cargo clippy --workspace --all-targets -- -D warnings
    just deps
    just wasm

# a2ui-runtime: no direct UI, async or JS dependency; wasm-bindgen only through getrandom.
deps:
    #!/usr/bin/env bash
    set -euo pipefail
    direct="$(cargo tree -p a2ui-runtime --target all -e normal --depth 1 --prefix none)"
    if grep -E '^(leptos|tokio|async-std|wasm-bindgen|js-sys|web-sys) ' <<< "$direct"; then
        echo "a2ui-runtime has a forbidden direct dependency" >&2
        exit 1
    fi
    parents="$(cargo tree -p a2ui-runtime --target wasm32-unknown-unknown -e normal \
        -i wasm-bindgen --depth 1 --prefix none | sed 's/ v.*//' | grep -vx wasm-bindgen | sort -u)"
    test "$parents" = getrandom
```

Run: `cargo build -p a2ui-runtime && just deps && just wasm && cargo deny check licenses`
Expected: all succeed (`just deps` exits 0 and prints nothing).

- [ ] **Step 2: Write the failing tests**

Create `crates/a2ui-runtime/src/ident_tests.rs`:

```rust
use rstest::rstest;
use speculoos::prelude::*;

use super::*;

#[rstest]
#[case("Component_ℓ", true)]
#[case("_private", true)]
#[case("ünïcode", true)]
#[case("format_text_1", true)]
#[case("1st", false)]
#[case("has-dash", false)]
#[case("has space", false)]
#[case("", false)]
#[case("Component–Name", false)]
#[case("prop—name", false)]
#[case("func֊name", false)]
#[case("@index", false)]
fn is_identifier_uax31_names_are_classified(#[case] name: &str, #[case] expected: bool) {
    assert_that!(is_identifier(name)).is_equal_to(expected);
}
```

Create `crates/a2ui-runtime/src/limits_tests.rs`:

```rust
use rstest::rstest;
use serde_json::{Value, json};
use speculoos::prelude::*;

use super::*;

fn nested(levels: usize) -> Value {
    (0..levels).fold(json!(1), |inner, _| json!({"a": inner}))
}

#[rstest]
#[case(json!(1), 0, false)]
#[case(json!({"a": 1}), 1, false)]
#[case(json!({"a": [1]}), 1, true)]
#[case(json!([[], {}]), 2, false)]
fn deeper_than_small_values_are_measured(
    #[case] value: Value,
    #[case] limit: usize,
    #[case] expected: bool,
) {
    assert_that!(deeper_than(&value, limit)).is_equal_to(expected);
}

#[test]
fn deeper_than_value_at_the_cap_is_accepted() {
    assert_that!(deeper_than(&nested(MAX_JSON_DEPTH), MAX_JSON_DEPTH)).is_false();
}

#[test]
fn deeper_than_value_past_the_cap_is_rejected() {
    assert_that!(deeper_than(&nested(MAX_JSON_DEPTH + 1), MAX_JSON_DEPTH)).is_true();
}

#[test]
fn deeper_than_hundred_thousand_levels_in_a_small_stack_is_rejected() {
    let rejected = std::thread::Builder::new()
        .stack_size(256 * 1024)
        .spawn(|| {
            let value = nested(100_000);
            let rejected = deeper_than(&value, MAX_JSON_DEPTH);
            // Dropping 100 000 levels would recurse through serde_json's `Drop` and
            // overflow this thread's stack: the test leaks the value on purpose.
            std::mem::forget(value);
            rejected
        })
        .unwrap()
        .join()
        .unwrap();
    assert_that!(rejected).is_true();
}
```

Create `crates/a2ui-runtime/src/validator_tests.rs`:

```rust
use serde_json::{Map, Value, json};
use speculoos::prelude::*;

use super::*;
use crate::Category;

fn top(value: Value) -> Map<String, Value> {
    value.as_object().unwrap().clone()
}

fn compile_def(definition: Value) -> Result<Validator, crate::Error> {
    compile(&rooted(&top(json!({"$defs": {"D": definition}})), "#/$defs/D"))
}

#[test]
fn check_matching_instance_passes() {
    let validator = compile_def(json!({"type": "string"})).unwrap();
    assert_that!(check(&validator, &json!("a"))).is_ok();
}

#[test]
fn check_violation_is_validation_error() {
    let validator = compile_def(json!({"type": "string"})).unwrap();
    let err = check(&validator, &json!(1)).unwrap_err();
    assert_that!(err.category()).is_equal_to(Category::Validation);
}

#[test]
fn check_violation_message_names_the_instance_path() {
    let validator = compile_def(json!({"properties": {"a": {"type": "string"}}})).unwrap();
    let err = check(&validator, &json!({"a": 1})).unwrap_err();
    assert_that!(err.to_string()).ends_with(" at '/a'");
}

fn extensions() -> Value {
    json!({
        "type": "object",
        "patternProperties": {"^[\\p{XID_Start}_][\\p{XID_Continue}]*$": {}},
        "additionalProperties": false
    })
}

#[test]
fn check_unicode_property_pattern_rejects_a_dash() {
    let validator = compile_def(extensions()).unwrap();
    assert_that!(check(&validator, &json!({"has-dash": 1}))).is_err();
}

#[test]
fn check_unicode_property_pattern_accepts_non_ascii_letters() {
    let validator = compile_def(extensions()).unwrap();
    assert_that!(check(&validator, &json!({"ünïcode": 1}))).is_ok();
}

#[test]
fn compile_lookahead_pattern_is_catalog_error() {
    let err = compile_def(json!({"type": "string", "pattern": "(?=a)b"})).unwrap_err();
    assert_that!(err.category()).is_equal_to(Category::Catalog);
    assert_that!(err.to_string()).contains("is not a \"regex\"");
}

#[test]
fn compile_pattern_past_the_size_cap_is_catalog_error() {
    let err = compile_def(json!({"type": "string", "pattern": "\\w{5000}"})).unwrap_err();
    assert_that!(err.to_string()).contains("is not a \"regex\"");
}

#[test]
fn compile_external_ref_is_catalog_error() {
    let err = compile_def(json!({"$ref": "https://example.com/x.json"})).unwrap_err();
    assert_that!(err.to_string()).contains("Retrieval is disabled");
}

#[test]
fn compile_external_ref_no_root_reaches_is_catalog_error() {
    let document = json!({
        "$defs": {"D": {"type": "string"}, "E": {"$ref": "https://example.com/x.json"}}
    });
    let err = compile(&rooted(&top(document), "#/$defs/D")).unwrap_err();
    assert_that!(err.category()).is_equal_to(Category::Catalog);
    assert_that!(err.to_string()).contains("Retrieval is disabled");
}

#[test]
fn compile_non_ascii_name_through_local_ref_builds() {
    let document = json!({"$defs": {"Component_ℓ": {"type": "string"}}});
    let reference = crate::bundle::local_ref("/$defs/Component_ℓ");
    assert_that!(compile(&rooted(&top(document), &reference))).is_ok();
}
```

(`\w{5000}` builds with `regex`'s default limits and was probed to fail under a 256 KiB cap,
so it fails under 64 KiB. The fragments
`is not a "regex"` and `Retrieval is disabled` are the texts of `jsonschema` 0.60.2
(`keywords/pattern.rs`, `retriever.rs`), unchanged in 0.61.1.)

Create `crates/a2ui-runtime/src/bundle_tests.rs`:

```rust
use rstest::rstest;
use serde_json::json;
use speculoos::prelude::*;

use super::*;

#[rstest]
#[case("#/$defs/Weight", true)]
#[case("#", true)]
#[case("catalog.json#/$defs/anyFunction", true)]
#[case("common_types.json#/$defs/DynamicString", true)]
#[case("./common_types.json#/$defs/DynamicString", true)]
#[case("https://a2ui.org/specification/v1_0/common_types.json#/$defs/DynamicString", true)]
#[case("common_types.json", false)]
#[case("evil/common_types.json#/$defs/X", false)]
#[case("https://evil.example/common_types.json#/$defs/X", false)]
#[case("basic_catalog.json#/components/Text", false)]
#[case("https://a2ui.org/specification/v1_0/catalogs/basic/catalog.json#/components/Button", false)]
fn is_supported_ref_v1_0_targets_are_classified(#[case] reference: &str, #[case] expected: bool) {
    assert_that!(is_supported_ref(reference)).is_equal_to(expected);
}

#[rstest]
#[case("/$defs/Child", "#/$defs/Child")]
#[case("/components/Component_ℓ", "#/components/Component_%E2%84%93")]
#[case("/$defs/a b", "#/$defs/a%20b")]
#[case("/$defs/a~1b", "#/$defs/a~1b")]
fn local_ref_pointer_is_percent_encoded(#[case] pointer: &str, #[case] expected: &str) {
    assert_that!(local_ref(pointer)).is_equal_to(expected.to_owned());
}

#[rstest]
#[case("common_types.json#/$defs/Child", "#/$defs/Child")]
#[case("https://a2ui.org/specification/v1_0/common_types.json#/$defs/Child", "#/$defs/Child")]
#[case("catalog.json#/$defs/anyFunction", "#/$defs/anyFunction")]
#[case("#/components/Text", "#/components/Text")]
#[case("#/components/Component_ℓ", "#/components/Component_%E2%84%93")]
#[case("#/components/Component_%E2%84%93", "#/components/Component_%E2%84%93")]
#[case("other.json#/x", "other.json#/x")]
fn localize_ref_supported_targets_become_local(#[case] reference: &str, #[case] expected: &str) {
    assert_that!(localize_ref(reference)).is_equal_to(expected.to_owned());
}

#[test]
fn localize_refs_rewrites_every_ref_string() {
    let mut node = json!({
        "a": {"$ref": "common_types.json#/$defs/X"},
        "b": [{"$ref": "catalog.json#/c"}]
    });
    localize_refs(&mut node);
    assert_that!(node).is_equal_to(json!({"a": {"$ref": "#/$defs/X"}, "b": [{"$ref": "#/c"}]}));
}

#[test]
fn localize_refs_property_named_ref_holding_a_schema_is_walked() {
    let mut node = json!({"properties": {"$ref": {"$ref": "common_types.json#/$defs/X"}}});
    localize_refs(&mut node);
    assert_that!(node).is_equal_to(json!({"properties": {"$ref": {"$ref": "#/$defs/X"}}}));
}
```

Append to `crates/a2ui-runtime/src/schema_tests.rs` (and merge the `use` lines into the file's
single import block at its top):

```rust
use rstest::rstest;
use serde_json::{Value, json};

use crate::Category;

#[test]
fn common_type_defs_embedded_document_holds_dynamic_string() {
    assert_that!(common_type_defs().unwrap().contains_key("DynamicString")).is_true();
}

#[rstest]
#[case("Extensions", json!({"vendor": {"any": "value"}}))]
#[case("Extensions", json!({"_private": 1}))]
#[case("Extensions", json!({"ünïcode": 1}))]
#[case("DynamicValue", json!({"@@path": "/a"}))]
#[case("DynamicValue", json!({"@call": "now"}))]
#[case("FunctionCall", json!({"@call": "x", "catalogId": "c", "args": {"a": 1}}))]
#[case("FunctionCall", json!({"@call": "@index"}))]
#[case("FunctionResponse", json!({"functionCallId": "call_1", "value": null}))]
#[case("Surface", json!({"component": "Surface", "child": "root"}))]
fn validate_common_type_valid_value_is_accepted(#[case] definition: &str, #[case] value: Value) {
    assert_that!(validate_common_type(definition, &value)).is_ok();
}

#[rstest]
#[case("Extensions", json!({"has-dash": 1}), "'has-dash' was unexpected")]
#[case("DynamicValue", json!({"@unknown": 1}), "'oneOf'")]
#[case("DynamicValue", json!({"@call": "now", "@path": "/a"}), "'oneOf'")]
#[case("FunctionCall", json!({"@call": "x", "bogus": 1}), "'bogus' was unexpected")]
#[case("FunctionResponse", json!({"functionCallId": "call_1"}), "'oneOf'")]
#[case("IndexSystemFunction", json!({"@call": "@index", "args": {"offset": 1, "step": 2}}), "'step' was unexpected")]
#[case("Surface", json!({"component": "Surface", "child": "root", "id": "surface"}), "'id' was unexpected")]
fn validate_common_type_invalid_value_is_validation_error(
    #[case] definition: &str,
    #[case] value: Value,
    #[case] fragment: &str,
) {
    let err = validate_common_type(definition, &value).unwrap_err();
    assert_that!(err.category()).is_equal_to(Category::Validation);
    assert_that!(err.to_string()).contains(fragment);
}

#[test]
fn validate_common_type_unknown_definition_is_validation_error() {
    let err = validate_common_type("Nope", &json!({})).unwrap_err();
    assert_that!(err.to_string()).contains("Unknown common type 'Nope'");
}

#[test]
fn validate_common_type_too_deep_value_is_validation_error() {
    let value = (0..65).fold(json!(1), |inner, _| json!([inner]));
    let err = validate_common_type("DynamicValue", &value).unwrap_err();
    assert_that!(err.to_string()).contains("nesting exceeds 64");
}
```

(The two assertions of the invalid table check one behavior, the rejection, and its reason.)

- [ ] **Step 3: Run them to see them fail**

Run: `cargo nextest run -p a2ui-runtime ident limits validator bundle schema`
Expected: fails to compile: unresolved items.

- [ ] **Step 4: Add the `Validation` category and the new errors**

In `error.rs`, add to `Category`:

```rust
    /// A value that breaks a schema, such as a component its catalog rejects.
    Validation,
```

`as_str`: `Self::Validation => "ValidationError",`.

Add to `Inner`:

```rust
    #[snafu(display("Schema does not compile: {message}"))]
    SchemaBuild {
        message: String,
        backtrace: Backtrace,
    },

    #[snafu(display("{message} at '{at}'"))]
    SchemaViolation {
        at: String,
        message: String,
        backtrace: Backtrace,
    },

    #[snafu(display("Unknown common type '{definition}'"))]
    UnknownDefinition {
        definition: String,
        backtrace: Backtrace,
    },

    #[snafu(display("Value nesting exceeds {limit} levels"))]
    ValueLimit {
        limit: usize,
        backtrace: Backtrace,
    },
```

and in `Inner::category`: `Self::EmbeddedSchema { .. } | Self::SchemaBuild { .. } => Category::Catalog,`
and `Self::SchemaViolation { .. } | Self::UnknownDefinition { .. } | Self::ValueLimit { .. } => Category::Validation,`.

- [ ] **Step 5: Write `ident.rs` and reuse it in the parser**

Create `crates/a2ui-runtime/src/ident.rs`:

```rust
//! UAX #31 identifiers, the names of components, functions, arguments and properties.

use unicode_ident::{is_xid_continue, is_xid_start};

/// Whether `name` is a UAX #31 identifier, with `_` also allowed first.
pub(crate) fn is_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|c| c == '_' || is_xid_start(c)) && chars.all(is_xid_continue)
}

#[cfg(test)]
#[path = "ident_tests.rs"]
mod tests;
```

In `expression/parser.rs`, delete the private `fn is_identifier` and add
`use crate::ident::is_identifier;`; change its `unicode_ident` import to
`use unicode_ident::is_xid_continue;` (the path-token scan still uses it).
In `lib.rs` add `mod ident;`.

- [ ] **Step 6: Write `limits.rs`**

```rust
//! Depth and size caps on JSON that agents send: catalogs and validated values.

use serde_json::Value;

/// Deepest nesting of arrays and objects accepted in a catalog or a validated value.
pub(crate) const MAX_JSON_DEPTH: usize = 64;

/// Compiled size cap of one schema `pattern`, in bytes, for the regex and its lazy DFA.
pub(crate) const MAX_PATTERN_BYTES: usize = 64 * 1024;

/// Whether `value` nests arrays and objects more than `limit` levels deep.
///
/// A scalar has depth 0 and `{"a": 1}` depth 1. The walk is iterative, allocates nothing per
/// node, and checks a container's level before pushing its children, so a value deeper than the
/// stack allows is rejected at the first level past `limit`, not followed.
pub(crate) fn deeper_than(value: &Value, limit: usize) -> bool {
    let mut stack = vec![(value, 0)];
    while let Some((node, level)) = stack.pop() {
        match node {
            Value::Array(items) => {
                if level >= limit {
                    return true;
                }
                stack.extend(items.iter().map(|child| (child, level + 1)));
            }
            Value::Object(map) => {
                if level >= limit {
                    return true;
                }
                stack.extend(map.values().map(|child| (child, level + 1)));
            }
            _ => {}
        }
    }
    false
}

#[cfg(test)]
#[path = "limits_tests.rs"]
mod tests;
```

In `lib.rs` add `mod limits;`.

- [ ] **Step 7: Write `validator.rs`**

```rust
//! JSON Schema 2020-12 validation on the linear-time `regex` engine, offline.
//!
//! Catalog schemas come from agents, so patterns compile with `regex`, which never backtracks,
//! under a size cap, and no `$ref` is ever fetched: one to another document fails the build.

use jsonschema::{Draft, PatternOptions, Validator};
use serde_json::{Map, Value};

use crate::error::{Error, SchemaBuildSnafu, SchemaViolationSnafu};
use crate::limits::MAX_PATTERN_BYTES;

/// The JSON Schema dialect every runtime schema declares.
pub(crate) const DRAFT_2020_12: &str = "https://json-schema.org/draft/2020-12/schema";

/// Compiles `document`, whose root `$ref` names the schema instances must match.
pub(crate) fn compile(document: &Value) -> Result<Validator, Error> {
    build(document).map_err(|message| -> Error { SchemaBuildSnafu { message }.build().into() })
}

/// [`compile`] with the bare `jsonschema` build message as the error, for cached validators.
pub(crate) fn build(document: &Value) -> Result<Validator, String> {
    let patterns = PatternOptions::regex()
        .size_limit(MAX_PATTERN_BYTES)
        .dfa_size_limit(MAX_PATTERN_BYTES);
    jsonschema::options()
        .with_draft(Draft::Draft202012)
        .with_pattern_options(patterns)
        .offline()
        .build(document)
        .map_err(|err| err.to_string())
}

/// `top` as a schema document whose root `$ref` is `reference`, such as `#/components/Text`.
///
/// `reference` must come from `bundle::local_ref`, which percent-encodes it.
pub(crate) fn rooted(top: &Map<String, Value>, reference: &str) -> Value {
    let mut document = top.clone();
    document.insert("$schema".to_owned(), Value::from(DRAFT_2020_12));
    document.insert("$ref".to_owned(), Value::from(reference));
    Value::Object(document)
}

/// Checks `instance`; the first violation becomes a `ValidationError`.
pub(crate) fn check(validator: &Validator, instance: &Value) -> Result<(), Error> {
    validator.validate(instance).map_err(|err| -> Error {
        SchemaViolationSnafu {
            at: err.instance_path().to_string(),
            message: err.to_string(),
        }
        .build()
        .into()
    })
}

#[cfg(test)]
#[path = "validator_tests.rs"]
mod tests;
```

- [ ] **Step 8: Write `bundle.rs`**

```rust
//! `$ref` rules of v1.0 catalogs, and the local references the runtime writes.
//!
//! A v1.0 catalog may reference itself (`#…`, `catalog.json#…`) and `common_types.json` (by a
//! relative or the published absolute URI). Localizing rewrites those to `#` fragments,
//! percent-encoded as URI references require (a raw `ℓ` makes the schema build fail).

use percent_encoding::{AsciiSet, CONTROLS, percent_decode_str, utf8_percent_encode};
use serde_json::{Value, json};

/// The `common_types.json` URIs a v1.0 catalog may reference.
const COMMON_TYPES_URIS: [&str; 3] = [
    "common_types.json",
    "./common_types.json",
    "https://a2ui.org/specification/v1_0/common_types.json",
];

/// Characters a URI fragment cannot hold literally; non-ASCII bytes are always encoded.
const FRAGMENT: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'#')
    .add(b'%')
    .add(b'<')
    .add(b'>')
    .add(b'[')
    .add(b'\\')
    .add(b']')
    .add(b'^')
    .add(b'`')
    .add(b'{')
    .add(b'|')
    .add(b'}');

/// Whether a v1.0 catalog may use `reference` as a `$ref` target.
///
/// The reference needs a fragment: a whole document is never a target.
pub(crate) fn is_supported_ref(reference: &str) -> bool {
    reference.split_once('#').is_some_and(|(document, _)| {
        document.is_empty() || document == "catalog.json" || COMMON_TYPES_URIS.contains(&document)
    })
}

/// A `$ref` to the JSON Pointer `pointer` of the same document, such as `#/$defs/Child`.
pub(crate) fn local_ref(pointer: &str) -> String {
    format!("#{}", utf8_percent_encode(pointer, FRAGMENT))
}

/// `reference` as a local, percent-encoded `$ref`; an unsupported one is returned unchanged.
///
/// A fragment whose escapes are not UTF-8 is kept as written; the schema build then reports
/// it (catalogs never get here with one: Task 4's reference check rejects it first).
pub(crate) fn localize_ref(reference: &str) -> String {
    match reference.split_once('#') {
        Some((_, fragment)) if is_supported_ref(reference) => {
            match percent_decode_str(fragment).decode_utf8() {
                Ok(pointer) => local_ref(&pointer),
                Err(_) => format!("#{fragment}"),
            }
        }
        _ => reference.to_owned(),
    }
}

/// Rewrites every `$ref` string under `node` with [`localize_ref`].
pub(crate) fn localize_refs(node: &mut Value) {
    match node {
        Value::Object(map) => {
            for (key, value) in map.iter_mut() {
                match value {
                    Value::String(reference) if key == "$ref" => {
                        *reference = localize_ref(reference);
                    }
                    _ => localize_refs(value),
                }
            }
        }
        Value::Array(items) => items.iter_mut().for_each(localize_refs),
        _ => {}
    }
}

/// An `anyFunction` for checking a call with no catalog in scope: any function but `@index`.
pub(crate) fn any_function_envelope() -> Value {
    json!({
        "type": "object",
        "properties": {
            "@call": {"not": {"const": "@index"}},
            "args": {"type": "object"}
        }
    })
}

#[cfg(test)]
#[path = "bundle_tests.rs"]
mod tests;
```

`localize_refs` recurses: it runs on the embedded common types and, from Task 4, only on
catalogs already checked to be at most `MAX_JSON_DEPTH` deep. In `lib.rs` add `mod bundle;` and
`mod validator;`.

- [ ] **Step 9: Add the common types to `schema.rs`**

Extend `schema.rs`: its imports become

```rust
use std::collections::BTreeMap;
use std::sync::LazyLock;

use jsonschema::Validator;
use serde_json::{Map, Value};
use snafu::{OptionExt, ensure};

use crate::bundle::{any_function_envelope, local_ref, localize_refs};
use crate::error::{EmbeddedSchemaSnafu, Error, UnknownDefinitionSnafu, ValueLimitSnafu};
use crate::limits::{MAX_JSON_DEPTH, deeper_than};
use crate::validator::{build, check, rooted};
```

and it gains:

```rust
/// One compiled validator per common type, built on first use.
static COMMON_VALIDATORS: LazyLock<Result<BTreeMap<String, Validator>, String>> =
    LazyLock::new(common_validators);

/// The `$defs` of `common_types.json`, by name.
pub(crate) fn common_type_defs() -> Result<&'static Map<String, Value>, Error> {
    Ok(common_types_schema()?
        .get("$defs")
        .and_then(Value::as_object)
        .with_context(|| EmbeddedSchemaSnafu {
            name: "common_types.json",
            message: "it has no $defs object".to_owned(),
        })?)
}

/// The common types as one schema document, every `$ref` local.
///
/// No catalog is in scope, so `anyFunction` is the envelope of a call to any function but
/// `@index`, whose own definition applies.
pub(crate) fn common_document() -> Result<Map<String, Value>, Error> {
    let mut defs = common_type_defs()?.clone();
    defs.values_mut().for_each(localize_refs);
    defs.insert("anyFunction".to_owned(), any_function_envelope());
    Ok(Map::from_iter([("$defs".to_owned(), Value::Object(defs))]))
}

/// Builds the validators; the error is the cause only (the document's own error, or the
/// `jsonschema` build message), which `embedded_error` prefixes once.
fn common_validators() -> Result<BTreeMap<String, Validator>, String> {
    let top = common_document().map_err(|err| err.to_string())?;
    common_type_defs()
        .map_err(|err| err.to_string())?
        .keys()
        .map(|name| {
            let reference = local_ref(&format!("/$defs/{name}"));
            Ok((name.clone(), build(&rooted(&top, &reference))?))
        })
        .collect()
}

fn embedded_error(message: &str) -> Error {
    EmbeddedSchemaSnafu {
        name: "common_types.json",
        message: message.to_owned(),
    }
    .build()
    .into()
}

/// Validates `value` as the v1.0 common type `definition`, such as `DynamicValue`.
///
/// No catalog is in scope, so a `FunctionCall` may call any function but `@index`, whose own
/// definition applies. The value is checked as is and never rewritten.
pub fn validate_common_type(definition: &str, value: &Value) -> Result<(), Error> {
    let validators = COMMON_VALIDATORS.as_ref().map_err(|message| embedded_error(message))?;
    let validator = validators
        .get(definition)
        .context(UnknownDefinitionSnafu { definition })?;
    ensure!(
        !deeper_than(value, MAX_JSON_DEPTH),
        ValueLimitSnafu {
            limit: MAX_JSON_DEPTH
        }
    );
    check(validator, value)
}
```

In `lib.rs` change the re-export to
`pub use schema::{agent_to_renderer_schema, common_types_schema, validate_common_type};` and
add [`validate_common_type`] to the crate doc's key items.

- [ ] **Step 10: Run the runtime tests**

Run: `cargo nextest run -p a2ui-runtime`
Expected: all pass (Plan 1 tests included).

- [ ] **Step 11: Write the `validate_common_type` handler**

Create `crates/a2ui-conformance/src/handlers/common_type.rs`:

```rust
//! The `validate_common_type` action: each step's value as one common type.
//!
//! The runtime validates the JSON value as is and never rebuilds it from a typed model, so the
//! suite's "serializes back to the same JSON" half holds by construction.

use serde_json::Value;

use crate::case::Case;
use crate::compare::expect_error;
use crate::keys::{assert_case_keys, assert_known_keys};

/// Validates each step's `value` as `definition`; `expectError` steps must be rejected.
pub fn run(case: &Case) -> Result<(), String> {
    assert_case_keys(case, &["protocolVersion", "definition", "steps"])?;
    let at = case.at();
    let definition = case
        .get("definition")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{at}: `definition` must be a string"))?;
    let steps = case
        .get("steps")
        .and_then(Value::as_array)
        .filter(|steps| !steps.is_empty())
        .ok_or_else(|| format!("{at}: a case needs non-empty `steps`"))?;
    for (index, step) in steps.iter().enumerate() {
        let at = format!("{at} step {index}");
        let step = step
            .as_object()
            .ok_or_else(|| format!("{at}: a step is a mapping"))?;
        assert_known_keys(step, &["value", "expectError"], &at)?;
        let value = step
            .get("value")
            .ok_or_else(|| format!("{at}: a step needs `value`"))?;
        match (
            step.get("expectError"),
            a2ui_runtime::validate_common_type(definition, value),
        ) {
            (None, Ok(())) => {}
            (None, Err(err)) => return Err(format!("{at}: rejected: {err}")),
            (Some(expected), Err(err)) => expect_error(&err, expected, &at)?,
            (Some(_), Ok(())) => return Err(format!("{at}: accepted, expected an error")),
        }
    }
    Ok(())
}
```

`handlers/mod.rs`: `mod common_type;` and `"validate_common_type" => Some(common_type::run),`.
Remove `validate_common_type` from `skips.toml`.

- [ ] **Step 12: Run the conformance cases**

Run: `cargo nextest run -p a2ui-conformance --test core common_types`
Expected: every `test_v10_*` case of `core/common_types` passes; v0.9/v0.9.1 cases ignored.

- [ ] **Step 13: Update the architecture notes**

`crates/a2ui-runtime/ARCHITECTURE.md`, add under Modules (one line each; the file stays at most
50 lines):

```markdown
- `ident`: UAX #31 `is_identifier` (`_` allowed first), shared by the parser and catalogs.
- `limits`: caps on agent JSON (64 levels deep, 64 KiB per compiled pattern).
- `validator`: `jsonschema` 2020-12, `regex` engine, offline; build errors are `CatalogError`,
  the first violation a `ValidationError`.
- `bundle`: v1.0 `$ref` rules (self, `catalog.json`, `common_types.json`), percent-encoded
  `local_ref`/`localize_refs`.
```

Extend the `schema` bullet with: `validate_common_type` (cached validators; `FunctionCall`
accepts any function but `@index`). Replace the Dependencies line with: `` `snafu`,
`serde_json`, `unicode-ident`, `percent-encoding`, `jsonschema` (default features off; brings
`wasm-bindgen` through `getrandom` on wasm32, checked by `just deps`). ``

`crates/a2ui-conformance/ARCHITECTURE.md`: `handlers` gains `common_type`.

- [ ] **Step 14: Gate and commit**

Run: `just check` then `just test`. Expected: green.

```bash
git add Cargo.toml Cargo.lock deny.toml justfile AGENTS.md crates/a2ui-runtime crates/a2ui-conformance
```

Message:

```
feat(runtime): validate common types with jsonschema on the regex engine (t-1555)

Adds UAX #31 identifiers as a shared module, depth caps, the offline jsonschema wrapper,
percent-encoded local references, and validate_common_type, which the validate_common_type
conformance cases now run. just deps checks that wasm-bindgen reaches the runtime only through
getrandom.
```

Then `oku cli task done --project slider --id t-1555 --result "<one line>"`.

---

### Task 3: Harness groundwork for catalog cases

Oku: t-1556.

**Files:**
- Modify: `crates/a2ui-conformance/src/{case.rs,case_tests.rs,skips.rs,skips_tests.rs,compare.rs,compare_tests.rs,lib.rs}`,
  `crates/a2ui-conformance/src/handlers/data_model.rs`, `crates/a2ui-conformance/tests/core.rs`,
  `crates/a2ui-conformance/skips.toml`, `crates/a2ui-conformance/ARCHITECTURE.md`

**Interfaces:**
- Consumes: `read_json`, `Case::at` (Task 1).
- Produces:
  - `Case::effective_version(&self, root: &Path) -> Result<Option<String>, String>` and
    `pub const UPSTREAM_DEFAULT_VERSION: &str = "0.9"` in `case.rs`.
  - `Skips::reason(&self, case: &Case, version: Option<&str>) -> Option<String>` and
    `Skips::unused(&self, cases: &[(&Case, Option<&str>)]) -> Vec<String>` (breaking: new
    parameters), and a `[versions]` table in `skips.toml` replacing the hardcoded
    `SUPPORTED_VERSIONS` filter (Plan 1 final review item M1).
  - `a2ui_conformance::schema_eq(a: &Value, b: &Value) -> bool`.
  - `expect_error` matches `message` as a case-insensitive literal substring or a case-insensitive
    regex.

Why: catalog cases often declare their version only inside the catalog file (`catalogPath`) or
not at all (upstream then means 0.9); a version skip must be a `skips.toml` entry that the
unused-entry guard checks; comparisons of catalog documents ignore the order of `enum` values
and `required` names (catalog.yaml header); and `"Unsupported external $ref"` is a literal that
does not match itself as a regex. A case whose catalog file cannot be read fails as its own
trial, not as harness setup.

- [ ] **Step 0: Record the executed-case manifest before the change**

The loosened message match and the `[versions]` move must not change what any Plan 1 case does.
Record the baseline on the tree as Task 2 left it (nothing of Task 3 applied yet):

```bash
mkdir -p target/manifest-t3-before
just conformance-manifest target/manifest-t3-before
cat target/manifest-t3-before/rust-core-*.jsonl | jq -r .status | sort | uniq -c \
  | tee target/manifest-t3-before.counts
```

Expected: one line per status (`failed` absent), for example `N passed` and `M skipped`.

- [ ] **Step 1: Write the failing tests**

Append to `crates/a2ui-conformance/src/case_tests.rs`:

```rust
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
    let case = one(&format!("- {{name: t, action: derive, catalogPath: '{DERIVE_SOURCE}'}}"));
    assert_that!(case.effective_version(&root()).unwrap()).is_equal_to(Some("1.0".to_owned()));
}

#[test]
fn effective_version_catalog_file_without_version_is_upstream_default() {
    let case = one(&format!("- {{name: t, action: from_json, catalogPath: '{WITHOUT_VERSION}'}}"));
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
```

(`UPSTREAM_DEFAULT_VERSION` comes in through `use super::*;`.)

Replace `crates/a2ui-conformance/src/skips_tests.rs` with:

```rust
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
```

Append to `crates/a2ui-conformance/src/compare_tests.rs` (and move every `use` of the file,
including the existing mid-file `use a2ui_runtime::Pointer;`, into one block at its top):

```rust
#[rstest]
#[case(json!({"enum": ["a", "b"]}), json!({"enum": ["b", "a"]}))]
#[case(json!({"required": ["x", "y"]}), json!({"required": ["y", "x"]}))]
#[case(json!({"properties": {"enum": {"enum": [1, 2]}}}), json!({"properties": {"enum": {"enum": [2, 1]}}}))]
#[case(json!({"a": [1, {"b": 2.0}]}), json!({"a": [1.0, {"b": 2}]}))]
fn schema_eq_equivalent_schemas_are_equal(#[case] a: Value, #[case] b: Value) {
    assert_that!(schema_eq(&a, &b)).is_true();
}

#[rstest]
#[case(json!({"oneOf": [{"type": "string"}, {"type": "number"}]}), json!({"oneOf": [{"type": "number"}, {"type": "string"}]}))]
#[case(json!({"enum": ["a", "a"]}), json!({"enum": ["a", "b"]}))]
#[case(json!({"required": ["x"]}), json!({"required": ["x", "y"]}))]
#[case(json!({"a": 1}), json!({"a": 1, "b": 2}))]
fn schema_eq_other_differences_are_detected(#[case] a: Value, #[case] b: Value) {
    assert_that!(schema_eq(&a, &b)).is_false();
}

#[test]
fn expect_error_literal_message_with_regex_metacharacters_matches() {
    let err = Pointer::parse("$x").unwrap_err();
    let expected = json!({"category": "DataError", "message": "Pointer '$x'"});
    assert_that!(expect_error(&err, &expected, "here")).is_ok();
}

#[test]
fn expect_error_literal_message_is_case_insensitive() {
    let err = Pointer::parse("$x").unwrap_err();
    let expected = json!({"category": "DataError", "message": "invalid json pointer"});
    assert_that!(expect_error(&err, &expected, "here")).is_ok();
}

#[test]
fn expect_error_regex_message_matches() {
    let err = Pointer::parse("$x").unwrap_err();
    let expected = json!({"category": "DataError", "message": "^Invalid JSON Pointer '.x'"});
    assert_that!(expect_error(&err, &expected, "here")).is_ok();
}

#[test]
fn expect_error_regex_message_is_case_insensitive() {
    let err = Pointer::parse("$x").unwrap_err();
    let expected = json!({"category": "DataError", "message": "^INVALID json pointer '.x'"});
    assert_that!(expect_error(&err, &expected, "here")).is_ok();
}

#[test]
fn expect_error_invalid_regex_found_nowhere_is_reported() {
    let err = Pointer::parse("$x").unwrap_err();
    let expected = json!({"category": "DataError", "message": "something else ("});
    let result = expect_error(&err, &expected, "here");
    assert_that!(result.unwrap_err()).contains("does not match");
}
```

The existing `expect_error_message_mismatch_fails_naming_the_mismatch` keeps passing: the new
message still says "does not match".

- [ ] **Step 2: Run them to see them fail**

Run: `cargo nextest run -p a2ui-conformance --lib`
Expected: compile errors (`effective_version`, `schema_eq`, the new `reason`/`unused`
parameters, the `versions` table).

- [ ] **Step 3: Implement `effective_version`**

In `case.rs` add `use crate::paths::read_json;`, and:

```rust
/// The version upstream assumes for a catalog that declares none (`catalog_definition.json`).
pub const UPSTREAM_DEFAULT_VERSION: &str = "0.9";
```

In `impl Case`:

```rust
    /// The protocol version the case runs under, without a leading `v`.
    ///
    /// The declared version (top-level, then inline catalog) wins; else the `catalogPath`
    /// file's own; a catalog without one is at [`UPSTREAM_DEFAULT_VERSION`].
    pub fn effective_version(&self, root: &Path) -> Result<Option<String>, String> {
        let version = match (self.protocol_version(), self.get("catalogPath")) {
            (Some(declared), _) => Some(declared.to_owned()),
            (None, Some(Value::String(path))) => {
                Some(catalog_version(&read_json(&root.join(path))?, &self.at())?)
            }
            (None, Some(_)) => return Err(format!("{}: `catalogPath` must be a string", self.at())),
            (None, None) => self
                .get("catalog")
                .map(|_| UPSTREAM_DEFAULT_VERSION.to_owned()),
        };
        Ok(version.map(|version| {
            version
                .strip_prefix('v')
                .map_or_else(|| version.clone(), str::to_owned)
        }))
    }
```

and below `impl Case`:

```rust
fn catalog_version(document: &Value, at: &str) -> Result<String, String> {
    match document.get("protocolVersion") {
        None => Ok(UPSTREAM_DEFAULT_VERSION.to_owned()),
        Some(Value::String(version)) => Ok(version.clone()),
        Some(other) => Err(format!("{at}: the catalog's protocolVersion is {other}, not a string")),
    }
}
```

`lib.rs`: `pub use case::{Case, UPSTREAM_DEFAULT_VERSION, load_suite, parse_suite};`.

- [ ] **Step 4: Move the version filter to `skips.toml`**

In `skips.rs`, delete `SUPPORTED_VERSIONS`, add the field

```rust
    #[serde(default)]
    versions: BTreeMap<String, String>,
```

to `Skips`, set the module doc to `//! The skip list: unsupported actions, skipped cases and unsupported versions.`,
and replace `reason` and `unused` with:

```rust
    /// Why `case` is skipped under `version`, its effective version, or `None` when it runs.
    ///
    /// A version absent from `[versions]` runs, so an unexpected one fails loudly.
    pub fn reason(&self, case: &Case, version: Option<&str>) -> Option<String> {
        if let Some(reason) = self.cases.get(&case.name) {
            return Some(reason.clone());
        }
        if let Some(reason) = self.actions.get(&case.action) {
            return Some(reason.clone());
        }
        if is_version_neutral(case) {
            return None;
        }
        self.versions.get(version?).cloned()
    }

    /// Entries that match no case, as `actions.<name>`, `cases.<name>` or `versions.<version>`.
    ///
    /// Each case comes with its effective version; a version-neutral case uses no version entry.
    pub fn unused(&self, cases: &[(&Case, Option<&str>)]) -> Vec<String> {
        let actions = self
            .actions
            .keys()
            .filter(|action| !cases.iter().any(|(case, _)| &case.action == *action))
            .map(|action| format!("actions.{action}"));
        let named = self
            .cases
            .keys()
            .filter(|name| !cases.iter().any(|(case, _)| &case.name == *name))
            .map(|name| format!("cases.{name}"));
        let versions = self
            .versions
            .keys()
            .filter(|listed| {
                !cases.iter().any(|(case, version)| {
                    !is_version_neutral(case) && *version == Some(listed.as_str())
                })
            })
            .map(|version| format!("versions.{version}"));
        actions.chain(named).chain(versions).collect()
    }
```

and below the `impl`:

```rust
fn is_version_neutral(case: &Case) -> bool {
    VERSION_NEUTRAL_ACTIONS.contains(&case.action.as_str())
}
```

Append to `skips.toml`:

```toml

# Protocol versions this runtime does not implement (written without the leading `v`); the
# version-neutral data_model, resolve_path and parse_expression_template run whatever version.
[versions]
"0.8" = "protocol v0.8: a2ui-rs implements v1.0 only"
"0.9" = "protocol v0.9 (also upstream's default for a catalog that declares no version): a2ui-rs implements v1.0 only"
"0.9.1" = "protocol v0.9.1: a2ui-rs implements v1.0 only"
```

In `tests/core.rs`, replace the body of `trials()` from `let cases = all_cases(&root)?;` to the
end with:

```rust
    let cases = all_cases(&root)?;
    let versions: Vec<Result<Option<String>, String>> = cases
        .iter()
        .map(|case| case.effective_version(&root))
        .collect();
    let classified: Vec<(&Case, Option<&str>)> = cases
        .iter()
        .zip(&versions)
        .filter_map(|(case, version)| Some((case, version.as_ref().ok()?.as_deref())))
        .collect();
    let unused = skips.unused(&classified);
    let mut trials = vec![Trial::test(
        "skips_toml_entries_all_match_a_case",
        move || {
            if unused.is_empty() {
                Ok(())
            } else {
                Err(format!("unused skip entries: {unused:?}").into())
            }
        },
    )];
    for (case, version) in cases.into_iter().zip(versions) {
        trials.push(trial(case, version, &skips, manifest.clone())?);
    }
    Ok(trials)
```

and the start of `trial` with:

```rust
fn trial(
    case: Case,
    version: Result<Option<String>, String>,
    skips: &Skips,
    manifest: Option<Arc<Manifest>>,
) -> Result<Trial, String> {
    let name = format!("{}::{}", case.suite.trim_end_matches(".yaml"), case.name);
    let version = match version {
        Ok(version) => version,
        Err(message) => {
            return Ok(Trial::test(name, move || {
                if let Some(manifest) = &manifest {
                    manifest.record(&case.suite, &case.name, Status::Failed, None)?;
                }
                Err(Failed::from(message))
            }));
        }
    };
    if let Some(reason) = skips.reason(&case, version.as_deref()) {
```

(the rest of `trial` is unchanged).

- [ ] **Step 5: Add `schema_eq`, the substring-or-regex match and the `Case::at` sweep**

In `compare.rs`:

```rust
/// [`json_eq`], except that arrays under `enum` and `required` compare as multisets.
pub fn schema_eq(a: &Value, b: &Value) -> bool {
    schema_eq_at(a, b, false)
}

fn schema_eq_at(a: &Value, b: &Value, unordered: bool) -> bool {
    match (a, b) {
        (Value::Array(xs), Value::Array(ys)) if unordered => same_multiset(xs, ys),
        (Value::Array(xs), Value::Array(ys)) => {
            xs.len() == ys.len() && xs.iter().zip(ys).all(|(x, y)| schema_eq_at(x, y, false))
        }
        (Value::Object(xs), Value::Object(ys)) => {
            xs.len() == ys.len()
                && xs.iter().all(|(key, x)| {
                    let unordered = key == "enum" || key == "required";
                    ys.get(key).is_some_and(|y| schema_eq_at(x, y, unordered))
                })
        }
        _ => json_eq(a, b),
    }
}

fn same_multiset(xs: &[Value], ys: &[Value]) -> bool {
    let mut left: Vec<&Value> = ys.iter().collect();
    xs.len() == ys.len()
        && xs.iter().all(|x| match left.iter().position(|y| json_eq(x, y)) {
            Some(index) => {
                left.swap_remove(index);
                true
            }
            None => false,
        })
}
```

Replace the end of `expect_error` (from `let regex =`) with:

```rust
    let text = actual.to_string();
    let substring = text.to_lowercase().contains(&pattern.to_lowercase());
    let regex = regex::RegexBuilder::new(pattern)
        .case_insensitive(true)
        .build()
        .is_ok_and(|regex| regex.is_match(&text));
    if substring || regex {
        Ok(())
    } else {
        Err(format!(
            "{at}: message {actual} does not match {pattern:?} as a substring or a regex"
        ))
    }
```

and its doc's second line to:

```rust
/// `message` matches as a case-insensitive substring or regular expression, as upstream's
/// reference harness does (`_exception_matches_expected_message`).
```

`lib.rs`: `pub use compare::{expect_error, json_eq, schema_eq};`.

`handlers/data_model.rs` still builds `suite#name` by hand (Task 1 swept `keys.rs`,
`expression.rs` and `resolve_path.rs`). In `run`, replace

```rust
        let at = format!("{}#{} step {index}", case.suite, case.name);
```

with

```rust
        let at = format!("{} step {index}", case.at());
```

- [ ] **Step 6: Run the harness tests and the whole suite**

Run: `cargo nextest run -p a2ui-conformance`
Expected: all lib tests pass; the `core` binary passes `skips_toml_entries_all_match_a_case`.

Record the manifest again and assert that every status count equals the Step 0 baseline (no
handler logic and no skip entry changed meaning; the version skips now come from `[versions]`):

```bash
mkdir -p target/manifest-t3-after
just conformance-manifest target/manifest-t3-after
cat target/manifest-t3-after/rust-core-*.jsonl | jq -r .status | sort | uniq -c \
  > target/manifest-t3-after.counts
diff target/manifest-t3-before.counts target/manifest-t3-after.counts && echo "counts equal"
```

Expected: `counts equal`. A difference means the new message match or the version move changed a
Plan 1 case; find it with

```bash
tsv='[.suite, .case, .status] | @tsv'
diff <(cat target/manifest-t3-before/rust-core-*.jsonl | jq -r "$tsv" | sort) \
     <(cat target/manifest-t3-after/rust-core-*.jsonl | jq -r "$tsv" | sort)
```

and fix the runtime or the skip entry, never the baseline.

- [ ] **Step 7: Update `crates/a2ui-conformance/ARCHITECTURE.md`**

Change the `case` bullet (the one saying "the protocol version is the top-level
`protocolVersion`, else `catalog.protocolVersion`") to say that `effective_version` gives the
version (declared, else the `catalogPath` file's, else `0.9` for a catalog; no leading `v`; a
read error fails that trial), keeping Task 1's clause "`at()` is the `suite#name` location".

Replace the `skips` bullet (it still describes the hardcoded version filter) with:

```markdown
- `skips`: `skips.toml` `[actions]`, `[cases]`, `[versions]` (each with a reason; the
  version-neutral `data_model`, `resolve_path`, `parse_expression_template` ignore `[versions]`).
  An entry matching no case fails `skips_toml_entries_all_match_a_case`.
```

Replace the `compare` bullet with:

```markdown
- `compare`: `json_eq`, `schema_eq` (`enum`/`required` as multisets), `expect_error` (category,
  then message as a case-insensitive substring or regex).
```

The file stays under 50 lines.

- [ ] **Step 8: Gate and commit**

Run: `just check` then `just test`. Expected: green.

```bash
git add crates/a2ui-conformance
```

Message:

```
test(conformance)!: skip versions through skips.toml and compare catalog schemas (t-1556)

A case's effective version now comes from its catalogPath file or upstream's 0.9 default, and
version skips are [versions] entries the unused-entry guard checks (Skips::reason and
Skips::unused take versions). schema_eq ignores enum and required order, and expectError
messages match as a substring or a regex, as upstream's reference harness does.
```

Then `oku cli task done --project slider --id t-1556 --result "<one line>"`.

---

### Task 4: Catalog loading, document and validation schema

Oku: t-1557.

**Files:**
- Create: `crates/a2ui-runtime/src/catalog/{mod.rs,test_support.rs,load.rs,load_tests.rs,entries.rs,entries_tests.rs,compile.rs,compile_tests.rs,document.rs,document_tests.rs,shape.rs,shape_tests.rs}`,
  `crates/a2ui-conformance/src/handlers/{catalog_input.rs,round_trip.rs}`
- Modify: `crates/a2ui-runtime/src/{lib.rs,error.rs,bundle.rs,bundle_tests.rs,limits.rs,limits_tests.rs,schema.rs,schema_tests.rs}`,
  `crates/a2ui-runtime/ARCHITECTURE.md`, `crates/a2ui-conformance/src/handlers/mod.rs`,
  `crates/a2ui-conformance/skips.toml`, `crates/a2ui-conformance/ARCHITECTURE.md`

**Interfaces:**
- Consumes: `is_identifier`, `deeper_than`, `MAX_JSON_DEPTH`, `bundle::{is_supported_ref,
  local_ref, localize_ref, localize_refs}`, `validator::{compile, rooted, check, DRAFT_2020_12}`,
  `schema::{common_type_defs, common_document}` (Task 2); `json_eq`, `Case::at`, `read_json`
  (Tasks 1, 3).
- Produces (re-exported from `a2ui_runtime`):
  - `#[non_exhaustive] AllowedCallers { RendererOnly, AgentOnly, RendererOrAgent }`
    (`Debug, Clone, Copy, PartialEq, Eq, Hash`) with `as_str(self) -> &'static str`.
  - `ComponentDef` with `name() -> &str`, `schema() -> &Value`, `allowed_parents() -> Option<&[String]>`,
    `allowed_children() -> Option<&[String]>`, `metadata() -> Option<&Value>`, `shape() -> &ComponentShape`.
  - `ComponentShape` with `properties() -> &BTreeSet<String>`, `required() -> &BTreeSet<String>`.
  - `FunctionDef` with `name() -> &str`, `schema() -> &Value`, `return_type() -> Option<&str>`,
    `allowed_callers() -> AllowedCallers`, `requires_user_activation() -> bool`.
  - `Catalog` with `from_json(document: &Value, catalog_id: Option<&str>, protocol_version: Option<&str>) -> Result<Catalog, Error>`,
    `parse(text: &str, catalog_id: Option<&str>, protocol_version: Option<&str>) -> Result<Catalog, Error>`,
    `id() -> &str`, `protocol_version() -> &str`, `component(name) -> Option<&ComponentDef>`,
    `components() -> impl Iterator<Item = &ComponentDef>`, `function(name) -> Option<&FunctionDef>`,
    `functions() -> impl Iterator<Item = &FunctionDef>`, `theme() -> Option<&Value>`,
    `instructions() -> Option<&str>`, `validation_schema() -> &Value`, `to_json() -> Value`.
  - crate-private: `bundle::{pointer_of, entry_of, resolve, check_references,
    add_common_types, function_call_envelope}`, `limits::{MAX_CATALOG_BYTES,
    MAX_CATALOG_ENTRIES, MAX_CATALOG_NODES, MAX_ENTRY_EXPANSION, MAX_CATALOG_EXPANSION,
    MAX_SCHEMA_DEPTH, MAX_CATALOG_PATTERNS, Expansion, expand, more_nodes_than}`,
    `schema::validate_component_metadata`, `catalog::document::union(section: &str, names: &[&str],
    discriminator: Option<&str>) -> Value` (sorts `names` itself; Task 7 regenerates unions
    with it), `pub(super) Catalog::kept_document() -> Map<String, Value>` (Task 7 derives from it),
    `ComponentDef::validator()` and `FunctionDef::validator()` `-> &jsonschema::Validator`
    (Task 5 reads them and removes their `dead_code` expectations),
    `AllowedCallers::from_catalog_str(text: &str) -> Option<AllowedCallers>`.
  - `error.rs`: `const _: () = assert!(size_of::<Error>() <= 128);`.
  - Harness: `handlers::catalog_input::{document(case: &Case) -> Result<Value, String>,
    load(case: &Case, document: &Value) -> Result<Result<Catalog, a2ui_runtime::Error>, String>}`.

Load rules (from `catalog.yaml`, `catalog_definition.json` and upstream's `Catalog.from_json`),
in the order `from_json` applies them:

1. The document is a JSON object nested at most `MAX_JSON_DEPTH` (64) levels and holding at
   most `MAX_CATALOG_NODES` (200 000) JSON nodes; `parse` also caps the text at
   `MAX_CATALOG_BYTES` (4 MiB).
2. The id is the `catalog_id` argument, else the string `catalogId`, else `CatalogError`
   "catalog_id must be provided or exist in catalog schema"; an empty id, given or declared,
   is that same error. A non-string `catalogId` is malformed.
3. The version is the argument, else the string `protocolVersion`, else `0.9`
   (`catalog_definition.json`); it must be `1.0` or `1.0.N`.
4. References: every `$ref` is a string naming the document itself (`#…`, `catalog.json#…`)
   or `common_types.json` ("Unsupported external $ref '…' at <pointer>"), whose fragment is
   empty or a JSON Pointer (`/…`) with UTF-8 percent escapes. Anchors (`$anchor`,
   `$dynamicAnchor`, `$recursiveAnchor`) and dynamic references (`$dynamicRef`,
   `$recursiveRef`) are refused; `$id` and `$schema` appear only at the top level. A key
   holding an object or array (a property named `$ref`) is walked, not checked as a keyword.
5. `$defs` names never equal a common type name (a catalog cannot redefine `DynamicString`).
6. `components`, `functions`, `$defs` are objects when present; together at most
   `MAX_CATALOG_ENTRIES` (1024) components and functions. `Surface` is reserved.
7. Component names, their property names (top level and `allOf`/`anyOf`/`oneOf` branches),
   function names and argument names are UAX #31 identifiers ("Invalid UAX #31 … identifier").
   Argument names come from `properties.args` and `args`, which are schemas (their
   `properties` keys), and from `parameters`, a shorthand map of name to schema (its keys).
8. `allowedCallers` is `rendererOnly` (default), `agentOnly` or `rendererOrAgent`;
   `requiresUserActivation` (default false) set to true requires `rendererOnly`. Checked for
   every function, also those a union leaves out. `allowedParents`/`allowedChildren` are lists
   of names. A component's `metadata` must match `ComponentCommon.metadata` (`ValidationError`).
9. A `$defs.anyComponent` / `$defs.anyFunction` `oneOf` listing `/components/…` or
   `/functions/…` entries keeps only those entries; no union, or one listing none, keeps all.
10. The validation schema bundles the kept entries, the `$defs` (with a generated union when the
    document has none) and the common types they reach, every `$ref` local and percent-encoded.
11. With `FunctionCall` reduced to its envelope (Task 5 checks calls against the catalog they
    run in), each kept entry, its `$ref`s followed, has no `$ref` cycle, no `$ref` outside a
    `$defs`, `components` or `functions` entry, at most `MAX_ENTRY_EXPANSION` (16 384) schema
    nodes and at most `MAX_SCHEMA_DEPTH` (128) levels, each `$ref` hop one level; all entries
    together expand to at most `MAX_CATALOG_EXPANSION` (262 144) nodes and
    `MAX_CATALOG_PATTERNS` (256) patterns. Every entry passes before any compiles; then each
    entry's validator compiles against a document pruned to the entry and the entries it reaches.
12. Each component's `ComponentShape` (property and required names through `allOf` and `$ref`)
    is read from the validation schema.

- [ ] **Step 1: Write the failing tests**

Append to `crates/a2ui-runtime/src/bundle_tests.rs` (its imports gain `rstest` cases below and
`use crate::Category;`, `use serde_json::{Map, Value};`):

```rust
#[rstest]
#[case("#/$defs/Child", Some("/$defs/Child"))]
#[case("common_types.json#/$defs/Child", Some("/$defs/Child"))]
#[case("#/components/Component_%E2%84%93", Some("/components/Component_ℓ"))]
#[case("#", Some(""))]
#[case("catalog.json#", Some(""))]
#[case("other.json#/x", None)]
#[case("#A", None)]
#[case("#/components/%FF", None)]
fn pointer_of_supported_refs_are_decoded(#[case] reference: &str, #[case] expected: Option<&str>) {
    assert_that!(pointer_of(reference).as_deref()).is_equal_to(expected);
}

#[test]
fn local_ref_percent_sign_is_encoded() {
    assert_that!(local_ref("/$defs/a%b")).is_equal_to("#/$defs/a%25b".to_owned());
}

#[test]
fn pointer_of_encoded_percent_sign_is_decoded() {
    assert_that!(pointer_of("#/$defs/a%25b").as_deref()).is_equal_to(Some("/$defs/a%b"));
}

#[rstest]
#[case("/$defs/Child", Some(("$defs", "Child")))]
#[case("/components/Card/properties/child", Some(("components", "Card")))]
#[case("/functions/a~1b", Some(("functions", "a/b")))]
#[case("", None)]
#[case("/theme/x", None)]
#[case("/$defs", None)]
fn entry_of_pointer_names_its_entry(
    #[case] pointer: &str,
    #[case] expected: Option<(&str, &str)>,
) {
    let entry = entry_of(pointer);
    let entry = entry.as_ref().map(|(section, name)| (*section, name.as_str()));
    assert_that!(entry).is_equal_to(expected);
}

#[test]
fn resolve_percent_encoded_ref_finds_the_entry() {
    let root = json!({"components": {"Component_ℓ": {"type": "object"}}});
    let found = resolve(&root, "#/components/Component_%E2%84%93");
    assert_that!(found).is_equal_to(Some(&json!({"type": "object"})));
}

fn references(document: Value) -> Result<(), crate::Error> {
    check_references(document.as_object().unwrap())
}

fn reference_error(document: Value) -> String {
    let err = references(document).unwrap_err();
    assert_that!(err.category()).is_equal_to(Category::Catalog);
    err.to_string()
}

#[test]
fn check_references_supported_refs_pass() {
    let document = json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": "https://example.com/c.json",
        "components": {"A": {"allOf": [{"$ref": "common_types.json#/$defs/Checkable"}]}},
        "$defs": {"whole": {"$ref": "catalog.json#"}, "local": {"$ref": "#/$defs/whole"}}
    });
    assert_that!(references(document)).is_ok();
}

#[test]
fn check_references_properties_named_like_keywords_pass() {
    let document = json!({"$defs": {"x": {"properties": {
        "$ref": {"type": "string"},
        "$anchor": {"type": "string"},
        "$id": {"type": "string"}
    }}}});
    assert_that!(references(document)).is_ok();
}

#[rstest]
#[case(
    json!({"components": {"A": {"allOf": [{"$ref": "x.json#/a"}]}}}),
    "Unsupported external $ref 'x.json#/a' at /components/A/allOf/0/$ref"
)]
#[case(json!({"$defs": {"a/b": {"$ref": "x.json#/a"}}}), "at /$defs/a~1b/$ref")]
#[case(
    json!({"components": {"A": {"$id": "https://example.com/a"}}}),
    "Unsupported '$id' at /components/A/$id: only the catalog's top level may declare it"
)]
#[case(
    json!({"components": {"A": {"$schema": "https://json-schema.org/draft/2020-12/schema"}}}),
    "Unsupported '$schema' at /components/A/$schema"
)]
#[case(
    json!({"$defs": {
        "x": {"$anchor": "A"},
        "y": {"allOf": [{"$ref": "#A"}, {"$ref": "#A"}]}
    }}),
    "Unsupported '$anchor' at /$defs/x/$anchor: anchors are not supported; use a JSON Pointer"
)]
#[case(
    json!({"$defs": {"x": {"$dynamicAnchor": "meta"}}}),
    "Unsupported '$dynamicAnchor' at /$defs/x/$dynamicAnchor"
)]
#[case(
    json!({"$defs": {"x": {"$recursiveAnchor": true}}}),
    "Unsupported '$recursiveAnchor' at /$defs/x/$recursiveAnchor"
)]
#[case(
    json!({"$defs": {"x": {"$dynamicRef": "#meta"}}}),
    "Unsupported '$dynamicRef' at /$defs/x/$dynamicRef: dynamic references are not supported"
)]
#[case(
    json!({"$defs": {"x": {"$recursiveRef": "#"}}}),
    "Unsupported '$recursiveRef' at /$defs/x/$recursiveRef"
)]
#[case(
    json!({"$defs": {"x": {"$ref": "#A"}}}),
    "$ref '#A' at /$defs/x/$ref has a fragment that is neither empty nor a JSON Pointer"
)]
#[case(
    json!({"$defs": {"x": {"$ref": "#/components/%FF"}}}),
    "$ref '#/components/%FF' at /$defs/x/$ref has a fragment that is not UTF-8"
)]
#[case(
    json!({"$defs": {"x": {"$ref": 5}}}),
    "Malformed catalog: $ref at /$defs/x/$ref must be a string"
)]
fn check_references_unsupported_reference_is_catalog_error(
    #[case] document: Value,
    #[case] fragment: &str,
) {
    assert_that!(reference_error(document)).contains(fragment);
}

#[test]
fn check_references_property_named_like_a_keyword_is_walked() {
    let document = json!({"$defs": {"x": {"properties": {"$ref": {"$ref": "x.json#/a"}}}}});
    assert_that!(reference_error(document)).contains("at /$defs/x/properties/$ref/$ref");
}

#[test]
fn add_common_types_copies_the_reachable_closure() {
    let common = json!({
        "A": {"$ref": "#/$defs/B"},
        "B": {"type": "string"},
        "C": {"type": "number"}
    });
    let mut defs = Map::new();
    add_common_types(&mut defs, &[&json!({"$ref": "#/$defs/A"})], common.as_object().unwrap());
    let names: Vec<&str> = defs.keys().map(String::as_str).collect();
    assert_that!(names).is_equal_to(vec!["A", "B"]);
}

#[test]
fn add_common_types_cycle_between_common_types_terminates() {
    let common = json!({"A": {"$ref": "#/$defs/B"}, "B": {"$ref": "#/$defs/A"}});
    let mut defs = Map::new();
    add_common_types(&mut defs, &[&json!({"$ref": "#/$defs/A"})], common.as_object().unwrap());
    let names: Vec<&str> = defs.keys().map(String::as_str).collect();
    assert_that!(names).is_equal_to(vec!["A", "B"]);
}

#[test]
fn add_common_types_definition_already_present_is_kept() {
    let common = json!({"A": {"type": "string"}});
    let mut defs = Map::from_iter([("A".to_owned(), json!({"type": "number"}))]);
    add_common_types(&mut defs, &[&json!({"$ref": "#/$defs/A"})], common.as_object().unwrap());
    assert_that!(defs["A"].clone()).is_equal_to(json!({"type": "number"}));
}

#[test]
fn add_common_types_encoded_and_escaped_names_are_decoded() {
    let common = json!({"a/ℓ": {"type": "string"}});
    let mut defs = Map::new();
    let root = json!({"$ref": "#/$defs/a~1%E2%84%93"});
    add_common_types(&mut defs, &[&root], common.as_object().unwrap());
    assert_that!(defs.contains_key("a/ℓ")).is_true();
}

#[test]
fn add_common_types_copied_refs_are_localized() {
    let common = json!({"A": {"$ref": "common_types.json#/$defs/B"}, "B": {}});
    let mut defs = Map::new();
    add_common_types(&mut defs, &[&json!({"$ref": "#/$defs/A"})], common.as_object().unwrap());
    assert_that!(defs["A"].clone()).is_equal_to(json!({"$ref": "#/$defs/B"}));
}

#[test]
fn function_call_envelope_closes_the_call_over_function_common() {
    assert_that!(function_call_envelope()).is_equal_to(json!({
        "type": "object",
        "allOf": [{"$ref": "#/$defs/FunctionCommon"}],
        "properties": {"args": {"type": "object"}},
        "unevaluatedProperties": false
    }));
}
```

Append to `crates/a2ui-runtime/src/limits_tests.rs` (its imports gain
`use std::collections::BTreeSet;` and
`use crate::catalog::test_support::{fan_out, nested};`; delete its own `nested` helper from
Task 2, since the shared one below replaces it):

```rust
#[rstest]
#[case(json!(1), 0, true)]
#[case(json!(1), 1, false)]
#[case(json!({"a": [1, 2]}), 4, false)]
#[case(json!({"a": [1, 2]}), 3, true)]
fn more_nodes_than_every_node_is_counted(
    #[case] value: Value,
    #[case] limit: usize,
    #[case] expected: bool,
) {
    assert_that!(more_nodes_than(&value, limit)).is_equal_to(expected);
}

fn fan_out_document(levels: usize) -> Value {
    json!({"$defs": fan_out(levels, false)})
}

fn chain(links: usize) -> Value {
    let mut defs = serde_json::Map::new();
    defs.insert("D0".to_owned(), json!({}));
    for link in 1..=links {
        defs.insert(format!("D{link}"), json!({"$ref": format!("#/$defs/D{}", link - 1)}));
    }
    json!({"$defs": defs})
}

fn within(expansion: Expansion<'_>) -> (usize, usize, BTreeSet<(&str, String)>) {
    match expansion {
        Expansion::Within {
            size,
            patterns,
            reached,
        } => (size, patterns, reached),
        other => panic!("expected an expansion within the caps, got {other:?}"),
    }
}

#[test]
fn expand_plain_schema_counts_every_node() {
    let root = json!({"type": "object", "properties": {"a": {"type": "string"}}});
    assert_that!(within(expand(&root, &root, 100)).0).is_equal_to(5);
}

#[test]
fn expand_ref_counts_its_target() {
    let root = json!({"$defs": {"S": {"type": "string"}}, "x": {"$ref": "#/$defs/S"}});
    assert_that!(within(expand(&root, &root["x"], 100)).0).is_equal_to(4);
}

#[test]
fn expand_fan_out_counts_every_followed_ref() {
    let root = fan_out_document(6);
    assert_that!(within(expand(&root, &root["$defs"]["L6"], 10_000)).0).is_equal_to(506);
}

#[test]
fn expand_past_the_budget_is_too_large() {
    let root = fan_out_document(40);
    assert_that!(expand(&root, &root["$defs"]["L40"], 1000)).is_equal_to(Expansion::TooLarge);
}

#[test]
fn expand_ref_cycle_is_named() {
    let root = json!({"$defs": {"A": {"$ref": "#/$defs/B"}, "B": {"$ref": "#/$defs/A"}}});
    let expansion = expand(&root, &root["$defs"]["A"], 100);
    assert_that!(expansion).is_equal_to(Expansion::Cycle("#/$defs/A".to_owned()));
}

#[test]
fn expand_sibling_refs_to_one_target_are_no_cycle() {
    let root = json!({
        "$defs": {"S": {"type": "string"}},
        "x": {"$ref": "#/$defs/S", "properties": {"a": {"$ref": "#/$defs/S"}}}
    });
    assert_that!(within(expand(&root, &root["x"], 100)).0).is_equal_to(9);
}

#[rstest]
#[case("#/$defs/Missing")]
#[case("#/theme")]
#[case("#")]
#[case("#A")]
fn expand_ref_outside_any_entry_is_unresolved(#[case] reference: &str) {
    let root = json!({"theme": {}, "x": {"$ref": reference}});
    let expansion = expand(&root, &root["x"], 100);
    assert_that!(expansion).is_equal_to(Expansion::Unresolved(reference.to_owned()));
}

#[test]
fn expand_ref_chain_at_the_depth_cap_is_within() {
    let root = chain(MAX_SCHEMA_DEPTH);
    let start = &root["$defs"][format!("D{MAX_SCHEMA_DEPTH}")];
    assert_that!(within(expand(&root, start, 10_000)).0).is_equal_to(2 * MAX_SCHEMA_DEPTH + 1);
}

#[test]
fn expand_ref_chain_past_the_depth_cap_is_too_deep() {
    let links = MAX_SCHEMA_DEPTH + 1;
    let root = chain(links);
    let start = &root["$defs"][format!("D{links}")];
    assert_that!(expand(&root, start, 10_000)).is_equal_to(Expansion::TooDeep);
}

#[test]
fn expand_patterns_and_pattern_properties_are_counted() {
    let root = json!({
        "properties": {"a": {"type": "string", "pattern": "^a"}},
        "patternProperties": {"^x": {}, "^y": {}}
    });
    assert_that!(within(expand(&root, &root, 100)).1).is_equal_to(3);
}

#[test]
fn expand_reached_entries_are_listed() {
    let root = json!({
        "$defs": {"S": {"$ref": "#/components/C"}},
        "components": {"C": {}},
        "x": {"allOf": [{"$ref": "#/$defs/S"}]}
    });
    let expected = BTreeSet::from([("$defs", "S".to_owned()), ("components", "C".to_owned())]);
    assert_that!(within(expand(&root, &root["x"], 100)).2).is_equal_to(expected);
}
```

(Counts under the rule `expand` documents, each node counted once when queued: the plain
schema is the root, `"object"`, the `properties` map, the `a` map and `"string"` (5); a `$ref`
adds its string and then its target's nodes, so `x` is the map, the string, `S` and `"string"`
(4), and the fan-out level `n` holds `6 + 2 × size(n - 1)` nodes from `size(0) = 2` (506 at 6).
Two sibling `$ref`s to `S` are no cycle: `S` is on the chain only while its own subtree is
walked (9: `x`, two `$ref` strings, `S` twice with its `"string"`, `properties`, `a`). The
chain starts with the entry itself, so `A` → `B` → `A` is reported at the `$ref` back to `A`.
A chain of `n` links ends `n` levels down, `D0` counted at depth `n`: 128 links fit, 129 do not.)

Append to `crates/a2ui-runtime/src/schema_tests.rs`:

```rust
#[test]
fn validate_component_metadata_dotted_extension_key_is_validation_error() {
    let err = validate_component_metadata(&json!({"extensions": {"com.example": 1}})).unwrap_err();
    assert_that!(err.category()).is_equal_to(Category::Validation);
}

#[test]
fn validate_component_metadata_dotted_extension_key_is_named() {
    let err = validate_component_metadata(&json!({"extensions": {"com.example": 1}})).unwrap_err();
    assert_that!(err.to_string()).contains("'com.example' was unexpected");
}

#[test]
fn validate_component_metadata_extensions_only_is_accepted() {
    let metadata = json!({"extensions": {"com_example": {"track": true}}});
    assert_that!(validate_component_metadata(&metadata)).is_ok();
}
```

Create `crates/a2ui-runtime/src/catalog/test_support.rs`:

```rust
//! Catalog documents and assertions shared by the catalog unit tests.

use serde_json::{Map, Value, json};
use speculoos::prelude::*;

use crate::{Catalog, Category, Error};

/// Loads `document` with no overrides.
pub(crate) fn load(document: Value) -> Result<Catalog, Error> {
    Catalog::from_json(&document, None, None)
}

/// A v1.0 catalog `c` holding the keys of `extra`.
pub(crate) fn v10(extra: Value) -> Value {
    let mut document = json!({"catalogId": "c", "protocolVersion": "1.0"});
    let target = document.as_object_mut().unwrap();
    for (key, value) in extra.as_object().unwrap() {
        target.insert(key.clone(), value.clone());
    }
    document
}

/// The message of a load that must fail with a `CatalogError`.
pub(crate) fn catalog_error(result: Result<Catalog, Error>) -> String {
    let err = result.unwrap_err();
    assert_that!(err.category()).is_equal_to(Category::Catalog);
    err.to_string()
}

/// A scalar wrapped in `levels` single-key objects.
pub(crate) fn nested(levels: usize) -> Value {
    (0..levels).fold(json!(1), |inner, _| json!({"a": inner}))
}

/// `$defs` `L0` to `L<levels>`, each level an `allOf` of two `$ref`s to the one below.
///
/// `unevaluated` adds `"unevaluatedProperties": false` to each level. Without it, `L<n>`
/// expands to `8 × 2^n - 6` nodes: 16 378 at 11, 32 762 at 12.
pub(crate) fn fan_out(levels: usize, unevaluated: bool) -> Map<String, Value> {
    let mut defs = Map::from_iter([("L0".to_owned(), json!({"type": "string"}))]);
    for level in 1..=levels {
        let below = format!("#/$defs/L{}", level - 1);
        let mut definition = json!({"allOf": [{"$ref": below}, {"$ref": below}]});
        if unevaluated {
            definition["unevaluatedProperties"] = json!(false);
        }
        defs.insert(format!("L{level}"), definition);
    }
    defs
}
```

Create `crates/a2ui-runtime/src/catalog/document_tests.rs`:

```rust
use rstest::rstest;
use serde_json::{Map, Value, json};
use speculoos::prelude::*;

use super::*;

fn map(value: Value) -> Map<String, Value> {
    value.as_object().unwrap().clone()
}

#[rstest]
#[case(json!({"properties": {"args": {"type": "object", "properties": {"url": {}}}}}), vec!["url"])]
#[case(json!({"parameters": {"arg_ℓ": {"type": "string"}}}), vec!["arg_ℓ"])]
#[case(
    json!({"args": {
        "type": "object",
        "properties": {"a": {}},
        "allOf": [{"properties": {"b": {}}}]
    }}),
    vec!["a", "b"]
)]
#[case(json!({"args": {"a": {"type": "string"}}}), vec![])]
#[case(
    json!({"parameters": {"type": {"type": "string"}, "properties": {"type": "object"}}}),
    vec!["properties", "type"]
)]
#[case(json!({"returnType": "string"}), vec![])]
fn argument_names_every_form_is_read(#[case] schema: Value, #[case] expected: Vec<&str>) {
    let names = argument_names(&map(schema));
    assert_that!(names).is_equal_to(expected.into_iter().map(str::to_owned).collect::<Vec<_>>());
}

#[test]
fn property_names_composition_branches_are_included() {
    let schema = map(json!({
        "properties": {"a": {}},
        "allOf": [{"$ref": "#/x"}, {"properties": {"b": {}}}]
    }));
    assert_that!(property_names(&schema)).is_equal_to(vec!["a".to_owned(), "b".to_owned()]);
}

#[rstest]
#[case(json!({"returnType": "void"}), Some("void"))]
#[case(json!({"properties": {"returnType": {"const": "string"}}}), Some("string"))]
#[case(json!({"type": "object"}), None)]
fn return_type_both_forms_are_read(#[case] schema: Value, #[case] expected: Option<&str>) {
    assert_that!(return_type(&map(schema)).as_deref()).is_equal_to(expected);
}

#[test]
fn union_members_referenced_entries_are_listed() {
    let defs = map(json!({"anyComponent": {"oneOf": [
        {"$ref": "#/components/A"},
        {"$ref": "catalog.json#/components/B"},
        {"$ref": "#/components/Component_%E2%84%93"}
    ]}}));
    let members = union_members(&defs, "anyComponent", "components").unwrap();
    assert_that!(members.into_iter().collect::<Vec<_>>())
        .is_equal_to(vec!["A".to_owned(), "B".to_owned(), "Component_ℓ".to_owned()]);
}

#[rstest]
#[case(json!({}))]
#[case(json!({"anyFunction": {"not": {}}}))]
#[case(json!({"anyFunction": {"oneOf": []}}))]
#[case(json!({"anyFunction": {"oneOf": [{"$ref": "#/$defs/Other"}]}}))]
fn union_members_no_listed_entry_is_none(#[case] defs: Value) {
    assert_that!(union_members(&map(defs), "anyFunction", "functions")).is_none();
}

#[test]
fn union_with_discriminator_lists_local_refs() {
    let value = union("components", &["A", "B"], Some("component"));
    assert_that!(value).is_equal_to(json!({
        "oneOf": [{"$ref": "#/components/A"}, {"$ref": "#/components/B"}],
        "discriminator": {"propertyName": "component"}
    }));
}

#[test]
fn union_names_are_sorted() {
    let value = union("components", &["B", "A"], None);
    assert_that!(value)
        .is_equal_to(json!({"oneOf": [{"$ref": "#/components/A"}, {"$ref": "#/components/B"}]}));
}

#[test]
fn union_non_ascii_name_is_percent_encoded() {
    let value = union("functions", &["func_ℓ"], None);
    assert_that!(value).is_equal_to(json!({"oneOf": [{"$ref": "#/functions/func_%E2%84%93"}]}));
}

#[test]
fn union_without_entries_matches_nothing() {
    assert_that!(union("functions", &[], None)).is_equal_to(json!({"not": {}}));
}
```

Create `crates/a2ui-runtime/src/catalog/load_tests.rs`:

```rust
use rstest::rstest;
use serde_json::{Map, Value, json};
use speculoos::prelude::*;

use super::super::test_support::{catalog_error, load, v10};
use crate::Catalog;
use crate::limits::{MAX_CATALOG_BYTES, MAX_CATALOG_ENTRIES, MAX_CATALOG_NODES, MAX_JSON_DEPTH};

/// A v1.0 catalog nesting exactly `levels` levels of objects (`levels` at least 2).
fn nested_document(levels: usize) -> Value {
    let deep = (2..levels).fold(json!(true), |inner, _| json!({"type": "object", "x": inner}));
    v10(json!({"$defs": {"deep": deep}}))
}

/// A v1.0 catalog of `count` components.
fn components(count: usize) -> Value {
    let components: Map<String, Value> = (0..count)
        .map(|index| (format!("C{index}"), json!({"type": "object"})))
        .collect();
    v10(json!({"components": components}))
}

#[test]
fn from_json_array_document_is_catalog_error() {
    let message = catalog_error(load(json!([])));
    let expected = "Malformed catalog: a catalog document is a JSON object";
    assert_that!(message).is_equal_to(expected.to_owned());
}

#[test]
fn from_json_document_at_the_depth_cap_is_accepted() {
    assert_that!(load(nested_document(MAX_JSON_DEPTH))).is_ok();
}

#[test]
fn from_json_document_past_the_depth_cap_is_catalog_error() {
    let message = catalog_error(load(nested_document(MAX_JSON_DEPTH + 1)));
    assert_that!(message).is_equal_to("Catalog exceeds 64 levels of nesting".to_owned());
}

#[test]
fn from_json_document_past_the_node_cap_is_catalog_error() {
    let document = v10(json!({"$defs": {"big": {"enum": vec![0; MAX_CATALOG_NODES]}}}));
    let message = catalog_error(load(document));
    assert_that!(message).is_equal_to("Catalog exceeds 200000 JSON nodes".to_owned());
}

#[test]
fn from_json_missing_catalog_id_is_catalog_error() {
    let message = catalog_error(load(json!({"protocolVersion": "1.0"})));
    assert_that!(message)
        .is_equal_to("catalog_id must be provided or exist in catalog schema".to_owned());
}

#[rstest]
#[case(json!({"catalogId": "", "protocolVersion": "1.0"}), None)]
#[case(json!({"catalogId": "c", "protocolVersion": "1.0"}), Some(""))]
fn from_json_empty_catalog_id_is_catalog_error(
    #[case] document: Value,
    #[case] catalog_id: Option<&str>,
) {
    let message = catalog_error(Catalog::from_json(&document, catalog_id, None));
    assert_that!(message).contains("catalog_id must be provided");
}

#[test]
fn from_json_non_string_catalog_id_is_catalog_error() {
    let message = catalog_error(load(json!({"catalogId": 7, "protocolVersion": "1.0"})));
    assert_that!(message).contains("`catalogId` must be a string");
}

#[test]
fn from_json_catalog_id_argument_overrides_the_document() {
    let catalog = Catalog::from_json(&v10(json!({})), Some("other"), None).unwrap();
    assert_that!(catalog.id()).is_equal_to("other");
}

#[rstest]
#[case(json!({"catalogId": "c"}), "'0.9'")]
#[case(json!({"catalogId": "c", "protocolVersion": "v0.9"}), "'v0.9'")]
#[case(json!({"catalogId": "c", "protocolVersion": "1.1"}), "'1.1'")]
#[case(json!({"catalogId": "c", "protocolVersion": "1.0."}), "'1.0.'")]
fn from_json_non_v1_0_version_is_catalog_error(#[case] document: Value, #[case] version: &str) {
    assert_that!(catalog_error(load(document))).contains(version);
}

#[test]
fn from_json_non_string_version_is_catalog_error() {
    let message = catalog_error(load(json!({"catalogId": "c", "protocolVersion": 1.0})));
    assert_that!(message).contains("`protocolVersion` must be a string");
}

#[test]
fn from_json_patch_version_is_accepted() {
    let catalog = load(json!({"catalogId": "c", "protocolVersion": "1.0.2"})).unwrap();
    assert_that!(catalog.protocol_version()).is_equal_to("1.0.2");
}

#[test]
fn from_json_version_argument_overrides_the_document() {
    let catalog = Catalog::from_json(&json!({"catalogId": "c"}), None, Some("1.0")).unwrap();
    assert_that!(catalog.protocol_version()).is_equal_to("1.0");
}

#[test]
fn from_json_external_ref_is_catalog_error() {
    let document = v10(json!({"components": {"A": {"allOf": [
        {"$ref": "basic_catalog.json#/components/Text"}
    ]}}}));
    assert_that!(catalog_error(load(document))).contains("Unsupported external $ref");
}

#[test]
fn from_json_defs_shadowing_a_common_type_is_catalog_error() {
    let document = v10(json!({"$defs": {"DynamicString": {"type": "string"}}}));
    assert_that!(catalog_error(load(document))).contains("'DynamicString'");
}

#[rstest]
#[case(json!({"components": []}), "`components` must be an object")]
#[case(json!({"functions": "f"}), "`functions` must be an object")]
#[case(json!({"$defs": 1}), "`$defs` must be an object")]
fn from_json_non_object_section_is_catalog_error(#[case] extra: Value, #[case] fragment: &str) {
    assert_that!(catalog_error(load(v10(extra)))).contains(fragment);
}

#[test]
fn from_json_entries_at_the_cap_are_accepted() {
    assert_that!(load(components(MAX_CATALOG_ENTRIES))).is_ok();
}

#[test]
fn from_json_entries_past_the_cap_are_catalog_error() {
    let message = catalog_error(load(components(MAX_CATALOG_ENTRIES + 1)));
    assert_that!(message)
        .is_equal_to("Catalog exceeds 1024 components and functions entries".to_owned());
}

#[test]
fn parse_invalid_json_is_catalog_error() {
    assert_that!(catalog_error(Catalog::parse("{", None, None))).contains("not valid JSON");
}

#[test]
fn parse_text_past_the_size_cap_is_catalog_error() {
    let text = format!("{{\"catalogId\": \"c\", \"pad\": \"{}\"}}", "x".repeat(MAX_CATALOG_BYTES));
    let message = catalog_error(Catalog::parse(&text, None, None));
    assert_that!(message).is_equal_to("Catalog exceeds 4194304 bytes of catalog text".to_owned());
}

#[test]
fn to_json_returns_the_authored_document() {
    let document = v10(json!({
        "$id": "https://example.com/c.json",
        "instructions": "Use it.",
        "components": {"Label": {"allOf": [{"$ref": "common_types.json#/$defs/Checkable"}]}}
    }));
    assert_that!(load(document.clone()).unwrap().to_json()).is_equal_to(document);
}

#[test]
fn to_json_entries_left_out_of_unions_are_dropped() {
    let document = v10(json!({
        "components": {"A": {}, "B": {}},
        "$defs": {"anyComponent": {"oneOf": [{"$ref": "#/components/A"}]}}
    }));
    let json = load(document).unwrap().to_json();
    assert_that!(json["components"].clone()).is_equal_to(json!({"A": {}}));
}

#[test]
fn to_json_overridden_id_replaces_the_declared_one() {
    let catalog = Catalog::from_json(&v10(json!({})), Some("other"), None).unwrap();
    assert_that!(catalog.to_json()["catalogId"].clone()).is_equal_to(json!("other"));
}

#[test]
fn to_json_overridden_id_is_written_when_the_document_has_none() {
    let document = json!({"protocolVersion": "1.0"});
    let catalog = Catalog::from_json(&document, Some("given"), None).unwrap();
    assert_that!(catalog.to_json()["catalogId"].clone()).is_equal_to(json!("given"));
}

#[test]
fn to_json_overridden_version_reloads_without_the_override() {
    let catalog = Catalog::from_json(&json!({"catalogId": "c"}), None, Some("1.0")).unwrap();
    let reloaded = Catalog::from_json(&catalog.to_json(), None, None).unwrap();
    assert_that!(reloaded.protocol_version()).is_equal_to("1.0");
}

#[test]
fn accessors_theme_is_read_from_the_document() {
    let catalog = load(v10(json!({"theme": {"type": "object"}}))).unwrap();
    assert_that!(catalog.theme()).is_equal_to(Some(&json!({"type": "object"})));
}

#[test]
fn accessors_instructions_are_read_from_the_document() {
    let catalog = load(v10(json!({"instructions": "Be brief."}))).unwrap();
    assert_that!(catalog.instructions()).is_equal_to(Some("Be brief."));
}
```

(`nested_document(n)` nests exactly `n` levels, as Task 2's `deeper_than` counts them (a scalar
is 0, `{"a": 1}` is 1): the root object is level 1, `$defs` 2, and each of the `n - 2` folds adds
one object around a scalar, so `n = 64` is accepted and `n = 65` is rejected. The node cap
document holds the root, `catalogId`, `protocolVersion`, `$defs`, `big`, `enum`, its array and
200 000 zeros.)

Create `crates/a2ui-runtime/src/catalog/entries_tests.rs`:

```rust
use rstest::rstest;
use serde_json::{Value, json};
use speculoos::prelude::*;

use super::super::test_support::{catalog_error, load, v10};
use crate::{AllowedCallers, Category};

#[test]
fn component_def_surface_is_reserved() {
    let document = v10(json!({"components": {"Surface": {"type": "object"}}}));
    assert_that!(catalog_error(load(document))).contains("'Surface' is reserved");
}

#[rstest]
#[case(
    json!({"components": {"My-Component": {"type": "object"}}}),
    "Invalid UAX #31 component identifier: 'My-Component'"
)]
#[case(
    json!({"components": {"V": {"properties": {"prop—name": {}}}}}),
    "Invalid UAX #31 property identifier: 'prop—name' in component 'V'"
)]
#[case(
    json!({"components": {"V": {"allOf": [{"properties": {"bad-name": {}}}]}}}),
    "Invalid UAX #31 property identifier: 'bad-name'"
)]
#[case(
    json!({"functions": {"func֊name": {"returnType": "string"}}}),
    "Invalid UAX #31 function identifier: 'func֊name'"
)]
#[case(
    json!({"functions": {"@index": {"returnType": "number"}}}),
    "Invalid UAX #31 function identifier: '@index'"
)]
#[case(
    json!({"functions": {"f": {"parameters": {"123invalid_arg": {"type": "string"}}}}}),
    "Invalid UAX #31 argument identifier: '123invalid_arg' in function 'f'"
)]
#[case(
    json!({"functions": {"f": {"properties": {"args": {"properties": {"bad-arg": {}}}}}}}),
    "Invalid UAX #31 argument identifier: 'bad-arg'"
)]
fn entry_invalid_identifier_is_catalog_error(#[case] extra: Value, #[case] message: &str) {
    assert_that!(catalog_error(load(v10(extra)))).contains(message);
}

#[test]
fn entry_non_ascii_identifiers_are_accepted() {
    let document = v10(json!({
        "components": {"Component_ℓ": {
            "type": "object",
            "properties": {"property_ℓ": {"type": "string"}}
        }},
        "functions": {"func_ℓ": {
            "returnType": "string",
            "parameters": {"arg_ℓ": {"type": "string"}}
        }}
    }));
    assert_that!(load(document)).is_ok();
}

#[test]
fn entry_shorthand_arguments_named_like_keywords_are_accepted() {
    let document = v10(json!({"functions": {"f": {
        "returnType": "string",
        "parameters": {"type": {"type": "string"}, "properties": {"type": "object"}}
    }}}));
    assert_that!(load(document)).is_ok();
}

#[rstest]
#[case("agentOnly")]
#[case("rendererOrAgent")]
fn function_def_user_activation_needs_renderer_only(#[case] callers: &str) {
    let document = v10(json!({"functions": {"openUrl": {
        "returnType": "void", "allowedCallers": callers, "requiresUserActivation": true
    }}}));
    assert_that!(catalog_error(load(document))).contains("requiresUserActivation");
}

#[test]
fn function_def_user_activation_is_checked_outside_the_union() {
    let document = v10(json!({
        "functions": {
            "good": {"returnType": "void"},
            "bad": {
                "returnType": "void",
                "allowedCallers": "agentOnly",
                "requiresUserActivation": true
            }
        },
        "$defs": {"anyFunction": {"oneOf": [{"$ref": "#/functions/good"}]}}
    }));
    assert_that!(catalog_error(load(document))).contains("requiresUserActivation");
}

#[test]
fn function_def_omitted_callers_are_renderer_only() {
    let document = v10(json!({"functions": {"copy": {
        "returnType": "void", "requiresUserActivation": true
    }}}));
    let catalog = load(document).unwrap();
    assert_that!(catalog.function("copy").unwrap().allowed_callers())
        .is_equal_to(AllowedCallers::RendererOnly);
}

#[rstest]
#[case(json!({"functions": {"f": {"allowedCallers": "everyone"}}}), "unknown allowedCallers")]
#[case(json!({"functions": {"f": {"requiresUserActivation": "yes"}}}), "must be a boolean")]
#[case(json!({"functions": {"f": 3}}), "function 'f' must be a schema object")]
#[case(
    json!({"components": {"A": {"allowedParents": "Row"}}}),
    "`allowedParents` of component 'A'"
)]
#[case(
    json!({"components": {"A": {"allowedChildren": [1]}}}),
    "`allowedChildren` of component 'A'"
)]
fn entry_malformed_attribute_is_catalog_error(#[case] extra: Value, #[case] fragment: &str) {
    assert_that!(catalog_error(load(v10(extra)))).contains(fragment);
}

#[test]
fn component_def_allowed_parents_are_exposed() {
    let document = v10(json!({"components": {"Card": {
        "type": "object", "allowedParents": ["Surface", "Row"]
    }}}));
    let catalog = load(document).unwrap();
    let parents = catalog.component("Card").unwrap().allowed_parents();
    assert_that!(parents).is_equal_to(Some(&["Surface".to_owned(), "Row".to_owned()][..]));
}

#[test]
fn component_def_allowed_children_are_exposed() {
    let document = v10(json!({"components": {"Card": {
        "type": "object", "allowedChildren": ["Button"]
    }}}));
    let catalog = load(document).unwrap();
    let children = catalog.component("Card").unwrap().allowed_children();
    assert_that!(children).is_equal_to(Some(&["Button".to_owned()][..]));
}

#[test]
fn function_def_return_type_is_exposed() {
    let document = v10(json!({"functions": {"fetchData": {
        "returnType": "object", "allowedCallers": "agentOnly"
    }}}));
    let catalog = load(document).unwrap();
    assert_that!(catalog.function("fetchData").unwrap().return_type()).is_equal_to(Some("object"));
}

#[test]
fn component_def_metadata_bad_key_is_validation_error() {
    let document = v10(json!({"components": {"Card": {
        "type": "object", "metadata": {"extensions": {"com.example": 1}}
    }}}));
    let err = load(document).unwrap_err();
    assert_that!(err.category()).is_equal_to(Category::Validation);
}

#[test]
fn component_def_metadata_is_exposed() {
    let metadata = json!({"extensions": {"com_example": {"track": true}}});
    let document = v10(json!({"components": {"Text": {
        "type": "object", "metadata": metadata.clone()
    }}}));
    let catalog = load(document).unwrap();
    assert_that!(catalog.component("Text").unwrap().metadata()).is_equal_to(Some(&metadata));
}
```

Create `crates/a2ui-runtime/src/catalog/compile_tests.rs`:

```rust
use serde_json::{Map, Value, json};
use speculoos::prelude::*;

use super::super::test_support::{catalog_error, fan_out, load, v10};
use crate::bundle::resolve;
use crate::limits::MAX_SCHEMA_DEPTH;

fn refs(node: &Value, out: &mut Vec<String>) {
    match node {
        Value::Object(map) => map.iter().for_each(|(key, value)| match value {
            Value::String(target) if key == "$ref" => out.push(target.clone()),
            _ => refs(value, out),
        }),
        Value::Array(items) => items.iter().for_each(|item| refs(item, out)),
        _ => {}
    }
}

/// A catalog whose `section` holds `entry`, with the `$defs` of a fan-out `levels` deep.
fn fanned(section: &str, entry: &str, levels: usize, unevaluated: bool) -> Value {
    let mut extra = Map::new();
    extra.insert("$defs".to_owned(), Value::Object(fan_out(levels, unevaluated)));
    let mut entries = Map::new();
    entries.insert(entry.to_owned(), json!({"$ref": format!("#/$defs/L{levels}")}));
    extra.insert(section.to_owned(), Value::Object(entries));
    v10(Value::Object(extra))
}

#[test]
fn bundle_unions_keep_only_listed_components() {
    let document = v10(json!({
        "components": {"Listed": {"type": "object"}, "Unlisted": {"type": "object"}},
        "$defs": {"anyComponent": {
            "oneOf": [{"$ref": "#/components/Listed"}],
            "discriminator": {"propertyName": "component"}
        }}
    }));
    let catalog = load(document).unwrap();
    let names: Vec<&str> = catalog.components().map(|def| def.name()).collect();
    assert_that!(names).is_equal_to(vec!["Listed"]);
}

#[test]
fn bundle_unions_keep_only_listed_functions() {
    let document = v10(json!({
        "functions": {"listed": {"returnType": "string"}, "unlisted": {"returnType": "string"}},
        "$defs": {"anyFunction": {"oneOf": [{"$ref": "#/functions/listed"}]}}
    }));
    let catalog = load(document).unwrap();
    let names: Vec<&str> = catalog.functions().map(|def| def.name()).collect();
    assert_that!(names).is_equal_to(vec!["listed"]);
}

#[test]
fn validation_schema_every_ref_resolves_locally() {
    let document = v10(json!({"components": {"Label_ℓ": {
        "allOf": [{"$ref": "common_types.json#/$defs/Checkable"}],
        "properties": {"text": {
            "$ref": "https://a2ui.org/specification/v1_0/common_types.json#/$defs/DynamicString"
        }}
    }}}));
    let catalog = load(document).unwrap();
    let schema = catalog.validation_schema();
    let mut found = Vec::new();
    refs(schema, &mut found);
    assert_that!(found.is_empty()).is_false();
    for target in found {
        assert_that!(target.starts_with('#')).is_true();
        assert_that!(resolve(schema, &target)).is_some();
    }
}

#[test]
fn validation_schema_missing_unions_are_generated() {
    let catalog = load(v10(json!({"components": {"A": {}}}))).unwrap();
    let defs = &catalog.validation_schema()["$defs"];
    assert_that!(defs["anyComponent"].clone()).is_equal_to(json!({
        "oneOf": [{"$ref": "#/components/A"}],
        "discriminator": {"propertyName": "component"}
    }));
}

#[test]
fn validation_schema_missing_function_union_matches_nothing() {
    let catalog = load(v10(json!({"components": {"A": {}}}))).unwrap();
    let union = catalog.validation_schema()["$defs"]["anyFunction"].clone();
    assert_that!(union).is_equal_to(json!({"not": {}}));
}

#[test]
fn from_json_lookahead_pattern_is_catalog_error() {
    let document = v10(json!({"components": {"A": {"properties": {
        "p": {"type": "string", "pattern": "(?=a)"}
    }}}}));
    assert_that!(catalog_error(load(document))).contains("is not a \"regex\"");
}

#[test]
fn from_json_unreached_defs_are_not_compiled() {
    let document = v10(json!({
        "components": {"A": {"type": "object"}},
        "$defs": {"Unused": {"type": "string", "pattern": "(?=a)"}}
    }));
    assert_that!(load(document)).is_ok();
}

#[test]
fn from_json_unresolvable_local_ref_is_catalog_error() {
    let document = v10(json!({"components": {"A": {"$ref": "#/$defs/Missing"}}}));
    let message = catalog_error(load(document));
    let expected = "Malformed catalog: $ref '#/$defs/Missing' does not resolve";
    assert_that!(message).is_equal_to(expected.to_owned());
}

#[test]
fn from_json_same_instance_ref_cycle_is_catalog_error() {
    let document = v10(json!({
        "components": {"A": {"$ref": "#/$defs/X"}},
        "$defs": {"X": {"anyOf": [{"$ref": "#/$defs/Y"}]}, "Y": {"allOf": [{"$ref": "#/$defs/X"}]}}
    }));
    let message = catalog_error(load(document));
    let expected = "Malformed catalog: $ref cycle through '#/$defs/X'";
    assert_that!(message).is_equal_to(expected.to_owned());
}

#[test]
fn from_json_exponential_ref_fan_out_is_catalog_error() {
    let message = catalog_error(load(fanned("components", "A", 16, true)));
    let expected = "Catalog exceeds 16384 schema nodes in 'A' once its $refs are expanded";
    assert_that!(message).is_equal_to(expected.to_owned());
}

#[test]
fn from_json_entry_just_under_the_expansion_cap_is_accepted() {
    assert_that!(load(fanned("components", "A", 11, false))).is_ok();
}

#[test]
fn from_json_function_past_the_expansion_cap_is_catalog_error() {
    let message = catalog_error(load(fanned("functions", "f", 12, false)));
    let expected = "Catalog exceeds 16384 schema nodes in 'f' once its $refs are expanded";
    assert_that!(message).is_equal_to(expected.to_owned());
}

#[test]
fn from_json_entries_past_the_catalog_expansion_cap_are_catalog_error() {
    let components: Map<String, Value> = (0..17)
        .map(|index| (format!("C{index}"), json!({"$ref": "#/$defs/L11"})))
        .collect();
    let document = v10(json!({"components": components, "$defs": fan_out(11, false)}));
    let message = catalog_error(load(document));
    assert_that!(message).is_equal_to(
        "Catalog exceeds 262144 schema nodes in all entries once their $refs are expanded"
            .to_owned(),
    );
}

#[test]
fn from_json_patterns_past_the_cap_are_catalog_error() {
    let properties: Map<String, Value> = (0..=256)
        .map(|index| (format!("p{index}"), json!({"type": "string", "pattern": "^a"})))
        .collect();
    let document = v10(json!({"components": {"A": {"properties": properties}}}));
    let message = catalog_error(load(document));
    assert_that!(message).is_equal_to("Catalog exceeds 256 patterns".to_owned());
}

#[test]
fn from_json_ref_chain_past_the_depth_cap_is_catalog_error() {
    let mut defs = Map::from_iter([("D0".to_owned(), json!({}))]);
    for link in 1..=MAX_SCHEMA_DEPTH {
        defs.insert(format!("D{link}"), json!({"$ref": format!("#/$defs/D{}", link - 1)}));
    }
    let start = format!("#/$defs/D{MAX_SCHEMA_DEPTH}");
    let document = v10(json!({"components": {"A": {"$ref": start}}, "$defs": defs}));
    let message = catalog_error(load(document));
    let expected = "Catalog exceeds 128 levels of 'A' once its $refs are expanded";
    assert_that!(message).is_equal_to(expected.to_owned());
}

#[test]
fn from_json_same_document_fails_with_the_same_message_twice() {
    let document = v10(json!({"components": {"A": {"properties": {
        "p": {"type": "string", "pattern": "(?=a)"},
        "q": {"type": "string", "pattern": "(?!b)"}
    }}}}));
    let first = catalog_error(load(document.clone()));
    assert_that!(catalog_error(load(document))).is_equal_to(first);
}
```

(The fan-out tests fail fast: every entry passes the expansion caps before any compiles, and
compiling the 16-level fan-out under `unevaluatedProperties` would take minutes. If one hangs,
the check is in the wrong place. Counts: `A` is its map and `$ref` string plus `L11` (16 378),
16 380 nodes, under 16 384; 17 such entries make 278 460, past 262 144, while 16 make
262 080. The chain puts `D0` at level 129: `A` is level 0 and each of the 129 hops one more.
The determinism test loads twice in one process: `jsonschema` builds fresh hash maps, with
fresh `ahash` seeds, for each validator.)

Create `crates/a2ui-runtime/src/catalog/shape_tests.rs`:

```rust
use std::collections::BTreeSet;

use serde_json::json;
use speculoos::prelude::*;

use super::super::test_support::{load, v10};
use super::ComponentShape;

fn names(items: &BTreeSet<String>) -> Vec<&str> {
    items.iter().map(String::as_str).collect()
}

fn boxed() -> crate::Catalog {
    load(v10(json!({"components": {"Box": {"allOf": [
        {"$ref": "common_types.json#/$defs/ComponentCommon"},
        {"properties": {"component": {"const": "Box"}, "child": {}}, "required": ["component"]}
    ]}}})))
    .unwrap()
}

#[test]
fn shape_properties_follow_all_of_and_refs() {
    let catalog = boxed();
    let shape = catalog.component("Box").unwrap().shape();
    assert_that!(names(shape.properties()))
        .is_equal_to(vec!["accessibility", "catalogId", "child", "component", "id", "metadata"]);
}

#[test]
fn shape_required_follow_all_of_and_refs() {
    let catalog = boxed();
    let shape = catalog.component("Box").unwrap().shape();
    assert_that!(names(shape.required())).is_equal_to(vec!["component", "id"]);
}

#[test]
fn of_ref_cycle_terminates_with_every_name() {
    let root = json!({"$defs": {
        "X": {"properties": {"x": {}}, "allOf": [{"$ref": "#/$defs/Y"}]},
        "Y": {"properties": {"y": {}}, "allOf": [{"$ref": "#/$defs/X"}]}
    }});
    let shape = ComponentShape::of(&root, &json!({"$ref": "#/$defs/X"}));
    assert_that!(names(shape.properties())).is_equal_to(vec!["x", "y"]);
}

#[test]
fn shape_percent_encoded_ref_is_followed() {
    let document = v10(json!({
        "components": {"A": {"$ref": "#/$defs/Base_%E2%84%93"}},
        "$defs": {"Base_ℓ": {"properties": {"b": {}}}}
    }));
    let catalog = load(document).unwrap();
    let shape = catalog.component("A").unwrap().shape();
    assert_that!(names(shape.properties())).is_equal_to(vec!["b"]);
}
```

(Catalogs with `$ref` cycles no longer load (rule 11), so the cycle test calls
`ComponentShape::of` directly: it stays cycle-safe for any schema it is handed.)

- [ ] **Step 2: Run them to see them fail**

Run: `cargo nextest run -p a2ui-runtime`
Expected: compile errors (`catalog` module, the new `bundle`, `limits` and `schema` items missing).

- [ ] **Step 3: Add the catalog errors**

In `error.rs`, add to `Inner`:

```rust
    #[snafu(display("Catalog document is not valid JSON: {message}"))]
    CatalogJson {
        message: String,
        backtrace: Backtrace,
    },

    #[snafu(display("Malformed catalog: {reason}"))]
    MalformedCatalog {
        reason: String,
        backtrace: Backtrace,
    },

    #[snafu(display("catalog_id must be provided or exist in catalog schema"))]
    MissingCatalogId { backtrace: Backtrace },

    #[snafu(display(
        "Catalog protocol version '{version}' is not supported; this runtime implements 1.0"
    ))]
    UnsupportedVersion {
        version: String,
        backtrace: Backtrace,
    },

    #[snafu(display(
        "Unsupported external $ref '{reference}' at {pointer}: a v1.0 catalog may reference \
         only itself and common_types.json"
    ))]
    ExternalRef {
        reference: String,
        pointer: String,
        backtrace: Backtrace,
    },

    #[snafu(display("Unsupported '{keyword}' at {pointer}: {reason}"))]
    ForbiddenKeyword {
        keyword: &'static str,
        pointer: String,
        reason: &'static str,
        backtrace: Backtrace,
    },

    #[snafu(display("Catalog $defs '{name}' shadows the common type of the same name"))]
    DefsShadowCommonType {
        name: String,
        backtrace: Backtrace,
    },

    #[snafu(display("Catalog exceeds {limit} {what}"))]
    CatalogLimit {
        what: String,
        limit: usize,
        backtrace: Backtrace,
    },

    #[snafu(display("Component name 'Surface' is reserved for the surface root"))]
    ReservedComponent { backtrace: Backtrace },

    #[snafu(display("Invalid UAX #31 {kind} identifier: '{name}'{context}"))]
    InvalidIdentifier {
        kind: &'static str,
        name: String,
        context: String,
        backtrace: Backtrace,
    },

    #[snafu(display(
        "Function '{function}' sets requiresUserActivation, so its allowedCallers must be \
         rendererOnly"
    ))]
    UserActivation {
        function: String,
        backtrace: Backtrace,
    },
```

and add them all to the `Category::Catalog` arm of `Inner::category`. Below
`pub struct Error(Inner);` add:

```rust
// `Result<_, Error>` stays under clippy's `result_large_err` threshold of 128 bytes.
const _: () = assert!(size_of::<Error>() <= 128);
```

The largest variant, `InvalidIdentifier`, holds a `&'static str`, two `String`s and a 48-byte
`Backtrace`: 112 bytes. `ForbiddenKeyword` names its keyword with a `&'static str`, since it is
always one of the seven literals `bundle::keyword` returns.

- [ ] **Step 4: Extend `bundle.rs`**

Its imports become:

```rust
use std::borrow::Cow;

use percent_encoding::{AsciiSet, CONTROLS, percent_decode_str, utf8_percent_encode};
use serde_json::{Map, Value, json};
use snafu::ensure;

use crate::error::{Error, ExternalRefSnafu, ForbiddenKeywordSnafu, MalformedCatalogSnafu};
```

Replace `localize_ref` (one decoding rule, in `pointer_of`: a fragment that is not a JSON
Pointer or whose escapes are not UTF-8 is now returned unchanged, and every row of Task 2's
`localize_ref` table still holds) and add the new items:

```rust
/// The sections of a catalog document whose entries a `$ref` may land in.
const ENTRY_SECTIONS: [&str; 3] = ["$defs", "components", "functions"];

/// The keywords the reference check reads when they hold a scalar.
const KEYWORDS: [&str; 7] = [
    "$anchor",
    "$dynamicAnchor",
    "$recursiveAnchor",
    "$dynamicRef",
    "$recursiveRef",
    "$id",
    "$schema",
];

const ANCHORS: &str = "anchors are not supported; use a JSON Pointer";
const DYNAMIC: &str = "dynamic references are not supported; use $ref";
const NESTED: &str = "only the catalog's top level may declare it";

/// The decoded JSON Pointer a supported `$ref` names, such as `/$defs/Child`.
///
/// `None` for an unsupported reference, for a fragment that is neither empty nor a JSON
/// Pointer (an anchor such as `#Name`), and for one whose escapes are not UTF-8.
pub(crate) fn pointer_of(reference: &str) -> Option<String> {
    let (_, fragment) = reference
        .split_once('#')
        .filter(|(_, fragment)| is_supported_ref(reference) && is_pointer_fragment(fragment))?;
    percent_decode_str(fragment).decode_utf8().ok().map(Cow::into_owned)
}

fn is_pointer_fragment(fragment: &str) -> bool {
    fragment.is_empty() || fragment.starts_with('/')
}

/// The node of `root` a supported `$ref` names, decoding its fragment.
pub(crate) fn resolve<'a>(root: &'a Value, reference: &str) -> Option<&'a Value> {
    root.pointer(&pointer_of(reference)?)
}

/// The section and decoded name of the entry a JSON Pointer falls in.
///
/// `/components/Card/properties/child` falls in `("components", "Card")`. `None` when the
/// pointer names no node inside a `$defs`, `components` or `functions` entry.
pub(crate) fn entry_of(pointer: &str) -> Option<(&'static str, String)> {
    let (section, rest) = pointer.strip_prefix('/')?.split_once('/')?;
    let section = ENTRY_SECTIONS.into_iter().find(|known| *known == section)?;
    let token = rest.split_once('/').map_or(rest, |(token, _)| token);
    Some((section, token.replace("~1", "/").replace("~0", "~")))
}

/// `reference` as a local, percent-encoded `$ref`; any other one is returned unchanged.
pub(crate) fn localize_ref(reference: &str) -> String {
    pointer_of(reference).map_or_else(|| reference.to_owned(), |pointer| local_ref(&pointer))
}

/// Checks the reference keywords of a catalog document (load rule 4).
///
/// A key holding an object or array is walked whatever its name, so a property named `$ref`
/// holding a schema is a property. A scalar under a keyword is checked: `$ref` must be a
/// supported string with an empty or JSON Pointer fragment, anchors and dynamic references are
/// refused, and `$id` and `$schema` may only sit at the top level. The walk does not tell
/// schemas from data, so a `const` or `default` holding such a key is refused too. A numeric
/// `$anchor` or `$id` is left to the schema build, whose meta-schema check rejects it.
///
/// `#` and `catalog.json#` name the whole document and pass as self-references; an entry that
/// reaches one fails load rule 11, since such a `$ref` names no entry. Pointers are built only
/// for an error. The document is at most `MAX_JSON_DEPTH` deep, so the walk recurses.
pub(crate) fn check_references(document: &Map<String, Value>) -> Result<(), Error> {
    check_map(document, &mut Vec::new())
}

/// One step of the path from the document root to the node being checked.
enum Segment<'a> {
    Key(&'a str),
    Index(usize),
}

fn check_map<'a>(map: &'a Map<String, Value>, path: &mut Vec<Segment<'a>>) -> Result<(), Error> {
    for (key, value) in map {
        path.push(Segment::Key(key));
        match value {
            Value::Object(_) | Value::Array(_) => check_node(value, path)?,
            Value::String(reference) if key == "$ref" => check_ref(reference, path)?,
            _ if key == "$ref" => {
                return Err(MalformedCatalogSnafu {
                    reason: format!("$ref at {} must be a string", pointer(path)),
                }
                .build()
                .into());
            }
            _ => check_keyword(key, value, path)?,
        }
        path.pop();
    }
    Ok(())
}

fn check_node<'a>(node: &'a Value, path: &mut Vec<Segment<'a>>) -> Result<(), Error> {
    match node {
        Value::Object(map) => check_map(map, path),
        Value::Array(items) => {
            for (index, item) in items.iter().enumerate() {
                path.push(Segment::Index(index));
                check_node(item, path)?;
                path.pop();
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn check_ref(reference: &str, path: &[Segment<'_>]) -> Result<(), Error> {
    ensure!(
        is_supported_ref(reference),
        ExternalRefSnafu {
            reference,
            pointer: pointer(path),
        }
    );
    let fragment = reference.split_once('#').map(|(_, fragment)| fragment);
    ensure!(
        fragment.is_some_and(is_pointer_fragment),
        MalformedCatalogSnafu {
            reason: format!(
                "$ref '{reference}' at {} has a fragment that is neither empty nor a JSON Pointer",
                pointer(path)
            ),
        }
    );
    ensure!(
        pointer_of(reference).is_some(),
        MalformedCatalogSnafu {
            reason: format!(
                "$ref '{reference}' at {} has a fragment that is not UTF-8",
                pointer(path)
            ),
        }
    );
    Ok(())
}

/// `key` as one of the static [`KEYWORDS`], when it is one.
fn keyword(key: &str) -> Option<&'static str> {
    KEYWORDS.into_iter().find(|keyword| *keyword == key)
}

fn check_keyword(key: &str, value: &Value, path: &[Segment<'_>]) -> Result<(), Error> {
    let Some(name) = keyword(key) else {
        return Ok(());
    };
    let reason = match (name, value) {
        ("$anchor" | "$dynamicAnchor", Value::String(_)) | ("$recursiveAnchor", Value::Bool(_)) => {
            ANCHORS
        }
        ("$dynamicRef" | "$recursiveRef", Value::String(_)) => DYNAMIC,
        ("$id" | "$schema", Value::String(_)) if path.len() > 1 => NESTED,
        _ => return Ok(()),
    };
    Err(ForbiddenKeywordSnafu {
        keyword: name,
        pointer: pointer(path),
        reason,
    }
    .build()
    .into())
}

/// The JSON Pointer of `path`, each key escaped.
fn pointer(path: &[Segment<'_>]) -> String {
    path.iter()
        .map(|segment| match segment {
            Segment::Key(key) => format!("/{}", key.replace('~', "~0").replace('/', "~1")),
            Segment::Index(index) => format!("/{index}"),
        })
        .collect()
}

/// Copies into `defs` each common type that `roots`, or a copied type, names as `#/$defs/<name>`.
///
/// `roots` must already be local. A name `defs` already holds is left alone, so a cycle
/// between common types ends; a name found in neither stays unresolved, and load rule 11
/// then reports the dangling `$ref`.
pub(crate) fn add_common_types(
    defs: &mut Map<String, Value>,
    roots: &[&Value],
    common: &Map<String, Value>,
) {
    let mut pending = Vec::new();
    for root in roots {
        def_names(root, &mut pending);
    }
    while let Some(name) = pending.pop() {
        if defs.contains_key(&name) {
            continue;
        }
        let Some(definition) = common.get(&name) else {
            continue;
        };
        let mut definition = definition.clone();
        localize_refs(&mut definition);
        def_names(&definition, &mut pending);
        defs.insert(name, definition);
    }
}

fn def_names(node: &Value, out: &mut Vec<String>) {
    match node {
        Value::Object(map) => {
            for (key, value) in map {
                match value {
                    Value::String(reference) if key == "$ref" => out.extend(def_name(reference)),
                    _ => def_names(value, out),
                }
            }
        }
        Value::Array(items) => items.iter().for_each(|item| def_names(item, out)),
        _ => {}
    }
}

/// The `$defs` entry a local `$ref` names, decoded and unescaped.
fn def_name(reference: &str) -> Option<String> {
    entry_of(&pointer_of(reference)?)
        .filter(|(section, _)| *section == "$defs")
        .map(|(_, name)| name)
}

/// A `FunctionCall` checking only the envelope: `@call`, `catalogId`, `args`, nothing else.
///
/// A call may run in another catalog, so a component check cannot judge its function; the
/// nested-call walk (Task 5) checks it against the catalog it runs in. It replaces
/// `FunctionCall` itself, not only `anyFunction` as Task 2's `any_function_envelope` does:
/// `FunctionCall` → `IndexSystemFunction` → `DynamicNumber` → `FunctionCall` is a cycle of the
/// common types, which load rule 11 rejects.
pub(crate) fn function_call_envelope() -> Value {
    json!({
        "type": "object",
        "allOf": [{"$ref": local_ref("/$defs/FunctionCommon")}],
        "properties": {"args": {"type": "object"}},
        "unevaluatedProperties": false
    })
}
```

`def_names` and the reference check recurse: their inputs are the catalog (depth-checked) and
the embedded common types.

- [ ] **Step 5: Extend `limits.rs`**

Its imports become:

```rust
use std::collections::BTreeSet;
use std::ptr;

use serde_json::Value;

use crate::bundle::{entry_of, pointer_of};
```

Add:

```rust
/// Largest catalog text [`crate::Catalog::parse`] reads, in bytes.
pub(crate) const MAX_CATALOG_BYTES: usize = 4 * 1024 * 1024;

/// Most JSON nodes, of any kind, one catalog document may hold.
pub(crate) const MAX_CATALOG_NODES: usize = 200_000;

/// Most components and functions, together, one catalog may define.
pub(crate) const MAX_CATALOG_ENTRIES: usize = 1024;

/// Most schema nodes one catalog entry may expand to once its `$ref`s are followed.
///
/// Compiling and checking an `allOf` fan-out under `unevaluatedProperties` is exponential in its
/// depth; the published basic catalog's largest entry is 4 493 nodes.
pub(crate) const MAX_ENTRY_EXPANSION: usize = 16_384;

/// Most schema nodes all entries of one catalog may expand to together.
pub(crate) const MAX_CATALOG_EXPANSION: usize = 262_144;

/// Deepest level of a catalog entry once its `$ref`s are followed, each hop one level.
///
/// It bounds the recursion of the schema build; the basic catalog reaches 23.
pub(crate) const MAX_SCHEMA_DEPTH: usize = 128;

/// Most `pattern` and `patternProperties` regexes all entries of one catalog may compile.
pub(crate) const MAX_CATALOG_PATTERNS: usize = 256;

/// Whether `value` holds more than `limit` JSON nodes, each object, array and scalar one.
///
/// The walk is iterative and counts a container's children before queuing them, so it stops
/// as soon as the count passes `limit`.
pub(crate) fn more_nodes_than(value: &Value, limit: usize) -> bool {
    let mut count = 1;
    let mut stack = vec![value];
    while let Some(node) = stack.pop() {
        count += match node {
            Value::Array(items) => items.len(),
            Value::Object(map) => map.len(),
            _ => 0,
        };
        if count > limit {
            return true;
        }
        match node {
            Value::Array(items) => stack.extend(items),
            Value::Object(map) => stack.extend(map.values()),
            _ => {}
        }
    }
    false
}

/// How one catalog entry expands once its local `$ref`s are followed.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Expansion<'a> {
    /// The entry fits the budget and [`MAX_SCHEMA_DEPTH`].
    Within {
        /// Nodes counted, a `$ref` target each time it is followed.
        size: usize,
        /// `pattern` strings and `patternProperties` keys counted.
        patterns: usize,
        /// The `(section, name)` of every entry a followed `$ref` lands in.
        reached: BTreeSet<(&'a str, String)>,
    },
    /// The count passed the budget.
    TooLarge,
    /// A node lies more than [`MAX_SCHEMA_DEPTH`] levels down, `$ref` hops included.
    TooDeep,
    /// This `$ref` leads back into a node it is being expanded from.
    Cycle(String),
    /// This `$ref` names no node inside a `$defs`, `components` or `functions` entry.
    Unresolved(String),
}

enum Step<'a> {
    Visit(&'a Value, usize),
    Enter(&'a str, &'a Value, usize),
    Leave,
}

struct Walk<'a> {
    root: &'a Value,
    budget: usize,
    size: usize,
    patterns: usize,
    reached: BTreeSet<(&'a str, String)>,
    chain: Vec<&'a Value>,
    stack: Vec<Step<'a>>,
}

/// Expands `node` with its local `$ref`s resolved in `root`, counting at most `budget` nodes.
///
/// Every object, array and scalar is one node, counted when queued; a followed `$ref` counts
/// its string, then its target's nodes again each time. A node lies one level below its
/// parent, and a `$ref` target one level below the object holding the `$ref`. The chain holds
/// `node` and the target of every `$ref` whose subtree is being walked: a `$ref` to a node on
/// it is a cycle, so every walk ends. The walk is iterative and stops at the first node past
/// `budget` or [`MAX_SCHEMA_DEPTH`], so it never holds more than `budget` queued nodes.
pub(crate) fn expand<'a>(root: &'a Value, node: &'a Value, budget: usize) -> Expansion<'a> {
    let mut walk = Walk {
        root,
        budget,
        size: 0,
        patterns: 0,
        reached: BTreeSet::new(),
        chain: vec![node],
        stack: Vec::new(),
    };
    match walk.run(node) {
        Ok(()) => Expansion::Within {
            size: walk.size,
            patterns: walk.patterns,
            reached: walk.reached,
        },
        Err(stop) => stop,
    }
}

impl<'a> Walk<'a> {
    /// Walks from `node`; `Err` carries the reason the walk stopped.
    fn run(&mut self, node: &'a Value) -> Result<(), Expansion<'a>> {
        self.push(node, 0)?;
        while let Some(step) = self.stack.pop() {
            match step {
                Step::Visit(value, depth) => self.visit(value, depth)?,
                Step::Enter(reference, target, depth) => self.enter(reference, target, depth)?,
                Step::Leave => {
                    self.chain.pop();
                }
            }
        }
        Ok(())
    }

    fn count(&mut self, depth: usize) -> Result<(), Expansion<'a>> {
        self.size += 1;
        if self.size > self.budget {
            return Err(Expansion::TooLarge);
        }
        if depth > MAX_SCHEMA_DEPTH {
            return Err(Expansion::TooDeep);
        }
        Ok(())
    }

    fn push(&mut self, node: &'a Value, depth: usize) -> Result<(), Expansion<'a>> {
        self.count(depth)?;
        if matches!(node, Value::Object(_) | Value::Array(_)) {
            self.stack.push(Step::Visit(node, depth));
        }
        Ok(())
    }

    fn visit(&mut self, node: &'a Value, depth: usize) -> Result<(), Expansion<'a>> {
        match node {
            Value::Object(map) => {
                for (key, child) in map {
                    self.count_patterns(key, child);
                    self.push(child, depth + 1)?;
                    if let Value::String(reference) = child
                        && key == "$ref"
                    {
                        self.follow(reference, depth + 1)?;
                    }
                }
            }
            Value::Array(items) => {
                for item in items {
                    self.push(item, depth + 1)?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Counts patterns wherever the keys appear, so a property of that name counts too.
    fn count_patterns(&mut self, key: &str, child: &Value) {
        match (key, child) {
            ("pattern", Value::String(_)) => self.patterns += 1,
            ("patternProperties", Value::Object(patterns)) => self.patterns += patterns.len(),
            _ => {}
        }
    }

    /// Counts the target of `reference` at `depth` and queues it for entering.
    fn follow(&mut self, reference: &'a str, depth: usize) -> Result<(), Expansion<'a>> {
        let unresolved = || Expansion::Unresolved(reference.to_owned());
        let pointer = pointer_of(reference).ok_or_else(unresolved)?;
        let entry = entry_of(&pointer).ok_or_else(unresolved)?;
        let target = self.root.pointer(&pointer).ok_or_else(unresolved)?;
        self.reached.insert(entry);
        self.count(depth)?;
        self.stack.push(Step::Enter(reference, target, depth));
        Ok(())
    }

    /// Puts `target` on the chain while its subtree is walked.
    fn enter(
        &mut self,
        reference: &'a str,
        target: &'a Value,
        depth: usize,
    ) -> Result<(), Expansion<'a>> {
        if self.chain.iter().any(|on_chain| ptr::eq(*on_chain, target)) {
            return Err(Expansion::Cycle(reference.to_owned()));
        }
        self.chain.push(target);
        self.stack.push(Step::Leave);
        if matches!(target, Value::Object(_) | Value::Array(_)) {
            self.stack.push(Step::Visit(target, depth));
        }
        Ok(())
    }
}
```

The chain changes only when a queued `Enter` or `Leave` is popped, never while a node's
children are queued, so a `$ref` sitting beside another `$ref` to the same target is no cycle.
`entry_of` maps `(section, name)` with `&'static str` sections, which `Expansion<'a>` stores as
`&'a str`.

- [ ] **Step 6: Add metadata validation to `schema.rs`**

Its imports gain `bundle::local_ref` (already there). Add:

```rust
/// The validator of a component definition's `metadata`, built on first use.
static METADATA_VALIDATOR: LazyLock<Result<Validator, String>> =
    LazyLock::new(metadata_validator);

/// Builds the validator; the error is the cause only, as for `common_validators`.
fn metadata_validator() -> Result<Validator, String> {
    let reference = local_ref("/$defs/ComponentCommon/properties/metadata");
    let top = common_document().map_err(|err| err.to_string())?;
    build(&rooted(&top, &reference))
}

/// Validates a component definition's `metadata`, which holds only UAX #31 `extensions`.
///
/// The catalog holding it is already depth-checked.
pub(crate) fn validate_component_metadata(metadata: &Value) -> Result<(), Error> {
    let validator = METADATA_VALIDATOR.as_ref().map_err(|message| embedded_error(message))?;
    check(validator, metadata)
}
```

- [ ] **Step 7: Write `catalog/document.rs`**

```rust
//! Reading the parts of a catalog document that JSON Schema does not model.

use std::collections::BTreeSet;

use serde_json::{Map, Value, json};

use crate::bundle::{local_ref, pointer_of};

const BRANCHES: [&str; 3] = ["allOf", "anyOf", "oneOf"];

/// Property names a component schema declares, at top level and in composition branches.
pub(crate) fn property_names(schema: &Map<String, Value>) -> Vec<String> {
    let mut names = Vec::new();
    schema_keys(schema, &mut names);
    names
}

/// Argument names of a function, read by the key that holds them.
///
/// `properties.args` and `args` are schemas, whose `properties` (also in composition
/// branches) name the arguments; `parameters` is a shorthand map from argument name to schema,
/// so an argument may be named `type` or `properties`.
pub(crate) fn argument_names(schema: &Map<String, Value>) -> Vec<String> {
    let nested = schema
        .get("properties")
        .and_then(|properties| properties.get("args"));
    let mut names = Vec::new();
    for arguments in [nested, schema.get("args")]
        .into_iter()
        .flatten()
        .filter_map(Value::as_object)
    {
        schema_keys(arguments, &mut names);
    }
    if let Some(parameters) = schema.get("parameters").and_then(Value::as_object) {
        names.extend(parameters.keys().cloned());
    }
    names
}

/// Collects `properties` keys of `schema` and of its composition branches.
///
/// It recurses on branches of a depth-checked catalog document.
fn schema_keys(schema: &Map<String, Value>, out: &mut Vec<String>) {
    if let Some(properties) = schema.get("properties").and_then(Value::as_object) {
        out.extend(properties.keys().cloned());
    }
    for branch in BRANCHES {
        let items = schema.get(branch).and_then(Value::as_array);
        for item in items.into_iter().flatten().filter_map(Value::as_object) {
            schema_keys(item, out);
        }
    }
}

/// A function's return type: `returnType`, or the `const` of `properties.returnType`.
pub(crate) fn return_type(schema: &Map<String, Value>) -> Option<String> {
    schema
        .get("returnType")
        .and_then(Value::as_str)
        .or_else(|| {
            schema
                .get("properties")?
                .get("returnType")?
                .get("const")?
                .as_str()
        })
        .map(str::to_owned)
}

/// The entries of `section` that `$defs.<union>` lists; `None` when it lists none.
pub(crate) fn union_members(
    defs: &Map<String, Value>,
    union: &str,
    section: &str,
) -> Option<BTreeSet<String>> {
    let prefix = format!("/{section}/");
    let members: BTreeSet<String> = defs
        .get(union)?
        .get("oneOf")?
        .as_array()?
        .iter()
        .filter_map(|item| pointer_of(item.get("$ref")?.as_str()?))
        .filter_map(|pointer| pointer.strip_prefix(prefix.as_str()).map(str::to_owned))
        .collect();
    (!members.is_empty()).then_some(members)
}

/// A `oneOf` union of local refs to `section` entries; an empty union matches nothing.
///
/// The names are sorted here, so the union never depends on the caller's order. They are
/// UAX #31 identifiers, so they need no JSON Pointer escaping; `local_ref` percent-encodes the
/// non-ASCII ones.
pub(crate) fn union(section: &str, names: &[&str], discriminator: Option<&str>) -> Value {
    if names.is_empty() {
        return json!({"not": {}});
    }
    let mut names = names.to_vec();
    names.sort_unstable();
    let refs: Vec<Value> = names
        .iter()
        .map(|name| json!({"$ref": local_ref(&format!("/{section}/{name}"))}))
        .collect();
    match discriminator {
        Some(property) => json!({"oneOf": refs, "discriminator": {"propertyName": property}}),
        None => json!({"oneOf": refs}),
    }
}

#[cfg(test)]
#[path = "document_tests.rs"]
mod tests;
```

- [ ] **Step 8: Write `catalog/shape.rs`**

```rust
//! The property names a component declares, for envelope checks and catalog comparison.

use std::collections::BTreeSet;

use serde_json::Value;

use crate::bundle::resolve;

/// The property and required names of a component schema.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ComponentShape {
    properties: BTreeSet<String>,
    required: BTreeSet<String>,
}

impl ComponentShape {
    /// Reads `schema` through `allOf` and `$ref`s resolved in `root`, each `$ref` once.
    pub(crate) fn of(root: &Value, schema: &Value) -> Self {
        let mut shape = Self::default();
        let mut seen = BTreeSet::new();
        let mut pending = vec![schema];
        while let Some(node) = pending.pop() {
            if let Some(properties) = node.get("properties").and_then(Value::as_object) {
                shape.properties.extend(properties.keys().cloned());
            }
            if let Some(required) = node.get("required").and_then(Value::as_array) {
                let names = required.iter().filter_map(Value::as_str).map(str::to_owned);
                shape.required.extend(names);
            }
            pending.extend(node.get("allOf").and_then(Value::as_array).into_iter().flatten());
            if let Some(reference) = node.get("$ref").and_then(Value::as_str)
                && seen.insert(reference)
                && let Some(target) = resolve(root, reference)
            {
                pending.push(target);
            }
        }
        shape
    }

    /// Property names, through `allOf` and `$ref`s.
    pub fn properties(&self) -> &BTreeSet<String> {
        &self.properties
    }

    /// Required property names, through `allOf` and `$ref`s.
    pub fn required(&self) -> &BTreeSet<String> {
        &self.required
    }
}

#[cfg(test)]
#[path = "shape_tests.rs"]
mod tests;
```

`resolve` accepts the authored spelling (`common_types.json#/$defs/…`) as well as the local
one, so a definition's authored schema is read directly against the validation schema, where
every common type it reaches was copied. The `seen` set keeps `of` cycle-safe for any schema,
although a loaded catalog has no `$ref` cycle (rule 11).

- [ ] **Step 9: Write `catalog/mod.rs`**

```rust
//! v1.0 catalogs: the components and functions an agent may use on a surface.
//!
//! [`Catalog::from_json`] checks an authored catalog document, keeps it for
//! [`Catalog::to_json`], bundles the self-contained [`Catalog::validation_schema`] and compiles
//! one validator per entry, which its [`ComponentDef`] or [`FunctionDef`] holds.

mod compile;
mod document;
mod entries;
mod load;
mod shape;
#[cfg(test)]
pub(crate) mod test_support;

use std::collections::BTreeMap;
use std::sync::Arc;

use jsonschema::Validator;
use serde_json::{Map, Value};

pub use shape::ComponentShape;

/// Who may call a catalog function.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum AllowedCallers {
    /// Only the renderer, from component bindings and actions (the default).
    RendererOnly,
    /// Only the agent, through a function call message.
    AgentOnly,
    /// Both the renderer and the agent.
    RendererOrAgent,
}

impl AllowedCallers {
    /// The catalog spelling, such as `"rendererOnly"`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::RendererOnly => "rendererOnly",
            Self::AgentOnly => "agentOnly",
            Self::RendererOrAgent => "rendererOrAgent",
        }
    }

    /// The value a catalog's `allowedCallers` spells as `text`, if it is one.
    pub(crate) fn from_catalog_str(text: &str) -> Option<Self> {
        match text {
            "rendererOnly" => Some(Self::RendererOnly),
            "agentOnly" => Some(Self::AgentOnly),
            "rendererOrAgent" => Some(Self::RendererOrAgent),
            _ => None,
        }
    }
}

/// One component definition of a catalog.
#[derive(Debug, Clone)]
pub struct ComponentDef {
    name: String,
    schema: Value,
    allowed_parents: Option<Vec<String>>,
    allowed_children: Option<Vec<String>>,
    metadata: Option<Value>,
    shape: ComponentShape,
    validator: Arc<Validator>,
}

impl ComponentDef {
    /// The component type name, such as `Button`.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The schema as authored in the catalog document.
    pub fn schema(&self) -> &Value {
        &self.schema
    }

    /// The component types this one may be placed under, when restricted.
    pub fn allowed_parents(&self) -> Option<&[String]> {
        self.allowed_parents.as_deref()
    }

    /// The component types this one may hold, when restricted.
    pub fn allowed_children(&self) -> Option<&[String]> {
        self.allowed_children.as_deref()
    }

    /// The definition's `metadata` envelope (vendor `extensions`), if any.
    pub fn metadata(&self) -> Option<&Value> {
        self.metadata.as_ref()
    }

    /// The property and required names, through `allOf` and `$ref`s.
    pub fn shape(&self) -> &ComponentShape {
        &self.shape
    }

    /// The compiled schema, with function calls checked only as envelopes.
    #[expect(dead_code, reason = "validate_component (Task 5) reads it")]
    pub(crate) fn validator(&self) -> &Validator {
        &self.validator
    }
}

/// One function definition of a catalog.
#[derive(Debug, Clone)]
pub struct FunctionDef {
    name: String,
    schema: Value,
    return_type: Option<String>,
    allowed_callers: AllowedCallers,
    requires_user_activation: bool,
    validator: Arc<Validator>,
}

impl FunctionDef {
    /// The function name, such as `formatString`.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The schema as authored in the catalog document.
    pub fn schema(&self) -> &Value {
        &self.schema
    }

    /// The declared return type, such as `string` or `void`.
    pub fn return_type(&self) -> Option<&str> {
        self.return_type.as_deref()
    }

    /// Who may call the function.
    pub fn allowed_callers(&self) -> AllowedCallers {
        self.allowed_callers
    }

    /// Whether a call must come from a user gesture.
    pub fn requires_user_activation(&self) -> bool {
        self.requires_user_activation
    }

    /// The compiled schema of a call, with nested calls checked only as envelopes.
    #[expect(dead_code, reason = "validate_component (Task 5) reads it")]
    pub(crate) fn validator(&self) -> &Validator {
        &self.validator
    }
}

/// A loaded v1.0 catalog; see the module documentation.
#[derive(Debug)]
pub struct Catalog {
    id: String,
    protocol_version: String,
    document: Map<String, Value>,
    components: BTreeMap<String, ComponentDef>,
    functions: BTreeMap<String, FunctionDef>,
    bundle: Value,
}

impl Catalog {
    /// The catalog id, such as the basic catalog's URL.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// The protocol version the catalog targets, `1.0` or a `1.0.N` patch.
    pub fn protocol_version(&self) -> &str {
        &self.protocol_version
    }

    /// The component definition named `name`.
    pub fn component(&self, name: &str) -> Option<&ComponentDef> {
        self.components.get(name)
    }

    /// Every component definition, by name.
    pub fn components(&self) -> impl Iterator<Item = &ComponentDef> {
        self.components.values()
    }

    /// The function definition named `name`.
    pub fn function(&self, name: &str) -> Option<&FunctionDef> {
        self.functions.get(name)
    }

    /// Every function definition, by name.
    pub fn functions(&self) -> impl Iterator<Item = &FunctionDef> {
        self.functions.values()
    }

    /// The `theme` schema of the document, if any.
    pub fn theme(&self) -> Option<&Value> {
        self.document.get("theme")
    }

    /// The `instructions` for agents, if any.
    pub fn instructions(&self) -> Option<&str> {
        self.document.get("instructions").and_then(Value::as_str)
    }

    /// The self-contained schema: kept entries, `$defs` and the common types they reach.
    ///
    /// Until Plan 2b it is the unflattened bundle, not upstream's `validation_schema` golden.
    pub fn validation_schema(&self) -> &Value {
        &self.bundle
    }
}
```

In `lib.rs` add `mod catalog;`,
`pub use catalog::{AllowedCallers, Catalog, ComponentDef, ComponentShape, FunctionDef};`, and to
the crate doc's key items: `` [`Catalog`] (a loaded v1.0 catalog, with [`ComponentDef`],
[`FunctionDef`] and [`AllowedCallers`]) ``.

The validators live in the definitions, behind an `Arc` so the definitions stay cheap to
clone (`jsonschema::Validator`'s `Clone` is a deep copy of the compiled tree). Nothing in this
task reads them, so each `validator()` accessor carries a `dead_code` expectation (an unread
accessor also leaves its field unread; the expectation on the method covers both, as probed). Task 5's `validate_component` reads
both accessors and deletes both expectations; no test of this task calls them, so the
expectations hold in test builds too.

- [ ] **Step 10: Write `catalog/entries.rs`**

```rust
//! Component and function attributes read from a catalog document (load rules 6 to 8).

use std::sync::Arc;

use jsonschema::Validator;
use serde_json::{Map, Value};
use snafu::{OptionExt, ensure};

use super::document::{argument_names, property_names, return_type};
use super::{AllowedCallers, ComponentDef, ComponentShape, FunctionDef};
use crate::error::{
    Error, InvalidIdentifierSnafu, MalformedCatalogSnafu, ReservedComponentSnafu,
    UserActivationSnafu,
};
use crate::ident::is_identifier;
use crate::schema::validate_component_metadata;

/// A checked component definition, before its shape and validator exist.
pub(super) struct ComponentAttrs {
    name: String,
    schema: Value,
    allowed_parents: Option<Vec<String>>,
    allowed_children: Option<Vec<String>>,
    metadata: Option<Value>,
}

impl ComponentAttrs {
    /// Checks and reads the component `name` of the document.
    pub(super) fn read(name: &str, schema: &Value) -> Result<Self, Error> {
        ensure!(name != "Surface", ReservedComponentSnafu);
        check_identifier("component", name, None)?;
        let map = entry("component", name, schema)?;
        for property in property_names(map) {
            check_identifier("property", &property, Some(("component", name)))?;
        }
        let metadata = map.get("metadata").cloned();
        if let Some(metadata) = &metadata {
            validate_component_metadata(metadata)?;
        }
        Ok(Self {
            name: name.to_owned(),
            schema: schema.clone(),
            allowed_parents: names_list(map, "allowedParents", name)?,
            allowed_children: names_list(map, "allowedChildren", name)?,
            metadata,
        })
    }

    /// The component type name.
    pub(super) fn name(&self) -> &str {
        &self.name
    }

    /// The schema as authored.
    pub(super) fn schema(&self) -> &Value {
        &self.schema
    }

    /// The definition, completed with its shape and validator.
    pub(super) fn into_def(
        self,
        shape: ComponentShape,
        validator: Arc<Validator>,
    ) -> ComponentDef {
        ComponentDef {
            name: self.name,
            schema: self.schema,
            allowed_parents: self.allowed_parents,
            allowed_children: self.allowed_children,
            metadata: self.metadata,
            shape,
            validator,
        }
    }
}

/// A checked function definition, before its validator exists.
pub(super) struct FunctionAttrs {
    name: String,
    schema: Value,
    return_type: Option<String>,
    allowed_callers: AllowedCallers,
    requires_user_activation: bool,
}

impl FunctionAttrs {
    /// Checks and reads the function `name` of the document.
    pub(super) fn read(name: &str, schema: &Value) -> Result<Self, Error> {
        check_identifier("function", name, None)?;
        let map = entry("function", name, schema)?;
        for argument in argument_names(map) {
            check_identifier("argument", &argument, Some(("function", name)))?;
        }
        let allowed_callers = match map.get("allowedCallers") {
            None => AllowedCallers::RendererOnly,
            Some(value) => value
                .as_str()
                .and_then(AllowedCallers::from_catalog_str)
                .with_context(|| MalformedCatalogSnafu {
                    reason: format!("function '{name}' has an unknown allowedCallers"),
                })?,
        };
        let requires_user_activation = match map.get("requiresUserActivation") {
            None => false,
            Some(value) => value.as_bool().with_context(|| MalformedCatalogSnafu {
                reason: format!("requiresUserActivation of function '{name}' must be a boolean"),
            })?,
        };
        ensure!(
            !requires_user_activation || allowed_callers == AllowedCallers::RendererOnly,
            UserActivationSnafu { function: name }
        );
        Ok(Self {
            name: name.to_owned(),
            schema: schema.clone(),
            return_type: return_type(map),
            allowed_callers,
            requires_user_activation,
        })
    }

    /// The function name.
    pub(super) fn name(&self) -> &str {
        &self.name
    }

    /// The schema as authored.
    pub(super) fn schema(&self) -> &Value {
        &self.schema
    }

    /// The definition, completed with its validator.
    pub(super) fn into_def(self, validator: Arc<Validator>) -> FunctionDef {
        FunctionDef {
            name: self.name,
            schema: self.schema,
            return_type: self.return_type,
            allowed_callers: self.allowed_callers,
            requires_user_activation: self.requires_user_activation,
            validator,
        }
    }
}

/// Checks `name` is a UAX #31 identifier; `owner` is the `(kind, name)` declaring it.
///
/// `ensure!` builds the context only on failure, so a valid name formats nothing.
fn check_identifier(
    kind: &'static str,
    name: &str,
    owner: Option<(&'static str, &str)>,
) -> Result<(), Error> {
    ensure!(
        is_identifier(name),
        InvalidIdentifierSnafu {
            kind,
            name,
            context: owner
                .map(|(owner_kind, owner)| format!(" in {owner_kind} '{owner}'"))
                .unwrap_or_default(),
        }
    );
    Ok(())
}

fn entry<'a>(kind: &str, name: &str, schema: &'a Value) -> Result<&'a Map<String, Value>, Error> {
    Ok(schema.as_object().with_context(|| MalformedCatalogSnafu {
        reason: format!("{kind} '{name}' must be a schema object"),
    })?)
}

fn names_list(
    map: &Map<String, Value>,
    key: &str,
    component: &str,
) -> Result<Option<Vec<String>>, Error> {
    let Some(value) = map.get(key) else {
        return Ok(None);
    };
    let names = value
        .as_array()
        .and_then(|items| {
            items
                .iter()
                .map(|item| item.as_str().map(str::to_owned))
                .collect::<Option<Vec<_>>>()
        })
        .with_context(|| MalformedCatalogSnafu {
            reason: format!("`{key}` of component '{component}' must be a list of names"),
        })?;
    Ok(Some(names))
}

#[cfg(test)]
#[path = "entries_tests.rs"]
mod tests;
```

(`OptionExt::with_context` yields the private `Inner`; `?` converts it into `Error` through the
`From<Inner>` impl Plan 1 defines, hence the `Ok(…?)` shape in `entry`. `unwrap_or_default`
turns "no owner" into an empty context: it runs only on the failure path. `entries.rs` is a
child of `catalog`, so `into_def` may fill the private fields of `ComponentDef` and
`FunctionDef`.)

- [ ] **Step 11: Write `catalog/compile.rs`**

```rust
//! The validation schema and the per-entry validators (load rules 10 and 11).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use jsonschema::Validator;
use serde_json::{Map, Value};
use snafu::ensure;

use super::document::union;
use super::entries::{ComponentAttrs, FunctionAttrs};
use crate::bundle::{add_common_types, function_call_envelope, local_ref, localize_refs};
use crate::error::{CatalogLimitSnafu, Error, MalformedCatalogSnafu};
use crate::limits::{
    Expansion, MAX_CATALOG_EXPANSION, MAX_CATALOG_PATTERNS, MAX_ENTRY_EXPANSION,
    MAX_SCHEMA_DEPTH, expand,
};
use crate::schema::common_type_defs;
use crate::validator::{DRAFT_2020_12, compile, rooted};

/// The self-contained schema of the kept entries, `defs` and the common types they reach.
pub(super) fn bundle(
    id: &str,
    version: &str,
    components: &BTreeMap<String, ComponentAttrs>,
    functions: &BTreeMap<String, FunctionAttrs>,
    defs: Option<&Map<String, Value>>,
) -> Result<Map<String, Value>, Error> {
    let mut defs = defs.cloned().unwrap_or_default();
    if !defs.contains_key("anyComponent") {
        let names: Vec<&str> = components.keys().map(String::as_str).collect();
        defs.insert(
            "anyComponent".to_owned(),
            union("components", &names, Some("component")),
        );
    }
    if !defs.contains_key("anyFunction") {
        let names: Vec<&str> = functions.keys().map(String::as_str).collect();
        defs.insert("anyFunction".to_owned(), union("functions", &names, None));
    }
    defs.values_mut().for_each(localize_refs);
    let components = localized(components.values().map(|attrs| (attrs.name(), attrs.schema())));
    let functions = localized(functions.values().map(|attrs| (attrs.name(), attrs.schema())));
    let own_defs = Value::Object(defs.clone());
    let roots: Vec<&Value> = components
        .values()
        .chain(functions.values())
        .chain([&own_defs])
        .collect();
    add_common_types(&mut defs, &roots, common_type_defs()?);
    Ok(Map::from_iter([
        ("$schema".to_owned(), Value::from(DRAFT_2020_12)),
        ("catalogId".to_owned(), Value::from(id)),
        ("protocolVersion".to_owned(), Value::from(version)),
        ("components".to_owned(), Value::Object(components)),
        ("functions".to_owned(), Value::Object(functions)),
        ("$defs".to_owned(), Value::Object(defs)),
    ]))
}

fn localized<'a>(entries: impl Iterator<Item = (&'a str, &'a Value)>) -> Map<String, Value> {
    entries
        .map(|(name, schema)| {
            let mut schema = schema.clone();
            localize_refs(&mut schema);
            (name.to_owned(), schema)
        })
        .collect()
}

/// The validator top of a bundle whose kept entries all passed the expansion caps.
pub(super) struct Compiler {
    root: Value,
}

impl Compiler {
    /// Checks every kept entry, as `(name, authored schema)`, against load rule 11.
    ///
    /// All entries pass before any compiles: compiling an oversized one could take minutes.
    pub(super) fn new<'a>(
        bundle: &Map<String, Value>,
        entries: impl IntoIterator<Item = (&'a str, &'a Value)>,
    ) -> Result<Self, Error> {
        let root = validator_top(bundle);
        let (mut size, mut patterns) = (0, 0);
        for (name, schema) in entries {
            let reach = reach(&root, name, schema)?;
            size += reach.size;
            patterns += reach.patterns;
            ensure!(
                size <= MAX_CATALOG_EXPANSION,
                CatalogLimitSnafu {
                    what: "schema nodes in all entries once their $refs are expanded",
                    limit: MAX_CATALOG_EXPANSION,
                }
            );
            ensure!(
                patterns <= MAX_CATALOG_PATTERNS,
                CatalogLimitSnafu {
                    what: "patterns",
                    limit: MAX_CATALOG_PATTERNS,
                }
            );
        }
        Ok(Self { root })
    }

    /// Compiles the entry `name` of `section` against a document pruned to what it reaches.
    ///
    /// The pruned document holds `$schema`, the entry and each `$defs`, `components` or
    /// `functions` entry its `$ref`s land in, so a build costs the entry, not the catalog.
    pub(super) fn compile(
        &self,
        section: &'static str,
        name: &str,
        schema: &Value,
    ) -> Result<Arc<Validator>, Error> {
        let reach = reach(&self.root, name, schema)?;
        let mut entry = schema.clone();
        localize_refs(&mut entry);
        let mut sections: BTreeMap<&str, Map<String, Value>> = BTreeMap::new();
        sections
            .entry(section)
            .or_default()
            .insert(name.to_owned(), entry);
        for (target_section, target) in reach.targets {
            let definition = self
                .root
                .get(target_section)
                .and_then(|entries| entries.get(target.as_str()));
            if let Some(definition) = definition {
                sections
                    .entry(target_section)
                    .or_default()
                    .insert(target, definition.clone());
            }
        }
        let top: Map<String, Value> = sections
            .into_iter()
            .map(|(key, entries)| (key.to_owned(), Value::Object(entries)))
            .collect();
        let reference = local_ref(&format!("/{section}/{name}"));
        Ok(Arc::new(compile(&rooted(&top, &reference))?))
    }
}

/// `bundle` with `FunctionCall` reduced to its envelope, as the root `$ref`s resolve in.
///
/// `FunctionCall` is present only when an entry reaches it, and its own definition names
/// `FunctionCommon`, so the common-type closure already copied the envelope's one target.
fn validator_top(bundle: &Map<String, Value>) -> Value {
    let mut top = Value::Object(bundle.clone());
    if let Some(call) = top.pointer_mut("/$defs/FunctionCall") {
        *call = function_call_envelope();
    }
    top
}

/// What one entry reaches once its `$ref`s are followed.
struct Reach<'a> {
    size: usize,
    patterns: usize,
    targets: BTreeSet<(&'a str, String)>,
}

/// Expands the entry `name` in `root`; each way it breaks load rule 11 is a `CatalogError`.
fn reach<'a>(root: &'a Value, name: &str, schema: &'a Value) -> Result<Reach<'a>, Error> {
    match expand(root, schema, MAX_ENTRY_EXPANSION) {
        Expansion::Within {
            size,
            patterns,
            reached,
        } => Ok(Reach {
            size,
            patterns,
            targets: reached,
        }),
        Expansion::TooLarge => Err(CatalogLimitSnafu {
            what: format!("schema nodes in '{name}' once its $refs are expanded"),
            limit: MAX_ENTRY_EXPANSION,
        }
        .build()
        .into()),
        Expansion::TooDeep => Err(CatalogLimitSnafu {
            what: format!("levels of '{name}' once its $refs are expanded"),
            limit: MAX_SCHEMA_DEPTH,
        }
        .build()
        .into()),
        Expansion::Cycle(reference) => Err(MalformedCatalogSnafu {
            reason: format!("$ref cycle through '{reference}'"),
        }
        .build()
        .into()),
        Expansion::Unresolved(reference) => Err(MalformedCatalogSnafu {
            reason: format!("$ref '{reference}' does not resolve"),
        }
        .build()
        .into()),
    }
}

#[cfg(test)]
#[path = "compile_tests.rs"]
mod tests;
```

Notes:
- `ensure!` builds its context only on failure, so its `format!` costs nothing on success;
  `unwrap_or_default` on the optional `$defs` is the documented "no `$defs`" case.
- Same-instance `$ref` cycles are rejected: checking an instance against a cycle with fan-out
  (an `allOf` of two `$ref`s back into it) is exponential in the instance's depth, and the
  expansion count, which follows each `$ref` once per chain, cannot bound it.
- `Compiler::compile` expands the entry a second time instead of keeping up to 1024 reach sets
  keyed by name: phase 1 already bounded each expansion to 16 384 nodes, and a name lookup
  would add a "missing" state no input can produce. Every target it lists was resolved in
  `root` by that expansion, so each lookup finds it; were one absent, the pruned document would
  keep a dangling `$ref` and the build would report it.
- Each entry is expanded from its authored schema: `pointer_of` reads authored spellings
  (`common_types.json#/$defs/…`) as well as local ones. A `$ref` back to the entry's own
  pointer reaches the entry's copy in `root`, which the chain catches one hop later.

- [ ] **Step 12: Write `catalog/load.rs`**

```rust
//! Loading a catalog document (load rules 1 to 9 and 12) and writing it back.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Value};
use snafu::{OptionExt, ensure};

use super::compile::{Compiler, bundle};
use super::document::union_members;
use super::entries::{ComponentAttrs, FunctionAttrs};
use super::{Catalog, ComponentShape};
use crate::bundle::check_references;
use crate::error::{
    CatalogJsonSnafu, CatalogLimitSnafu, DefsShadowCommonTypeSnafu, Error, MalformedCatalogSnafu,
    MissingCatalogIdSnafu, UnsupportedVersionSnafu,
};
use crate::limits::{
    MAX_CATALOG_BYTES, MAX_CATALOG_ENTRIES, MAX_CATALOG_NODES, MAX_JSON_DEPTH, deeper_than,
    more_nodes_than,
};
use crate::schema::common_type_defs;

/// The version a catalog targets when it declares none (`catalog_definition.json` default).
const DEFAULT_VERSION: &str = "0.9";

/// The kept components and functions, by name, before compiling.
type KeptEntries = (
    BTreeMap<String, ComponentAttrs>,
    BTreeMap<String, FunctionAttrs>,
);

impl Catalog {
    /// Loads a catalog document; `catalog_id` and `protocol_version` override its own.
    pub fn from_json(
        document: &Value,
        catalog_id: Option<&str>,
        protocol_version: Option<&str>,
    ) -> Result<Self, Error> {
        ensure!(
            !deeper_than(document, MAX_JSON_DEPTH),
            CatalogLimitSnafu {
                what: "levels of nesting",
                limit: MAX_JSON_DEPTH,
            }
        );
        ensure!(
            !more_nodes_than(document, MAX_CATALOG_NODES),
            CatalogLimitSnafu {
                what: "JSON nodes",
                limit: MAX_CATALOG_NODES,
            }
        );
        let document = document.as_object().context(MalformedCatalogSnafu {
            reason: "a catalog document is a JSON object",
        })?;
        let id = match catalog_id {
            Some(id) => id,
            None => text(document, "catalogId")?.context(MissingCatalogIdSnafu)?,
        };
        ensure!(!id.is_empty(), MissingCatalogIdSnafu);
        let version = match protocol_version {
            Some(version) => version,
            None => text(document, "protocolVersion")?.unwrap_or(DEFAULT_VERSION),
        };
        ensure!(is_supported_version(version), UnsupportedVersionSnafu { version });
        check_references(document)?;
        let defs = section(document, "$defs")?;
        check_defs(defs)?;
        let (components, functions) = kept_entries(document, defs)?;
        let bundled = bundle(id, version, &components, &functions, defs)?;
        let entries = components
            .values()
            .map(|attrs| (attrs.name(), attrs.schema()))
            .chain(functions.values().map(|attrs| (attrs.name(), attrs.schema())));
        let compiler = Compiler::new(&bundled, entries)?;
        let bundle = Value::Object(bundled);
        let components = components
            .into_iter()
            .map(|(name, attrs)| {
                let validator = compiler.compile("components", &name, attrs.schema())?;
                let shape = ComponentShape::of(&bundle, attrs.schema());
                Ok((name, attrs.into_def(shape, validator)))
            })
            .collect::<Result<BTreeMap<_, _>, Error>>()?;
        let functions = functions
            .into_iter()
            .map(|(name, attrs)| {
                let validator = compiler.compile("functions", &name, attrs.schema())?;
                Ok((name, attrs.into_def(validator)))
            })
            .collect::<Result<BTreeMap<_, _>, Error>>()?;
        Ok(Self {
            id: id.to_owned(),
            protocol_version: version.to_owned(),
            document: document.clone(),
            components,
            functions,
            bundle,
        })
    }

    /// Parses JSON text of at most 4 MiB, then loads it as [`Catalog::from_json`] does.
    pub fn parse(
        text: &str,
        catalog_id: Option<&str>,
        protocol_version: Option<&str>,
    ) -> Result<Self, Error> {
        ensure!(
            text.len() <= MAX_CATALOG_BYTES,
            CatalogLimitSnafu {
                what: "bytes of catalog text",
                limit: MAX_CATALOG_BYTES,
            }
        );
        let document: Value = serde_json::from_str(text).map_err(|err| -> Error {
            CatalogJsonSnafu {
                message: err.to_string(),
            }
            .build()
            .into()
        })?;
        Self::from_json(&document, catalog_id, protocol_version)
    }

    /// The authored document with only the kept entries, and the effective id and version.
    pub fn to_json(&self) -> Value {
        Value::Object(self.kept_document())
    }

    /// The map [`Catalog::to_json`] returns; `derive` starts from it.
    pub(super) fn kept_document(&self) -> Map<String, Value> {
        let mut document = self.document.clone();
        retain(&mut document, "components", |name| {
            self.components.contains_key(name)
        });
        retain(&mut document, "functions", |name| {
            self.functions.contains_key(name)
        });
        document.insert("catalogId".to_owned(), Value::from(self.id.as_str()));
        document.insert(
            "protocolVersion".to_owned(),
            Value::from(self.protocol_version.as_str()),
        );
        document
    }
}

fn text<'a>(document: &'a Map<String, Value>, key: &str) -> Result<Option<&'a str>, Error> {
    match document.get(key) {
        None => Ok(None),
        Some(Value::String(text)) => Ok(Some(text)),
        Some(_) => Err(MalformedCatalogSnafu {
            reason: format!("`{key}` must be a string"),
        }
        .build()
        .into()),
    }
}

fn is_supported_version(version: &str) -> bool {
    version == "1.0"
        || version.strip_prefix("1.0.").is_some_and(|patch| {
            !patch.is_empty() && patch.bytes().all(|byte| byte.is_ascii_digit())
        })
}

fn section<'a>(
    document: &'a Map<String, Value>,
    key: &str,
) -> Result<Option<&'a Map<String, Value>>, Error> {
    match document.get(key) {
        None => Ok(None),
        Some(Value::Object(map)) => Ok(Some(map)),
        Some(_) => Err(MalformedCatalogSnafu {
            reason: format!("`{key}` must be an object"),
        }
        .build()
        .into()),
    }
}

fn check_defs(defs: Option<&Map<String, Value>>) -> Result<(), Error> {
    let common = common_type_defs()?;
    match defs.into_iter().flatten().find(|(name, _)| common.contains_key(*name)) {
        Some((name, _)) => Err(DefsShadowCommonTypeSnafu { name }.build().into()),
        None => Ok(()),
    }
}

/// Reads every component and function, then keeps those the unions list (rules 6 to 9).
fn kept_entries(
    document: &Map<String, Value>,
    defs: Option<&Map<String, Value>>,
) -> Result<KeptEntries, Error> {
    let components = section(document, "components")?;
    let functions = section(document, "functions")?;
    let count = components.map_or(0, Map::len) + functions.map_or(0, Map::len);
    ensure!(
        count <= MAX_CATALOG_ENTRIES,
        CatalogLimitSnafu {
            what: "components and functions entries",
            limit: MAX_CATALOG_ENTRIES,
        }
    );
    let components = components
        .into_iter()
        .flatten()
        .map(|(name, schema)| ComponentAttrs::read(name, schema))
        .collect::<Result<Vec<_>, _>>()?;
    let functions = functions
        .into_iter()
        .flatten()
        .map(|(name, schema)| FunctionAttrs::read(name, schema))
        .collect::<Result<Vec<_>, _>>()?;
    let no_defs = Map::new();
    let defs = defs.unwrap_or(&no_defs);
    Ok((
        kept(
            components,
            union_members(defs, "anyComponent", "components"),
            ComponentAttrs::name,
        ),
        kept(
            functions,
            union_members(defs, "anyFunction", "functions"),
            FunctionAttrs::name,
        ),
    ))
}

/// The entries a union lists (all when `members` is `None`), by name.
///
/// `name` is a method path such as `ComponentAttrs::name`: a fn item is generic over the
/// borrow, which a closure returning a reference is not.
fn kept<T>(
    entries: Vec<T>,
    members: Option<BTreeSet<String>>,
    name: fn(&T) -> &str,
) -> BTreeMap<String, T> {
    entries
        .into_iter()
        .filter(|entry| members.as_ref().is_none_or(|members| members.contains(name(entry))))
        .map(|entry| (name(&entry).to_owned(), entry))
        .collect()
}

fn retain(document: &mut Map<String, Value>, key: &str, keep: impl Fn(&str) -> bool) {
    if let Some(Value::Object(entries)) = document.get_mut(key) {
        entries.retain(|name, _| keep(name));
    }
}

#[cfg(test)]
#[path = "load_tests.rs"]
mod tests;
```

Notes for the implementer:
- `unwrap_or(DEFAULT_VERSION)` is the documented default of rule 3 (clippy's `unwrap_used`
  does not cover it); `defs.unwrap_or(&no_defs)` reads "no `$defs`" as an empty map.
- Writing the effective `catalogId` and `protocolVersion` always (also when the document has
  neither, or declares others) makes `from_json(to_json())` reload without the overrides. The
  v1.0 round-trip catalogs declare both, and the cases' overrides equal them.
- The depth check runs first, so the node count and every later walk see at most 64 levels.
- `let` chains and `Option::is_none_or` are stable on the workspace MSRV (1.88).

- [ ] **Step 13: Run the runtime tests**

Run: `cargo nextest run -p a2ui-runtime`
Expected: all pass.

- [ ] **Step 14: Write the harness input helper and the `round_trip` handler**

Create `crates/a2ui-conformance/src/handlers/catalog_input.rs`:

```rust
//! The catalog a case names, and the overrides it passes to `Catalog::from_json`.

use a2ui_runtime::Catalog;
use serde_json::Value;

use crate::case::Case;
use crate::paths::{read_json, upstream_root};

/// The case's catalog document: exactly one of inline `catalog` or `catalogPath`.
pub fn document(case: &Case) -> Result<Value, String> {
    match (case.get("catalog"), case.get("catalogPath")) {
        (Some(catalog), None) => Ok(catalog.clone()),
        (None, Some(Value::String(path))) => read_json(&upstream_root()?.join(path)),
        (None, Some(_)) => Err(format!("{}: `catalogPath` must be a string", case.at())),
        _ => Err(format!(
            "{}: a case needs exactly one of `catalog` and `catalogPath`",
            case.at()
        )),
    }
}

/// Loads `document` with the case's top-level `catalogId` and `protocolVersion` overrides.
///
/// The outer `Err` is a malformed case; the inner one is the runtime's answer.
pub fn load(case: &Case, document: &Value) -> Result<Result<Catalog, a2ui_runtime::Error>, String> {
    let text = |key: &str| match case.get(key) {
        None => Ok(None),
        Some(Value::String(value)) => Ok(Some(value.as_str())),
        Some(_) => Err(format!("{}: `{key}` must be a string", case.at())),
    };
    Ok(Catalog::from_json(
        document,
        text("catalogId")?,
        text("protocolVersion")?,
    ))
}
```

Create `crates/a2ui-conformance/src/handlers/round_trip.rs`:

```rust
//! The `round_trip` action: `from_json(c).to_json()` equals `c`, and stays stable.

use crate::case::Case;
use crate::compare::json_eq;
use crate::handlers::catalog_input::{document, load};
use crate::keys::assert_case_keys;

/// Loads the case's catalog, serializes it twice and compares both with the input.
pub fn run(case: &Case) -> Result<(), String> {
    assert_case_keys(case, &["protocolVersion", "catalogId", "catalog", "catalogPath"])?;
    let at = case.at();
    let input = document(case)?;
    let first = load(case, &input)?
        .map_err(|err| format!("{at}: load failed: {err}"))?
        .to_json();
    if !json_eq(&first, &input) {
        return Err(format!("{at}: to_json differs from the input"));
    }
    let second = load(case, &first)?
        .map_err(|err| format!("{at}: reload failed: {err}"))?
        .to_json();
    if json_eq(&second, &first) {
        Ok(())
    } else {
        Err(format!("{at}: a second round trip changed the document"))
    }
}
```

`handlers/mod.rs`: `mod catalog_input;`, `mod round_trip;`, `"round_trip" => Some(round_trip::run),`.
Remove `round_trip` from the `[actions]` table of `skips.toml`.

- [ ] **Step 15: Run the round-trip cases**

Run: `cargo nextest run -p a2ui-conformance --test core round_trip`
Expected: passed: `test_v10_basic_catalog_round_trip`, `test_v10_mcp_catalog_round_trip`,
`test_v10_custom_catalog_round_trip_function_metadata`,
`test_v10_custom_catalog_round_trip_leaf_defs`, `test_v10_component_metadata_catalog_round_trip`;
the v0.8, v0.9 and v0.9.1 ones (`test_mcp_catalog_round_trip` included) ignored through
`[versions]`.

- [ ] **Step 16: Update the architecture notes**

`crates/a2ui-runtime/ARCHITECTURE.md`, add under Modules (keep the file at most 50 lines):

```markdown
- `catalog`: `Catalog::from_json` checks a v1.0 catalog (depth, nodes, id, version, `$ref`
  rules, UAX #31 names, callers, metadata, entry caps), keeps the entries its unions list and
  the authored document (`to_json`), bundles `validation_schema` (unflattened until Plan 2b)
  and compiles each entry against a document pruned to what it reaches; the validator lives
  in its `ComponentDef`/`FunctionDef`. Submodules `load`, `entries`, `compile`, `document`,
  `shape`.
- `bundle`, `limits`: `$ref` rules (no anchors, dynamic refs or nested `$id`), percent-encoded
  local refs, common-type closure; caps, and `expand`, which follows `$ref`s and rejects
  cycles, depth past 128 and oversized or pattern-heavy entries before any compile.
```

`crates/a2ui-conformance/ARCHITECTURE.md`: extend `handlers` with `catalog_input` (the case's
catalog and overrides) and `round_trip`.

- [ ] **Step 17: Gate and commit**

Run: `just check` then `just test`. Expected: green.

```bash
git add crates/a2ui-runtime crates/a2ui-conformance
```

Message (`git commit -F <file>`, with the `Reviewed-by:` trailer):

```
feat(runtime): load, check and serialize v1.0 catalogs (t-1557)

Catalog::from_json enforces the v1.0 catalog rules and size caps, rejects $ref cycles,
anchors and dynamic references, keeps the entries the unions list, bundles a self-contained
validation schema and compiles each entry against a document pruned to what it reaches;
to_json returns the authored document with the effective id and version. The round_trip
cases run.
```

Then `oku cli task done --project slider --id t-1557 --result "<one line>"`.

---
### Task 5: Component validation with nested calls

Oku: t-1558.

**Files:**
- Create: `crates/a2ui-runtime/src/catalog/{validate.rs,validate_tests.rs}`
- Modify: `crates/a2ui-runtime/src/{error.rs,lib.rs,catalog/mod.rs}`,
  `crates/a2ui-runtime/ARCHITECTURE.md`

**Interfaces:**
- Consumes: `Catalog` and its `components` / `functions` maps, `ComponentDef::shape()`,
  `ComponentDef::validator()`, `FunctionDef::validator()`, `FunctionDef::allowed_callers()`,
  `AllowedCallers` (Task 4), `catalog::test_support::load` (Task 4),
  `validator::check`, `ident::is_identifier`, `limits::{MAX_JSON_DEPTH, deeper_than}`,
  `validate_common_type` (Task 2), `schema::validate_component_metadata` (Task 4).
- Produces:
  - `pub trait CatalogLookup { fn catalog(&self, id: &str) -> Option<&Catalog>; }`, implemented
    for `Catalog` (finds itself) and for `[Catalog]` (finds any of its catalogs).
  - `Catalog::validate_component(&self, component: &Value, default_catalog: &str,
    catalogs: &(impl CatalogLookup + ?Sized)) -> Result<(), Error>` (errors in
    `Category::Validation`). Callers pass `&catalog`, `&catalogs[..]` or a
    `&dyn CatalogLookup`; the Task 6 harness passes `catalog.id(), catalog`.

The lookup is taken as `&(impl CatalogLookup + ?Sized)` rather than `&dyn CatalogLookup`: a
`&[Catalog]` cannot coerce to `&dyn CatalogLookup` (a slice is unsized), and the generic form
accepts `&Catalog`, `&[Catalog]` and `&dyn CatalogLookup` alike.

Rules (`payload_validator.py`, catalog.yaml 1c):

1. The component nests at most `MAX_JSON_DEPTH` levels (checked first: the walk below recurses).
2. It is an object; none of its own keys is reserved (rule 6).
3. Its `component` is an identifier naming a kept component.
4. Envelope keys (`id`, `component`, `catalogId`, `metadata`) its shape does not declare are
   removed before the schema check (a closed schema, `unevaluatedProperties: false`, would
   reject them otherwise): a removed `id` or `catalogId` must be a string, a removed `metadata`
   must match `ComponentCommon.metadata`. Declared ones are left to the schema.
5. The component's validator (`ComponentDef::validator()`) runs; calls are checked there only
   as envelopes (Task 4).
6. Every value under a non-envelope key is then walked, through objects and arrays:
   - a key starting with one `@` other than `@path` and `@call` is reserved (bare `@` too);
   - an object with `@call` is a call: `@call` a string, `catalogId` a string when present,
     `args` an object whose keys are identifiers when present;
   - a `@` name is a system function: no `catalogId`, only `@index`, checked as the common type
     `IndexSystemFunction`;
   - any other name is an identifier. The call runs in its `catalogId`, else in
     `default_catalog` (the surface's). That catalog is this one when the ids match, else the
     one `catalogs` finds. When found, the function must exist there, must not be `agentOnly`,
     and the call without its `catalogId` must match `FunctionDef::validator()`; when not
     found, only the identifiers above are checked;
   - the walk continues inside every call's `args`, whichever catalog it runs in; a nested
     call without `catalogId` runs in `default_catalog`, not in its parent's catalog.

- [ ] **Step 1: Write the failing tests**

Create `crates/a2ui-runtime/src/catalog/validate_tests.rs`:

```rust
use rstest::rstest;
use serde_json::{Value, json};
use speculoos::prelude::*;

use super::super::test_support::{load, nested};
use crate::limits::MAX_JSON_DEPTH;
use crate::{Catalog, Category, Error};

const ID: &str = "https://example.com/catalog";
const SECOND: &str = "https://example.com/second";

fn catalog() -> Catalog {
    let string = json!({"$ref": "common_types.json#/$defs/DynamicString"});
    load(json!({
        "catalogId": ID,
        "protocolVersion": "1.0",
        "components": {
            "Text": {
                "type": "object",
                "properties": {"component": {"const": "Text"}, "text": string},
                "required": ["component", "text"]
            },
            "Closed": {
                "type": "object",
                "properties": {"component": {"const": "Closed"}, "label": {"type": "string"}},
                "unevaluatedProperties": false
            },
            "Field": {
                "type": "object",
                "allOf": [{"$ref": "common_types.json#/$defs/ComponentCommon"}],
                "properties": {"component": {"const": "Field"}, "value": string},
                "required": ["component", "value"],
                "unevaluatedProperties": false
            },
            "Any": {"type": "object", "properties": {"component": {"const": "Any"}}}
        },
        "functions": {
            "upper": {
                "type": "object",
                "returnType": "string",
                "properties": {
                    "@call": {"const": "upper"},
                    "args": {
                        "type": "object",
                        "properties": {"value": string},
                        "required": ["value"],
                        "unevaluatedProperties": false
                    }
                },
                "required": ["@call", "args"]
            },
            "fetch": {"type": "object", "returnType": "string", "allowedCallers": "agentOnly"},
            "both": {
                "type": "object",
                "returnType": "string",
                "allowedCallers": "rendererOrAgent"
            }
        }
    }))
    .unwrap()
}

fn second() -> Catalog {
    load(json!({
        "catalogId": SECOND,
        "protocolVersion": "1.0",
        "functions": {
            "shout": {
                "type": "object",
                "returnType": "string",
                "properties": {
                    "@call": {"const": "shout"},
                    "args": {
                        "type": "object",
                        "properties": {
                            "value": {"$ref": "common_types.json#/$defs/DynamicString"}
                        },
                        "required": ["value"],
                        "unevaluatedProperties": false
                    }
                },
                "required": ["@call", "args"]
            },
            "secret": {"type": "object", "returnType": "string", "allowedCallers": "agentOnly"}
        }
    }))
    .unwrap()
}

fn text(value: Value) -> Value {
    json!({"id": "t1", "component": "Text", "text": value})
}

/// A `Text` whose `text` calls `f` in a catalog that is not loaded, with `args`.
fn foreign(args: Value) -> Value {
    text(json!({"@call": "f", "catalogId": "elsewhere", "args": args}))
}

fn any(key: &str, value: Value) -> Value {
    let mut component = json!({"id": "a1", "component": "Any"});
    component[key] = value;
    component
}

fn validate(component: &Value) -> Result<(), Error> {
    let catalog = catalog();
    catalog.validate_component(component, ID, &catalog)
}

fn validate_in_both(component: &Value) -> Result<(), Error> {
    let catalogs = [catalog(), second()];
    catalogs[0].validate_component(component, ID, &catalogs[..])
}

fn validation_message(result: Result<(), Error>) -> String {
    let err = result.unwrap_err();
    assert_that!(err.category()).is_equal_to(Category::Validation);
    err.to_string()
}

#[rstest]
#[case::literal(text(json!("hello")))]
#[case::binding(text(json!({"@path": "/a"})))]
#[case::call_here(text(json!({"@call": "upper", "args": {"value": "hi"}})))]
#[case::call_naming_this_catalog(
    text(json!({"@call": "upper", "catalogId": ID, "args": {"value": "hi"}}))
)]
#[case::call_in_unknown_catalog(
    text(json!({"@call": "anything", "catalogId": "elsewhere", "args": {"x": 1, "loud": true}}))
)]
#[case::renderer_or_agent(text(json!({"@call": "both"})))]
#[case::index_in_args(
    text(json!({"@call": "upper", "args": {"value": {"@call": "@index", "args": {"offset": 1}}}}))
)]
#[case::escaped_at_key(
    foreign(json!({"v": {"@@path": 1}}))
)]
#[case::call_in_array(any("items", json!([{"@call": "upper", "args": {"value": "hi"}}])))]
#[case::closed_declared_key(json!({"id": "c1", "component": "Closed", "label": "x"}))]
#[case::closed_undeclared_catalog_id(json!({"id": "c1", "component": "Closed", "catalogId": ID}))]
#[case::closed_undeclared_id(json!({"id": "my-button", "component": "Closed"}))]
#[case::declared_envelope(json!({
    "id": "f1",
    "component": "Field",
    "value": {"@path": "/v"},
    "metadata": {"extensions": {"com_example": 1}}
}))]
fn validate_component_valid_component_is_accepted(#[case] component: Value) {
    assert_that!(validate(&component)).is_ok();
}

#[rstest]
#[case::not_an_object(json!("Text"), "Invalid component: a component is a JSON object")]
#[case::missing_type(json!({"id": "t1", "text": "x"}), "Component has no 'component' type name")]
#[case::unknown_type(
    json!({"id": "t1", "component": "Nope"}),
    "Unrecognized component type 'Nope'"
)]
#[case::bad_type_name(
    json!({"id": "t1", "component": "Te-xt"}),
    "component type 'Te-xt' must be a valid UAX #31 identifier"
)]
#[case::top_level_reserved_key(
    json!({"id": "t1", "component": "Text", "text": "x", "@x": 1}),
    "Key '@x' is reserved: only '@path' and '@call' may start with one '@'"
)]
#[case::top_level_bare_at(
    json!({"id": "t1", "component": "Text", "text": "x", "@": 1}),
    "Key '@' is reserved: only '@path' and '@call' may start with one '@'"
)]
#[case::schema_violation(
    json!({"id": "t1", "component": "Text"}),
    "\"text\" is a required property at ''"
)]
#[case::closed_extra_key(
    json!({"id": "c1", "component": "Closed", "bogus": 1}),
    "Unevaluated properties are not allowed ('bogus' was unexpected) at ''"
)]
#[case::envelope_id_not_string(
    json!({"id": 3, "component": "Closed"}),
    "Invalid component: id must be a string"
)]
#[case::envelope_catalog_id_not_string(
    json!({"id": "c1", "component": "Closed", "catalogId": 3}),
    "Invalid component: catalogId must be a string"
)]
#[case::envelope_metadata_bad_key(
    json!({"id": "c1", "component": "Closed", "metadata": {"extensions": {"a.b": 1}}}),
    "Additional properties are not allowed ('a.b' was unexpected)"
)]
#[case::unknown_function(
    text(json!({"@call": "notAFunction", "args": {}})),
    "Unrecognized function 'notAFunction'"
)]
#[case::unknown_function_named_here(
    text(json!({"@call": "notAFunction", "catalogId": ID, "args": {}})),
    "Unrecognized function 'notAFunction'"
)]
#[case::extra_arg(
    text(json!({"@call": "upper", "catalogId": ID, "args": {"value": "hi", "extra": 1}})),
    "Unevaluated properties are not allowed ('extra' was unexpected) at '/args'"
)]
#[case::extra_call_key(
    text(json!({"@call": "upper", "args": {"value": "hi"}, "bogus": 1})),
    "is not valid under any of the schemas listed in the 'oneOf' keyword at '/text'"
)]
#[case::agent_only(
    text(json!({"@call": "fetch", "args": {}})),
    "Function 'fetch' is agentOnly and cannot be called from a component"
)]
#[case::nested_unknown(
    text(json!({"@call": "upper", "args": {"value": {"@call": "notAFunction", "args": {}}}})),
    "Unrecognized function 'notAFunction'"
)]
#[case::nested_in_unknown_catalog(
    text(json!({
        "@call": "shout",
        "catalogId": "elsewhere",
        "args": {"value": {"@call": "notAFunction", "args": {}}}
    })),
    "Unrecognized function 'notAFunction'"
)]
#[case::nested_in_index(
    text(json!({"@call": "@index", "args": {"offset": {"@call": "notAFunction", "args": {}}}})),
    "Unrecognized function 'notAFunction'"
)]
#[case::call_in_array(
    any("items", json!(["x", {"@call": "notAFunction"}])),
    "Unrecognized function 'notAFunction'"
)]
#[case::bad_function_name_elsewhere(
    text(json!({"@call": "not-a-function", "catalogId": "elsewhere"})),
    "function 'not-a-function' must be a valid UAX #31 identifier"
)]
#[case::bad_arg_name_elsewhere(
    foreign(json!({"bad-arg": "x"})),
    "argument 'bad-arg' must be a valid UAX #31 identifier"
)]
#[case::args_not_object(
    foreign(json!(1)),
    "is not valid under any of the schemas listed in the 'oneOf' keyword at '/text'"
)]
#[case::foreign_args_call_not_string(
    foreign(json!({"v": {"@call": 5}})),
    "Invalid function call: @call must be a string"
)]
#[case::foreign_args_catalog_id_not_string(
    foreign(json!({"v": {"@call": "g", "catalogId": 5}})),
    "Invalid function call: catalogId of 'g' must be a string"
)]
#[case::foreign_args_args_not_object(
    foreign(json!({"v": {"@call": "g", "args": 1}})),
    "Invalid function call: args of 'g' must be an object"
)]
#[case::unknown_system_function(text(json!({"@call": "@foo"})), "Unrecognized function '@foo'")]
#[case::system_function_with_catalog(
    text(json!({"@call": "@index", "catalogId": "elsewhere"})),
    "System function '@index' cannot name a catalogId"
)]
#[case::index_extra_arg(
    text(json!({"@call": "@index", "args": {"offset": 1, "step": 2}})),
    "Unevaluated properties are not allowed ('step' was unexpected) at '/args'"
)]
#[case::reserved_key(
    text(json!({"@call": "shout", "catalogId": "elsewhere", "args": {"value": {"@unknown": 1}}})),
    "Key '@unknown' is reserved: only '@path' and '@call' may start with one '@'"
)]
fn validate_component_invalid_component_is_validation_error(
    #[case] component: Value,
    #[case] message: &str,
) {
    assert_that!(validation_message(validate(&component))).contains(message);
}

#[test]
fn validate_component_unqualified_call_in_unknown_default_checks_identifiers_only() {
    let catalog = catalog();
    let component = text(json!({"@call": "anything", "args": {}}));
    assert_that!(catalog.validate_component(&component, "elsewhere", &catalog)).is_ok();
}

#[test]
fn validate_component_call_naming_this_catalog_under_foreign_default_is_checked_here() {
    let catalog = catalog();
    let component = text(json!({"@call": "notAFunction", "catalogId": ID, "args": {}}));
    let result = catalog.validate_component(&component, "elsewhere", &catalog);
    assert_that!(validation_message(result)).contains("Unrecognized function 'notAFunction'");
}

#[test]
fn validate_component_call_into_second_known_catalog_is_accepted() {
    let component = text(json!({"@call": "shout", "catalogId": SECOND, "args": {"value": "hi"}}));
    assert_that!(validate_in_both(&component)).is_ok();
}

#[rstest]
#[case::unknown_function(
    json!({"@call": "upper", "catalogId": SECOND, "args": {"value": "hi"}}),
    "Unrecognized function 'upper'"
)]
#[case::extra_arg(
    json!({"@call": "shout", "catalogId": SECOND, "args": {"value": "hi", "extra": 1}}),
    "Unevaluated properties are not allowed ('extra' was unexpected) at '/args'"
)]
#[case::agent_only(
    json!({"@call": "secret", "catalogId": SECOND}),
    "Function 'secret' is agentOnly and cannot be called from a component"
)]
fn validate_component_bad_call_into_second_known_catalog_is_validation_error(
    #[case] call: Value,
    #[case] message: &str,
) {
    assert_that!(validation_message(validate_in_both(&text(call)))).contains(message);
}

#[test]
fn validate_component_call_into_unlisted_catalog_checks_identifiers_only() {
    let call = json!({"@call": "shout", "catalogId": SECOND, "args": {"value": "hi", "extra": 1}});
    assert_that!(validate(&text(call))).is_ok();
}

#[test]
fn validate_component_value_at_the_depth_cap_is_accepted() {
    let component = any("data", nested(MAX_JSON_DEPTH - 1));
    assert_that!(validate(&component)).is_ok();
}

#[test]
fn validate_component_value_past_the_depth_cap_is_rejected() {
    let component = any("data", nested(MAX_JSON_DEPTH));
    let message = validation_message(validate(&component));
    assert_that!(message).is_equal_to("Value nesting exceeds 64 levels".to_owned());
}
```

`any("data", nested(MAX_JSON_DEPTH - 1))` nests exactly 64 levels (the component object is
one), and `nested(MAX_JSON_DEPTH)` 65. `validation_message` asserts the category and returns
the message: the message pins which check failed, so a case cannot pass for another reason.
The `oneOf` and `Unevaluated properties` texts are jsonschema 0.61.1's, followed by the
`SchemaViolation` suffix ` at '{instance path}'`.

- [ ] **Step 2: Run them to see them fail**

Run: `cargo nextest run -p a2ui-runtime validate_component`
Expected: fails to compile: unresolved items.

- [ ] **Step 3: Add the validation errors**

In `error.rs`, add to `Inner` (all in the `Category::Validation` arm with `SchemaViolation`):

```rust
    #[snafu(display("Invalid component: {reason}"))]
    ComponentShape {
        reason: String,
        backtrace: Backtrace,
    },

    #[snafu(display("Component has no 'component' type name"))]
    MissingComponentType { backtrace: Backtrace },

    #[snafu(display("Unrecognized component type '{component}'"))]
    UnknownComponent {
        component: String,
        backtrace: Backtrace,
    },

    #[snafu(display("{what} '{name}' must be a valid UAX #31 identifier"))]
    BadIdentifier {
        what: &'static str,
        name: String,
        backtrace: Backtrace,
    },

    #[snafu(display("Key '{key}' is reserved: only '@path' and '@call' may start with one '@'"))]
    ReservedKey {
        key: String,
        backtrace: Backtrace,
    },

    #[snafu(display("Invalid function call: {reason}"))]
    CallShape {
        reason: String,
        backtrace: Backtrace,
    },

    #[snafu(display("System function '{function}' cannot name a catalogId"))]
    SystemFunctionCatalog {
        function: String,
        backtrace: Backtrace,
    },

    #[snafu(display("Unrecognized function '{function}'"))]
    UnknownFunction {
        function: String,
        backtrace: Backtrace,
    },

    #[snafu(display("Function '{function}' is agentOnly and cannot be called from a component"))]
    AgentOnlyCall {
        function: String,
        backtrace: Backtrace,
    },
```

Each holds at most one `String`, a `&'static str` and the backtrace, so the
`const _: () = assert!(size_of::<Error>() <= 128);` from Task 4 keeps holding.

- [ ] **Step 4: Write `catalog/validate.rs`**

```rust
//! Validating a component instance against a catalog, nested function calls included.

use serde_json::{Map, Value};
use snafu::{OptionExt, ensure};

use super::{AllowedCallers, Catalog};
use crate::error::{
    AgentOnlyCallSnafu, BadIdentifierSnafu, CallShapeSnafu, ComponentShapeSnafu, Error,
    MissingComponentTypeSnafu, ReservedKeySnafu, SystemFunctionCatalogSnafu,
    UnknownComponentSnafu, UnknownFunctionSnafu, ValueLimitSnafu,
};
use crate::ident::is_identifier;
use crate::limits::{MAX_JSON_DEPTH, deeper_than};
use crate::schema::{validate_common_type, validate_component_metadata};
use crate::validator::check;

/// Keys a component may carry whether or not its schema declares them.
const ENVELOPE: [&str; 4] = ["id", "component", "catalogId", "metadata"];

/// Finds a loaded catalog by its id, for calls that name another catalog.
///
/// Implement this trait to let [`Catalog::validate_component`] check calls into the other
/// catalogs your surface has loaded, kept in a map or registry of your own. A [`Catalog`] finds
/// itself and a `[Catalog]` slice finds any of its catalogs. A call into a catalog that is not
/// found is checked only for its identifiers.
pub trait CatalogLookup {
    /// The loaded catalog whose id is `id`, if any.
    fn catalog(&self, id: &str) -> Option<&Catalog>;
}

impl CatalogLookup for Catalog {
    fn catalog(&self, id: &str) -> Option<&Catalog> {
        (self.id == id).then_some(self)
    }
}

impl CatalogLookup for [Catalog] {
    fn catalog(&self, id: &str) -> Option<&Catalog> {
        self.iter().find(|catalog| catalog.id == id)
    }
}

impl Catalog {
    /// Validates one component instance, such as `{"id": "t1", "component": "Text"}`.
    ///
    /// A nested call runs in the catalog its `catalogId` names, else in `default_catalog` (the
    /// surface's). When that catalog is this one or `catalogs` finds it, the call is checked
    /// against its function there: the function exists, is not `agentOnly`, and the call
    /// matches its schema. A call into a catalog not found is checked only for its identifiers.
    pub fn validate_component(
        &self,
        component: &Value,
        default_catalog: &str,
        catalogs: &(impl CatalogLookup + ?Sized),
    ) -> Result<(), Error> {
        ensure!(
            !deeper_than(component, MAX_JSON_DEPTH),
            ValueLimitSnafu {
                limit: MAX_JSON_DEPTH
            }
        );
        let map = component.as_object().context(ComponentShapeSnafu {
            reason: "a component is a JSON object",
        })?;
        map.keys().map(String::as_str).try_for_each(unreserved)?;
        let kind = map
            .get("component")
            .and_then(Value::as_str)
            .context(MissingComponentTypeSnafu)?;
        identifier("component type", kind)?;
        let def = self
            .components
            .get(kind)
            .context(UnknownComponentSnafu { component: kind })?;
        let mut instance = map.clone();
        for key in ENVELOPE {
            if def.shape().properties().contains(key) {
                continue;
            }
            match (key, instance.remove(key)) {
                ("id" | "catalogId", Some(value)) => ensure!(
                    value.is_string(),
                    ComponentShapeSnafu {
                        reason: format!("{key} must be a string")
                    }
                ),
                ("metadata", Some(value)) => validate_component_metadata(&value)?,
                _ => {}
            }
        }
        check(def.validator(), &Value::Object(instance))?;
        let walk = Walk {
            home: self,
            default_catalog,
            catalogs,
        };
        map.iter()
            .filter(|(key, _)| !ENVELOPE.contains(&key.as_str()))
            .try_for_each(|(_, value)| walk.node(value))
    }
}

/// The nested-call walk of one component: where its calls run and which catalogs are known.
struct Walk<'a, L: ?Sized> {
    home: &'a Catalog,
    default_catalog: &'a str,
    catalogs: &'a L,
}

impl<'a, L: CatalogLookup + ?Sized> Walk<'a, L> {
    /// Checks reserved keys and calls under `node`; it is depth-checked, so this recurses.
    fn node(&self, node: &Value) -> Result<(), Error> {
        match node {
            Value::Object(map) => {
                map.keys().map(String::as_str).try_for_each(unreserved)?;
                if map.contains_key("@call") {
                    self.call(node, map)?;
                }
                map.values().try_for_each(|value| self.node(value))
            }
            Value::Array(items) => items.iter().try_for_each(|item| self.node(item)),
            _ => Ok(()),
        }
    }

    /// The catalog whose id is `id`: the component's own, else one `catalogs` finds.
    fn find(&self, id: &str) -> Option<&'a Catalog> {
        if self.home.id == id {
            Some(self.home)
        } else {
            self.catalogs.catalog(id)
        }
    }

    /// Checks the call `node`, whose entries are `call`, in the catalog it runs in.
    fn call(&self, node: &Value, call: &Map<String, Value>) -> Result<(), Error> {
        let name = call
            .get("@call")
            .and_then(Value::as_str)
            .context(CallShapeSnafu {
                reason: "@call must be a string",
            })?;
        let named = call
            .get("catalogId")
            .map(|id| {
                id.as_str().with_context(|| CallShapeSnafu {
                    reason: format!("catalogId of '{name}' must be a string"),
                })
            })
            .transpose()?;
        if let Some(args) = call.get("args") {
            let args = args.as_object().with_context(|| CallShapeSnafu {
                reason: format!("args of '{name}' must be an object"),
            })?;
            args.keys()
                .try_for_each(|argument| identifier("argument", argument))?;
        }
        if name.starts_with('@') {
            ensure!(named.is_none(), SystemFunctionCatalogSnafu { function: name });
            ensure!(name == "@index", UnknownFunctionSnafu { function: name });
            return validate_common_type("IndexSystemFunction", node);
        }
        identifier("function", name)?;
        let Some(catalog) = self.find(named.unwrap_or(self.default_catalog)) else {
            return Ok(());
        };
        let function = catalog
            .functions
            .get(name)
            .context(UnknownFunctionSnafu { function: name })?;
        ensure!(
            function.allowed_callers() != AllowedCallers::AgentOnly,
            AgentOnlyCallSnafu { function: name }
        );
        if named.is_none() {
            return check(function.validator(), node);
        }
        let mut instance = call.clone();
        instance.remove("catalogId");
        check(function.validator(), &Value::Object(instance))
    }
}

/// Checks that `name`, a `what` such as `"function"`, is a UAX #31 identifier.
fn identifier(what: &'static str, name: &str) -> Result<(), Error> {
    ensure!(is_identifier(name), BadIdentifierSnafu { what, name });
    Ok(())
}

/// Rejects a key starting with one `@` other than `@path` and `@call`, bare `@` included.
fn unreserved(key: &str) -> Result<(), Error> {
    let reserved =
        key.starts_with('@') && !key.starts_with("@@") && key != "@path" && key != "@call";
    ensure!(!reserved, ReservedKeySnafu { key });
    Ok(())
}

#[cfg(test)]
#[path = "validate_tests.rs"]
mod tests;
```

Notes:
- The `args_not_object` and `extra_call_key` cases fail in the component's own schema check
  (the call sits under a `DynamicString`, whose `oneOf` reports first); the walk's own checks
  cover calls the component schema does not reach: in an open component's extra keys
  (`call_in_array`) and in an envelope's `args`, which the envelope types only as an object
  (the `foreign_args_*` cases).
- `named.unwrap_or(self.default_catalog)` is rule 6's "else the surface's", not a fallback;
  `find` returning `None` is the documented identifiers-only path for catalogs not loaded.
- A call without `catalogId` is checked as is (no clone); only a call naming its catalog is
  copied, to drop the `catalogId` a closed function schema does not declare.
- `@index` is checked as the call value itself: it has no `catalogId`, since a system function
  naming one is rejected just before.

In `catalog/mod.rs` add `mod validate;` and `pub use validate::CatalogLookup;`. In `lib.rs`
change the re-export to

```rust
pub use catalog::{
    AllowedCallers, Catalog, CatalogLookup, ComponentDef, ComponentShape, FunctionDef,
};
```

and add `` [`CatalogLookup`] (finds the catalogs nested calls run in) `` to
the crate doc's key items, next to [`Catalog`].

In `catalog/mod.rs`, delete the two `#[expect(dead_code, reason = "validate_component (Task 5) reads it")]`
attributes on `ComponentDef::validator` and `FunctionDef::validator`: `validate.rs` now reads both,
so the expectations would go unfulfilled and fail `-D warnings`.

- [ ] **Step 5: Run the runtime tests**

Run: `cargo nextest run -p a2ui-runtime`
Expected: all pass.

- [ ] **Step 6: Update `crates/a2ui-runtime/ARCHITECTURE.md`**

Extend the `catalog` bullet: `validate` checks a component instance (depth, reserved keys,
envelope keys its schema does not declare removed, its validator, then every nested call in
the catalog it runs in, found through `CatalogLookup`: exists, not `agentOnly`, matches the
function's validator; `@index` as `IndexSystemFunction`; calls into catalogs not found only for
identifiers).

This task's additions bring the file past 50 lines: condense the older bullets (Plan 1's
and Tasks 1 to 4's) until the file is at most 50 lines, keeping every module named, and check
the final count with `wc -l crates/a2ui-runtime/ARCHITECTURE.md`.

- [ ] **Step 7: Gate and commit**

Run: `cargo fmt --all`, then `just check` then `just test`. Expected: green.

```bash
git add crates/a2ui-runtime
```

Message:

```
feat(runtime): validate components and their nested calls against catalogs (t-1558)

A call runs in its catalogId or the surface default. When CatalogLookup finds that catalog,
the call is checked against its function there (exists, agentOnly rejected, args schema);
@index is checked against IndexSystemFunction, calls into catalogs not found only for their
identifiers, and the walk continues into every call's args. Reserved @ keys are rejected at
every level, the component's own included.
```

Then `oku cli task done --project slider --id t-1558 --result "<one line>"`.

---
### Task 6: Basic catalog crate and from_json/to_json cases

Oku: t-1559.

**Files:**
- Create: `crates/a2ui-catalog-basic/{Cargo.toml,ARCHITECTURE.md,src/lib.rs,src/lib_tests.rs}`,
  `crates/a2ui-catalog-basic/{catalog.json,LICENSE-A2UI}` and, when upstream has a `NOTICE`,
  `NOTICE-A2UI` (copied by sync.sh),
  `crates/a2ui-conformance/src/handlers/{catalog_checks.rs,catalog_checks_tests.rs}`,
  `crates/a2ui-conformance/src/handlers/{from_json.rs,to_json.rs}`
- Modify: `Cargo.toml`, `justfile`, `vendor/sync.sh`, `README.md`,
  `crates/a2ui-conformance/{Cargo.toml,skips.toml,ARCHITECTURE.md}`,
  `crates/a2ui-conformance/src/handlers/mod.rs`

**Interfaces:**
- Consumes: `Catalog` API, `ComponentDef::shape` (Task 4), `Catalog::validate_component(component,
  default_catalog, catalogs: &(impl CatalogLookup + ?Sized))`, with `Catalog: CatalogLookup`
  (Task 5);
  `catalog_input::{document, load}`, `schema_eq`, `json_eq`, `expect_error`,
  `assert_known_keys`, `Case::at` (Tasks 1, 3, 4).
- Produces:
  - `a2ui_catalog_basic::{CATALOG_ID, CATALOG_JSON,
    catalog() -> Result<Catalog, a2ui_runtime::Error>}`.
  - Harness `handlers::catalog_checks`:
    `entries(expect: &Map<String, Value>, catalog: &Catalog, at: &str) -> Result<(), String>`
    (Task 7's derive handler reuses it),
    `self_contained(schema: &Value) -> Result<(), String>`,
    `matches_builtin(loaded: &Catalog, builtin: &Catalog) -> Result<(), String>`.

`matchesBuiltinCatalog` has one case, which loads the published basic catalog and compares it
with `a2ui-catalog-basic`, which embeds the same file: here it is tautological. The real check
(a catalog built by code against the published one) arrives with Plan 4's function runtime; the
unit tests below prove `matches_builtin` can fail.

- [ ] **Step 1: Copy the basic catalog into its crate from `vendor/sync.sh`**

The crate ships upstream's Apache-2.0 data, so it also ships upstream's license and, when
upstream has one, its `NOTICE` (Apache-2.0 section 4(d)). In `vendor/sync.sh` (as Task 1 left
it), insert before the `printf` line:

```bash
basic="$repo/crates/a2ui-catalog-basic"
mkdir -p "$basic"
cp "$work/a2ui/catalogs/basic/v1/catalog.json" "$basic/catalog.json"
cp "$work/a2ui/LICENSE" "$basic/LICENSE-A2UI"
rm -f "$basic/NOTICE-A2UI"
if [[ -f "$work/a2ui/NOTICE" ]]; then
  cp "$work/a2ui/NOTICE" "$basic/NOTICE-A2UI"
fi
```

A sparse pattern that matches no file is not an error, and the `rm -f` drops a stale copy when
upstream removes its `NOTICE`.

Run: `just vendor-sync 3295e67b5f4820639aa88c578a64af2196d0b752`
Then: `git status --short` lists only `crates/a2ui-catalog-basic/` as new and `vendor/sync.sh`
as modified (`vendor/UPSTREAM` unchanged: same commit). `ls crates/a2ui-catalog-basic` shows
whether upstream has a `NOTICE` at this commit; the crate's `include` covers both cases.

- [ ] **Step 2: Create the crate with a failing test**

`crates/a2ui-catalog-basic/Cargo.toml`:

```toml
[package]
name = "a2ui-catalog-basic"
description = "The A2UI v1.0 basic catalog for a2ui-runtime."
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license = "Apache-2.0"
repository.workspace = true
include = ["src/**", "catalog.json", "LICENSE-A2UI", "NOTICE-A2UI", "Cargo.toml"]

[dependencies]
a2ui-runtime.workspace = true

[dev-dependencies]
serde_json.workspace = true
speculoos.workspace = true

[lints]
workspace = true
```

`license` is `Apache-2.0`, not the workspace's `MIT OR Apache-2.0`: the crate's substance is
upstream's Apache-2.0 catalog, which cannot be offered under MIT. (The runtime keeps its
dual licence: Apache-2.0 files may ship inside a dual-licensed crate with their notice, here the
upstream `LICENSE` beside the embedded schemas.) `cargo deny` accepts it
(`Apache-2.0` is on the allow list). An `include` entry naming an absent file is skipped by
`cargo package`, so `NOTICE-A2UI` ships exactly when sync.sh copied it. (`src/**` also ships
`lib_tests.rs`, which `#[cfg(test)]` keeps out of builds, as in `a2ui-runtime`.)

Root `Cargo.toml`: add `"crates/a2ui-catalog-basic"` to `members` between the
runtime and the harness, and to `[workspace.dependencies]`:
`a2ui-catalog-basic = { path = "crates/a2ui-catalog-basic", version = "0.1.0" }`.
release-plz publishes every crate not listed with `release = false`, so this one will be
published: before the first release PR is merged, check the name is free on crates.io
(`cargo search a2ui-catalog-basic`) and record the result in the commit message.

`crates/a2ui-catalog-basic/src/lib_tests.rs`:

```rust
use a2ui_runtime::Category;
use serde_json::{Value, json};
use speculoos::prelude::*;

use super::*;

#[test]
fn catalog_published_document_loads_with_its_id() {
    assert_that!(catalog().unwrap().id()).is_equal_to(CATALOG_ID);
}

#[test]
fn catalog_eighteen_basic_components_are_kept() {
    assert_that!(catalog().unwrap().components().count()).is_equal_to(18);
}

#[test]
fn catalog_fourteen_basic_functions_are_kept() {
    assert_that!(catalog().unwrap().functions().count()).is_equal_to(14);
}

fn open_url_button(args: Value) -> Value {
    json!({
        "id": "b1", "component": "Button", "child": "t1",
        "action": {"functionCall": {"@call": "openUrl", "args": args}}
    })
}

#[test]
fn catalog_button_calling_open_url_is_valid() {
    let basic = catalog().unwrap();
    let button = open_url_button(json!({"url": "https://example.com"}));
    assert_that!(basic.validate_component(&button, CATALOG_ID, &basic)).is_ok();
}

#[test]
fn catalog_open_url_extra_argument_is_invalid() {
    let basic = catalog().unwrap();
    let button = open_url_button(json!({"url": "https://example.com", "target": "_blank"}));
    let err = basic.validate_component(&button, CATALOG_ID, &basic).unwrap_err();
    assert_that!(err.category()).is_equal_to(Category::Validation);
    assert_that!(err.to_string()).contains("'target' was unexpected");
}

#[test]
fn catalog_text_field_without_value_is_invalid() {
    let basic = catalog().unwrap();
    let field = json!({"id": "f1", "component": "TextField", "label": "Email"});
    let err = basic.validate_component(&field, CATALOG_ID, &basic).unwrap_err();
    assert_that!(err.to_string()).contains("\"value\" is a required property");
}
```

`TextField` requires `component`, `label` and `value`, and `Button` requires `component`,
`child` and `action` (`catalog.json` at the vendored commit). `openUrl`'s `args` are closed
(`unevaluatedProperties: false`), so the extra `target` fails the call's full check, which runs
because the call's catalog, the default `CATALOG_ID`, is found through the lookup.

Run: `cargo nextest run -p a2ui-catalog-basic`
Expected: fails to compile: unresolved items.

- [ ] **Step 3: Write `src/lib.rs`**

```rust
//! The A2UI v1.0 basic catalog, embedded for `a2ui-runtime`.
//!
//! [`catalog`] loads the published `catalog.json` ([`CATALOG_JSON`], copied by
//! `vendor/sync.sh`, Apache-2.0: see `LICENSE-A2UI`), whose id is [`CATALOG_ID`]. The
//! renderer-side implementations of its functions arrive with the function runtime (Plan 4).

use a2ui_runtime::{Catalog, Error};

/// The basic catalog's id.
pub const CATALOG_ID: &str = "https://a2ui.org/specification/v1_0/catalogs/basic/catalog.json";

/// The published catalog document, as JSON text.
pub const CATALOG_JSON: &str = include_str!("../catalog.json");

/// Loads the basic catalog; each call parses and compiles it anew (about 20 ms).
pub fn catalog() -> Result<Catalog, Error> {
    Catalog::parse(CATALOG_JSON, None, None)
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
```

`crates/a2ui-catalog-basic/ARCHITECTURE.md`:

```markdown
# a2ui-catalog-basic

The A2UI v1.0 basic catalog: 18 components and 14 functions, as published upstream.

- `catalog.json`: the published document, copied by `vendor/sync.sh` with upstream's
  `LICENSE-A2UI` (and `NOTICE-A2UI` when upstream has a `NOTICE`); never edit it by hand.
  The crate is licensed Apache-2.0 only, as that document is.
- `src/lib.rs`: `CATALOG_ID`, `CATALOG_JSON` and `catalog()`, which loads it with
  `Catalog::parse` on every call.

Plan 4 adds the renderer-side implementations of the basic functions here.

Depends on: `a2ui-runtime`.
```

`justfile`, `wasm` recipe:
`cargo build -p a2ui-runtime -p a2ui-catalog-basic --target wasm32-unknown-unknown`.

Run: `cargo nextest run -p a2ui-catalog-basic && just wasm`
Expected: 6 passed; the wasm build succeeds.

- [ ] **Step 4: Write the failing tests of the shared catalog checks**

Root `Cargo.toml` already declares `percent-encoding` (Task 2).
`crates/a2ui-conformance/Cargo.toml` `[dependencies]`: `a2ui-catalog-basic.workspace = true`
and `percent-encoding.workspace = true`; `[dev-dependencies]` already holds `speculoos` and
`rstest` (Plan 1).

Create `crates/a2ui-conformance/src/handlers/catalog_checks_tests.rs`:

```rust
use a2ui_runtime::Catalog;
use rstest::rstest;
use serde_json::{Value, json};
use speculoos::prelude::*;

use super::*;

fn catalog(extra: Value) -> Catalog {
    let mut document = json!({"catalogId": "c", "protocolVersion": "1.0"});
    for (key, value) in extra.as_object().unwrap() {
        document[key] = value.clone();
    }
    Catalog::from_json(&document, None, None).unwrap()
}

fn expect(value: Value) -> serde_json::Map<String, Value> {
    value.as_object().unwrap().clone()
}

#[test]
fn self_contained_resolving_refs_pass() {
    let schema = json!({"a": {"$ref": "#/$defs/B_%E2%84%93"}, "$defs": {"B_ℓ": {}}});
    assert_that!(self_contained(&schema)).is_ok();
}

#[rstest]
#[case(json!({"a": {"type": "string"}}), "has no $ref")]
#[case(
    json!({"a": {"$ref": "common_types.json#/$defs/B"}}),
    "'common_types.json#/$defs/B' does not resolve"
)]
#[case(json!({"a": {"$ref": "#/$defs/Missing"}}), "'#/$defs/Missing' does not resolve")]
#[case(json!({"a": {"$ref": "#/$defs/%FF"}}), "'#/$defs/%FF' does not resolve")]
fn self_contained_broken_schema_is_reported(#[case] schema: Value, #[case] fragment: &str) {
    assert_that!(self_contained(&schema).unwrap_err()).contains(fragment);
}

#[test]
fn matches_builtin_same_catalog_passes() {
    let one = catalog(json!({"components": {"A": {"properties": {"x": {}}}}}));
    let two = catalog(json!({"components": {"A": {"properties": {"x": {}}}}}));
    assert_that!(matches_builtin(&one, &two)).is_ok();
}

#[rstest]
#[case(json!({"components": {"B": {}}}), "component names differ")]
#[case(json!({"components": {"A": {"properties": {"y": {}}}}}), "component A differs")]
#[case(
    json!({
        "components": {"A": {"properties": {"x": {}}}},
        "functions": {"f": {"returnType": "string"}}
    }),
    "function signatures differ"
)]
fn matches_builtin_difference_is_reported(#[case] extra: Value, #[case] fragment: &str) {
    let builtin = catalog(json!({"components": {"A": {"properties": {"x": {}}}}}));
    assert_that!(matches_builtin(&catalog(extra), &builtin).unwrap_err()).contains(fragment);
}

#[test]
fn matches_builtin_other_id_is_reported() {
    let builtin = catalog(json!({}));
    let document = json!({"catalogId": "d", "protocolVersion": "1.0"});
    let other = Catalog::from_json(&document, None, None).unwrap();
    assert_that!(matches_builtin(&other, &builtin).unwrap_err())
        .contains("differs from the builtin");
}

#[rstest]
#[case(json!({"components": ["A"]}))]
#[case(json!({"components": {"A": {"properties": {"x": {}}}}}))]
#[case(json!({"catalogId": "c", "protocolVersion": "1.0"}))]
fn entries_matching_expectation_passes(#[case] expected: Value) {
    let loaded = catalog(json!({"components": {"A": {"properties": {"x": {}}}}}));
    assert_that!(entries(&expect(expected), &loaded, "t")).is_ok();
}

#[test]
fn entries_matching_theme_passes() {
    let theme = json!({"type": "object", "properties": {"primaryColor": {"type": "string"}}});
    let loaded = catalog(json!({"theme": theme.clone()}));
    assert_that!(entries(&expect(json!({"theme": theme})), &loaded, "t")).is_ok();
}

#[test]
fn entries_other_theme_is_reported() {
    let loaded = catalog(json!({"theme": {"type": "object"}}));
    let expected = expect(json!({"theme": {"type": "string"}}));
    assert_that!(entries(&expected, &loaded, "t").unwrap_err())
        .is_equal_to(r#"t: theme {"type":"object"}, expected {"type":"string"}"#.to_owned());
}

#[rstest]
#[case(json!({"components": ["A", "B"]}), "components")]
#[case(json!({"components": {"A": {"properties": {"y": {}}}}}), "components.A")]
#[case(json!({"components": "A"}), "must be a list or a map")]
#[case(json!({"components": [1]}), "lists a non-string")]
#[case(json!({"catalogId": "d"}), "catalogId is 'c'")]
#[case(json!({"theme": {"type": "object"}}), "t: no theme, expected {\"type\":\"object\"}")]
fn entries_mismatch_is_reported(#[case] expected: Value, #[case] fragment: &str) {
    let loaded = catalog(json!({"components": {"A": {"properties": {"x": {}}}}}));
    assert_that!(entries(&expect(expected), &loaded, "t").unwrap_err()).contains(fragment);
}
```

Run: `cargo nextest run -p a2ui-conformance --lib catalog_checks`
Expected: fails to compile: unresolved items.

- [ ] **Step 5: Write `catalog_checks.rs`**

```rust
//! Expectations on a loaded catalog, shared by the `from_json` and `derive` actions.

use std::collections::BTreeSet;

use a2ui_runtime::Catalog;
use percent_encoding::percent_decode_str;
use serde_json::{Map, Value};

use crate::compare::schema_eq;

/// Checks `catalogId`, `protocolVersion`, `components`, `functions` and `theme` of `expect`.
///
/// `components` / `functions` are a list of the exact names, or a map of the exact names to
/// their authored schemas.
pub fn entries(expect: &Map<String, Value>, catalog: &Catalog, at: &str) -> Result<(), String> {
    if let Some(expected) = expect.get("catalogId") {
        same_text(expected, catalog.id(), "catalogId", at)?;
    }
    if let Some(expected) = expect.get("protocolVersion") {
        same_text(expected, catalog.protocol_version(), "protocolVersion", at)?;
    }
    if let Some(expected) = expect.get("components") {
        let actual = catalog.components().map(|def| (def.name(), def.schema()));
        named(expected, actual.collect(), "components", at)?;
    }
    if let Some(expected) = expect.get("functions") {
        let actual = catalog.functions().map(|def| (def.name(), def.schema()));
        named(expected, actual.collect(), "functions", at)?;
    }
    if let Some(expected) = expect.get("theme") {
        match catalog.theme() {
            Some(theme) if schema_eq(theme, expected) => {}
            Some(theme) => return Err(format!("{at}: theme {theme}, expected {expected}")),
            None => return Err(format!("{at}: no theme, expected {expected}")),
        }
    }
    Ok(())
}

fn same_text(expected: &Value, actual: &str, what: &str, at: &str) -> Result<(), String> {
    if expected.as_str() == Some(actual) {
        Ok(())
    } else {
        Err(format!("{at}: {what} is '{actual}', expected {expected}"))
    }
}

fn named(
    expected: &Value,
    actual: Vec<(&str, &Value)>,
    what: &str,
    at: &str,
) -> Result<(), String> {
    let actual_names: BTreeSet<&str> = actual.iter().map(|(name, _)| *name).collect();
    let expected_names: BTreeSet<&str> = match expected {
        Value::Array(names) => names
            .iter()
            .map(|name| name.as_str().ok_or_else(|| format!("{at}: {what} lists a non-string")))
            .collect::<Result<_, _>>()?,
        Value::Object(entries) => entries.keys().map(String::as_str).collect(),
        _ => return Err(format!("{at}: {what} must be a list or a map")),
    };
    if expected_names != actual_names {
        return Err(format!("{at}: {what} {actual_names:?}, expected {expected_names:?}"));
    }
    let Value::Object(entries) = expected else {
        return Ok(());
    };
    for (name, schema) in actual {
        let expected = entries.get(name);
        if expected.is_none_or(|expected| !schema_eq(schema, expected)) {
            return Err(format!("{at}: {what}.{name} is {schema}, expected {expected:?}"));
        }
    }
    Ok(())
}

/// Checks every `$ref` of `schema` is local and resolves.
///
/// Fragments are percent-decoded first, as a URI reference requires.
pub fn self_contained(schema: &Value) -> Result<(), String> {
    let mut refs = Vec::new();
    collect_refs(schema, &mut refs);
    if refs.is_empty() {
        return Err("the validation schema has no $ref".to_owned());
    }
    let resolves = |target: &str| {
        target
            .strip_prefix('#')
            .and_then(|fragment| percent_decode_str(fragment).decode_utf8().ok())
            .is_some_and(|pointer| schema.pointer(&pointer).is_some())
    };
    match refs.into_iter().find(|target| !resolves(target)) {
        Some(target) => Err(format!("$ref '{target}' does not resolve in the validation schema")),
        None => Ok(()),
    }
}

fn collect_refs<'a>(node: &'a Value, out: &mut Vec<&'a str>) {
    match node {
        Value::Object(map) => {
            for (key, value) in map {
                match value {
                    Value::String(target) if key == "$ref" => out.push(target),
                    _ => collect_refs(value, out),
                }
            }
        }
        Value::Array(items) => items.iter().for_each(|item| collect_refs(item, out)),
        _ => {}
    }
}

/// `loaded` defines what `builtin` does: id, version, component shapes, function signatures.
///
/// Component shapes are compared without the envelope keys `id` and `component`.
pub fn matches_builtin(loaded: &Catalog, builtin: &Catalog) -> Result<(), String> {
    if (loaded.id(), loaded.protocol_version()) != (builtin.id(), builtin.protocol_version()) {
        return Err(format!(
            "catalog {} {} differs from the builtin {} {}",
            loaded.id(),
            loaded.protocol_version(),
            builtin.id(),
            builtin.protocol_version()
        ));
    }
    let names = |catalog: &Catalog| -> BTreeSet<String> {
        catalog.components().map(|def| def.name().to_owned()).collect()
    };
    if names(loaded) != names(builtin) {
        return Err("component names differ from the builtin catalog".to_owned());
    }
    for name in names(builtin) {
        if shape(loaded, &name) != shape(builtin, &name) {
            return Err(format!("component {name} differs from the builtin catalog"));
        }
    }
    let signatures = |catalog: &Catalog| {
        catalog
            .functions()
            .map(|def| {
                let signature = (
                    def.return_type().map(str::to_owned),
                    def.allowed_callers(),
                    def.requires_user_activation(),
                );
                (def.name().to_owned(), signature)
            })
            .collect::<Vec<_>>()
    };
    if signatures(loaded) == signatures(builtin) {
        Ok(())
    } else {
        Err("function signatures differ from the builtin catalog".to_owned())
    }
}

fn shape(catalog: &Catalog, name: &str) -> Option<(BTreeSet<String>, BTreeSet<String>)> {
    let shape = catalog.component(name)?.shape();
    let strip = |names: &BTreeSet<String>| -> BTreeSet<String> {
        names
            .iter()
            .filter(|key| !matches!(key.as_str(), "id" | "component"))
            .cloned()
            .collect()
    };
    Some((strip(shape.properties()), strip(shape.required())))
}

#[cfg(test)]
#[path = "catalog_checks_tests.rs"]
mod tests;
```

(`functions()` iterates by name, so the two signature lists compare in the same order.)
`handlers/mod.rs`: `mod catalog_checks;`.

Run: `cargo nextest run -p a2ui-conformance --lib catalog_checks`
Expected: all pass.

- [ ] **Step 6: Write the `from_json` and `to_json` handlers**

Create `crates/a2ui-conformance/src/handlers/from_json.rs`:

```rust
//! The `from_json` action: load a catalog, then check entries and component validation.

use a2ui_runtime::{Catalog, Category};
use serde_json::{Map, Value};

use crate::case::Case;
use crate::compare::{expect_error, json_eq};
use crate::handlers::catalog_checks::{entries, matches_builtin, self_contained};
use crate::handlers::catalog_input::{document, load};
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

/// Loads the case's catalog and checks `expect`, or the `expectError` it must raise.
pub fn run(case: &Case) -> Result<(), String> {
    assert_case_keys(
        case,
        &["catalog", "catalogPath", "catalogId", "protocolVersion", "expect", "expectError"],
    )?;
    let at = case.at();
    let input = document(case)?;
    match (case.get("expectError"), load(case, &input)?) {
        (Some(expected), Err(err)) => expect_error(&err, expected, &at),
        (Some(_), Ok(_)) => Err(format!("{at}: loaded, expected an error")),
        (None, Err(err)) => Err(format!("{at}: load failed: {err}")),
        (None, Ok(catalog)) => match case.get("expect") {
            None => Ok(()),
            Some(Value::Object(expect)) => check(expect, &catalog, &at),
            Some(_) => Err(format!("{at}: `expect` must be a mapping")),
        },
    }
}

fn check(expect: &Map<String, Value>, catalog: &Catalog, at: &str) -> Result<(), String> {
    assert_known_keys(expect, &EXPECT_KEYS, at)?;
    entries(expect, catalog, at)?;
    if flag(expect, "selfContained", at)? {
        self_contained(catalog.validation_schema()).map_err(|err| format!("{at}: {err}"))?;
    }
    if flag(expect, "matchesBuiltinCatalog", at)? {
        let builtin = a2ui_catalog_basic::catalog().map_err(|err| format!("{at}: {err}"))?;
        matches_builtin(catalog, &builtin).map_err(|err| format!("{at}: {err}"))?;
    }
    for (index, component) in list(expect, "validComponents", at)?.iter().enumerate() {
        catalog
            .validate_component(component, catalog.id(), catalog)
            .map_err(|err| format!("{at}: validComponents[{index}] rejected: {err}"))?;
    }
    for (index, component) in list(expect, "invalidComponents", at)?.iter().enumerate() {
        match catalog.validate_component(component, catalog.id(), catalog) {
            Err(err) if err.category() == Category::Validation => {}
            Err(err) => return Err(format!("{at}: invalidComponents[{index}]: {err}")),
            Ok(()) => return Err(format!("{at}: invalidComponents[{index}] accepted")),
        }
    }
    if let Some(expected) = expect.get("componentMetadata") {
        component_metadata(expected, catalog, at)?;
    }
    Ok(())
}

fn component_metadata(expected: &Value, catalog: &Catalog, at: &str) -> Result<(), String> {
    let expected = expected
        .as_object()
        .ok_or_else(|| format!("{at}: componentMetadata must be a mapping"))?;
    for (name, metadata) in expected {
        let def = catalog
            .component(name)
            .ok_or_else(|| format!("{at}: no component {name}"))?;
        let actual = def.metadata().cloned().unwrap_or(Value::Null);
        if !json_eq(&actual, metadata) {
            return Err(format!("{at}: {name} metadata {actual}, expected {metadata}"));
        }
    }
    Ok(())
}

fn flag(expect: &Map<String, Value>, key: &str, at: &str) -> Result<bool, String> {
    match expect.get(key) {
        None => Ok(false),
        Some(Value::Bool(true)) => Ok(true),
        Some(other) => Err(format!("{at}: only `{key}: true` is supported, got {other}")),
    }
}

fn list<'a>(expect: &'a Map<String, Value>, key: &str, at: &str) -> Result<&'a [Value], String> {
    match expect.get(key) {
        None => Ok(&[]),
        Some(Value::Array(items)) => Ok(items),
        Some(_) => Err(format!("{at}: `{key}` must be a list")),
    }
}
```

`unwrap_or(Value::Null)` on the metadata: the suite writes "no metadata" as `null`, so `None`
compares as `null`; this is the expectation's encoding, not a fallback. Components are
validated with the catalog's own id as the surface default and the catalog itself as the
`CatalogLookup` (the handler passes the catalog itself), as the suite's single-catalog
cases mean: a call into another catalog id is checked for identifiers only.

Create `crates/a2ui-conformance/src/handlers/to_json.rs`:

```rust
//! The `to_json` action: the SDK's own basic catalog serializes to the published file.

use serde_json::Value;

use crate::case::Case;
use crate::compare::schema_eq;
use crate::keys::assert_case_keys;
use crate::paths::{read_json, upstream_root};

/// Compares `a2ui_catalog_basic::catalog().to_json()` with `expectFile`.
pub fn run(case: &Case) -> Result<(), String> {
    assert_case_keys(case, &["protocolVersion", "builtinCatalog", "expectFile"])?;
    let at = case.at();
    if case.get("builtinCatalog").and_then(Value::as_str) != Some("basic") {
        return Err(format!("{at}: only `builtinCatalog: basic` exists"));
    }
    let path = case
        .get("expectFile")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{at}: `expectFile` must be a string"))?;
    let expected = read_json(&upstream_root()?.join(path))?;
    let catalog = a2ui_catalog_basic::catalog().map_err(|err| format!("{at}: {err}"))?;
    if schema_eq(&catalog.to_json(), &expected) {
        Ok(())
    } else {
        Err(format!("{at}: the basic catalog's to_json differs from {path}"))
    }
}
```

`handlers/mod.rs`: `mod from_json;`, `mod to_json;`, `"from_json" => Some(from_json::run),`,
`"to_json" => Some(to_json::run),`. Remove `from_json` and `to_json` from the `[actions]`
table of `skips.toml`.

- [ ] **Step 7: Run the catalog suite**

Run: `cargo nextest run -p a2ui-conformance --test core catalog`
Expected: every v1.0 `from_json` (32), `to_json` (1) and `round_trip` (5) case passes; ignored:
the other versions through `[versions]` (among them
`test_catalog_from_json_missing_catalog_id_error` and
`test_catalog_from_json_override_catalog_id`, v0.9 by upstream's default), `validation_schema`
and `derive` through `[actions]`.

A failing v1.0 case is a runtime bug: fix it in `a2ui-runtime` with a unit test first, never in
the handler (AGENTS.md). The call envelope cannot make a `oneOf` of the common types match
twice: `DataBinding` is closed and the envelope requires `@call`.

- [ ] **Step 8: Update the docs**

`README.md` crate table, between the runtime and harness rows:
`| \`a2ui-catalog-basic\` | The published v1.0 basic catalog, loaded with \`a2ui-runtime\` |`.
The license line becomes:

```markdown
Licensed under MIT or Apache-2.0, at your option, except `a2ui-catalog-basic`, which is
Apache-2.0 only, as the upstream catalog it embeds.
```

`crates/a2ui-conformance/ARCHITECTURE.md`: `handlers` gains `from_json`, `to_json` and
`catalog_checks` (entries, `selfContained`, `matchesBuiltinCatalog` against
`a2ui-catalog-basic`); Dependencies gains `a2ui-catalog-basic` and `percent-encoding`.

- [ ] **Step 9: Gate and commit**

Run: `just check`, `just test`, `just audit`. Expected: green.

```bash
git add Cargo.toml Cargo.lock justfile vendor/sync.sh README.md \
  crates/a2ui-catalog-basic crates/a2ui-conformance
```

Message:

```
feat(catalog-basic): embed the v1.0 basic catalog and run the from_json cases (t-1559)

The new a2ui-catalog-basic crate (Apache-2.0, as upstream) loads the published catalog
document, which sync.sh copies with upstream's LICENSE (and NOTICE, when upstream has one).
The harness runs from_json (entries, selfContained, matchesBuiltinCatalog, component
validation, component metadata) and to_json. crates.io name check: <result of cargo search>.
```

Then `oku cli task done --project slider --id t-1559 --result "<one line>"`.

---
### Task 7: Catalog derivation

Oku: t-1560.

**Files:**
- Create: `crates/a2ui-runtime/src/catalog/{derive.rs,derive_tests.rs}`,
  `crates/a2ui-conformance/src/handlers/derive.rs`
- Modify: `crates/a2ui-runtime/src/{lib.rs,error.rs,catalog/mod.rs}`,
  `crates/a2ui-runtime/ARCHITECTURE.md`, `crates/a2ui-conformance/src/handlers/mod.rs`,
  `crates/a2ui-conformance/skips.toml`, `crates/a2ui-conformance/ARCHITECTURE.md`

**Interfaces:**
- Consumes: `Catalog::kept_document` (writes the effective `catalogId` and `protocolVersion`),
  `catalog::document::union` (sorts the names it is given), `Catalog::from_json` (Task 4),
  `Catalog::validate_component` (Task 5), `catalog_checks::entries`,
  `catalog_input::{document, load}` (Tasks 4, 6).
- Produces (re-exported from `a2ui_runtime`):
  `#[non_exhaustive] DeriveEntry { Keep(String), Define(String, Value) }`,
  `Derivation` with `new(catalog_id: impl Into<String>) -> Derivation`,
  `components(self, entries: Vec<DeriveEntry>) -> Derivation`,
  `functions(self, entries: Vec<DeriveEntry>) -> Derivation`,
  `instructions(self, text: impl Into<String>) -> Derivation`;
  `Catalog::derive(&self, derivation: &Derivation) -> Result<Catalog, Error>`.

Rules (catalog.yaml section 5): a name keeps the source's entry (unknown name: `CatalogError`
naming it); a one-key map is a new entry, replacing a source entry of that name; a name listed
twice in one list, as kept or new entries, is a `CatalogError` naming it; a list left out
keeps every source entry of that kind; the derived catalog keeps the source's protocol version,
theme and `$defs`, drops `$id`, `title`, `description`, `instructions`, and takes the new
`instructions` when given. When a list is given and the source has that kind's union, the union
is regenerated for the new entries (a stale union would filter the new entries out). The derived
document is loaded with `Catalog::from_json`, so every load rule and cap applies to it, and the
derived `ComponentDef`s and `FunctionDef`s carry their own `Arc<Validator>`, each compiled
against its pruned document, exactly as for any loaded catalog. Kept entries are recompiled,
not shared with the source: their pruned documents may differ (a regenerated union, new
entries), and derivation is not on a hot path.

- [ ] **Step 1: Write the failing tests**

Create `crates/a2ui-runtime/src/catalog/derive_tests.rs`:

```rust
use rstest::rstest;
use serde_json::json;
use speculoos::prelude::*;

use crate::{Catalog, Category, Derivation, DeriveEntry};

fn source() -> Catalog {
    let document = json!({
        "$id": "https://example.com/source.json",
        "title": "Source",
        "description": "The source.",
        "instructions": "Write components of source.",
        "catalogId": "source",
        "protocolVersion": "1.0",
        "components": {
            "Text": {"type": "object", "properties": {"text": {"type": "string"}}},
            "Image": {"type": "object", "properties": {"url": {"type": "string"}}},
            "Button": {"type": "object", "properties": {"label": {"type": "string"}}}
        },
        "functions": {
            "upper": {"type": "object", "properties": {"returnType": {"const": "string"}}},
            "lower": {"type": "object", "properties": {"returnType": {"const": "string"}}}
        },
        "theme": {"type": "object", "properties": {"primaryColor": {"type": "string"}}},
        "$defs": {
            "anyComponent": {
                "oneOf": [
                    {"$ref": "#/components/Text"},
                    {"$ref": "#/components/Image"},
                    {"$ref": "#/components/Button"}
                ],
                "discriminator": {"propertyName": "component"}
            }
        }
    });
    Catalog::from_json(&document, None, None).unwrap()
}

fn keep(name: &str) -> DeriveEntry {
    DeriveEntry::Keep(name.to_owned())
}

fn component_names(catalog: &Catalog) -> Vec<&str> {
    catalog.components().map(|def| def.name()).collect()
}

#[test]
fn derive_named_entries_are_kept_and_others_dropped() {
    let derived = source()
        .derive(&Derivation::new("derived").components(vec![keep("Text"), keep("Button")]))
        .unwrap();
    assert_that!(component_names(&derived)).is_equal_to(vec!["Button", "Text"]);
}

#[test]
fn derive_kept_entry_keeps_its_schema() {
    let derived = source()
        .derive(&Derivation::new("derived").components(vec![keep("Text")]))
        .unwrap();
    assert_that!(derived.component("Text").unwrap().schema().clone())
        .is_equal_to(json!({"type": "object", "properties": {"text": {"type": "string"}}}));
}

#[test]
fn derive_new_entry_is_added() {
    let card = json!({"type": "object", "properties": {"title": {"type": "string"}}});
    let derived = source()
        .derive(&Derivation::new("derived").components(vec![
            keep("Text"),
            DeriveEntry::Define("Card".to_owned(), card),
        ]))
        .unwrap();
    assert_that!(component_names(&derived)).is_equal_to(vec!["Card", "Text"]);
}

#[test]
fn derive_new_entry_replaces_a_source_entry() {
    let button = json!({"type": "object", "properties": {"primary": {"type": "boolean"}}});
    let derived = source()
        .derive(&Derivation::new("derived").components(vec![DeriveEntry::Define(
            "Button".to_owned(),
            button.clone(),
        )]))
        .unwrap();
    assert_that!(derived.component("Button").unwrap().schema().clone()).is_equal_to(button);
}

#[test]
fn derive_omitted_list_keeps_every_entry() {
    let derived = source()
        .derive(&Derivation::new("derived").components(vec![keep("Text")]))
        .unwrap();
    let names: Vec<&str> = derived.functions().map(|def| def.name()).collect();
    assert_that!(names).is_equal_to(vec!["lower", "upper"]);
}

#[test]
fn derive_source_description_keys_are_dropped() {
    let json = source().derive(&Derivation::new("derived")).unwrap().to_json();
    let present: Vec<&str> = ["$id", "title", "description", "instructions"]
        .into_iter()
        .filter(|key| json.get(*key).is_some())
        .collect();
    assert_that!(present).is_empty();
}

#[test]
fn derive_new_id_is_used() {
    let derived = source().derive(&Derivation::new("derived")).unwrap();
    assert_that!(derived.id()).is_equal_to("derived");
}

#[test]
fn derive_source_version_is_kept() {
    let derived = source().derive(&Derivation::new("derived")).unwrap();
    assert_that!(derived.protocol_version()).is_equal_to("1.0");
}

#[test]
fn derive_source_theme_is_kept() {
    let derived = source().derive(&Derivation::new("derived")).unwrap();
    assert_that!(derived.theme()).is_equal_to(Some(&json!({
        "type": "object",
        "properties": {"primaryColor": {"type": "string"}}
    })));
}

#[test]
fn derive_to_json_writes_the_new_catalog_id() {
    let json = source().derive(&Derivation::new("derived")).unwrap().to_json();
    assert_that!(json["catalogId"].clone()).is_equal_to(json!("derived"));
}

#[test]
fn derive_new_entry_is_checked_by_its_own_validator() {
    let card = json!({"type": "object", "properties": {"title": {"type": "string"}}});
    let derived = source()
        .derive(&Derivation::new("derived").components(vec![DeriveEntry::Define(
            "Card".to_owned(),
            card,
        )]))
        .unwrap();
    let component = json!({"id": "c1", "component": "Card", "title": 5});
    let err = derived
        .validate_component(&component, "derived", &derived)
        .unwrap_err();
    assert_that!(err.to_string()).contains("5 is not of type \"string\"");
}

#[test]
fn derive_instructions_replace_the_source_ones() {
    let derived = source()
        .derive(&Derivation::new("derived").instructions("Write components of derived."))
        .unwrap();
    assert_that!(derived.instructions()).is_equal_to(Some("Write components of derived."));
}

#[test]
fn derive_given_list_regenerates_the_source_union() {
    let derived = source()
        .derive(&Derivation::new("derived").components(vec![
            keep("Text"),
            DeriveEntry::Define("Card_ℓ".to_owned(), json!({"type": "object"})),
        ]))
        .unwrap();
    assert_that!(derived.to_json()["$defs"]["anyComponent"].clone()).is_equal_to(json!({
        "oneOf": [{"$ref": "#/components/Card_%E2%84%93"}, {"$ref": "#/components/Text"}],
        "discriminator": {"propertyName": "component"}
    }));
}

#[test]
fn derive_unknown_component_is_catalog_error_naming_it() {
    let err = source()
        .derive(&Derivation::new("derived").components(vec![keep("Text"), keep("Slider")]))
        .unwrap_err();
    assert_that!(err.category()).is_equal_to(Category::Catalog);
    assert_that!(err.to_string()).contains("has no component 'Slider'");
}

#[test]
fn derive_unknown_function_is_catalog_error_naming_it() {
    let err = source()
        .derive(&Derivation::new("derived").functions(vec![keep("capitalize")]))
        .unwrap_err();
    assert_that!(err.to_string()).contains("has no function 'capitalize'");
}

#[rstest]
#[case::kept_twice(vec![keep("Text"), keep("Text")])]
#[case::kept_and_defined(vec![keep("Text"), DeriveEntry::Define("Text".to_owned(), json!({}))])]
#[case::defined_twice(vec![
    DeriveEntry::Define("Text".to_owned(), json!({})),
    DeriveEntry::Define("Text".to_owned(), json!({"type": "object"})),
])]
fn derive_duplicate_entry_is_catalog_error_naming_it(#[case] entries: Vec<DeriveEntry>) {
    let err = source()
        .derive(&Derivation::new("derived").components(entries))
        .unwrap_err();
    assert_that!(err.category()).is_equal_to(Category::Catalog);
    assert_that!(err.to_string())
        .is_equal_to("Derivation lists component 'Text' more than once".to_owned());
}

#[test]
fn derive_invalid_new_entry_name_is_catalog_error() {
    let err = source()
        .derive(&Derivation::new("derived").components(vec![DeriveEntry::Define(
            "Bad-Name".to_owned(),
            json!({}),
        )]))
        .unwrap_err();
    assert_that!(err.to_string()).contains("Invalid UAX #31 component identifier: 'Bad-Name'");
}
```

(`derive_new_entry_is_checked_by_its_own_validator` pins jsonschema's type message, as Task 5's
tests do; the default catalog `"derived"` is the derived catalog, found through it as the
`CatalogLookup`.)

- [ ] **Step 2: Run them to see them fail**

Run: `cargo nextest run -p a2ui-runtime derive`
Expected: fails to compile: unresolved items.

- [ ] **Step 3: Add the errors**

In `error.rs`, add to `Inner`, and both to the `Category::Catalog` arm of `category`:

```rust
    #[snafu(display("Catalog '{catalog}' has no {kind} '{name}' to derive from"))]
    DeriveUnknownEntry {
        catalog: String,
        kind: &'static str,
        name: String,
        backtrace: Backtrace,
    },

    #[snafu(display("Derivation lists {kind} '{name}' more than once"))]
    DeriveDuplicateEntry {
        kind: &'static str,
        name: String,
        backtrace: Backtrace,
    },
```

(No derived id field: the caller knows it, and a third `String` would push `Error` past the
128 bytes that `error.rs`'s `const _: () = assert!(size_of::<Error>() <= 128);` (Task 4)
enforces. `DeriveDuplicateEntry` holds one `String` fewer than `DeriveUnknownEntry`.)

- [ ] **Step 4: Write `catalog/derive.rs`**

```rust
//! Deriving a catalog with a new id from named entries of a catalog and new entries.

use serde_json::{Map, Value};
use snafu::{OptionExt, ensure};

use super::Catalog;
use super::document::union;
use crate::error::{DeriveDuplicateEntrySnafu, DeriveUnknownEntrySnafu, Error};

/// Document keys that describe the source catalog, so a derived catalog drops them.
const SOURCE_ONLY: [&str; 4] = ["$id", "title", "description", "instructions"];

/// One entry of a derived catalog: kept from the source, or newly defined.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum DeriveEntry {
    /// Keep the source catalog's entry of this name.
    Keep(String),
    /// A new entry, replacing any source entry of that name.
    Define(String, Value),
}

/// What [`Catalog::derive`] builds: a new id, the entries to keep or add, instructions.
#[derive(Debug, Clone, PartialEq)]
pub struct Derivation {
    catalog_id: String,
    components: Option<Vec<DeriveEntry>>,
    functions: Option<Vec<DeriveEntry>>,
    instructions: Option<String>,
}

impl Derivation {
    /// A derivation to `catalog_id` that keeps every entry and sets no instructions.
    pub fn new(catalog_id: impl Into<String>) -> Self {
        Self {
            catalog_id: catalog_id.into(),
            components: None,
            functions: None,
            instructions: None,
        }
    }

    /// Only these components; without this call, every source component is kept.
    pub fn components(mut self, entries: Vec<DeriveEntry>) -> Self {
        self.components = Some(entries);
        self
    }

    /// Only these functions; without this call, every source function is kept.
    pub fn functions(mut self, entries: Vec<DeriveEntry>) -> Self {
        self.functions = Some(entries);
        self
    }

    /// The derived catalog's instructions for agents.
    pub fn instructions(mut self, text: impl Into<String>) -> Self {
        self.instructions = Some(text.into());
        self
    }
}

/// One kind of entry: its singular name, document section, union and discriminator.
struct Kind {
    name: &'static str,
    section: &'static str,
    union: &'static str,
    discriminator: Option<&'static str>,
}

const COMPONENTS: Kind = Kind {
    name: "component",
    section: "components",
    union: "anyComponent",
    discriminator: Some("component"),
};

const FUNCTIONS: Kind = Kind {
    name: "function",
    section: "functions",
    union: "anyFunction",
    discriminator: None,
};

impl Catalog {
    /// A catalog with a new id, built from entries of this one and new entries.
    ///
    /// It keeps this catalog's protocol version, theme and `$defs`, and drops `$id`, `title`,
    /// `description` and `instructions`, which describe this catalog. A name listed twice in
    /// one list is an error. The result is loaded with [`Catalog::from_json`], so new entries
    /// meet every catalog rule and every entry gets its own compiled validator.
    pub fn derive(&self, derivation: &Derivation) -> Result<Catalog, Error> {
        let mut document = self.kept_document();
        for key in SOURCE_ONLY {
            document.remove(key);
        }
        document.insert(
            "catalogId".to_owned(),
            Value::from(derivation.catalog_id.as_str()),
        );
        if let Some(entries) = &derivation.components {
            self.replace(&mut document, &COMPONENTS, entries)?;
        }
        if let Some(entries) = &derivation.functions {
            self.replace(&mut document, &FUNCTIONS, entries)?;
        }
        if let Some(instructions) = &derivation.instructions {
            document.insert("instructions".to_owned(), Value::from(instructions.as_str()));
        }
        Catalog::from_json(&Value::Object(document), None, None)
    }

    fn replace(
        &self,
        document: &mut Map<String, Value>,
        kind: &Kind,
        entries: &[DeriveEntry],
    ) -> Result<(), Error> {
        let source = document.get(kind.section).and_then(Value::as_object);
        let mut picked = Map::new();
        for entry in entries {
            let (name, schema) = match entry {
                DeriveEntry::Keep(name) => {
                    let schema = source.and_then(|source| source.get(name)).with_context(|| {
                        DeriveUnknownEntrySnafu {
                            catalog: self.id.as_str(),
                            kind: kind.name,
                            name: name.as_str(),
                        }
                    })?;
                    (name, schema.clone())
                }
                DeriveEntry::Define(name, schema) => (name, schema.clone()),
            };
            ensure!(
                !picked.contains_key(name),
                DeriveDuplicateEntrySnafu {
                    kind: kind.name,
                    name: name.as_str(),
                }
            );
            picked.insert(name.clone(), schema);
        }
        let names: Vec<&str> = picked.keys().map(String::as_str).collect();
        let regenerated = union(kind.section, &names, kind.discriminator);
        if let Some(Value::Object(defs)) = document.get_mut("$defs")
            && defs.contains_key(kind.union)
        {
            defs.insert(kind.union.to_owned(), regenerated);
        }
        document.insert(kind.section.to_owned(), Value::Object(picked));
        Ok(())
    }
}

#[cfg(test)]
#[path = "derive_tests.rs"]
mod tests;
```

`kept_document` already writes the source's effective `protocolVersion` (Task 4), so `derive`
only replaces `catalogId`. `union` sorts the names it is given (Task 4), so the regenerated
union lists entries by name whatever order `picked` iterates in.

`catalog/mod.rs`: `mod derive;` and `pub use derive::{Derivation, DeriveEntry};`.
`lib.rs`: extend the catalog re-export with `Derivation, DeriveEntry`, and the crate doc's
catalog item with `` , derived with [`Catalog::derive`] ``.

- [ ] **Step 5: Run the runtime tests**

Run: `cargo nextest run -p a2ui-runtime`
Expected: all pass.

- [ ] **Step 6: Write the `derive` handler**

Create `crates/a2ui-conformance/src/handlers/derive.rs`:

```rust
//! The `derive` action: derive a catalog, then check its entries and its `to_json` document.

use a2ui_runtime::{Catalog, Derivation, DeriveEntry};
use serde_json::{Map, Value};

use crate::case::Case;
use crate::compare::expect_error;
use crate::handlers::catalog_checks::entries;
use crate::handlers::catalog_input::{document, load};
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

/// Loads the case's catalog, derives from it with `args`, and checks the result.
pub fn run(case: &Case) -> Result<(), String> {
    assert_case_keys(
        case,
        &[
            "catalog",
            "catalogPath",
            "catalogId",
            "protocolVersion",
            "args",
            "expect",
            "expectError",
        ],
    )?;
    let at = case.at();
    let input = document(case)?;
    let source = load(case, &input)?.map_err(|err| format!("{at}: load failed: {err}"))?;
    let derivation = derivation(case.get("args"), &at)?;
    match (case.get("expectError"), source.derive(&derivation)) {
        (Some(expected), Err(err)) => expect_error(&err, expected, &at),
        (Some(_), Ok(_)) => Err(format!("{at}: derived, expected an error")),
        (None, Err(err)) => Err(format!("{at}: derive failed: {err}")),
        (None, Ok(derived)) => match case.get("expect") {
            Some(Value::Object(expect)) => check(expect, &derived, &at),
            _ => Err(format!("{at}: a derive case needs an `expect` mapping")),
        },
    }
}

fn derivation(args: Option<&Value>, at: &str) -> Result<Derivation, String> {
    let args = args
        .and_then(Value::as_object)
        .ok_or_else(|| format!("{at}: `args` must be a mapping"))?;
    assert_known_keys(args, &["catalogId", "components", "functions", "instructions"], at)?;
    let id = args
        .get("catalogId")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{at}: `args.catalogId` must be a string"))?;
    let mut derivation = Derivation::new(id);
    if let Some(list) = args.get("components") {
        derivation = derivation.components(derive_entries(list, at)?);
    }
    if let Some(list) = args.get("functions") {
        derivation = derivation.functions(derive_entries(list, at)?);
    }
    match args.get("instructions") {
        None => {}
        Some(Value::String(text)) => derivation = derivation.instructions(text.as_str()),
        Some(_) => return Err(format!("{at}: `args.instructions` must be a string")),
    }
    Ok(derivation)
}

fn derive_entries(list: &Value, at: &str) -> Result<Vec<DeriveEntry>, String> {
    let items = list
        .as_array()
        .ok_or_else(|| format!("{at}: a derive list must be a list"))?;
    items
        .iter()
        .map(|item| match item {
            Value::String(name) => Ok(DeriveEntry::Keep(name.clone())),
            Value::Object(entry) => match entry.iter().collect::<Vec<_>>().as_slice() {
                [(name, schema)] => Ok(DeriveEntry::Define((*name).clone(), (*schema).clone())),
                _ => Err(format!("{at}: a derive entry map has exactly one key, got {item}")),
            },
            other => Err(format!("{at}: a derive entry is a name or a one-key map, got {other}")),
        })
        .collect()
}

fn check(expect: &Map<String, Value>, derived: &Catalog, at: &str) -> Result<(), String> {
    assert_known_keys(expect, &EXPECT_KEYS, at)?;
    entries(expect, derived, at)?;
    if let Some(expected) = expect.get("instructions")
        && expected.as_str() != derived.instructions()
    {
        let actual = derived.instructions();
        return Err(format!("{at}: instructions {actual:?}, expected {expected}"));
    }
    let Some(absent) = expect.get("absentKeys") else {
        return Ok(());
    };
    let json = derived.to_json();
    let keys = absent
        .as_array()
        .ok_or_else(|| format!("{at}: `absentKeys` must be a list"))?;
    match keys
        .iter()
        .find(|key| key.as_str().is_none_or(|key| json.get(key).is_some()))
    {
        Some(key) => Err(format!("{at}: key {key} is present in to_json")),
        None => Ok(()),
    }
}
```

`handlers/mod.rs`: `mod derive;`, `"derive" => Some(derive::run),`. Remove `derive` from the
`[actions]` table of `skips.toml`.

- [ ] **Step 7: Run the derive cases**

Run: `cargo nextest run -p a2ui-conformance --test core derive`
Expected: the seven v1.0 derive cases pass;
`test_v09_catalog_derive_keeps_protocol_version` ignored through `[versions]`.

- [ ] **Step 8: Update the architecture notes**

`crates/a2ui-runtime/ARCHITECTURE.md`: extend the `catalog` bullet with `derive` (a catalog with
a new id from a `Derivation`, keeping the source's version, theme and `$defs`, rejecting
duplicate names, regenerating the unions of the lists it changes, loaded by `from_json`).
`crates/a2ui-conformance/ARCHITECTURE.md`: `handlers` gains `derive`. Both stay at most 50
lines: condense older runtime bullets again if needed, and check both with `wc -l`.

- [ ] **Step 9: Gate and commit**

Run: `just check` then `just test`. Expected: green.

```bash
git add crates/a2ui-runtime crates/a2ui-conformance
```

Message:

```
feat(runtime): derive a catalog from named and new entries (t-1560)

Catalog::derive keeps the source's version, theme and $defs, drops the fields that describe the
source, rejects a name listed twice, regenerates the unions of the entry lists it changes and
loads the result with from_json. The derive cases run.
```

Then `oku cli task done --project slider --id t-1560 --result "<one line>"`.

---

### Task 8: Consolidation and Plan 2 gate

Oku: t-1561.

**Files:**
- Modify: `crates/a2ui-conformance/skips.toml`, `README.md`, `crates/a2ui-runtime/src/lib.rs`
  (crate doc), any file the checks below flag.

- [ ] **Step 1: Point the last C2 skip at Plan 2b**

In `crates/a2ui-conformance/skips.toml`, the `validation_schema` entry of `[actions]` becomes:

```toml
validation_schema = "Plan 2b: validationSchema flattening matching upstream goldens (t-1564)"
```

`[actions]` must now hold no other C2 action:
`grep -n "Plan 2" crates/a2ui-conformance/skips.toml` prints only that line.

- [ ] **Step 2: Consolidation pass**

Run each and fix what it finds (G10):

```bash
grep -rn "dead_code\|#\[allow" crates/
grep -rnE '\.(unwrap|expect)\(' crates/*/src --include='*.rs' \
  | grep -v '_tests\.rs:\|/test_support\.rs:'
grep -rn "fn is_identifier\|fn localize_ref\|fn pointer_of\|fn collect_refs\|fn refs(" crates/
find crates -name '*.rs' -exec wc -l {} + | sort -n | tail -5
wc -l crates/*/ARCHITECTURE.md
```

Expected: the first two print nothing (no `dead_code` expectation or `allow`; no `unwrap`
or `expect` outside tests). `is_identifier` is defined once
(`ident.rs`), `localize_ref` and `pointer_of` once (`bundle.rs`); the two `$ref` collectors are
the test helper in `compile_tests.rs` and `catalog_checks::collect_refs` (harness), which stay
apart because the harness must not reuse runtime internals to judge the runtime. No source file
is above 1000 lines and any above 500 is split by concept; every `ARCHITECTURE.md` is at most
50 lines.

Check the crate doc of `crates/a2ui-runtime/src/lib.rs` lists every re-export:
`Catalog`, `CatalogLookup`, `ComponentDef`, `ComponentShape`, `FunctionDef`, `AllowedCallers`,
`Derivation`, `DeriveEntry`, `common_types_schema`, `agent_to_renderer_schema`,
`validate_common_type`.

- [ ] **Step 3: Full gate and manifest**

Run: `just ci`
Expected: green (fmt, clippy, `just deps`, wasm for both crates, every test, `cargo deny`,
`cargo audit`).

Then write the executed-case manifest. `Manifest::from_env` (`src/manifest.rs`) appends one
compact JSON line per case (no spaces after `:`), with the keys `harness` (`rust-core`),
`suite` (`core/<file>.yaml`), `case`, `status` (`passed`, `failed`, `skipped`) and, for
`skipped` only, `reason`, to `$A2UI_CONFORMANCE_MANIFEST_DIR/rust-core-<pid>.jsonl`; it does
not create the directory. The `grep`s below match single key-value pairs, so key order does
not matter:

```bash
rm -rf target/manifest-plan2 && mkdir -p target/manifest-plan2
just conformance-manifest target/manifest-plan2
grep -h '"status":"failed"' target/manifest-plan2/*.jsonl
grep -hE '"suite":"core/(catalog|common_types|agent_to_renderer)\.yaml"' \
  target/manifest-plan2/*.jsonl | grep -c '"status":"passed"'
grep -h '"reason":"Plan 2b' target/manifest-plan2/*.jsonl | grep -o '"case":"[a-z0-9_]*"' | sort
```

Expected: `just conformance-manifest` exits 0; the first `grep` prints nothing; the second
prints `58`: every v1.0 case of the three C2 suites (catalog: from_json 32, to_json 1,
round_trip 5, derive 7; common_types: common_types_schema 1, validate_common_type 11;
agent_to_renderer: agent_to_renderer_schema 1) except the two v1.0 `validation_schema` cases;
no other version of those suites runs, each is skipped through `[versions]`. The third prints
the five `validation_schema` cases, whose `[actions]` entry wins over `[versions]`:

```
"case":"test_v08_basic_validation_schema"
"case":"test_v09_basic_validation_schema"
"case":"test_v09_custom_validation_schema_flattening"
"case":"test_v10_basic_validation_schema"
"case":"test_v10_custom_validation_schema_leaf_defs"
```

- [ ] **Step 4: Status in the README**

Replace the Status paragraph of `README.md` (the single line under `## Status`, before the
license line) with:

```markdown
Conformance: stages C1 (data model, paths, expression parsing) and C2 (catalogs, common types,
agent-to-renderer schema) pass, except the `validationSchema` golden comparison (Plan 2b);
later stages are listed in crates/a2ui-conformance/skips.toml.
```

- [ ] **Step 5: Commit and close**

```bash
git add README.md crates
```

Message:

```
chore(conformance): close stage C2, leaving validationSchema goldens to Plan 2b (t-1561)
```

Then
`oku cli task done --project slider --id t-1561 --result "<C2 passes except validation_schema>"`,
and close the parent: `oku cli task done --project slider --id t-1544 --result "<one line>"`.

---

## Deferred (backlog)

Found by the review of this plan and left out of its scope; each is an Oku backlog task
(t-1565 to t-1574, linked under t-1544):

- `Error::code()`: stable A2UI error codes beside the categories (Plan 3, with the message
  processor that reports them).
- `LoadOptions` for `Catalog::from_json`: caller-set caps instead of the fixed `limits`
  constants, once a host needs other values.
- Instance size and call caps for `validate_component` (Plan 3, in the message layer that
  receives components): a cap on a component's JSON nodes and on the nested calls one
  component holds, beside the depth cap it already has.
- `Catalog::from_json(&Value)` precondition: document that a caller parsing untrusted text
  must bound it first (as `Catalog::parse` does with `MAX_CATALOG_BYTES`), since the node cap
  runs after parsing; and fuzz `Catalog::parse` and `validate_component` (`cargo-fuzz`).
- Spec: the function registry of Plan 4 (`FunctionRegistry`) reuses `FunctionDef` and
  `AllowedCallers` rather than a second function description; amend the design spec.
- `$defs` shadowing: a catalog `$defs` named like a common type is rejected, so a 1.0.x
  `common_types.json` adding a type can reject a catalog valid under 1.0.0; decide between
  version-scoped common types and namespacing before the first 1.0.x schema update.
- Truncate `SchemaViolation` messages: jsonschema embeds the offending instance in its
  message, so one large invalid value makes a large error; cap the text (with a marker) at
  the error boundary.
- A shared basic catalog in a `std::sync::LazyLock` (`a2ui_catalog_basic::catalog()`
  recompiles it on each call, about 20 ms) once the renderer loads it on a hot path.
- wasm bundle size: measure what `jsonschema` adds (`fancy-regex`, `num-*`, `ahash`); if too
  large, evaluate `boon` as the validator.
- A wasm runtime smoke test that builds and runs a catalog validator in a browser (Plan 6,
  `wasm-bindgen-test`): today wasm32 is only built, never run.
