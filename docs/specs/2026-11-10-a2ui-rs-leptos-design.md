# a2ui-rs: a generic A2UI v1.0 runtime and Leptos renderer — design

Task: t-1544. Status: approved design, implementation not started.

## Context and goal

Oku's UI renders A2UI surfaces with `@a2ui/react` + `@a2ui/web_core` (protocol v0.9). The long-term
direction is a Leptos UI. No Leptos A2UI renderer exists, and the Rust A2UI crates surveyed on
2026-11-10 do not fit as a core: `a2ui`/`a2ui-base` (0.3.x, June 2026, depends on `thiserror` and
`chrono`, wasm unverified, no verified catalog registry), `car-a2ui` (server-side store, pulls
`tokio`), `fenestra-a2ui` and `a2ui-dioxus`/`a2ui-egui`/`a2ui-tui` (tied to their GUI toolkit).
Upstream (github.com/a2ui-project/a2ui, Apache-2.0) is now at protocol **v1.0** and ships a
language-neutral YAML conformance suite with no Rust harness. `@a2ui/react` 0.15.0 has no v1.0
renderer; only `@a2ui/web_core` (since 0.12) supports v1.0.

Goal: a **generic, reusable** Rust implementation of the A2UI v1.0 renderer side, in a **separate
repository**, with a Leptos renderer. Oku adopts it later, migrating to v1.0 and to Leptos-rendered
surfaces in one step (separate spec).

## Decisions

- Generic library, not Oku-specific; Oku's catalog becomes a separate crate plugged in later.
- Separate repository `a2ui-rs` (working name), license MIT OR Apache-2.0.
- Protocol v1.0 only (no v0.9/v0.8). Oku migrates when it adopts the Leptos renderer; no throwaway
  React v1.0 adapter.
- Architecture A: a framework-free core exposing path/component subscriptions; Leptos maps them to
  signals. Rejected: core on `reactive_graph` (ties the core to Leptos, conformance needs a
  reactive runtime) and a single crate (core not reusable, conformance coupled to rendering).
- Crate names checked free on crates.io (2026-11-10): `a2ui-runtime`, `a2ui-leptos`,
  `a2ui-conformance` (`a2ui-core` is taken); check `a2ui-catalog-basic` before the first publish.

## 1. Scope and repository layout

Cargo workspace; upstream `specification/v1_0`, `catalogs/basic/v1` and `conformance/` vendored by
`vendor/sync.sh` (a sparse clone copying only those directories; `git subtree` would pull every
upstream SDK) at a pinned commit recorded in `vendor/UPSTREAM`. The directories
copied also include `catalogs/mcp/v1` and `catalogs/vegachart/v1`, which the catalog suite
names. `vendor/sync.sh` further copies upstream's `LICENSE` (and its `NOTICE`, as `NOTICE-A2UI`,
when it exists) beside the schemas the runtime embeds, since a crate cannot include files outside
its directory.

| Crate | Published | Role |
|---|---|---|
| `a2ui-runtime` | yes | Full renderer-side v1.0 protocol, pure Rust |
| `a2ui-catalog-basic` | yes | Basic catalog (18 components) data + 14 functions, pure Rust |
| `a2ui-leptos` | yes | Leptos 0.8 CSR surface, renderer trait, basic catalog rendering |
| `a2ui-conformance` | no | YAML conformance harness over `a2ui-runtime` |

`a2ui-runtime` covers: the six agent→renderer messages (`createSurface`, `updateComponents`,
`updateDataModel`, `deleteSurface`, `callRendererFunction`, `agentFunctionResponse`) and the four
renderer→agent ones (`action`, `callAgentFunction`, `rendererFunctionResponse`, `error`); surface
lifecycle with buffering until `root` exists and integrity errors; the data model (JSON Pointer,
relative paths, `@index`, two-way writes); `@path`/`@call` expressions and `formatString`; the
function registry with `allowedCallers`, `requiresUserActivation` and local/agent routing; `checks`
and the guarded regex subset; catalog loading and validation (`allowedParents`/`allowedChildren`,
inline catalogs); `sendDataModel`; subscriptions.

Out of scope for 0.1: v0.9/v0.8, SSR/hydration, the per-framework `adapter_conformance` suite, the
agent side.

## 2. `a2ui-runtime` internals

It builds for wasm32-unknown-unknown; running needs a JS host (getrandom `wasm_js`).

One concept per module:

- `message`: serde types `AgentMessage` / `RendererMessage`; `"version": "1.0"` checked on read.
- `pointer`: RFC 6901 JSON Pointer and relative-path resolution against a template scope.
- `data`: `DataModel` over `serde_json::Value`; get/set/delete with missing-path creation; every
  write returns the paths it touched.
- `dynamic`: `Dynamic<T>` (literal, `@path`, `@call`) and an `Evaluator` resolving in a scope; a
  failed binding yields `null` plus an `EXPRESSION_ERROR`, siblings keep rendering.
- `function`: `FunctionRegistry` (`returnType`, `allowedCallers`, `requiresUserActivation`, local
  vs `callAgentFunction` routing); implementations are `Fn(&Args, &Ctx) -> Result<Value>` registered
  per catalog.
- `validator`: catalog `pattern`s compile with `jsonschema`'s `regex` engine (linear time, 128 KiB
  size and DFA limits, look-around refused, a per-catalog pattern cap); the runtime never runs a
  backtracking regex.
- `catalog`: catalog loading and validation (no allowed top-level key list, since upstream
  accepts `theme`, catalog-level `metadata` and vendor keys; restricted `$ref` targets never
  fetched; reserved `Surface` name; size and depth caps; parent/child rules); a surface's
  `CatalogSet`.
- `surface`: `Surface` = id → `ComponentNode` map, buffer until `root`, default catalog, `DataModel`.
- `processor`: `MessageProcessor::handle(msg) -> Outcome { outbound: Vec<RendererMessage>,
  changes: ChangeSet }`; no I/O, no async — transport belongs to the application. User input goes
  through `input(surface, path, value)` and `gesture(...)`.
- `action`: gesture → pipeline (write → function → event), stopping at the first failing step.
- `subscribe`: `ChangeSet` (surfaces created/deleted, components changed, paths changed) and
  `Watchers` mapping paths/components to subscriber ids; a path change notifies ancestors and
  descendants.

Errors: opaque Snafu `Error` with a backtrace in every variant, exposing `code() -> ErrorCode` (the
A2UI codes: `VALIDATION_FAILED`, `UNALLOWED_PARENT`, `UNALLOWED_CHILD`, `INTEGRITY_ERROR`,
`INVALID_FUNCTION_CALL`, `EXPRESSION_ERROR`, `EXECUTION_ERROR`, `UNKNOWN_FUNCTION`) and
`category()` matching the harness taxonomy (`Parse`, `Validation`, `Integrity`, `Recursion`,
`Catalog`, `Data`, `Expression`, `State`, `Compilation`). `Display` is English for logs; the
application localizes from the code.

Dependencies: `serde`, `serde_json`, `snafu`, `regex`, `time`, `unicode-ident`,
`percent-encoding`, and `jsonschema` (default features off: no HTTP or file `$ref` resolution;
patterns on the linear `regex` engine; builds offline). The published v1.0 schemas are the
runtime's models: it embeds `common_types.json` and `agent_to_renderer.json` and validates
against them. No `tokio`, no randomness in any result; the clock is injected (action
timestamps). On wasm32, `jsonschema` brings `wasm-bindgen` through `getrandom`, which seeds its
hash maps; that is the only path (`just deps` checks it) and no outcome depends on it.

## 3. `a2ui-catalog-basic` and `a2ui-leptos`

`a2ui-catalog-basic`: upstream basic `catalog.json` embedded with `include_str!` and loaded through
`a2ui-runtime::catalog`; the functions `required`, `regex`, `length`, `numeric`, `email`,
`formatString`, `formatNumber`, `formatCurrency`, `formatDate`, `pluralize`, `openUrl`, `and`,
`or`, `not`. Locale formatting takes an injected `Locale` (no ICU by default, optional `icu`
feature); `openUrl` delegates to an injected `UrlOpener`.

`a2ui-leptos`:

- `A2uiHost` owns the `MessageProcessor` in a `StoredValue`; `handle(msg)` applies it, passes
  `outbound` to the application's `on_outbound` callback and fires the signals named by the
  `ChangeSet`.
- One `Trigger` per subscribed component and observed path, created lazily on first render and
  released on unmount (`on_cleanup` unregisters it from `Watchers`).
- `<Surface host id/>` renders the implicit `Surface` then `root`; a missing component renders an
  `a2ui-placeholder`.
- `trait ComponentRenderer { fn render(&self, ctx: NodeCtx) -> AnyView }`, where `NodeCtx` offers
  `prop::<T>(name) -> Signal<T>`, `child(id)`, `children()`, `write(path, value)`, `fire(action)`.
  A `RendererRegistry` keys renderers by `(catalogId, component)`; custom catalogs (Oku's) plug in
  the same way.
- Basic catalog rendering: semantic HTML, `a2ui-<component>` classes and `data-variant`, no theme;
  optional `structural.css` for layout only. Accessibility: labels tied to inputs, `role`/`aria-*`
  for Tabs and Modal, focus management in Modal. Video and AudioPlayer use native elements.
- Clicks and submits go through `processor.gesture`, giving `openUrl` real user activation.

Dependencies: `leptos` 0.8 (CSR), `web-sys` (Modal focus only).

## 4. Testing, conformance, CI

- Unit tests in sibling `x_tests.rs`, speculoos, rstest tables, names `unit_scenario_result`, one
  behavior per test; `insta` snapshots for outbound `RendererMessage`s.
- `a2ui-conformance`: one nextest case per YAML case (`libtest-mimic`, since `datatest-stable`
  gives one test per file), parsed with `serde_yaml_ng`; strict key checks as the upstream harness
  guide requires; writes upstream's executed-case manifest on demand; loads catalogs through the
  public API, runs the action
  (`process_messages`, `validate`, `resolve_nodes`, `evaluate_function`,
  `parse_expression_template`, `handle_rpc`, `get_renderer_capabilities`, data-model ops),
  normalizes to JSON and compares `expect`/`expectError`, mapping `category()` to the harness
  categories. `skips.toml` lists `SKIP_TEST_NAMES` with a reason each; an unmatched skip fails the
  run. Catalog loading is relaxed outside `CatalogError` cases, as in the reference harnesses.
  Target: every v1.0 `core/*` case passes at 0.1; the only skips left are cases declaring another
  protocol version or whose catalog declares none (upstream's default is v0.9) (the
  version-neutral `data_model`, `resolve_path`, `parse_expression_template`
  run whatever version they declare) and the two adapter suites, which `a2ui-leptos` runs in the
  browser. Any other remaining skip blocks the release.
- Delivery: seven plans following the upstream harness stages (C1 data model and expressions; C2
  catalogs and schemas, with `a2ui-catalog-basic` holding the catalog document; C2b the
  `validationSchema` flattening matching upstream's goldens (the theme written as an open
  `$defs.theme`; a non-string `instructions`, a `theme` that is no schema or one beside a
  `$defs.theme` is refused at load); C3 validation and processing; C4
  functions/actions/RPC, adding the basic functions to `a2ui-catalog-basic`; C5 node resolution
  and timing; then `a2ui-leptos`); each plan is written when the previous lands.
- Leptos: `wasm-bindgen-test` in headless Chrome — mount `<Surface>` on message streams, check the
  DOM, check that only subscribed components re-render (counted by a test hook), check unmount.
  `examples/gallery` (Trunk) shows every basic component from the upstream fixtures.
- CI (GitHub Actions = `just ci`): fmt, clippy `-D warnings`, nextest, `cargo build --target
  wasm32-unknown-unknown` for `a2ui-runtime` and `a2ui-leptos`, headless wasm tests, `cargo audit`,
  `cargo deny` (licenses). Releases through release-plz, workspace at 0.1.0.

## 5. AGENTS.md carried over

The new repository's root `AGENTS.md` takes Oku's generic rules, each re-sourced to this spec:

- Snafu errors, a backtrace in every variant, opaque public types.
- No `unwrap`/`expect` outside tests, no silent fallback hiding a bug.
- Aim for 500-line files, never above 1000; split by concept.
- Sibling `x_tests.rs` tests, speculoos, rstest, `unit_scenario_result` names, regression tests in
  `tests/regressions/issue_NNNN_topic.rs`; tempfile for temp dirs.
- Versions in `workspace.dependencies` with `.workspace = true`; never `thiserror` or `chrono`.
- `#[expect(..., reason)]` over `#[allow]`; no dead code.
- Document every pub item and module, first sentence under 15 words; `ARCHITECTURE.md` per crate,
  at most 50 lines, current.
- Conventional Commits; `just check` / `just test` / `just ci` / `just wasm`.

New rules specific to the library:

- Errors carry stable A2UI codes; the library never localizes (no Fluent).
- `a2ui-runtime` has no direct Leptos, wasm-bindgen, async-runtime or I/O dependency
  (`wasm-bindgen` reaches it only through `jsonschema`'s `getrandom`, see §2).
- Vendored upstream files are updated only through `vendor/sync.sh`, which rewrites
  `vendor/UPSTREAM`.

Oku-only rules (Loro, Typst, OpenDAL storage, Fluent, the duplicated `CATALOG_ID`) stay in Oku.

## Follow-up sub-projects (separate specs)

- **Oku migration**: emit v1.0 from `oku-slides-a2ui`/`oku-slides-session`, port the Oku catalog
  (`Receipt`, `Timeline`, `Prompt`, `ListTable`, `ChartData`, `TableData`, `RichTextField`, …) to an
  `oku-slides-a2ui-leptos` crate, mount Leptos surfaces in the React UI.
- **Rich text editing**: evaluate `taino-edit` (Leptos WYSIWYG, 0.3) for the markdown fields
  (`TextEditor`, `RichTextField`); it needs a custom mark for `==highlight=={n}` and must keep the
  lossless markdown round trip. It does not replace CodeMirror for `TypstEditor` (code editing).
- **Parsing**: replace Lezer with Rust parsers. For Typst, prefer `typst-syntax` (pure Rust, the
  compiler's grammar, already in `Cargo.lock`) over tree-sitter-typst. For Oku's inline markdown
  subset, a Rust parser shared with `runs_to_markdown` (`oku-slides-session/src/inline.rs`) beats
  tree-sitter-markdown, which would need a grammar fork for `==…==` and a C runtime on
  `wasm32-unknown-unknown` (arborium sysroot or c2rust fork).
