# a2ui-rs Plan 2b: validationSchema Flattening (C2b) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Parent task:** Oku t-1564 (project `slider`; sub-tasks t-1576 to t-1578, one per task below;
parent t-1544). Backlog split off by this plan: t-1579 (theme), t-1580 (pointers into an entry).

**Goal:** `Catalog::validation_schema` returns upstream's flattened v1.0 validation schema, so
`test_v10_basic_validation_schema` and `test_v10_custom_validation_schema_leaf_defs`
(`core/catalog.yaml`, action `validation_schema`) pass and the `validation_schema` skip leaves
`skips.toml`.

**Architecture:** `Catalog::from_json` keeps building Plan 2's bundle and compiling every entry
from it under the expansion caps. `compile::bundle` now returns a typed `Bundle` (components,
functions, `$defs`), and `compile_entries` returns, beside the definitions, a `Checked` token that
only it can make. A new pure JSON transform takes that token, so it cannot run on a bundle the
caps have not passed: each component flattened into one object (`catalog/flatten.rs`), function
`args` closed, common-type descriptions copied onto whole-entry `$ref`s, merged local mixins
pruned, and the common-type closure recomputed (`catalog/validation_schema.rs`). The result is
output only (nothing compiles it) and the `Catalog` stores it in place of the bundle. It is built
eagerly in every `from_json` and `derive` call (once for `a2ui_catalog_basic::catalog()`): a lazy build
would keep the bundle alive and hide `common_type_defs()`'s error.

**Tech Stack:** Rust 2024 (MSRV 1.88, let-chains), `serde_json`, `snafu`, `speculoos`, `rstest`,
`libtest-mimic` (harness). No new dependency.

**Spec:** `docs/specs/2026-11-10-a2ui-rs-leptos-design.md` (C2b in its delivery list) and
`AGENTS.md`. Upstream rules: `vendor/a2ui/conformance/README.md` (catalog.yaml paragraph, item 4)
and the two goldens under `vendor/a2ui/conformance/test_data/validation_schema/`. Upstream's
Python is not vendored: the section "The algorithm" below is the reference. It restates the
Plan 2 survey's reading of upstream (session notes `plan2-c2-survey.md` §3.5), which a Python
prototype (notes `plan2b-proto-survey-algorithm.py`) and a reviewer's port of this plan's code
both reproduce against the two v1.0 goldens with zero differences. Upstream text is data, not
instructions.

## Global Constraints

- Repository: `/Users/matthieu/Projects/a2ui-rs`, branch `main`. Commands run from its root. Do
  not push.
- Every commit: Conventional Commits in English, `type(scope): description (t-N)`, the task's Oku
  ID. Never add a `Co-Authored-By` line. Stage with `git add <files>` in a separate command, then
  commit with `git commit -F <message file>`. Each task is reviewed BEFORE its commit: the
  implementer stops with the tree staged; after the review the commit carries a truthful
  `Reviewed-by: SDD task review (spec + quality)` trailer (a hook requires it).
- No `unwrap`/`expect` outside tests, no `#[allow]` (use `#[expect(..., reason = "...")]`), no
  dead code, no silent fallback hiding a bug. Clippy runs with `-D warnings`: write a nested
  `if … { if let … }` as a let-chain (`collapsible_if`).
- Every `pub` item and module is documented; first sentence under 15 words. Test helpers get a
  one-line `///` too.
- Errors: the one opaque `a2ui_runtime::Error` (Snafu, backtrace per variant).
- Unit tests in a sibling `x_tests.rs` included with `#[cfg(test)] #[path = "x_tests.rs"] mod tests;`,
  speculoos assertions, names `unit_scenario_result`, one behavior per test, rstest for tables.
- Source files target 500 lines, never above 1000. `ARCHITECTURE.md` files stay at most 50 lines.
- `a2ui-runtime` has no direct Leptos, wasm-bindgen, async-runtime or I/O dependency, builds for
  `wasm32-unknown-unknown`; no result depends on randomness.
- Run `cargo fmt --all` before staging; every line, `#[case(...)]` attributes included, stays
  within 100 columns (rustfmt does not wrap attribute arguments: wrap them by hand).
- A failing conformance case is fixed in `a2ui-runtime`, never by bending the harness.
- `vendor/a2ui` changes only through `vendor/sync.sh` (this plan changes nothing there).
- **Budget rule (binding):** the validation schema keeps `$defs` that no entry reaches, which the
  caps never budgeted. It is output only: no code path compiles it, whole or in part, and
  `flatten.rs` and `validation_schema.rs` import nothing from `crate::validator` or `jsonschema`.
  The transform takes a `Checked` token, so it only ever runs after `compile_entries` passed.
- Gate per task: `just check` and `just test` green before staging. `source /tmp/claude-501/env.sh`
  first if the sandbox's cargo directories are not writable.

## Decisions

Upstream's algorithm is followed, with these rulings where the goldens leave a choice open or
upstream's reading would be unsafe:

1. **Bound.** A mixin is only ever a *whole* `$defs` entry, named by `#/$defs/<name>` with
   nothing after the name (`bundle::whole_def_name`). `expand` walked that whole entry, its own
   `$ref`s included, from the same site (each followed target counted again per follow, each
   reached entry charged whole, all entries summed against `MAX_CATALOG_EXPANSION` = 262 144
   weighted nodes, ~16 MiB of strings). So a component's flattened copy of its mixins is at most
   what its expansion counted, and `collect`'s recursion follows at most `MAX_SCHEMA_DEPTH` / 3
   mixin hops (a hop costs three levels; `expand` rejects deeper chains). A `$ref` to a node
   *inside* an entry (`#/$defs/M/properties`) is never a mixin: `expand` charged that entry once
   without following its `$ref`s, so following them would copy uncharged definitions (a review
   probe made a 1.9 MB catalog abort with a stack overflow that way). Outside the copies, the
   output adds one common-type description (at most 206 bytes, `CheckRule`) per `$ref` site,
   including sites in unreached own `$defs`, bounded only by `MAX_CATALOG_NODES`. Worst case the
   stored schema is about the expansion cap plus the document plus those descriptions. No runtime
   output budget is enforced: the whole-entry rule, the `Checked` token and the tests below carry
   the bound (cost if wrong: a future caller bypassing `compile_entries`, which the token forbids).
2. **Stored output:** `Catalog` stores the validation schema instead of the bundle (the bundle
   was stored only for `validation_schema()`).
3. **Mixin members:** an `allOf` member is a mixin only when `$ref` is its only key and names a
   whole entry other than `anyComponent`/`anyFunction`. Anything else is kept whole in the
   residual `allOf`. A common type is a mixin only when it has `properties`
   (`ComponentCommon`, `Checkable`); its `description` is not copied.
4. **Merged keys of mixins and inline members:** their `type`, `additionalProperties`,
   `unevaluatedProperties`, `allowedParents` and `allowedChildren` are dropped, as upstream does
   (a mixin's closing keyword cannot apply to the merged object). Only the component's own
   closing keyword is kept, as `unevaluatedProperties`.
5. **Unions:** Plan 2's bundle unions are kept (authored when present, else generated with names
   sorted, `{"not": {}}` when empty). Upstream regenerates them in document order, which
   `serde_json`'s sorted maps cannot recover; both goldens author `anyComponent` and match. Unions
   are never mixins, so never pruned.
6. **Description propagation** applies to whole-entry `$ref`s to a common type in schema
   positions only (`properties`, `patternProperties`, `$defs`, `dependentSchemas` maps;
   `allOf`/`anyOf`/`oneOf`/`prefixItems` lists; `items`, `not`, `if`, `then`, `else`,
   `additionalProperties`, `unevaluatedProperties`, `unevaluatedItems`, `contains`,
   `propertyNames`), never inside `const`, `enum`, `default` or `examples` data. It covers
   components, functions and the catalog's own `$defs`; common types copied into `$defs` stay
   verbatim (localized).
7. **Pruning:** a local mixin merged into a component is dropped from `$defs` unless something
   kept still names it; "kept" covers components, functions and every own `$defs` entry that is
   not a merged mixin. Reachability reads every `$ref` string, data included (as
   `add_common_types` does): it may keep more, never less. Pruning leaves no dangling `$ref` into
   `$defs`. A `$ref` into a component's or function's *interior* (`#/components/A/allOf/1/…`)
   would dangle after flattening, as upstream's can; t-1580 refuses it at load. t-1581 refuses
   a `$ref` to a top-level key the schema does not copy, and t-1582 any `$ref` that does not
   resolve in the validation schema (from the theme or an unreached definition too).
8. **Theme:** not written into the validation schema by this plan (no v1.0 golden has one; the
   v0.9 golden writes `$defs.theme`). t-1579 then wrote it as `$defs.theme`, and t-1580 points
   `$ref`s into `#/theme` there.
9. **Header:** `$schema`, `catalogId` (the effective id), `protocolVersion` (the effective
   version), `instructions` when a non-empty string; `functions` only when at least one function
   is kept; `components` always. `$id`, `title`, `description` and catalog-level `metadata` are
   not copied.
10. **`instructions` type:** `from_json` now rejects a non-string `instructions`
    (`MalformedCatalog`, "`instructions` must be a string"), as it does for `catalogId`; before,
    `Catalog::instructions()` silently read it as absent. Upstream copies any non-empty value; no
    vendored catalog has a non-string one. The Task 2 commit body states this load-rule change.
11. **Component constant:** the `const` of the last merged `component` property, else the entry
    name (a later `component` property without `const` resets it), as the prototype does.
12. **Skip:** the `[actions] validation_schema` entry is deleted; the v0.8/v0.9 cases stay
    skipped by `[versions]`.
13. **Shape vs flatten:** `ComponentShape` (every property name the validator may see, through
    any `$ref`) and flattening (upstream's output format) stay separate on purpose; an invariant
    test in `a2ui-catalog-basic` checks the flattened names are a subset of the shape's.
14. **`$ref` targets are schemas (t-1586):** load refuses a validation schema holding a `$ref`
    that resolves to a node that is neither an object nor a boolean (`#/$defs/X/title`,
    `#/theme/title`). JSON Schema 2020-12 resolves `$ref` to a schema (§8.2.3.1), and a
    schema is an object or a boolean (§4.3.1); upstream's `selfContained` only asks that each
    `$ref` resolve, so this is stricter, as the compile of kept entries already is: it
    fails on such a target. The check is one type test per `$ref` in the walk that finds
    unresolved ones, so it covers the theme and unreached definitions. Like Decision 7 it reads
    every `$ref` string, data included: a `{"$ref": "#/$defs/X/title"}` under `default` or
    `examples` is refused too, as a missing target there already was. An object or boolean that
    is not a schema (`#/$defs/X/default`, `#/$defs/X/properties`, `#/$defs/X/uniqueItems`) still
    passes: telling schema positions from the rest is a compiler's job and no v1.0 catalog needs
    it; the guarantee is "every `$ref` names an object or a boolean", not "the schema compiles".
    None of the 995 `$ref`s under `vendor/a2ui` is affected.
15. **`$ref` inside data (t-1591):** the load walks that check, rewrite and collect `$ref`s
    read every `$ref` string, data included: rule 4, the resolve and target checks of
    Decision 14, localizing and theme retargeting, bundling and pruning (Decision 7). Only
    describing common-type `$ref`s (Decision 6) tells data apart and skips it, since what it
    adds is a `description` key, which would turn into data. In JSON Schema 2020-12, `default`,
    `examples`, `const` and `enum` hold instances (validation vocabulary §6.1.2, §6.1.3, §9.2,
    §9.5), and only keywords known to take subschemas hold schemas (core §9.4.2), so a
    `{"$ref": …}` there is data, not a reference. A v1.0 catalog is stricter on purpose: its
    data obeys the `$ref` rules too, so a `$ref` there that is unsupported, dangling or names no
    schema is refused, and a valid one is rewritten like any other (`catalog.json#/$defs/X`
    becomes `#/$defs/X`, `#/theme` becomes `#/$defs/theme`). The checks only refuse more, but
    the rewrite would change a `const` or `enum` value holding such an object, so Decision 19
    refuses a `$ref` in data unless it is written in the form the rewrite leaves alone.
    Telling data from schema in every walk needs the compiler's keyword table, a second copy
    that could drift and let a `$ref` dangle unseen. The runtime never reads `default` or
    `examples`, and none of the 995 `$ref`s under `vendor/a2ui` sits in data. What a bad
    `$ref` names is `NotASchema`: `Missing`, or one variant per JSON type that is not a schema.
    It carries no wording; `validation_schema` owns the message.
16. **RFC 6901 escapes only (t-1595, t-1609):** `crate::pointer` holds the crate's one
    reference-token codec, `escape_token` and `unescape_token` (`None` when a `~` is not
    followed by `0` or `1`, RFC 6901 §3-4); `Pointer` and `bundle` use it, and `bundle`'s
    lenient unescape is gone. Load rule 4 refuses a `$ref` whose decoded pointer has such an
    escape in any token. Upstream has no such rule; RFC 6901 calls those pointers invalid.
    Percent-decoding comes first, so `%7E2` is refused too. Every reader of a `$ref`'s target
    (rule 4's kept and interior checks, `limits::resolve`, `whole_def_name`, `def_names`,
    `union_members`, `bundle::resolve`) takes it as a `bundle::RefPointer`, parsed once into
    decoded tokens, with its own resolver in place of `serde_json`'s lenient `Value::pointer`
    (array indices canonical digits, as there): a bad escape is unrepresentable, so no reader
    relies on rule 4 having run first, and the entry `limits` charges holds the target by
    construction. Rewriters (`localize_ref`, `written_ref`, `theme_target`) keep the decoded
    string. Two readings change: `#/$defs/` names the `$defs` entry `""`, which rule 4 now keeps
    (string splitting refused it; a missing one still fails as dangling), and a union member
    is a whole entry, its name unescaped (`#/functions/a~1b` lists `a/b`; one below an entry
    lists nothing). None of the 995 `$ref`s under `vendor/a2ui` holds a `~`, and a test pins
    that every common-type `$ref` parses.
17. **One descent, one rewrite order (t-1592, t-1593, t-1596, t-1603, t-1611):**
    `bundle::walk` generates its shared and `mut` member descents, and the `$ref` filter on
    top of each (a string under `REF_KEY`, the one name of the keyword), from one macro body,
    so `find_ref` and `find_ref_mut` (behind `rewrite_refs`) visit the same `$ref`s, in the
    same order, at the same paths; a table test pins that the two agree. Every other walk
    and lookup of the keyword reads `REF_KEY` too. The descent still hands only scalar
    members to its visitor (t-1593): every load rule today reads a scalar keyword, so
    widening it now would only make each visitor skip containers. A rule on a keyword
    holding an object or array (`$vocabulary`) widens that one body then.
    `bundle::written_ref` (localize, then point `#/theme` at `#/$defs/theme`) is the
    one rewrite `build` applies to every `$defs` and theme `$ref`, and the one `authored`
    rebuilds with, so a new rewrite cannot reach one only. Collecting pointers with `find_ref`
    and writing through `Value::pointer_mut` was set aside: its `None` could only be a bug,
    needing a silent fallback or an `expect`. `written_ref` is idempotent (`FRAGMENT` encodes
    `%`), so re-localizing bundle `$defs` changes nothing.

18. **Composed wording stays `reason!` (t-1597, t-1598, t-1599, t-1600, t-1601):** trybuild
    cases (`tests/ui/*.rs`, which include `src/quoted.rs` with a stub `limits`) pin the compile
    errors of a `String`, a borrowed `&str` and a `Reason` piece. Their `.stderr` files hold
    the wording of the pinned rustc (Decision 20): a bump that rewords them turns `just test`
    red, and the fix is `TRYBUILD=overwrite` after re-reading that each still fails on the
    `Piece` bound or a `'static` borrow. `Reason::of` stays callable directly: its sealed
    `Piece` bound and constant `N` give it the macro's guarantees, so hiding it would guard
    nothing. No `Fixed` literal newtype: the macro would have to build it from any `&'static
    str` expression, which moves the gap without closing it; the std leaks (`Box`, `String`,
    `Vec`) are clippy-disallowed, and a static cell filled from input is left to review. No
    variant per message: rule 4 and the validation schema have dozens of wordings that share
    pieces, and `Quoted` already caps every input. A capped `Quoted` takes at most nine bytes
    per kept character (`\u{e007f}`, checked over every `char`) plus the elision marker.
19. **No rewritten `$ref` in data (t-1602, amends 15):** load rule 4 refuses a `$ref` in
    `const`, `enum`, `default` or `examples` data when the runtime would rewrite it, and names
    the form to write: `bundle::written_ref` in the theme and `$defs` entries, which the
    validation schema copies (`catalog.json#/theme` becomes `#/$defs/theme`), and
    `localize_ref` in components and functions, which are compiled from their bundle and never
    see the theme. A rewrite there would change the value the validator compares, and refusing
    it keeps every walk reading `$ref`s alike. Refusing is always safe, since the local form
    means the same as schema or as data, so finding data fails toward finding too much
    (`bundle::position`): it walks every node, and a data keyword is data wherever it sits
    except as a name under `properties`, `patternProperties`, `$defs` or `dependentSchemas`. A
    node outside a schema position may still be one, through a `$ref` to it (`#/$defs/X/x`) or
    a keyword the tables miss (jsonschema applies draft-7 `dependencies` under 2020-12). No
    vendored catalog holds a `$ref` in data. The keyword tables stay `describe`'s (Decision 6),
    in `bundle::position`.

20. **Pinned toolchain, cached CI (t-1612, t-1613):** `rust-toolchain.toml` pins rustc 1.99.0 with
    rustfmt, clippy and the `wasm32-unknown-unknown` target, for local runs and CI alike. Two gates
    read the compiler's exact output: trybuild's `tests/ui/*.stderr` (Decision 18) and `clippy -D
    warnings`, whose lint set grows each release; on a floating `stable` either turns CI red with no
    code change, and a local run on another version disagrees with CI. The wasm build needs its
    target installed, which the file now says once. A bump is its own `build(toolchain)` commit: new
    channel, `TRYBUILD=overwrite`, new lints fixed. `renovate.json` (t-1616) enables Renovate's
    `rust-toolchain` manager (and, since Decision 21, `github-actions`; since Decision 26, `cargo`
    for the lock alone), and never touches `rust-version`; once the Renovate GitHub App is installed
    on the repository (open: t-1622), it opens that pull request before 06:00 UTC on Mondays for
    releases at least a week old, with these steps in its body and its Dependency Dashboard as the
    sign it runs. A person finishes and squash-merges it, since the hosted app runs no commands and
    the `.stderr` files need re-reading. Patch releases are proposed too. Renovate's `cargo` manager
    only refreshes `Cargo.lock` (Decision 26). The pin applies in this repository only;
    `rust-version = "1.88"` stays the declared MSRV for users, and CI's `msrv` job checks it
    (t-1615): `just msrv` reads the version from Cargo.toml and sets `RUSTUP_TOOLCHAIN`, which
    overrides the file, then builds the workspace (wasm build included) and runs every test, all
    `--locked`, doctests too (t-1620: none exist yet, and nextest would skip them silently, so `just
    test` runs `cargo test --doc` as well). trybuild's `.stderr` files hold the pinned compiler's
    wording, so on the MSRV `just msrv-compile-fail` copies `tests/ui` beside it
    (`A2UI_TRYBUILD_CASES` points the test there), cases only, and runs it with
    `TRYBUILD=overwrite`, which still fails a case that compiles and writes each `.stderr` in the
    copy, so a case it skipped has none. Each written file must then hold the tracked file's errors,
    each as its code paired with the first case line among its spans (`-->` or `:::`), duplicates
    counted (t-1628). Two errors swapping lines fail, while a later span or note a release adds or
    drops does not; that first line may itself be a note's, as for the two E0277 cases, whose only
    case line is the `:::` macro invocation, so a release dropping it fails loudly. A `warning:`
    ends the error before it. A case without a `.stderr`, a code, or a case line for each error
    fails, and every mismatching case prints its `.stderr` diff and keeps the copy, which git
    ignores. The wording is checked on the pin only; a second, MSRV-worded `.stderr` set would need
    its own copy of each case, not worth it for three cases. `CARGO_TARGET_DIR` is made absolute
    there, since trybuild resolves it from the test's crate directory. `just msrv` proves our
    `Cargo.lock` (MSRV-aware with resolver 3). `just msrv-resolved` checks what a user resolves
    (t-1621): a crate of its own under `target/`, depending on both libraries' packages, unpacked
    (Decision 25), with no lock, no dev-dependencies and no `rust-version`, resolves afresh on the
    MSRV and must `cargo check` natively and for wasm. As an edition 2024 crate it resolves with
    resolver 3, which falls back to versions this rustc accepts when a crate declares its
    `rust-version`; it catches a crate that declares none or wrongly, or a requirement no accepted
    version meets. A user on resolver 2 gets the newest versions, unchecked here, and feature
    unification with a user's other dependencies is not modelled. Its crates are releases nobody
    reviewed: the lock is generated first, which runs no crate's code, and `cargo audit` (yanked
    releases denied too) reads it before any build; its builds stay in its own `target/`. A build
    script of a release not yet advised could still read the runner's cache token and plant cache
    entries for `main`: accepted, as a container would cost more than the weekly check is worth. So
    it is a recipe of its own, outside `just msrv`, and in CI a job of its own run weekly
    (`schedule`) or on demand, with no cache restored or saved; pull requests do not run it, so on
    `main` a red result is mostly upstream drift. Its fix, in order: a newer `rust-version`, an
    issue to the crate that misdeclares its own, or, last, a capped entry in
    `workspace.dependencies` that a2ui-runtime references as a direct dependency, since an
    unreferenced entry constrains nothing; that cap constrains users too, so it is temporary, with a
    task to remove it. A dependency change is checked before its merge by dispatching the workflow
    on its branch (AGENTS.md), since pull requests skip the job. GitHub disables a public
    repository's schedule after 60 days without activity and mails its failures to whoever last
    edited the cron line. The resolved versions are printed for the log. Checked by hand: a
    dependency needing rustc 1.92 fails it. It depends on the packaged crates, unpacked, through
    `[patch.crates-io]`, so a file `include` misses fails it too (t-1629, Decision 25). `-Z
    direct-minimal-versions` was set aside: it needs nightly and checks our lower bounds, not a
    user's newest resolution (t-1634). No clippy in `just msrv`, since the pinned clippy's
    `incompatible_msrv` already reads `rust-version`. If a dev-dependency ever needs a newer rustc,
    narrow the `msrv` job to the published crates' libraries rather than raise `rust-version`. Every
    other job, release-plz's included, installs the file's toolchain (`rustup toolchain install`),
    since the file overrides any other in the checkout. CI caches with `Swatinem/rust-cache`, saved
    even after a red run, which keeps dependency artifacts under `target/`, including trybuild's own
    build in `target/tests/trybuild` (~400 s cold; unconfirmed until two runs on one pull request or
    on `main`, t-1622); the key holds the rustc version, so a bump starts cold. A separate trybuild
    job was set aside: a second job and cache to keep for no gain over one warm cache.
21. **CI hardening (t-1619):** every workflow step names its action by a full commit SHA with the
    release in a trailing `# vX.Y.Z` comment, since a tag can be moved to other code that then runs
    with the job's token. `just actions-pinned`, part of `just check`, fails on any `uses:` in
    `.github/workflows` not written `owner/repo@<40-hex> # vX.Y.Z`, and when it finds no workflow.
    It checks the form offline, not that the SHA is that tag's commit: each pin was checked by hand
    against the peeled tag when set, and Renovate keeps SHA and comment together afterwards; `just
    zizmor-online` checks a hand edit in CI (Decision 22). Local `./` actions and `docker://` images
    would fail it; none exist. The tools `taiki-e/install-action` installs carry no version: the
    manifests in its pinned commit fix them, so a bump of that action moves them too; release-plz
    itself is one of them (Decision 23). Renovate's `github-actions` manager is enabled
    (`pinDigests` pins any action added by tag): one grouped `ci(deps)` pull request before 06:00
    UTC on Mondays, releases at least a week old, never automerged, once the app is installed
    (t-1622). The config was checked against Renovate's published JSON schema only. `ci.yml` runs on
    pushes to `main`, on pull requests, on demand (t-1623) and weekly on `main` (t-1621), so a
    branch with a pull request runs once, not once per event. A newer push to a pull request cancels
    its older run, and so does a new dispatch on a `release-plz-*` branch (Decision 32); every other
    run has its own concurrency group (`run_id`), since a group keeps only one pending run and would
    drop a queued push to `main`. A pull-request run restores its own cache, else `main`'s, so the
    trybuild build is warm only once a run on `main` saved it; a cancelled run saves none, so
    pushing again within a cold run of a toolchain bump starts the next one cold too. A
    `merge_group` trigger is there for a merge queue, should one be enabled. A branch without a pull
    request runs only on demand (`gh workflow run ci.yml --ref <branch>`). Release-plz's pull
    request, opened with `GITHUB_TOKEN`, starts no `pull_request` run; its workflow dispatches CI on
    it (Decision 24).

22. **zizmor (t-1625):** `just zizmor`, part of `just check`, audits all of `.github` (workflows,
    composite actions under `.github/actions`, flow-style `uses:` the `actions-pinned` grep
    misses) offline at zizmor's default persona. `just zizmor-online` adds the online audits and
    fails without `GH_TOKEN`, so no exported token turns `just check` online: each SHA is the
    commit of the tag in its `# vX.Y.Z` comment (`ref-version-mismatch`), lives in the named
    repository rather than only in a fork (`impostor-commit`), and has no known advisory
    (`known-vulnerable-actions`). `.github/zizmor.yml` states the `unpinned-uses` policy, a SHA
    for every action, `actions/*` included (1.32.0's default already, now independent of it);
    a config change or a `# zizmor: ignore` comment silences an audit, so review it as a
    workflow change. `just actions-pinned` stays for the comment's form, which Renovate reads.
    Both recipes print zizmor's version and say how to install it when it is missing (`brew
    install zizmor` or `cargo install --locked zizmor`). CI's `zizmor` job builds nothing and
    restores no cache: checkout, `taiki-e/install-action` (`just,zizmor`), then `just
    zizmor-online` with the job's read-only token, so no build script can read the token, and a
    red result, a finding or a GitHub API outage, leaves the `ci` job's own; the `ci` job installs
    zizmor too for `just check`'s offline run. Every `install-action` step sets `fallback: none`:
    each tool comes from the pinned commit's manifest, its sha256 verified (zizmor 1.32.0 at
    `cfdb7ed`), never from an unpinned cargo-binstall. A Renovate bump of `install-action` moves
    zizmor too, so a red bump may be a new audit; a local zizmor of another release may disagree
    with CI's, and its printed version tells which. It guards hand edits and Renovate drift, not a
    hostile pull request, which can rewrite the job itself; review does that. GitHub's
    repository setting requiring full-SHA pins also covers nested actions' own `uses:`, which
    zizmor does not follow; it is the user's to enable (t-1622). Checked by hand on copies: a
    tag-pinned `actions/checkout` fails offline, in a workflow step, a composite action and a
    flow-style `uses:`; a SHA whose comment names another tag fails online only; no impostor
    commit was forged. zizmor found nothing at the regular persona; the auditor persona's
    findings on `release-plz.yml` (no workflow-level `permissions`, secrets outside an
    environment) are resolved by Decision 23. Not verified: the jobs on GitHub (t-1622).
23. **Trusted publishing (t-1626):** `release-plz.yml` holds no long-lived registry token. Its
    `release` job (`id-token: write`, environment `release`) lets release-plz trade the job's OIDC
    token for a crates.io token valid about 30 minutes; `CARGO_REGISTRY_TOKEN` is gone from both
    jobs, as `release-pr` needs it only for a private registry. release-plz comes from
    `taiki-e/install-action`'s pinned manifest, checksum verified, `fallback: none`;
    `release-plz/action` is dropped, since it installed release-plz with cargo-binstall and no
    checksum, and its nested `git-config` action is two `git config` lines naming
    `github-actions[bot]`. The workflow grants nothing at its top (`permissions: {}`), each
    permission carries its reason; zizmor's auditor persona flags only that missing group. The
    `release` job has no `concurrency` group: a group keeps one pending run, so a push queued behind
    the release PR's merge would cancel the run that publishes it, and release-plz already skips
    versions on crates.io. `semver_check = false`: cargo-semver-checks resolves and builds the
    crates outside `Cargo.lock`, so newly released dependencies nobody reviewed would run beside
    `release-pr`'s write token; a read-only CI job runs it instead (Decision 28).
    `publish_no_verify = true` (t-1629, Decision 25): `cargo publish` no longer
    builds each package in `release`, which `needs` the `package` job's build of the same commit
    from the reviewed `Cargo.lock`, so no dependency build script runs beside `GITHUB_TOKEN`
    (contents write), the crates.io token release-plz hands to cargo, or the job's OIDC token
    request (`ACTIONS_ID_TOKEN_REQUEST_*`). Each push to `main` runs the job, so each records a
    deployment to `release`, though most publish nothing. The user's part (t-1633), before any
    release PR is merged (one merged earlier fails to publish): restrict the `release` environment
    to the `main` branch, with no required reviewers (else every push to `main` waits), before
    adding the trusted publishers, since crates.io checks the repository, workflow and environment,
    not the branch, and GitHub creates a missing environment unprotected; publish `a2ui-runtime`
    then `a2ui-catalog-basic` once by hand with a short-lived token scoped to publishing new crates
    (crates.io cannot publish a new crate through trusted publishing), and tag them in release-plz's
    form (`a2ui-runtime-v0.1.0`) or accept that release-plz's first tag is the next version; add a
    trusted publisher on crates.io for each crate (`oku-theory/a2ui-rs`, workflow `release-plz.yml`,
    environment `release`); then revoke that token and delete any `CARGO_REGISTRY_TOKEN` secret. Not
    verified: a release on GitHub.

24. **CI on the release PR (t-1627):** release-plz opens and updates its release PR with
    `GITHUB_TOKEN`, and GitHub starts no workflow for a push or pull request made with that token,
    so the PR showed no checks and a version bump or regenerated `Cargo.lock` merged untested. A
    dispatch is the exception. `release-pr` reads release-plz's `-o json` output (release-plz 0.4.3
    prints `{"prs": [...]}` on stdout, its logs on stderr): an empty `prs` means nothing to release,
    while any other shape, a second PR or a branch not named `release-plz-…` fails the job, since an
    empty branch would silently drop CI again. A job of its own, `release-pr-ci`, holding only
    `actions: write`, so release-plz's step never has it, checks the name again (it reaches a `gh`
    command line, where it must not read as an option) and runs `gh workflow run ci.yml --ref
    <branch> -f msrv_resolved=true -f minimal_versions=true -f semver_checks=true` (Decisions 28,
    32); if that fails, rerunning the job dispatches again. The run's
    checks land on the branch's head commit, so they show on the PR and match required checks by job
    name. No new secret: a GitHub App or personal token would trigger `pull_request` itself but is a
    long-lived credential beside `contents: write`; gating `release` with `workflow_run` would put
    no check on the PR, and zizmor flags that trigger. `ci.yml`'s dispatch gains a boolean
    `msrv_resolved` input (and later `minimal_versions`, Decision 29), true by default so a manual
    dispatch still runs that job; the release PR first skipped it, since it builds unreviewed crate
    releases, which the weekly run was taken to cover, and runs it since Decision 32. Each push to
    `main` that moves the PR dispatches a new run on its new head; since Decision 32 it cancels the
    older one. The `release` job still does not wait for CI on `main`. The user's part (t-1633):
    make `ci`, `msrv`, `zizmor`, `semver-checks`, `msrv-resolved` and `minimal-versions` required
    checks on `main` (other pull requests' runs skip the last three, which satisfies a required
    check; Decision 32), and require branches to be up to date before merging (or use the merge
    queue, which `ci.yml` already runs on), so the commit `release` publishes is the one CI tested;
    until then, and for an admin who bypasses them, a release PR can merge before its run ends. A
    required `zizmor` also blocks merges during a GitHub API outage or after a new advisory on a
    pinned action; rerun it, or update the pin. Not verified: a dispatch from `release-pr-ci` on
    GitHub, and that its checks satisfy the required ones.

25. **Packaged crates (t-1629):** `just package`, in `just ci` right after `check` (before any test
    can write a file under a crate), runs `cargo package --locked` on both libraries, which builds
    each from its packaged files alone, `a2ui-catalog-basic` against `a2ui-runtime`'s package: a
    file missing from `include` (`catalog.json`, the `include_str!` schemas) fails there, not for a
    user. It first requires, through `cargo package --list`, the legal files a build would not miss:
    `a2ui-runtime`'s `LICENSE-MIT` and `LICENSE-APACHE` (symlinks to the root's, which cargo
    packages as files; it shipped neither before), upstream's `schemas/v1_0/LICENSE`,
    `a2ui-catalog-basic`'s `LICENSE-A2UI`, and `NOTICE-A2UI` in both whenever `vendor/a2ui/NOTICE`
    exists. `NOTICE-A2UI` stays in `a2ui-catalog-basic`'s `include`: `vendor/sync.sh` copies it only
    while upstream has a `NOTICE` (none at the vendored commit). Locally it packages the working
    tree (`--allow-dirty`), since `just ci` runs before a commit; with `CI` set cargo refuses a tree
    a step dirtied. `publish_no_verify` (Decision 23) needs the published commit built:
    release-plz.yml's `release` job `needs` a `package` job that runs `just package` on that very
    commit with a read-only token, no OIDC and no cache, so a red package publishes nothing,
    whatever branch protection allows. `just msrv-resolved` packages them too (pinned toolchain,
    `RUSTUP_TOOLCHAIN` unset, `--no-verify`, since the MSRV's cargo cannot package a crate whose
    sibling is unpublished), unpacks them, and its user crate depends on `=<version>` with
    `[patch.crates-io]` pointing at the unpacked packages and `exclude` keeping them out of its
    workspace, so their dev-dependencies stay unresolved; it fails if its lock takes either crate
    from a registry, since cargo only warns about an unused patch. Each crate's version comes from
    `cargo metadata` since Decision 42 (it was the workspace's, read by `sed`). Checked locally:
    `just package` passes, fails without `catalog.json` in `include`, and with `CI` set refuses a
    dirty tree; the user crate's resolution and audit against the unpacked packages pass. Not run:
    its builds, which run unreviewed releases, nor the `package` job on GitHub. Our lower bounds were
    t-1634 (Decision 29): a nightly `-Z direct-minimal-versions` resolution failed, since jsonschema
    0.61.1 needs `percent-encoding ^2.3` while the workspace asked `2`.

26. **jsonschema's requirement and Cargo.lock maintenance (t-1630, t-1637):** plan 2 pinned
    `jsonschema = "=0.61.1"`. It froze the top crate only: its dependencies float within their range
    for a user, `referencing` (`$ref` resolution), `jsonschema-regex` and `jsonschema-value`
    (`^0.61.1`), `regex` (`^1.11.1`, which decides `pattern` under `PatternOptions::regex()`),
    `fancy-regex` (`^0.20.1`), so a user could run jsonschema 0.61.1 beside later `referencing` or
    `jsonschema-*` releases. And Cargo allows one 0.61.x per graph, so a graph needing 0.61.2 or
    later, a security fix included, would fail to resolve until we released. Our tests did not need
    it, since the reviewed lock holds jsonschema too. So, by the maintainer's choice (t-1637), it is
    now `0.61.1`, a caret below 0.62: a user takes 0.61.x patch releases, a fresh resolution taking
    the siblings of the same release, while `regex` and `fancy-regex` float as before, and the error
    texts both libraries' tests quote (`validator.rs` forwards them in `SchemaViolation`) change for
    us only through a reviewed lock refresh, whose CI runs those tests; a user may see a patch
    release's wording first. `Cargo.lock` drifted from what users resolve, since Renovate ran no
    `cargo` manager: it now runs it for `lockFileMaintenance` only, a `build(deps)` pull request
    before 06:00 UTC on Mondays, never automerged, rebased only on a conflict (`rebaseWhen`), so the
    lock a person reviewed is the one that merges; a package rule disables every other cargo update
    type (`!lockFileMaintenance`), so `Cargo.toml` requirements stay a person's change, and with
    them Renovate's per-crate security pull requests: a fix within range comes with the next
    refresh, CI's weekly audit flagging it meanwhile, any other by hand. Accepted: the refresh takes
    the newest compatible releases, with no 7-day wait, as neither Renovate nor cargo ages them, and
    its pull request CI builds them with a read-only token, its cache scoped to that pull request.
    So `just ci` now runs `audit` first, `cargo audit --deny yanked` and `cargo deny check sources`
    (crates.io only) reading the lock before any build script, the `msrv` job audits before it
    builds, and the pull request's body asks the reviewer to read each added crate and to skip a
    week when a release is younger than one. `msrv-resolved` ignores the lock, so a lock refresh
    needs no dispatch (AGENTS.md's rule is for requirement changes). Resolver 3 keeps the lock
    within `rust-version` where crates declare one; the `msrv` job, building it `--locked` on the
    MSRV, catches the rest. Checked against Renovate's published schema by hand (update types and
    their negation, `rebaseWhen`, manager name), not by `renovate-config-validator`; not verified: a
    live run (t-1622). A summary of the crates a lock diff adds is t-1636 (Decision 30).

27. **A pruned entry's absent target (t-1631):** `pruned` skips a target missing from `root`
    rather than failing: `reach` resolved every target in that same `root`, so none is ever
    absent, and an explicit error (a fallible `pruned` returning `MalformedCatalog`) would add a
    branch only a fabricated target reaches. The skip leaves a dangling `$ref` in the pruned
    document, which jsonschema's offline build rejects, as it resolves every `$ref` when building.
    `compile_entry_target_absent_from_root_is_catalog_error` pins that with a fabricated target:
    a `CatalogError` naming `'/$defs/Ghost'`, not a validator that accepts too much. Replacing the
    skip with an empty schema fails it, and so would a jsonschema or `referencing` release that
    resolves `$ref`s lazily (both float within Decision 26's caret): should it fail after a lock
    refresh, make the skip an error.

28. **cargo-semver-checks (t-1632):** `just semver-checks` runs it on each publishable workspace
    member found on crates.io's sparse index, against the release cargo-semver-checks picks there
    (the newest non-yanked one not above the current version); a member the index does not know
    (404) has no baseline and is skipped with a `::notice::`, any other answer fails (after three
    retries), and so does a name the index path cannot hold (not lowercase, under four characters).
    At t-1632, `a2ui-runtime` 0.1.0 was on crates.io (an early hand publish) and
    `a2ui-catalog-basic` was not (t-1633); once both are, a 404 is a bug, and the skip should become
    an error. Both sides resolve outside `Cargo.lock`, building unreviewed releases, so it runs in
    `ci.yml`'s `semver-checks` job only: the workflow's read-only token, no cache step, no secret,
    60 minutes at most, cargo-semver-checks from `taiki-e/install-action`'s pinned manifest
    (`fallback: none`), building rustdoc JSON with the pinned toolchain, so a toolchain or
    install-action bump can break it until the tool reads that rustdoc format; Decision 31's
    `semver-checks-tool` job checks that pair on every run (t-1638). This job runs on a dispatch
    with `semver_checks` set, never on `main`: `release-pr-ci` sets it, so the release PR checks
    that its version bumps cover the API changes, and a person may pass it on another branch. The
    input is off by default, and the job skips other pull requests and the weekly run, since a
    branch carries the last release's version until release-plz bumps it, so any API break, allowed
    between releases (MVP), would fail it. Not on `main` either: build scripts there could reach the
    runner's cache token and write to `main`'s cache scope, which every pull request restores
    (`msrv-resolved` accepts that weekly). A red release PR means release-plz's bump, taken from
    Conventional Commits alone (`semver_check = false`), is too small: mark the break on `main` (a
    `feat!` commit, or a `BREAKING CHANGE:` footer), and release-plz updates the PR on that push;
    editing the version on the release branch is lost when release-plz rewrites it. Requiring the
    check is the user's choice (t-1633): other pull requests' runs skip it, which satisfies a
    required check, and the release PR gets its result from the dispatched run. The recipe was run
    with stub `cargo-semver-checks` and `curl` commands on `PATH`: one crate checked and one notice,
    a failing tool fails it, an index answer of 500 fails it, and no crate on the index passes with
    a notice; the tool itself was not run locally, as it builds unreviewed releases. Not verified:
    the job on GitHub (t-1622).

29. **Our lower bounds (t-1634):** every requirement in `workspace.dependencies` now names the
    release `Cargo.lock` held when it was raised (`serde_json = "1.0.154"`, `percent-encoding =
    "2.3.2"`, ...; `jsonschema` stays at Decision 26's `0.61.1`), since that is what our builds and
    tests exercised then (later lock refreshes move past them); older releases were never checked,
    and the minimum resolution already failed: jsonschema 0.61.1 needs `percent-encoding ^2.3`, the
    workspace asked `2`, so nightly picked 2.0.0 and gave up. Finding each crate's true minimum
    would mean building old, unreviewed releases one by one, for a library whose users take recent
    releases anyway. Raising one later is a requirement change, so AGENTS.md's dispatch rule applies
    (its pull request skips the check). `just minimal-versions` guards them: on a copy of the
    tracked files under `target/`, nightly cargo resolves `-Z direct-minimal-versions` (each
    workspace member's direct dependency at its lowest allowed release, the rest the newest), `cargo
    audit --deny yanked` reads that lock before anything builds, then the MSRV's cargo checks
    `a2ui-runtime` and `a2ui-catalog-basic`, native and wasm32, `--locked`. So the published
    libraries' bounds compile at their minimum; the others (`a2ui-conformance`'s, which is not
    published, and dev-dependencies) only resolve, and nothing runs tests there, so behaviour at the
    minimum, such as jsonschema's error texts (Decision 26), is not covered. The job is t-1634's
    regression guard. Accepted gap: a pull request may start using an API newer than a bound after a
    lock refresh, and the job sees it only on its next weekly run; the release PR's dispatch now
    runs it too (Decision 32, t-1639). Nightly only resolves, as `-Z` flags need it, on the dated
    nightly the justfile pins since Decision 41, so a nightly that breaks the flag fails only
    Renovate's bump of it. Like `msrv-resolved` it builds unreviewed releases: `ci.yml`'s
    `minimal-versions` job runs weekly on `main` and on a dispatch (`minimal_versions`,
    on by default, the release PR's included since Decision 32), with the read-only token, no cache
    step and its own 60-minute bound; it accepts `msrv-resolved`'s exposure of `main`'s cache scope.
    Checked locally: the resolution fails on the old bounds and passes on the new ones, the audit
    passes (one allowed warning, `unsafe-libyaml` unmaintained, through `serde_yaml_ng` as in our
    lock), and the recipe runs to the end with `cargo check` stubbed; the checks themselves, which
    build those releases, were not run (t-1622). Before merging, `gh workflow run ci.yml --ref
    <branch>` (AGENTS.md), which runs the job; the schedule shows only once it is on `main`.

30. **Cargo.lock change summary (t-1636):** in a raw lock diff, a new transitive crate, the riskiest
    change, is one `[[package]]` block among many version and checksum lines. `scripts/lock_diff.py
    <base> [<head>]` (`just lock-diff`, against the merge base with `origin/main` by default)
    compares the crates of two locks (workspace members have no source and are left out) and prints
    a Markdown table: each crate added, removed or updated, with all its releases on each side in
    version order, so a duplicate shows as `1.0.0` to `1.0.0, 2.0.0`, and each release the change
    brings in with its crates.io publish date and age, flagged under a week (Decision 26's one-week
    rule) and when yanked. A release from any other source (git, another registry) is shown with
    that source and no date, only a request to read it by hand: crates.io's date for the same name
    and version would describe other code. It reads crates.io's API one request a second (its
    crawler policy) with the `User-Agent` `cargo-lock-diff`, which names no person or project, and
    fails on any error, and on a name, version or source holding a character a Markdown cell cannot
    show as is, rather than print a table that hides a change. CI's `lock-diff` job runs it on pull
    requests whose `Cargo.lock` differs from the merge commit's first parent (`fetch-depth: 2`),
    with that parent's own copy of the script, so a pull request cannot rewrite its own summary (on
    the pull request that brings the script in, whose base lacks it, the job says so and gives none;
    a base that lost it fails), and appends the table to the run's summary page. It builds nothing,
    needs no toolchain, and runs in a job of its own with the read-only token; `shell: bash` gives
    its `tee` pipeline `pipefail`. Python 3.11 or later (`tomllib`), not a Rust tool: the runner has
    it, and a crate would need a build, its dependencies and a lock of its own; the script says so
    and exits on an older one. Its unit tests (stdlib `unittest`, offline, fixed dates) run in `just
    test`, which therefore needs that Python too (stock macOS has 3.9). Considered, not taken:
    `cargo deny check bans` with `multiple-versions = "warn"`, which prints the dependency tree of
    every duplicate on each run, noise the summary's release lists replace;
    `actions/dependency-review-action` and `cargo vet`, which give no publish date. Checked locally:
    the tests, and the summary of 03b248b's refresh (2 removed, 7 updated, two duplicates gone). On
    GitHub, PR #2, which brings the script in, ran only that skip; release PR #3 (run 43591819617)
    ran the base's script and printed "No crate added, removed or updated." (only workspace members'
    versions moved), so no crates.io request has run on GitHub yet (t-1622). One error, added by
    t-1640: a crates.io request failing (a 429, a 5xx, a timeout, a reset connection) lost the whole
    table of a sixty-crate refresh. `lookup` now retries a transient error (429, 5xx, timeout,
    network, a connection reset or cut short while reading included) once after five seconds; a 4xx
    other than 429, or an answer it cannot read, would come back the same, so it is not retried.
    Otherwise it returns `Unavailable` with a reason the script writes itself (`HTTP 503`,
    `timeout`, `network error`, `bad answer`, safe in a cell). Once a release stays undated after a
    transient error, `collect` takes crates.io as down and marks the rest "not tried, crates.io
    unreachable" without a request: each costs up to 66 seconds (two 30-second timeouts, the wait,
    the pause), so asking on would outlast the job's 10 minutes and lose the table again. `render`
    marks each undated release `FAILED (...), read it on crates.io` and counts them in the header,
    and the script prints the whole table, then exits 1, so the step still fails and nothing passes
    silently. Honouring a 429's `Retry-After` is left out: at one request a second a 429 is
    unlikely, and its retry fails into a cell. As CI runs the base's copy, this takes effect on pull
    requests opened once it is on `main`.

    Correction (f5faf64), without rewriting pushed history: that commit's `Reviewed-by` trailer
    lists "source-only changes deferred", but a release whose source changes is already an `updated`
    row (`test_diff_same_version_from_another_source_is_updated`); only a checksum-only change goes
    unlisted, which crates.io's immutable releases make moot.

31. **The semver tool on toolchain bumps (t-1638):** cargo-semver-checks reads only the rustdoc
    JSON formats it was built for, and Renovate bumps `rust-toolchain.toml` and install-action's pin
    (which fixes the tool's version) in separate pull requests that never ran Decision 28's job, so
    an incompatible pair would first fail on the release PR. `just semver-checks-tool` writes a
    crate with no dependency under `target/` (inside the repository, so `rust-toolchain.toml`
    applies; not `$TMPDIR`): a struct, a `#[non_exhaustive]` enum, a trait with an associated type
    and constant, a generic function, a constant, a module and an alias, checked against itself
    (`--baseline-root`). No API change, nothing to resolve and no third-party code, so only the tool
    can fail it, an unreadable format included. As it builds nothing unreviewed, `ci.yml`'s
    `semver-checks-tool` job runs on every run, `main`'s and the merge queue's included, with no
    change filter: each bump's pull request checks its half against `main`'s other half, and the
    push to `main` checks the pair two bumps merged one after the other leave behind. It stops such
    a merge only as a required check (with the merge queue or up-to-date branches), the user's
    setting (t-1633); a red `main` is fixed by the next bump that makes the pair readable. Either
    bump's pull request may go red: the toolchain's `prBodyNotes` say to merge the `ci(deps)` pull
    request first, then tick the rebase checkbox (re-running the old run keeps the old merge commit,
    so the old pin), and the actions rule's say the reverse; if both are red, the new tool dropped
    the current format, and both bumps go into one branch. Not taken: our own libraries against
    themselves (their placeholder crates resolve outside `Cargo.lock`, so pull requests would build
    unreviewed releases, and an upstream break would fail it); a `prBodyNotes` request for a manual
    dispatch (a person must remember, and the crates.io baseline is red on any API break since the
    release, hiding a format error); a weekly `--release-type major` run, which builds unreviewed
    releases on `main` and still sees a bad pair only days after its merge. Accepted: the fixture
    covers the item kinds listed, so a format change limited to another kind passes here and fails
    on the release PR. Checked locally: the fixture compiles warning-free and the pinned 1.99.0
    writes its rustdoc JSON (format 61); the recipe, with a stub `cargo-semver-checks` on `PATH`,
    passes its arguments, fails when the tool fails and removes its directory. On GitHub, release
    PR #3's dispatched run (43591825855) showed cargo-semver-checks 0.51.0 reading 1.99.0's rustdoc
    JSON for `a2ui-runtime`. Not run: the recipe with the real tool (not installed locally); its
    first run is the next CI run on GitHub (t-1622), which should show the fixture checked and no
    lint.

32. **The release PR checks its lower bounds (t-1639):** `minimal-versions` (Decision 29) and
    `msrv-resolved` (t-1621) ran weekly on `main` and on a dispatch, not on the release PR's, so a
    pull request using an API above a bound without changing it, merged after the weekly run, could
    be published before the next one with a range that does not compile at its bottom. Both jobs,
    and `semver-checks` (Decision 28), now run on every run of a `release-plz-*` branch, whatever
    the event or inputs (`startsWith(github.head_ref || github.ref_name, 'release-plz-')`):
    `release-pr-ci`'s dispatch (which also passes the inputs as true), a manual dispatch with them
    off, and a `pull_request` run a person starts by pushing to or reopening the PR, so no run there
    reports them skipped over a red result on the same head. They check the commit to be published,
    its version bump included, given t-1633's up-to-date branches. They build unreviewed releases,
    the exposure Decision 28 already accepts for `semver-checks` there: the read-only token, no
    cache step, 60 minutes each; such a run writes only the release branch's cache scope (it reads
    `main`'s too), which only that branch's later `ci` and `msrv` runs restore, and nothing in
    `release-plz.yml` restores a cache or reads an artifact. Each push to `main` updates the release
    PR (same branch) and dispatches again, so `ci.yml`'s `concurrency` groups dispatches on a
    `release-plz-*` branch by ref and cancels the older one, as for a pull request; pushes under an
    hour apart keep restarting them, so a release waits for a quiet `main`. A red result holds the
    release once the jobs are required checks (t-1633, Decision 24's list): a bound to raise on
    `main` (release-plz then updates the PR); upstream drift `msrv-resolved` reports, which users
    would hit too (a temporary `<` bound on `main`, or an admin merge after reading the failure);
    or, before Decision 41 pinned a dated one, a nightly that broke `-Z direct-minimal-versions`. A
    merge queue's `merge_group` run skips them: the PR's head result is
    the gate there. Not taken: `scripts/release_gate.py`, a first design reviewed for this task
    that read `main`'s latest weekly or dispatched result through the API and refused a red or
    stale one (its review showed a green run from before the offending merge passes it, and tying
    it to the release's base commit meant one more manual dispatch per release); a release
    checklist step; and taiki-e's `cargo-minimal-versions`, which drops dev-dependencies through
    cargo-hack, while our recipe already checks only the two libraries on the MSRV and adds no
    tool. Checked locally: actionlint and zizmor. Not verified: a release-branch run on GitHub
    (t-1622); the release PR open today (#3, v0.2.0) was dispatched before this change.

33. **The catalog's first release, by hand (t-1642):** trusted publishing (Decision 23) cannot
    create a crate, and `a2ui-catalog-basic` 0.1.0 cannot be published first by hand: its dry run
    fails against `a2ui-runtime` 0.1.0 as published, which has no `Catalog`. Its first release is
    therefore 0.2.0, with the runtime's. `release-plz.toml` sets `publish = false` for it, so
    release-plz still bumps and tags it but publishes only `a2ui-runtime` 0.2.0, by trusted
    publishing; the user then publishes `a2ui-catalog-basic` 0.2.0 from that tag with a one-day
    `publish-new` token scoped to the crate, adds its trusted publisher, revokes the token, and a
    second change removes `publish = false`. Not taken: merging the release PR as is, which leaves a
    red `release` job after the runtime is published, with its tags uncertain; and publishing both
    crates by hand, which leaves trusted publishing untried until the next release. Done on
    2026-11-14: release run 43604416240 published `a2ui-runtime` 0.2.0 by trusted publishing
    (crates.io records the run and commit d984cc4) and tagged `a2ui-catalog-basic-v0.2.0` git-only;
    the user published `a2ui-catalog-basic` 0.2.0 by hand from that tag, after its dry run passed
    against the published runtime, added its trusted publisher and revoked the token; `publish =
    false` is gone, so the next release publishes both crates by trusted publishing. The 0.3.0
    release (run 43707676954) showed the catalog still had no trusted publisher: release-plz
    published `a2ui-runtime` 0.3.0, then crates.io refused the catalog with 403 "The provided
    access token is not valid for crate `a2ui-catalog-basic`". A crate published by hand needs its
    own trusted publisher (`oku-theory/a2ui-rs`, `release-plz.yml`, environment `release`) before
    release-plz can publish it; once the user added it on 2026-11-15, rerunning the failed jobs
    published `a2ui-catalog-basic` 0.3.0 by trusted publishing and tagged it.
34. **`Error::code()` (t-1565):** each error carries an `ErrorCode` beside its `Category`, the
    `code` a renderer `error` message or surface diagnostic reports. It lands ahead of Plan 3's
    message processor, against Plan 2's deferral, so each variant's code is fixed where the
    variant is. One private match decides both per variant (`Inner::class`), so a new variant must
    pick its code where it picks its category; codes mapped from categories would have put a
    future `UNALLOWED_PARENT` (a validation error) under `VALIDATION_FAILED`. The code is the one
    of the stage that raises the variant: upstream gives one fault different codes at different
    stages (an unknown component is `VALIDATION_FAILED` when validated, `UNKNOWN_COMPONENT_TYPE`
    as a surface diagnostic; an `agentOnly` call or an unknown `@` function is `VALIDATION_FAILED`
    when validated, `EXPRESSION_ERROR` when evaluated), so Plan 3 raises its own variants for
    evaluation and rendering rather than reusing `UnknownComponent`, `UnknownFunction` or
    `AgentOnlyCall`, which only `validate_component` raises. Today's variants map to four codes:
    data path errors to `DATA_ERROR` (`error_responses.yaml`, prototype pollution); template
    syntax and template limits to `EXPRESSION_ERROR` (the protocol reports a local evaluation
    failure so, and a `regex` pattern that does not parse); every validation error, an unknown
    component or function and an `agentOnly` call from a component included, to
    `VALIDATION_FAILED` (every `validator_v1_0.yaml` `rendererError`). `InvalidPointer` moves from
    `DataError` to `ValidationError`, `VALIDATION_FAILED`: upstream's one malformed-pointer case
    (`test_error_response_update_data_model_invalid_json_pointer`) expects both, and no case
    expects `DataError` for one. Catalog load, compile and embedded-schema errors map to
    `CATALOG_ERROR` by analogy only: upstream's `CATALOG_ERROR` cases
    (`agent_function_routing.yaml`) are calls naming a catalog the surface lacks, and no case
    reports a catalog that fails to load; `EmbeddedSchema` is a defect of this crate, which a
    renderer should log rather than blame on the agent, and `UserActivation` is a catalog
    declaring a function inconsistently, not the evaluation-time refusal (`EXPRESSION_ERROR`). The
    enum is `#[non_exhaustive]` and gains `UNALLOWED_PARENT`, `INTEGRITY_ERROR`,
    `CATALOG_NOT_FOUND`, `INVALID_FUNCTION_CALL` and the others with the first error that raises
    them (Plan 3), not before: a variant no error maps to would document a code the runtime never
    reports. `UNKNOWN_FUNCTION` is the agent's reply to `callAgentFunction`, which the renderer
    reads. Only `ErrorCode::as_str` is meant for the wire; `Display`, English and quoting capped
    input, is for logs. A `VALIDATION_FAILED` message also needs `path` and `surfaceId`, which
    `Error` does not expose yet (Plan 3). The harness's `expect_error` still compares categories
    only: the cases with a `rendererError` code are message-processor cases, skipped until Plan 3
    runs them and compares codes too (t-1643).

35. **Component size and call caps (t-1567):** `validate_component` refuses, before any other
    check, its depth cap included, an instance holding more than 65 536 JSON nodes or 1 024 calls,
    as `ComponentLimit` (`ValidationError`, `VALIDATION_FAILED`; the message names the cap, the
    code stays coarse as Decision 34's codes are). The depth cap bounded recursion, not work: a
    flat array of a million calls ran a million function validators. A call is any object holding
    an `@call` key, nested ones, calls into unknown catalogs, non-string `@call`s and ones under an
    envelope key the walk skips included: counting more than is checked errs on the safe side.
    Both caps are counted in one iterative pass that stops at the first node past a cap, the walk
    `oversize` (load rule 1) now shares as `first_oversize`, so refusing costs at most the cap in
    work; it runs first so the depth walk, which has no node bound, then sees at most the cap.
    Accepted work stays bounded but not linear: a call's validator checks its whole subtree, so the
    about 32 calls the depth cap lets nest revisit shared nodes, about 32 × 65 536 node visits at
    worst. The values are generous: the basic catalog's components hold tens of nodes, and a
    literal list of 60 000 child ids still passes; the conformance suites hold no component near
    either. They are fixed constants, like the catalog caps (t-1566 decides whether hosts may set
    them). A string is one node whatever its length, so string bytes are the message processor's
    to cap, per message rather than per component (Plan 3, t-1644), as `Catalog::parse` caps
    catalog text; `validate_component` must not face agent traffic before that cap exists.
    Instances it accepted before can now be refused: a behavioural tightening to note in the
    release notes, which `cargo-semver-checks` does not see.

36. **One shared basic catalog (t-1572):** `a2ui_catalog_basic::catalog()` returns
    `Result<&'static Catalog, Error>` and keeps the loaded catalog in a `std::sync::OnceLock`
    (stable since 1.70, under our 1.88 MSRV; on `wasm32-unknown-unknown` without threads it
    is a cell). It parsed and compiled the catalog, about 20 ms, on every call; a renderer
    creating surfaces would pay that per surface, and `validate_component` needs the basic
    catalog as a `CatalogLookup` on each one. `Catalog` is `Send + Sync` (asserted in
    `error.rs`), so it can be a static. Only success is kept, not a `LazyLock<Result<..>>`
    (review): the error stays owned, so `catalog()?` composes with callers' errors, each
    caller gets its own backtrace, and a panic while loading leaves the cell empty instead of
    poisoning every later call. The cost, a re-parse per call while the build is broken and a
    possible duplicate compile when threads race on the first load, falls only where nothing
    else works. Owned copies are not offered: `Catalog` is immutable and `derive` borrows its
    source, so a caller wanting a variant derives one. This is a breaking change to the
    published 0.2 API (`Catalog` becomes `&'static Catalog`): the commit is marked `!` so
    release-plz bumps to 0.3, and the release notes say callers borrow. The trigger the task
    named (a renderer on a hot path) has not arrived; the change is small and every caller in
    the workspace already only borrowed it.

37. **The wasm cost of `jsonschema` (t-1573):** measured on 2026-11-14 with Rust 1.99.0, each
    case a `cdylib` for `wasm32-unknown-unknown` built `--release` with `opt-level = "s"`,
    `lto = true`, `codegen-units = 1`, `panic = "abort"` and `strip = true`, without `wasm-opt`
    (not installed); compressed with `gzip -9`. Rows 2 and 4 compile a schema read from the
    input and check an instance against it:

    | Build | `.wasm` | gzip -9 |
    |---|---|---|
    | `serde_json` alone, parsing a value | 55 KiB | 24 KiB |
    | plus `jsonschema` 0.61.1 (no default features), compiling and checking | 2.52 MiB | 787 KiB |
    | runtime and catalog: `catalog()`/`from_json`, `validate_component`, `parse_template` | 2.81 MiB | 883 KiB |
    | `boon` 0.6.1 in place of `jsonschema` in row 2 | 1.37 MiB | 459 KiB |

    The `jsonschema` build is about 90 % of the runtime's wasm (88 % net of `serde_json`): the
    runtime and the embedded catalogs add about 300 KiB raw and 100 KiB compressed on top of it.
    Exact bytes: 56 554, 2 639 724, 2 945 437 and 1 434 223 (gzip 24 819, 805 682, 904 663 and 470
    278). By crate, from the v0 symbol names of the same runtime build left unstripped (code section
    2.22 MiB, data section 579 KiB, the embedded schemas and catalogs included): `jsonschema` 838
    KiB; the linear regex stack the runtime selects (`regex-automata`, `regex-syntax`,
    `aho-corasick`, `regex`, `memchr`, `jsonschema-regex`) 441 KiB; `fancy-regex` 126 KiB, linked
    although the runtime never selects it (`jsonschema` chooses the engine at run time);
    `referencing` 72 KiB; `num-bigint` 39 KiB; `fluent-uri` 19 KiB; `ahash` 15 KiB; `a2ui-runtime`
    103 KiB; `serde_json` 47 KiB; `core`, `alloc` and `hashbrown` generics, mostly instantiated by
    `jsonschema`, 493 KiB. No single dependency is a cheap cut: dropping `fancy-regex` would need an
    upstream feature gate and saves 5 %. `boon` does the same work in a little over half the bytes
    (-1.15 MiB raw, -328 KiB gzip). Both bring `getrandom`, and so `wasm-bindgen`, on wasm32:
    `jsonschema` directly and through `ahash`, `boon` through `ahash`; it only seeds hashing and
    reaches no result. We keep `jsonschema` for now: no wasm size budget exists yet (Plan 6 sets one
    with the Leptos renderer and `wasm-opt`, and re-measures), and a switch is not only bytes. The
    evaluation, t-1646, gated on that budget, must keep the agent-catalog protections as hard
    requirements: a non-backtracking regex engine whose compiled size is capped at
    `MAX_PATTERN_BYTES` (`size_limit`/`dfa_size_limit` today, not the pattern's length), and no
    network or file loader (a `file:` or remote `$ref` refused, with a test). Then the conformance
    suites must pass unchanged, error codes and locations included; the swap is local to
    `validator.rs` and `catalog/compile.rs`. The measurement crates were throwaway, a dozen lines
    each; Plan 6's budget should come with a committed size check instead.

38. **A `common_types.json` `$ref` names a common type (t-1575):** localizing drops the document
    part of a `$ref`, so `common_types.json#/$defs/Weight` resolved to the catalog's own
    `Weight`: lenient, not unsafe (the caps applied), but the reference meant one document and
    reached another. Load rule 4 now refuses a `common_types.json` reference (any of its three
    URIs) whose `$defs` name is not a common type; `common_types.json#/components/X` was already
    refused (only `$defs` is kept there). The task also proposed refusing a self-reference that
    lands on a copied common type (`#/$defs/DynamicString`); review found upstream's own v1.0
    conformance catalogs write that form in schema positions (`composition_constraints.yaml`,
    `multi_catalog.yaml`, run from Plan 3), and it is not ambiguous: `check_defs` refuses a
    definition named like a common type, so such a name can only mean the common type. It stays
    accepted, pinned by a test. A dangling `common_types.json` pointer below a real type
    (`#/$defs/DynamicString/oneOf/99`) is still the validation schema's check to report. The
    commit is marked breaking: a catalog that loaded before can be refused.

39. **Common type names are reserved per protocol version (t-1570):** `check_defs` refuses a
    catalog `$defs` entry named like a common type, so a 1.0.x `common_types.json` adding a type
    `T` would refuse a catalog declaring 1.0.0 whose own `$defs` has `T`, valid when written.
    Three answers were weighed:
    - Keep refusing: an additive patch release of the schemas breaks valid catalogs. Refused.
    - Namespacing the copied common types (`#/$defs/common/T` or a prefix): the validation
      schema must keep upstream's flattened shape, where common types sit at `#/$defs/<name>`
      (conformance compares it), and upstream's own v1.0 catalogs reference them by bare name
      (`#/$defs/ChildList`, Decision 38), so a namespace would break both. Refused.
    - **Chosen: version-scoped names.** The runtime keeps, per supported patch version, the set
      of common type names that version defines. A catalog reads the set of its
      `protocolVersion`: `1.0` is `1.0.0`, the set that reserves the fewest names and so refuses
      the fewest catalogs; a patch newer than any known reads the newest set (patches compared
      as digit strings, longest first, never parsed into an integer). Every place that keys on
      a common type name reads that set, not the embedded `$defs`: `check_defs`, Decision 38's
      check, the own-`$defs` filter and the `add_common_types` merge of
      `catalog/validation_schema.rs` (which today drop and overwrite a catalog entry named like
      a common type), the descriptions copied there, and `catalog/flatten.rs`'s mixin test.
      A 1.0.0 catalog may then define `T`: its `#/$defs/T` reaches its own entry, and its
      `common_types.json#/$defs/T` is refused, `T` not being a 1.0.0 type. A bare `#/$defs/T`
      in a 1.0.0 catalog with no own `T` is a dangling reference, not the newer common type:
      Decision 38's "such a name can only mean the common type" holds per version.
    - **The bodies are the newest embedded ones**, since one runtime validates every 1.0.x
      message with one set of schemas. So the closure of common types a catalog pulls in may
      reach a name only a newer version defines (a 1.0.1 `ChildList` whose body refers to a new
      `T`). If the catalog's own `$defs` holds that name, the copied body would bind to the
      catalog's entry, and a catalog could loosen a common type (`"T": true`). That load is
      refused with `DefsShadowCommonType`: the reserved names of a catalog are its version's
      set plus every name the common types it reaches reach in turn. A catalog that never
      reaches the changed type keeps loading.

    Nothing changes until the first 1.0.x schema update: only 1.0.0 exists, so the sets would be
    one. The implementation is t-1647. A tripwire holds the decision until then:
    `schema_tests.rs` pins the 21 names of 1.0.0 and its failure message names this decision, so
    the `vendor/sync.sh` that adds, removes or renames a common type fails `just test` before
    any catalog breaks. `agent_to_renderer.json` is not affected: it is the runtime's own
    schema, and no catalog `$defs` is merged into it.

40. **The function registry reuses `FunctionDef` (t-1569):** Plan 4's `FunctionRegistry` holds
    implementations only. What a function is (return type, `allowedCallers`,
    `requiresUserActivation`, the compiled validator of its calls) is already the catalog's
    `FunctionDef`, checked at load (a function requiring user activation must be `rendererOnly`)
    and used by `validate_component` (an `agentOnly` function cannot be called from a
    component). A second description in the registry could disagree with its catalog, so the
    registry reads it through `Catalog::function`. It shares the catalog, never copies its
    definitions; the handle takes a `&'static Catalog` (the shared basic catalog, Decision 36)
    as well as an `Arc<Catalog>`, the type being Plan 4's choice.

    Routing follows upstream's fallback rule (protocol, "Function Location Resolution"): `@`
    functions run locally, a function with a registered implementation runs locally, and any
    other call is sent as `callAgentFunction`. A function the catalog declares without an
    implementation (schema-only) is therefore legal and goes to the agent, never refused at
    registration. `AllowedCallers` and `requires_user_activation()` do not choose the route;
    they gate both routes of a declared function (an `agentOnly` call or a passive call needing
    a gesture is an `EXPRESSION_ERROR`, never dispatched). A call to an undeclared name is
    already refused by `validate_component` (`UnknownFunction`), which upstream allows a
    renderer that declares the agent functions it expects; so registering an implementation for
    a name the catalog lacks is an error too, as it could never be called.

    The policy is a trust boundary: an implementation is host code, and the catalog the host
    registered it with is the one whose policy guards it. So implementations bind to that
    loaded catalog, not to its id string: a catalog the agent sends (inline, or one reusing a
    registered id) never rebinds them or loosens their `allowedCallers` or
    `requiresUserActivation`. Left to Plan 4, tracked as t-1648: what re-registering an id
    does to the old bindings, and two catalogs sharing an id. The design spec's `function`
    module now says so.

41. **A dated nightly (t-1641):** `just minimal-versions` installed unpinned nightly for `-Z
    direct-minimal-versions`, so since Decision 32 a nightly that broke the flag would hold every
    release until nightly was fixed. The justfile now pins one, `nightly := "nightly-YYYY-MM-DD"`,
    which a private `nightly-toolchain` recipe checks is dated, installs and logs (`cargo
    +<nightly> -V`); `minimal-versions` depends on it, so a command-line override reaches both, and
    t-1649's opt-in AddressSanitizer is to reuse both. Renovate bumps it: a `custom.regex` manager
    reads the `# renovate:` comment above it with the `rust-version` datasource and
    `rust-release-channel` versioning, the pair its own `rust-toolchain` manager uses, which takes
    dated nightlies from static.rust-lang.org's `manifests.txt`, dates each one (so
    `minimumReleaseAge: "14 days"` holds) and compares a nightly with nightlies only. A bump that
    came out undated would fail the recipe's check on its own pull request. Its rule opens a
    `ci(nightly)` pull request on branch `renovate/rust-nightly`, weekly like the others, and
    `ci.yml` runs the `minimal-versions` job on that branch's runs too, since the pin serves that
    job only: the bump is checked before its merge, with the exposure Decision 32 accepts there (a
    fork's pull request may name its branch so, with the same read-only token and no cache). A hand
    edit of the pin is dispatched like a dependency change (AGENTS.md). Both ends fail silently: an
    edit the pattern no longer matches leaves the pin stale, and a branch name `ci.yml` does not
    expect skips the job, which leaves the checks green. So `scripts/nightly_pin_test.py`, in `just
    test`, applies renovate.json's own pattern to the justfile and requires exactly the one
    assignment, and requires the job's condition to name `renovate/` and the rule's `branchTopic`
    with no prefix, branch name or group overriding them. The first pin, `nightly-2026-10-31`, was a
    fortnight old when set. Not taken: a second `rust-toolchain.toml` in a subdirectory, which
    Renovate's own manager would bump, but under the pinned compiler's rule, and which rustup would
    apply to commands run there; a dated nightly in `ci.yml` alone, which a local run would not
    share; and the pinned stable cargo with `RUSTC_BOOTSTRAP=1`, which drops the second toolchain
    and its pull requests but leans on a discouraged switch and does not serve t-1649's sanitizer.
    Each bump leaves the previous nightly installed locally (`rustup toolchain list`). Checked
    locally: the recipe refuses `nightly` and a name carrying a shell command, and `just
    nightly=nightly minimal-versions` stops there before any copy; the pin installs (cargo
    1.101.0-nightly, 2026-10-27) and resolves `-Z direct-minimal-versions` on a copy of the
    workspace to our bounds; the pattern, run with Node's RegExp, extracts the pin; the new test
    passes and fails on a respaced assignment, a blank line under the comment, and a renamed
    `branchTopic`. Not verified: Renovate's own run (running Renovate locally was not allowed),
    whose Dependency Dashboard should list `rust-nightly`, nor the job on its first bump pull
    request, which should log the nightly's cargo (t-1652).

42. **Package gaps after the first release (t-1635):** three points Decision 25 left open, checked
    now that both crates are on crates.io at 0.2.0. (1) `cargo package -p a2ui-runtime -p
    a2ui-catalog-basic --locked` builds `a2ui-catalog-basic` against `a2ui-runtime`'s local
    package, not crates.io's release of the same version: with a function added to the runtime and
    called from the catalog, which 0.2.0 on crates.io lacks, the catalog's verification unpacked
    the runtime from cargo's temporary registry and passed, with no checksum error. Nothing to
    change. (2) `a2ui-runtime`'s package shipped `tests/` (the trybuild cases), which nothing builds
    from the package, and whose `.stderr` files hold the pinned rustc's wording, so any other rustc
    fails them: its manifest now has `exclude = ["/tests/"]` (anchored, as `exclude` follows
    gitignore rules and `tests/` would also drop a nested `tests` directory), and `just package`
    fails when either crate's package lists a `tests/` file (seen failing before the exclude). The
    packaged manifest then has no test target. The unit tests under `src/` still ship, since anyone
    running `cargo test` from the package needs their `#[path]` modules; they read no file outside
    the package and are not built from it here either. The changed file list likely makes
    release-plz propose a runtime release at the next release PR even with no code change. An
    `include` allowlist like the catalog's, which would also keep a stray local file out of a
    `--allow-dirty` package, is t-1651. Building
    the packaged tests once was set aside: the catalog's package resolves the runtime from
    crates.io outside `cargo package`, so it would need its own `[patch]` and an unlocked
    resolution of the dev-dependencies, unreviewed releases for a check of files we already test.
    (3) `just msrv-resolved` read one workspace version from the root `Cargo.toml` with `sed`; it
    now reads each crate's own version from `cargo metadata`, through a private `crate-version`
    recipe, so a pre-release or a per-crate version no longer stops it; `crate-version` fails on an
    unknown name.
    Checked locally on a copy with `a2ui-runtime` at `0.3.0-alpha.1`: `sed` read `0.2.0`,
    `crate-version` read `0.3.0-alpha.1` and `0.2.0`, the names `cargo package` gave the two
    `.crate` files; on the tree, `msrv-resolved` up to its audit packaged, unpacked and resolved
    both crates on 1.88 from the packages. Not run: its audit and builds, which build unreviewed
    releases (CI).

45. **fuzz/'s lock is generated from the root's (t-1650):** `fuzz/` is its own workspace (t-1568),
    so `libfuzzer-sys` and its C++ build stay out of the published crates' lock, MSRV and
    minimal-versions builds. Its committed `fuzz/Cargo.lock` had to follow `Cargo.lock` by hand
    (`just fuzz-lock`), and `fuzz-check` failed whenever the root lock moved alone: every weekly
    `lockFileMaintenance` pull request (Decision 26), since Renovate ignores `fuzz/**` and the
    hosted app cannot run `postUpgradeTasks`, went red until a person pushed to it. Weighed:
    - (a) Keep the manual step, documented in the pull request's body: a weekly red PR. Refused.
    - (b) A workflow on Renovate's branch running `just fuzz-lock` and pushing: `contents: write`
      in a job resolving crates, and a push stops Renovate updating the branch (`rebaseWhen`),
      while Decision 26 wants the lock a person reviewed to be the one that merges. Refused.
    - (d) Warn only on lock-maintenance branches: `main` would then fuzz releases nobody
      reviewed against it, the drift the check exists to stop. Refused.
    - (c) One lock, the fuzz crate in the root workspace behind a feature: `libfuzzer-sys` would
      enter `Cargo.lock`, and with it the `msrv`, `minimal-versions` and audit jobs. Refused.
    - **Chosen: (e), one reviewed lock by generation**, the task's (c) without a second
      committed lock. `fuzz/Cargo.lock` is ignored by git and generated (`just fuzz-lockfile`,
      run by `fuzz-check` and `fuzz`): `scripts/fuzz_lock.py assemble` writes `Cargo.lock`'s
      packages, then the pins of the committed `fuzz/fuzz-only.lock`, the releases
      `a2ui-fuzz`'s registry dependencies reach, each kept only when `Cargo.lock` holds no
      release on its caret line (the leftmost non-zero part; a pre-release is its own line).
      A bare dependency entry that the other file makes ambiguous gets its version. Cargo then
      drops what is unused, and `verify` fails naming each registry release (name, version,
      source) the assembled lock did not hold: a fuzz-only crate no pin covers, or a shared
      crate fuzz/ needs at another release than `Cargo.lock`'s, which the old `cargo tree`
      comparison caught and which needs a root lock change. So the shared crates are the
      reviewed root releases by construction, a root lock change needs nothing in `fuzz/`, and
      every release fuzz/ builds was in the assembled lock that `just audit` and CI's `msrv`
      job audit, made by Python alone before any build script. The pins reach `serde_json`'s
      and `cc`'s dependencies too, so a root lock dropping `libc` still builds. `cargo
      metadata` may fetch a newly resolved release's source before `verify` fails, running
      nothing. `just fuzz-lock` rewrites the pins and lists each release `Cargo.lock` lacks,
      to review; to bump one, `cargo update --manifest-path fuzz/Cargo.toml -p <crate>`, then
      `python3 -B scripts/fuzz_lock.py pin`. Tested: the generated lock equals PR #11's
      committed one byte for byte; moving `itoa` and `memchr` in `Cargo.lock` alone left
      `fuzz-check` green on the moved releases; removing the `shlex` pin made it fail naming
      `shlex 2.0.1`; requiring `itoa 1.0.19` in fuzz/ while `Cargo.lock` held 1.0.15 failed
      naming both; `scripts/fuzz_lock_test.py` covers assembly, caret lines, verification and
      pinning. Pins age until a person re-pins (Renovate ignores `fuzz/**`), with no publish
      dates or licence check: t-1653.

46. **Deeper fuzzing (t-1649):** t-1568's two targets fuzzed one text and one component. Now:
    - Inputs carry several parts split at NUL bytes (`a2ui_fuzz::fields`): JSON text never holds
      a raw NUL, so a seed of whole documents and ids keeps each part whole. Small parts come
      first and the catalog last, since libFuzzer's mutations mostly land near an input's start:
      `catalog_parse` takes [`catalog_id`, [`protocol_version`,]] then the text;
      `validate_component` a component, then a default catalog id, which may name no loaded
      catalog; the new `parse_then_validate` a component, [a default id,] then an agent catalog,
      validating the component in the agent's catalog (the basic one found through a `Catalog`
      lookup) and in the basic one (the agent's through a `[Catalog]` lookup).
      `template_parse` and `pointer_parse` fuzz `parse_template` and `Pointer::parse`. Plan 3's
      message layer gets a target when it lands.
    - Oracles, beyond "no panic, hang or out-of-memory": a loaded catalog's `to_json` loads again,
      with no override, to the same id, version and document; the agent catalog, parsed twice
      from one text, writes the same JSON and validates the component to the same outcome
      (value, or error code, category and message: the runtime's results hold no randomness,
      and two loads catch state kept from the text's hashing order); no parsed pointer holds a
      forbidden or empty segment, and its written text parses to the same pointer. Indices are
      checked on data access, not by `Pointer::parse`, so no index oracle.
    - `scripts/fuzz_targets.py` holds each target's `max_len` and timeout (1 or 2 s: no input
      took a second here, so a slower one is a finding) and its seeds: vendor/a2ui's catalogs,
      examples and test cases, plus components just within and past the node, call and depth
      caps. Its tests check the table against `fuzz/Cargo.toml` and the workflow's matrix.
      `fuzz/a2ui.dict` gives libFuzzer JSON and A2UI tokens (`-dict=`).
    - `.github/workflows/fuzz.yml` runs each target five minutes on Mondays and Thursdays and on
      demand, from the corpus the last run left in the Actions cache (twice a week, so an entry
      is read again before 7 idle days evict it); `just fuzz-minimize` (libFuzzer's `-merge=1`,
      under the target's limits) keeps the inputs that add coverage, saved even after a crash; a
      crash fails the job and uploads its input for 30 days. The token only reads, actions are
      SHA-pinned, the matrix target reaches `run` only through `env`; zizmor reports nothing.
      Scheduled on `main` only: a pull request would wait on it and fuzz unreviewed crates no
      faster.
    - AddressSanitizer needs a nightly (`-Zsanitizer`): t-1660, after t-1641's dated pin.
    Measured on 2026-11-15 (Apple M-series, 60 s each, from the seeds): `catalog_parse` 139 838
    runs, `validate_component` 520 895, `parse_then_validate` 69 089, `template_parse` 623 997,
    `pointer_parse` 618 729; no crash, no slow input and no oracle failed. Not verified: the workflow itself (it runs on `main` once
    pushed).

## The algorithm (reference for Tasks 1 and 2)

Inputs: the checked `Bundle` (localized `components`, `functions`, and `$defs` = own defs +
unions + common-type closure, every `$ref` local), the effective id and version, `instructions`,
and the common types map `C` (`common_type_defs()`). Own `$defs` never shadow a common type
(load rule), so a whole-entry `#/$defs/<n>` names a common type exactly when `C` has `n`.

**Component flattening** (`PROTOCOL_KEYS = id, component, catalogId`; `MERGED = type,
description, allOf, properties, required, unevaluatedProperties, additionalProperties,
allowedParents, allowedChildren`). Collect pieces from the component schema `S`:

- for each `allOf` member `m`, in order:
  - `m` is not an object: residual;
  - `m` has no `$ref`: collect from it (same rules, recursively);
  - `m` is `{"$ref": r}` alone, `r` names a whole entry `n` (not a union) of `$defs`:
    - `n` in `C` with `properties`: piece *Mixin* (its `properties` and `required`), once per `n`;
    - `n` in `C` without `properties`: residual;
    - `n` an own def not seen yet for this component: mark it merged and collect from it, every
      piece found under it being *Local*;
  - any other member: residual (kept whole);
- a schema other than `S` itself puts its keys outside `MERGED` in the residual `allOf`, as one
  object, when there are any;
- a schema with `properties`, `required` or `description` is itself a piece: *Local* under an own
  def, else *Specific* when its `properties` has `component`, else *Common*.

Stable-sort pieces Mixin < Local < Common < Specific. Properties: union in that order, a later
piece overriding an earlier one, `PROTOCOL_KEYS` left out. Required: the pieces' names minus
`PROTOCOL_KEYS`, deduplicated and sorted. Description: the last non-empty piece description.
Component constant: Decision 11.

Result: `{"type": "object"}`, then `S`'s keys outside `MERGED`, then `description` (if any),
`properties` = `{"id": {"$ref": "#/$defs/ComponentId"}, …, "component": {"const": <constant>}}`,
`required` = `["id", …sorted…, "component"]`, `allOf` = residual (if non-empty),
`unevaluatedProperties` = `S.unevaluatedProperties`, else `S.additionalProperties` (if either is
present), `allowedParents`/`allowedChildren` when non-empty arrays.

**Assembly:** header (Decision 9); components flattened then described; functions with `args`
closed then described (closing: when `properties` has `@call` or `call` and `properties.args` is
an object with `"type": "object"` and no `$ref`, remove `additionalProperties` and, unless
`unevaluatedProperties` is present, set it to the removed value or `false`; upstream closes only
`properties.args`, not the `args`/`parameters` spellings `document::argument_names` also reads);
own `$defs` described; merged mixins pruned (Decision 7); then the common types reached from
components, functions and own `$defs` added by `add_common_types`.

## File Structure

- Modify `crates/a2ui-runtime/src/bundle.rs` (+ `bundle_tests.rs`): `whole_def_name` (Task 1),
  `def_names` becomes `pub(crate)` (Task 2).
- Create `crates/a2ui-runtime/src/catalog/flatten.rs` + `flatten_tests.rs` (Task 1).
- Modify `crates/a2ui-runtime/src/catalog/compile.rs`: `Bundle`, `Checked` (Task 2).
- Create `crates/a2ui-runtime/src/catalog/validation_schema.rs` + `validation_schema_tests.rs`
  (Task 2).
- Modify `catalog/mod.rs`, `catalog/load.rs`, `catalog/load_tests.rs`, `catalog/compile_tests.rs`,
  `crates/a2ui-catalog-basic/src/lib_tests.rs`, `crates/a2ui-runtime/ARCHITECTURE.md`, the spec
  (Task 2).
- Create `crates/a2ui-conformance/src/handlers/validation_schema.rs`; modify `paths.rs`,
  `handlers/{mod,schema_doc,to_json}.rs`, `skips.toml`, `ARCHITECTURE.md`, `README.md` (Task 3).

---

### Task 1: Component flattening (`catalog/flatten.rs`)

**Oku:** t-1576 "Task 1: Component flattening (catalog/flatten.rs)".

**Files:**
- Create: `crates/a2ui-runtime/src/catalog/flatten.rs`, `crates/a2ui-runtime/src/catalog/flatten_tests.rs`
- Modify: `crates/a2ui-runtime/src/catalog/mod.rs` (`mod flatten;` after `mod entries;`),
  `crates/a2ui-runtime/src/bundle.rs`, `crates/a2ui-runtime/src/bundle_tests.rs`

**Interfaces:**
- Consumes: `crate::bundle::{local_ref, localize_refs, pointer_of}`, `crate::schema::common_type_defs`.
- Produces: `pub(crate) fn whole_def_name(reference: &str) -> Option<String>` (bundle.rs);
  `pub(super) fn flattened(name: &str, schema: &Value, defs: &Map<String, Value>, common: &Map<String, Value>) -> Flattened`
  and `pub(super) struct Flattened { pub(super) schema: Map<String, Value>, pub(super) merged: BTreeSet<String> }`
  (`merged`: the own `$defs` merged in as mixins). `defs` is the bundle's `$defs` (own defs,
  unions and localized common types); `common` is `common_type_defs()`.

- [ ] **Step 1: Create the module shell and write the failing tests**

`crates/a2ui-runtime/src/catalog/flatten.rs`, for now only:

```rust
//! Flattening a component schema into one object, as upstream's validation schema writes it.

#[cfg(test)]
#[path = "flatten_tests.rs"]
mod tests;
```

Add `mod flatten;` to `catalog/mod.rs` after `mod entries;`.

Append to `crates/a2ui-runtime/src/bundle_tests.rs`:

```rust
#[rstest]
#[case::entry("#/$defs/Weight", Some("Weight"))]
#[case::escaped("#/$defs/a~1b~0c", Some("a/b~c"))]
#[case::percent_encoded("#/$defs/%C2%B5", Some("µ"))]
#[case::below_the_entry("#/$defs/Weight/description", None)]
#[case::other_section("#/components/Weight", None)]
#[case::section_only("#/$defs", None)]
fn whole_def_name_names_only_a_whole_entry(#[case] reference: &str, #[case] name: Option<&str>) {
    assert_that!(whole_def_name(reference)).is_equal_to(name.map(str::to_owned));
}
```

`crates/a2ui-runtime/src/catalog/flatten_tests.rs`:

```rust
use std::collections::BTreeSet;

use rstest::rstest;
use serde_json::{Map, Value, json};
use speculoos::prelude::*;

use super::{Flattened, flattened};
use crate::bundle::localize_refs;
use crate::schema::common_type_defs;

/// The bundle `$defs` a component resolves in: `own` plus every common type, localized.
fn defs(own: Value) -> Map<String, Value> {
    let mut defs = own.as_object().unwrap().clone();
    for (name, definition) in common_type_defs().unwrap() {
        let mut definition = definition.clone();
        localize_refs(&mut definition);
        defs.insert(name.clone(), definition);
    }
    defs
}

/// Flattens `schema` as component `A`, with `own` as the catalog's `$defs`.
fn flat(schema: Value, own: Value) -> Flattened {
    flattened("A", &schema, &defs(own), common_type_defs().unwrap())
}

/// The flattened schema of `schema` as component `A`, with `own` as the catalog's `$defs`.
fn flat_with(schema: Value, own: Value) -> Value {
    Value::Object(flat(schema, own).schema)
}

/// The flattened schema of `schema` as component `A`, with no `$defs` of its own.
fn schema_of(schema: Value) -> Value {
    flat_with(schema, json!({}))
}

/// The property names of a flattened schema.
fn property_names(schema: &Value) -> BTreeSet<String> {
    schema["properties"].as_object().unwrap().keys().cloned().collect()
}

#[test]
fn flattened_empty_component_gets_the_envelope() {
    assert_that!(schema_of(json!({}))).is_equal_to(json!({
        "type": "object",
        "properties": {
            "id": {"$ref": "#/$defs/ComponentId"},
            "component": {"const": "A"}
        },
        "required": ["id", "component"]
    }));
}

#[test]
fn flattened_component_const_comes_from_its_property() {
    let schema = schema_of(json!({"properties": {"component": {"const": "Other"}}}));
    assert_that!(schema["properties"]["component"].clone()).is_equal_to(json!({"const": "Other"}));
}

#[test]
fn flattened_component_without_const_uses_entry_name() {
    let own = json!({"M": {"properties": {"component": {"const": "X"}}}});
    let schema = flat_with(
        json!({
            "allOf": [{"$ref": "#/$defs/M"}],
            "properties": {"component": {"description": "d"}}
        }),
        own,
    );
    assert_that!(schema["properties"]["component"].clone()).is_equal_to(json!({"const": "A"}));
}

#[test]
fn flattened_common_mixin_properties_are_merged() {
    let schema = schema_of(json!({"allOf": [{"$ref": "#/$defs/Checkable"}]}));
    assert_that!(schema["properties"]["checks"]["items"].clone())
        .is_equal_to(json!({"$ref": "#/$defs/CheckRule"}));
}

#[test]
fn flattened_merged_mixin_leaves_no_all_of() {
    let schema = schema_of(json!({"allOf": [{"$ref": "#/$defs/Checkable"}]}));
    assert_that!(schema.get("allOf")).is_none();
}

#[test]
fn flattened_common_mixin_description_is_not_copied() {
    let schema = schema_of(json!({"allOf": [{"$ref": "#/$defs/Checkable"}]}));
    assert_that!(schema.get("description")).is_none();
}

#[test]
fn flattened_component_common_drops_catalog_id() {
    let schema = schema_of(json!({"allOf": [{"$ref": "#/$defs/ComponentCommon"}]}));
    let expected: BTreeSet<String> = ["accessibility", "component", "id", "metadata"]
        .map(String::from)
        .into();
    assert_that!(property_names(&schema)).is_equal_to(expected);
}

#[test]
fn flattened_specific_piece_wins_over_local_mixin() {
    let own = json!({"M": {"properties": {"weight": {"type": "number"}}}});
    let schema = flat_with(
        json!({"allOf": [
            {"properties": {"component": {"const": "A"}, "weight": {"type": "integer"}}},
            {"$ref": "#/$defs/M"}
        ]}),
        own,
    );
    assert_that!(schema["properties"]["weight"].clone()).is_equal_to(json!({"type": "integer"}));
}

#[test]
fn flattened_local_mixin_with_component_property_stays_local() {
    let own = json!({"M": {"properties": {
        "component": {"const": "A"}, "w": {"type": "number"}
    }}});
    let schema = flat_with(
        json!({"properties": {"w": {"type": "string"}}, "allOf": [{"$ref": "#/$defs/M"}]}),
        own,
    );
    assert_that!(schema["properties"]["w"].clone()).is_equal_to(json!({"type": "string"}));
}

#[test]
fn flattened_local_mixin_is_reported_merged() {
    let own = json!({"M": {"properties": {"weight": {"type": "number"}}}});
    let merged = flat(json!({"allOf": [{"$ref": "#/$defs/M"}]}), own).merged;
    assert_that!(merged).is_equal_to(BTreeSet::from(["M".to_owned()]));
}

#[test]
fn flattened_mixin_cycle_merges_each_def_once() {
    let own = json!({
        "M": {"allOf": [{"$ref": "#/$defs/N"}], "properties": {"m": {}}},
        "N": {"allOf": [{"$ref": "#/$defs/M"}], "properties": {"n": {}}}
    });
    let merged = flat(json!({"allOf": [{"$ref": "#/$defs/M"}]}), own).merged;
    assert_that!(merged).is_equal_to(BTreeSet::from(["M".to_owned(), "N".to_owned()]));
}

#[rstest]
#[case::non_mixin_common_type(json!({"$ref": "#/$defs/DynamicString"}))]
#[case::ref_with_siblings(json!({"$ref": "#/$defs/Checkable", "description": "x"}))]
#[case::boolean_member(json!(true))]
#[case::sub_pointer_own_def(json!({"$ref": "#/$defs/M/properties"}))]
#[case::sub_pointer_common_type(json!({"$ref": "#/$defs/Checkable/properties"}))]
#[case::union(json!({"$ref": "#/$defs/anyComponent"}))]
fn flattened_unmergeable_member_is_kept_whole(#[case] member: Value) {
    let own = json!({
        "M": {"properties": {"m": {}}},
        "anyComponent": {"oneOf": [{"$ref": "#/components/A"}]}
    });
    let schema = flat_with(json!({"allOf": [member.clone()]}), own);
    assert_that!(schema["allOf"].clone()).is_equal_to(json!([member]));
}

#[test]
fn flattened_sub_pointer_member_merges_nothing() {
    let own = json!({"M": {
        "properties": {"m": {}},
        "allOf": [{"$ref": "#/$defs/N"}]
    }, "N": {"properties": {"n": {}}}});
    let found = flat(json!({"allOf": [{"$ref": "#/$defs/M/properties"}]}), own);
    assert_that!(found.merged.is_empty()).is_true();
}

#[test]
fn flattened_inline_member_extra_keys_stay_in_all_of() {
    let schema = schema_of(json!({"allOf": [
        {"properties": {"a": {"type": "string"}}, "minProperties": 1}
    ]}));
    assert_that!(schema["allOf"].clone()).is_equal_to(json!([{"minProperties": 1}]));
}

#[test]
fn flattened_inline_member_properties_are_merged() {
    let schema = schema_of(json!({"allOf": [
        {"properties": {"a": {"type": "string"}}, "minProperties": 1}
    ]}));
    assert_that!(schema["properties"]["a"].clone()).is_equal_to(json!({"type": "string"}));
}

#[test]
fn flattened_local_mixin_extra_keys_stay_in_all_of() {
    let own = json!({"M": {"properties": {"m": {}}, "minProperties": 1}});
    let schema = flat_with(json!({"allOf": [{"$ref": "#/$defs/M"}]}), own);
    assert_that!(schema["allOf"].clone()).is_equal_to(json!([{"minProperties": 1}]));
}

#[test]
fn flattened_member_closing_keyword_is_dropped() {
    let schema = schema_of(json!({"allOf": [
        {"properties": {"a": {}}, "additionalProperties": false}
    ]}));
    let kept = (schema.get("allOf"), schema.get("unevaluatedProperties"));
    assert_that!(kept).is_equal_to((None, None));
}

#[test]
fn flattened_required_drops_protocol_keys_and_sorts() {
    let schema = schema_of(json!({"required": ["b", "catalogId", "a", "b"]}));
    assert_that!(schema["required"].clone()).is_equal_to(json!(["id", "a", "b", "component"]));
}

#[test]
fn flattened_specific_description_wins_over_common() {
    let schema = schema_of(json!({
        "description": "Top",
        "allOf": [{"properties": {"component": {"const": "A"}}, "description": "Specific"}]
    }));
    assert_that!(schema["description"].clone()).is_equal_to(json!("Specific"));
}

#[test]
fn flattened_empty_description_keeps_the_earlier_one() {
    let schema = schema_of(json!({
        "description": "Top",
        "allOf": [{"properties": {"component": {"const": "A"}}, "description": ""}]
    }));
    assert_that!(schema["description"].clone()).is_equal_to(json!("Top"));
}

#[test]
fn flattened_additional_properties_is_removed() {
    let schema = schema_of(json!({"additionalProperties": false}));
    assert_that!(schema.get("additionalProperties")).is_none();
}

#[test]
fn flattened_additional_properties_becomes_unevaluated() {
    let schema = schema_of(json!({"additionalProperties": false}));
    assert_that!(schema["unevaluatedProperties"].clone()).is_equal_to(json!(false));
}

#[test]
fn flattened_unevaluated_properties_wins_over_additional() {
    let schema = schema_of(json!({
        "unevaluatedProperties": true, "additionalProperties": false
    }));
    assert_that!(schema["unevaluatedProperties"].clone()).is_equal_to(json!(true));
}

#[rstest]
#[case::parents_empty("allowedParents", json!([]), None)]
#[case::parents_listed("allowedParents", json!(["Row"]), Some(json!(["Row"])))]
#[case::children_empty("allowedChildren", json!([]), None)]
#[case::children_listed("allowedChildren", json!(["Row"]), Some(json!(["Row"])))]
fn flattened_allowed_list_kept_when_non_empty(
    #[case] key: &str,
    #[case] listed: Value,
    #[case] expected: Option<Value>,
) {
    let schema = schema_of(json!({key: listed}));
    assert_that!(schema.get(key).cloned()).is_equal_to(expected);
}

#[test]
fn flattened_other_top_level_key_is_kept() {
    let metadata = json!({"extensions": {"vendor": 1}});
    let schema = schema_of(json!({"metadata": metadata.clone()}));
    assert_that!(schema["metadata"].clone()).is_equal_to(metadata);
}

#[test]
fn flattened_own_id_property_is_replaced_by_the_envelope() {
    let schema = schema_of(json!({"properties": {"id": {"type": "integer"}}}));
    assert_that!(schema["properties"]["id"].clone())
        .is_equal_to(json!({"$ref": "#/$defs/ComponentId"}));
}
```

- [ ] **Step 2: Run the tests to see them fail**

Run: `cargo nextest run -p a2ui-runtime flatten whole_def_name`
Expected: fails to compile: unresolved imports `super::{Flattened, flattened}` and
`whole_def_name` (the compile failure is the red; Plan 2 rule: no stub bodies).

- [ ] **Step 3: Implement**

In `crates/a2ui-runtime/src/bundle.rs`, after `fn def_name`, add:

```rust
/// The `$defs` entry a local `$ref` names as a whole, decoded and unescaped.
///
/// `None` for a pointer below an entry (`#/$defs/A/properties`): only a whole entry is a mixin
/// or a described common type, because only a whole entry's `$ref`s were followed by `expand`.
pub(crate) fn whole_def_name(reference: &str) -> Option<String> {
    let pointer = pointer_of(reference)?;
    let token = pointer.strip_prefix("/$defs/")?;
    (!token.contains('/')).then(|| token.replace("~1", "/").replace("~0", "~"))
}
```

Replace `crates/a2ui-runtime/src/catalog/flatten.rs` with:

```rust
//! Flattening a component schema into one object, as upstream's validation schema writes it.
//!
//! A component mixes in common types (`ComponentCommon`, `Checkable`), own `$defs` and inline
//! schemas through `allOf`. Flattening merges their `properties` and `required`, writes the
//! `id` and `component` keys itself, drops `catalogId` and keeps what it cannot merge in a
//! residual `allOf`. A later piece's property replaces an earlier one where `allOf` would
//! require both, so the result can accept instances the entry's validator rejects.
//!
//! Unlike [`ComponentShape`](super::ComponentShape), which lists every property name the
//! validator may see through any `$ref`, flattening follows only `allOf` members that are a
//! whole-entry `$ref`, as upstream does.
//!
//! It runs only on a bundle `compile_entries` accepted: every mixin it follows is a whole
//! `$defs` entry that `expand` walked from the same site, so its copies stay within
//! `MAX_CATALOG_EXPANSION` and its recursion within `MAX_SCHEMA_DEPTH` / 3 mixin hops.

use std::collections::BTreeSet;

use serde_json::{Map, Value, json};

use crate::bundle::{local_ref, whole_def_name};

/// Properties the protocol owns: flattening writes `id` and `component` and drops `catalogId`.
///
/// Unlike the instance envelope `validate` strips, `metadata` is a catalog property here.
const PROTOCOL_KEYS: [&str; 3] = ["id", "component", "catalogId"];

/// Keys flattening merges or rewrites; a component's other keys are copied.
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

/// The union definitions, never merged as mixins.
const UNIONS: [&str; 2] = ["anyComponent", "anyFunction"];

/// Where a piece comes from; pieces merge in this order, so a later kind wins.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Kind {
    Mixin,
    Local,
    Common,
    Specific,
}

/// The `properties`, `required` and `description` one schema contributes.
struct Piece<'a> {
    kind: Kind,
    properties: Option<&'a Map<String, Value>>,
    required: Option<&'a Vec<Value>>,
    description: Option<&'a str>,
}

/// What collecting one component found.
#[derive(Default)]
struct Collected<'a> {
    pieces: Vec<Piece<'a>>,
    residual: Vec<Value>,
    /// Every `$defs` entry followed as a mixin, common or own.
    seen: BTreeSet<String>,
    /// The own `$defs` entries merged in.
    merged: BTreeSet<String>,
}

/// A flattened component and the own `$defs` it merged in as mixins.
#[cfg_attr(not(test), expect(dead_code, reason = "Plan 2b Task 2 builds it from load"))]
pub(super) struct Flattened {
    /// The flattened schema.
    pub(super) schema: Map<String, Value>,
    /// Names of the own `$defs` merged in, which `$defs` may then drop.
    pub(super) merged: BTreeSet<String>,
}

/// Flattens the component `name`; `defs` is the bundle's `$defs`, `common` the common types.
///
/// A loaded entry is always an object; any other value reads as an object with no keys.
#[cfg_attr(not(test), expect(dead_code, reason = "Plan 2b Task 2 calls it from load"))]
pub(super) fn flattened(
    name: &str,
    schema: &Value,
    defs: &Map<String, Value>,
    common: &Map<String, Value>,
) -> Flattened {
    let mut found = Collected::default();
    collect(schema, defs, common, false, true, &mut found);
    found.pieces.sort_by_key(|piece| piece.kind);
    let mut properties = Map::new();
    let mut required = BTreeSet::new();
    let mut description = None;
    let mut constant = Value::from(name);
    for piece in &found.pieces {
        for (key, value) in piece.properties.into_iter().flatten() {
            if key == "component" {
                constant = value.get("const").cloned().unwrap_or_else(|| Value::from(name));
            }
            if !PROTOCOL_KEYS.contains(&key.as_str()) {
                properties.insert(key.clone(), value.clone());
            }
        }
        let names = piece.required.into_iter().flatten().filter_map(Value::as_str);
        required.extend(names.filter(|name| !PROTOCOL_KEYS.contains(name)).map(str::to_owned));
        if let Some(text) = piece.description.filter(|text| !text.is_empty()) {
            description = Some(text);
        }
    }
    properties.insert("id".to_owned(), json!({"$ref": local_ref("/$defs/ComponentId")}));
    properties.insert("component".to_owned(), json!({"const": constant}));
    let required: Vec<Value> = ["id".to_owned()]
        .into_iter()
        .chain(required)
        .chain(["component".to_owned()])
        .map(Value::from)
        .collect();
    let mut out = Map::from_iter([("type".to_owned(), Value::from("object"))]);
    for (key, value) in schema.as_object().into_iter().flatten() {
        if !MERGED.contains(&key.as_str()) {
            out.insert(key.clone(), value.clone());
        }
    }
    if let Some(text) = description {
        out.insert("description".to_owned(), Value::from(text));
    }
    out.insert("properties".to_owned(), Value::Object(properties));
    out.insert("required".to_owned(), Value::Array(required));
    if !found.residual.is_empty() {
        out.insert("allOf".to_owned(), Value::Array(found.residual));
    }
    let closed = schema
        .get("unevaluatedProperties")
        .or_else(|| schema.get("additionalProperties"));
    if let Some(closed) = closed {
        out.insert("unevaluatedProperties".to_owned(), closed.clone());
    }
    for key in ["allowedParents", "allowedChildren"] {
        let listed = schema
            .get(key)
            .filter(|list| list.as_array().is_some_and(|items| !items.is_empty()));
        if let Some(listed) = listed {
            out.insert(key.to_owned(), listed.clone());
        }
    }
    Flattened {
        schema: out,
        merged: found.merged,
    }
}

/// Collects the pieces of `schema` and of the `allOf` members it can merge.
///
/// `local` marks a schema reached through an own mixin; `top` the component itself, whose
/// unmerged keys the result copies rather than the residual `allOf`. Recursion follows inline
/// members (bounded by the document's 64 levels) and whole-entry mixins, each once per
/// component (bounded by `expand`'s depth cap, see the module documentation).
fn collect<'a>(
    schema: &'a Value,
    defs: &'a Map<String, Value>,
    common: &Map<String, Value>,
    local: bool,
    top: bool,
    out: &mut Collected<'a>,
) {
    let members = schema.get("allOf").and_then(Value::as_array);
    for member in members.into_iter().flatten() {
        if !member.is_object() {
            out.residual.push(member.clone());
            continue;
        }
        if member.get("$ref").is_none() {
            collect(member, defs, common, local, false, out);
            continue;
        }
        match mixin_target(member, defs) {
            Some((name, target)) if common.contains_key(&name) => {
                let Some(properties) = target.get("properties").and_then(Value::as_object) else {
                    out.residual.push(member.clone());
                    continue;
                };
                if out.seen.insert(name) {
                    out.pieces.push(Piece {
                        kind: Kind::Mixin,
                        properties: Some(properties),
                        required: target.get("required").and_then(Value::as_array),
                        description: None,
                    });
                }
            }
            Some((name, target)) => {
                if out.seen.insert(name.clone()) {
                    out.merged.insert(name);
                    collect(target, defs, common, true, false, out);
                }
            }
            None => out.residual.push(member.clone()),
        }
    }
    if !top {
        let rest: Map<String, Value> = schema
            .as_object()
            .into_iter()
            .flatten()
            .filter(|(key, _)| !MERGED.contains(&key.as_str()))
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect();
        if !rest.is_empty() {
            out.residual.push(Value::Object(rest));
        }
    }
    if ["properties", "required", "description"]
        .iter()
        .any(|key| schema.get(*key).is_some())
    {
        let properties = schema.get("properties").and_then(Value::as_object);
        let kind = if local {
            Kind::Local
        } else if properties.is_some_and(|map| map.contains_key("component")) {
            Kind::Specific
        } else {
            Kind::Common
        };
        out.pieces.push(Piece {
            kind,
            properties,
            required: schema.get("required").and_then(Value::as_array),
            description: schema.get("description").and_then(Value::as_str),
        });
    }
}

/// The whole `$defs` entry a member names, when `$ref` is its only key and it is no union.
fn mixin_target<'a>(member: &Value, defs: &'a Map<String, Value>) -> Option<(String, &'a Value)> {
    let member = member.as_object()?;
    if member.len() != 1 {
        return None;
    }
    let name = whole_def_name(member.get("$ref")?.as_str()?)?;
    if UNIONS.contains(&name.as_str()) {
        return None;
    }
    let target = defs.get(&name)?;
    Some((name, target))
}

#[cfg(test)]
#[path = "flatten_tests.rs"]
mod tests;
```

The two `cfg_attr(not(test), expect(dead_code, …))` bridge Task 1 to Task 2, which removes
them (Plan 2 precedent; rustc treats an `expect`ed item as live, so its private callees raise no
warning — checked on rustc 1.99 by three reviewers). `whole_def_name` has a caller (`flatten`)
in every build.

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo nextest run -p a2ui-runtime flatten whole_def_name`
Expected: every `flattened_*` and `whole_def_name_*` test passes.

- [ ] **Step 5: Gate and stage**

Run: `cargo fmt --all && just check && just test` (all green), then

```bash
git add crates/a2ui-runtime/src/catalog/flatten.rs crates/a2ui-runtime/src/catalog/flatten_tests.rs crates/a2ui-runtime/src/catalog/mod.rs crates/a2ui-runtime/src/bundle.rs crates/a2ui-runtime/src/bundle_tests.rs
```

Stop for the review. After it, commit with
`feat(runtime): flatten component schemas as upstream's validation schema does (t-1576)` plus the
`Reviewed-by:` trailer, then
`oku cli task done --project slider --id t-1576 --result "catalog/flatten.rs merges whole-entry allOf mixins into upstream's flattened component shape"`.

---

### Task 2: Assemble `Catalog::validation_schema` from the checked bundle

**Oku:** t-1577 "Task 2: Assemble Catalog::validation_schema from the bundle".

**Files:**
- Create: `crates/a2ui-runtime/src/catalog/validation_schema.rs`,
  `crates/a2ui-runtime/src/catalog/validation_schema_tests.rs`
- Modify: `crates/a2ui-runtime/src/catalog/compile.rs`, `crates/a2ui-runtime/src/catalog/compile_tests.rs`,
  `crates/a2ui-runtime/src/catalog/test_support.rs`,
  `crates/a2ui-runtime/src/catalog/load.rs`, `crates/a2ui-runtime/src/catalog/load_tests.rs`,
  `crates/a2ui-runtime/src/catalog/mod.rs`, `crates/a2ui-runtime/src/catalog/flatten.rs`,
  `crates/a2ui-runtime/src/bundle.rs`, `crates/a2ui-catalog-basic/src/lib_tests.rs`,
  `crates/a2ui-runtime/ARCHITECTURE.md`, `docs/specs/2026-11-10-a2ui-rs-leptos-design.md`

**Interfaces:**
- Consumes: Task 1's `flattened`/`Flattened`, `whole_def_name`; `crate::bundle::{add_common_types, def_names}`.
- Produces: `pub(super) struct Bundle` with `components()`, `functions()`, `defs()` returning
  `&Map<String, Value>`; `pub(super) struct Checked<'a>` with `bundle(&self) -> &'a Bundle`;
  `pub(super) fn bundle(components: &BTreeMap<String, ComponentAttrs>, functions: &BTreeMap<String, FunctionAttrs>, defs: Option<&Map<String, Value>>) -> Result<Bundle, Error>`;
  `pub(super) fn compile_entries<'a>(bundle: &'a Bundle, components: BTreeMap<String, ComponentAttrs>, functions: BTreeMap<String, FunctionAttrs>) -> Result<(Definitions, Checked<'a>), Error>`;
  `pub(super) fn build(checked: Checked<'_>, id: &str, version: &str, instructions: Option<&str>, common: &Map<String, Value>) -> Value`
  in `catalog::validation_schema`; `Catalog::validation_schema(&self) -> &Value` (same signature,
  new content).

- [ ] **Step 1: Create the module shell and write the failing tests**

`crates/a2ui-runtime/src/catalog/validation_schema.rs`, for now only:

```rust
//! The validation schema: the checked bundle in upstream's flattened shape.

#[cfg(test)]
#[path = "validation_schema_tests.rs"]
mod tests;
```

Add `mod validation_schema;` to `catalog/mod.rs` after `mod validate;`.

`crates/a2ui-runtime/src/catalog/validation_schema_tests.rs` (these tests call the existing
accessor, so they compile now and fail on the Plan 2 bundle; the ones marked *guard* pass before
and after and pin behavior that must not change):

```rust
use rstest::rstest;
use serde_json::{Map, Value, json};
use speculoos::prelude::*;

use super::super::test_support::{load, mixin_copies, v10};
use crate::Catalog;
use crate::bundle::localize_refs;
use crate::limits::{MAX_CATALOG_EXPANSION, oversize};
use crate::schema::common_type_defs;

/// The authored `$ref` to the `DynamicString` common type.
const DYNAMIC_STRING: &str = "common_types.json#/$defs/DynamicString";

/// The validation schema of a v1.0 catalog holding `extra`.
fn schema(extra: Value) -> Value {
    load(v10(extra)).unwrap().validation_schema().clone()
}

/// The validation schema of a catalog whose component `A` has property `label`.
fn with_label(label: Value) -> Value {
    schema(json!({"components": {"A": {"properties": {"label": label}}}}))
}

/// The flattened `label` property of [`with_label`]'s component.
fn label(label: Value) -> Value {
    with_label(label)["components"]["A"]["properties"]["label"].clone()
}

/// The `args` of function `f` declared with `properties`.
fn args_of(properties: Value) -> Value {
    let result = schema(json!({
        "$defs": {"Args": {"type": "object"}},
        "functions": {"f": {"properties": properties}}
    }));
    result["functions"]["f"]["properties"]["args"].clone()
}

#[rstest]
#[case::id("$id")]
#[case::title("title")]
#[case::description("description")]
#[case::metadata("metadata")]
fn validation_schema_header_omits_document_key(#[case] key: &str) {
    let result = schema(json!({key: "x", "components": {"A": {}}}));
    assert_that!(result.get(key)).is_none();
}

#[test]
fn validation_schema_header_names_schema_id_and_version() {
    let result = schema(json!({"components": {"A": {}}}));
    let header = (
        result["$schema"].clone(),
        result["catalogId"].clone(),
        result["protocolVersion"].clone(),
    );
    assert_that!(header).is_equal_to((
        json!("https://json-schema.org/draft/2020-12/schema"),
        json!("c"),
        json!("1.0"),
    ));
}

#[test]
fn validation_schema_header_uses_the_effective_id() {
    let document = v10(json!({"components": {"A": {}}}));
    let catalog = Catalog::from_json(&document, Some("other"), None).unwrap();
    assert_that!(catalog.validation_schema()["catalogId"].clone()).is_equal_to(json!("other"));
}

#[rstest]
#[case::text("Use Rows.", Some(json!("Use Rows.")))]
#[case::empty("", None)]
fn validation_schema_instructions_kept_when_non_empty(
    #[case] text: &str,
    #[case] expected: Option<Value>,
) {
    let result = schema(json!({"instructions": text, "components": {"A": {}}}));
    assert_that!(result.get("instructions").cloned()).is_equal_to(expected);
}

#[test]
fn validation_schema_components_are_flattened() {
    let result = schema(json!({"components": {"A": {
        "allOf": [{"$ref": "common_types.json#/$defs/Checkable"}]
    }}}));
    assert_that!(result["components"]["A"]["properties"].get("checks")).is_some();
}

#[test]
fn validation_schema_without_functions_has_no_functions_key() {
    assert_that!(schema(json!({"components": {"A": {}}})).get("functions")).is_none();
}

#[test]
fn validation_schema_kept_function_is_listed() {
    let result = schema(json!({"functions": {"f": {"returnType": "string"}}}));
    assert_that!(result["functions"].get("f")).is_some();
}

#[rstest]
#[case::absent(
    json!({"type": "object"}),
    json!({"type": "object", "unevaluatedProperties": false})
)]
#[case::moved(
    json!({"type": "object", "additionalProperties": true}),
    json!({"type": "object", "unevaluatedProperties": true})
)]
#[case::unevaluated_kept(
    json!({"type": "object", "unevaluatedProperties": true, "additionalProperties": false}),
    json!({"type": "object", "unevaluatedProperties": true})
)]
#[case::by_ref(
    json!({"$ref": "#/$defs/Args", "type": "object"}),
    json!({"$ref": "#/$defs/Args", "type": "object"})
)]
#[case::not_an_object_schema(json!({"type": "array"}), json!({"type": "array"}))]
fn validation_schema_function_args_are_closed(#[case] args: Value, #[case] expected: Value) {
    let result = args_of(json!({"@call": {"const": "f"}, "args": args}));
    assert_that!(result).is_equal_to(expected);
}

#[rstest]
#[case::at_call("@call", true)]
#[case::call("call", true)]
#[case::no_call("name", false)]
fn validation_schema_args_closed_only_beside_a_call(#[case] key: &str, #[case] closed: bool) {
    let result = args_of(json!({key: {"const": "f"}, "args": {"type": "object"}}));
    assert_that!(result.get("unevaluatedProperties").is_some()).is_equal_to(closed);
}

#[test]
fn validation_schema_common_ref_gets_its_description() {
    let result = label(json!({"$ref": "common_types.json#/$defs/DynamicString"}));
    assert_that!(result["description"].clone()).is_equal_to(json!("Represents a string"));
}

#[test]
fn validation_schema_call_site_description_wins() {
    let result = label(json!({
        "$ref": "common_types.json#/$defs/DynamicString", "description": "Mine"
    }));
    assert_that!(result["description"].clone()).is_equal_to(json!("Mine"));
}

#[test]
fn validation_schema_own_def_ref_gets_no_description() {
    let result = schema(json!({
        "$defs": {"W": {"type": "number", "description": "Weight"}},
        "components": {"A": {"properties": {"w": {"$ref": "#/$defs/W"}}}}
    }));
    assert_that!(result["components"]["A"]["properties"]["w"].clone())
        .is_equal_to(json!({"$ref": "#/$defs/W"}));
}

#[test]
fn validation_schema_sub_pointer_common_ref_gets_no_description() {
    let reference = json!({"$ref": "common_types.json#/$defs/DynamicString/oneOf/0"});
    let result = label(reference);
    assert_that!(result.get("description")).is_none();
}

#[rstest]
#[case::const_data("const", json!({"$ref": "common_types.json#/$defs/DynamicString"}))]
#[case::enum_data("enum", json!([{"$ref": "common_types.json#/$defs/DynamicString"}]))]
#[case::default_data("default", json!({"$ref": "common_types.json#/$defs/DynamicString"}))]
#[case::examples_data("examples", json!([{"$ref": "common_types.json#/$defs/DynamicString"}]))]
fn validation_schema_ref_inside_data_is_not_described(#[case] keyword: &str, #[case] data: Value) {
    let result = label(json!({keyword: data}));
    assert_that!(result[keyword].to_string().contains("description")).is_false();
}

#[rstest]
#[case::items(json!({"items": {"$ref": DYNAMIC_STRING}}), "/items")]
#[case::any_of(json!({"anyOf": [{"$ref": DYNAMIC_STRING}]}), "/anyOf/0")]
#[case::prefix_items(json!({"prefixItems": [{"$ref": DYNAMIC_STRING}]}), "/prefixItems/0")]
#[case::additional(
    json!({"additionalProperties": {"$ref": DYNAMIC_STRING}}),
    "/additionalProperties"
)]
#[case::not(json!({"not": {"$ref": DYNAMIC_STRING}}), "/not")]
fn validation_schema_ref_in_schema_position_is_described(
    #[case] property: Value,
    #[case] at: &str,
) {
    let result = label(property);
    let described = result.pointer(&format!("{at}/description")).cloned();
    assert_that!(described).is_equal_to(Some(json!("Represents a string")));
}

#[test]
fn validation_schema_property_named_like_a_data_keyword_is_described() {
    let result = schema(json!({"components": {"A": {"properties": {
        "default": {"$ref": "common_types.json#/$defs/DynamicString"}
    }}}}));
    assert_that!(result["components"]["A"]["properties"]["default"]["description"].clone())
        .is_equal_to(json!("Represents a string"));
}

#[test]
fn validation_schema_own_defs_are_described() {
    let result = schema(json!({
        "$defs": {"Label": {"$ref": "common_types.json#/$defs/DynamicString"}},
        "components": {"A": {"properties": {"l": {"$ref": "#/$defs/Label"}}}}
    }));
    assert_that!(result["$defs"]["Label"]["description"].clone())
        .is_equal_to(json!("Represents a string"));
}

#[test]
fn validation_schema_functions_are_described() {
    let result = schema(json!({"functions": {"f": {"properties": {"args": {
        "type": "object",
        "properties": {"v": {"$ref": "common_types.json#/$defs/DynamicString"}}
    }}}}}));
    let argument = &result["functions"]["f"]["properties"]["args"]["properties"]["v"];
    assert_that!(argument["description"].clone()).is_equal_to(json!("Represents a string"));
}

#[test]
fn validation_schema_common_types_are_copied_verbatim() {
    let result = with_label(json!({"$ref": "common_types.json#/$defs/DynamicString"}));
    let mut expected = common_type_defs().unwrap()["DynamicString"].clone();
    localize_refs(&mut expected);
    assert_that!(result["$defs"]["DynamicString"].clone()).is_equal_to(expected);
}

#[rstest]
#[case::checkable("Checkable", false)]
#[case::component_common("ComponentCommon", false)]
#[case::check_rule("CheckRule", true)]
fn validation_schema_bundles_only_reached_common_types(#[case] name: &str, #[case] kept: bool) {
    let result = schema(json!({"components": {"A": {"allOf": [
        {"$ref": "common_types.json#/$defs/ComponentCommon"},
        {"$ref": "common_types.json#/$defs/Checkable"}
    ]}}}));
    assert_that!(result["$defs"].get(name).is_some()).is_equal_to(kept);
}

#[rstest]
#[case::merged_only(json!({"M": {"properties": {"m": {}}}}), json!({}), false)]
#[case::named_by_own_def(
    json!({"M": {"properties": {"m": {}}}, "U": {"$ref": "#/$defs/M"}}),
    json!({}),
    true
)]
#[case::named_by_a_property(
    json!({"M": {"properties": {"m": {}}}}),
    json!({"x": {"$ref": "#/$defs/M"}}),
    true
)]
fn validation_schema_merged_mixin_pruned_unless_named(
    #[case] defs: Value,
    #[case] properties: Value,
    #[case] kept: bool,
) {
    let result = schema(json!({
        "$defs": defs,
        "components": {"A": {"allOf": [{"$ref": "#/$defs/M"}], "properties": properties}}
    }));
    assert_that!(result["$defs"].get("M").is_some()).is_equal_to(kept);
}

#[test]
fn validation_schema_union_named_as_a_member_stays_defined() {
    let result = schema(json!({"components": {"A": {
        "allOf": [{"$ref": "#/$defs/anyComponent"}]
    }}}));
    assert_that!(result["$defs"].get("anyComponent")).is_some();
}

/// Guard: the budget rule; a lookahead pattern fails any compile, so this def was never built.
#[test]
fn validation_schema_unreached_def_is_copied_never_compiled() {
    let unused = json!({"type": "string", "pattern": "(?=a)"});
    let result = schema(json!({"$defs": {"Unused": unused.clone()}, "components": {"A": {}}}));
    assert_that!(result["$defs"]["Unused"].clone()).is_equal_to(unused);
}

#[test]
fn validation_schema_mixin_copies_stay_within_catalog_expansion() {
    let catalog = load(mixin_copies(21)).unwrap();
    assert_that!(oversize(catalog.validation_schema(), MAX_CATALOG_EXPANSION)).is_none();
}

#[test]
fn validation_schema_sub_pointer_mixin_chain_is_not_followed() {
    let result = schema(json!({
        "$defs": {
            "M": {"properties": {"m": {}}, "allOf": [{"$ref": "#/$defs/N"}]},
            "N": {"properties": {"n": {}}}
        },
        "components": {"A": {"allOf": [{"$ref": "#/$defs/M/properties"}]}}
    }));
    assert_that!(result["components"]["A"]["properties"].get("n")).is_none();
}

#[test]
fn from_json_long_sub_pointer_chain_loads_on_a_small_stack() {
    let links = 20_000;
    let mut defs: Map<String, Value> = (0..links - 1)
        .map(|index| {
            let next = format!("#/$defs/M{}", index + 1);
            (format!("M{index}"), json!({"properties": {"p": {}}, "allOf": [{"$ref": next}]}))
        })
        .collect();
    defs.insert(format!("M{}", links - 1), json!({"properties": {"p": {}}}));
    let document = v10(json!({
        "$defs": defs,
        "components": {"A": {"allOf": [{"$ref": "#/$defs/M0/properties"}]}}
    }));
    let loaded = std::thread::Builder::new()
        .stack_size(1 << 20)
        .spawn(move || load(document).map(|catalog| catalog.validation_schema().clone()))
        .unwrap()
        .join()
        .unwrap();
    assert_that!(loaded).is_ok();
}
```

(`from_json_long_sub_pointer_chain_loads_on_a_small_stack` is the regression for the review's
critical finding: with a first-token mixin rule it recursed 20 000 levels and aborted.)

Add to `crates/a2ui-runtime/src/catalog/test_support.rs` (shared by `compile_tests.rs` and
`validation_schema_tests.rs`):

```rust
/// A catalog of `count` components that each mix in a 3 000-property `$defs` entry.
///
/// Each component's expansion charges about 12 010 nodes: 21 fit `MAX_CATALOG_EXPANSION`
/// (262 144), 22 do not.
pub(crate) fn mixin_copies(count: usize) -> Value {
    let properties: Map<String, Value> = (0..3000)
        .map(|index| (format!("p{index}"), json!({"type": "string"})))
        .collect();
    let components: Map<String, Value> = (0..count)
        .map(|index| (format!("C{index}"), json!({"allOf": [{"$ref": "#/$defs/Big"}]})))
        .collect();
    v10(json!({"$defs": {"Big": {"properties": properties}}, "components": components}))
}
```

Append to `crates/a2ui-runtime/src/catalog/compile_tests.rs` (guards of the caps Decision 1
relies on; add `mixin_copies` to its `test_support` import):

```rust
#[test]
fn from_json_21_mixin_copies_fit_catalog_expansion() {
    assert_that!(load(mixin_copies(21))).is_ok();
}

#[test]
fn from_json_22_mixin_copies_past_catalog_expansion_is_catalog_error() {
    assert_that!(catalog_error(load(mixin_copies(22))))
        .contains("schema nodes in all entries once their $refs are expanded");
}

#[test]
fn from_json_mixin_chain_past_schema_depth_is_catalog_error() {
    let hops = 50;
    let mut defs: Map<String, Value> = (0..hops)
        .map(|index| {
            let next = format!("#/$defs/M{}", index + 1);
            (format!("M{index}"), json!({"allOf": [{"$ref": next}]}))
        })
        .collect();
    defs.insert(format!("M{hops}"), json!({"properties": {"p": {}}}));
    let document = v10(json!({"$defs": defs, "components": {"A": {
        "allOf": [{"$ref": "#/$defs/M0"}]
    }}}));
    assert_that!(catalog_error(load(document))).contains("levels of 'A'");
}
```

In `crates/a2ui-runtime/src/catalog/load_tests.rs` add (imports are already there):

```rust
#[test]
fn from_json_non_string_instructions_is_catalog_error() {
    assert_that!(catalog_error(load(v10(json!({"instructions": 3})))))
        .contains("`instructions` must be a string");
}
```

Before adding it, run `grep -rnE '"?instructions"?[[:space:]]*:' vendor/a2ui/conformance vendor/a2ui/catalogs`
and check that every hit in a catalog is a string; if one is not, stop and report.

In `crates/a2ui-catalog-basic/src/lib_tests.rs` add (Decision 13):

```rust
#[test]
fn catalog_validation_schema_properties_are_within_shapes() {
    let basic = catalog().unwrap();
    let outside: Vec<String> = basic
        .components()
        .flat_map(|def| {
            let flat = &basic.validation_schema()["components"][def.name()]["properties"];
            let names: Vec<String> = flat.as_object().unwrap().keys().cloned().collect();
            names
                .into_iter()
                .filter(|name| name != "id" && name != "component")
                .filter(|name| !def.shape().properties().contains(name))
                .map(|name| format!("{}.{name}", def.name()))
                .collect::<Vec<_>>()
        })
        .collect();
    assert_that!(outside).is_empty();
}
```

- [ ] **Step 2: Run the tests to see them fail**

Run: `cargo nextest run -p a2ui-runtime -p a2ui-catalog-basic validation_schema from_json catalog_validation_schema`
Expected: the tests reading flattened output fail on the Plan 2 bundle (among them
`validation_schema_components_are_flattened`, `validation_schema_without_functions_has_no_functions_key`,
the description, `close_args` and pruning tests, `catalog_validation_schema_properties_are_within_shapes`
may pass already), and `from_json_non_string_instructions_is_catalog_error` fails (the load
succeeds). The guards (`validation_schema_header_omits_document_key` for `metadata`/`title`,
`validation_schema_unreached_def_is_copied_never_compiled`,
`from_json_long_sub_pointer_chain_loads_on_a_small_stack`, the three `compile_tests` cases) pass
already. Record which failed and which passed in the task report.

- [ ] **Step 3: Implement**

In `crates/a2ui-runtime/src/bundle.rs`, change `fn def_names(` to `pub(crate) fn def_names(`.

In `crates/a2ui-runtime/src/catalog/compile.rs`:
- `bundle` loses its `id` and `version` parameters and returns a `Bundle`; its body is unchanged
  up to `add_common_types(...)`, then ends with `Ok(Bundle { components, functions, defs })`;
  drop the now-unused `DRAFT_2020_12` import if nothing else uses it;
- add, after `bundle`:

```rust
/// The bundled catalog: kept entries and `$defs`, every `$ref` local.
///
/// `$defs` holds the catalog's own definitions, the unions and the common types they reach.
pub(super) struct Bundle {
    components: Map<String, Value>,
    functions: Map<String, Value>,
    defs: Map<String, Value>,
}

impl Bundle {
    /// The kept components, localized, by name.
    pub(super) fn components(&self) -> &Map<String, Value> {
        &self.components
    }

    /// The kept functions, localized, by name.
    pub(super) fn functions(&self) -> &Map<String, Value> {
        &self.functions
    }

    /// The `$defs`: own definitions, unions and reached common types.
    pub(super) fn defs(&self) -> &Map<String, Value> {
        &self.defs
    }

    /// The bundle as one document, each section under its catalog key.
    fn document(&self) -> Value {
        Value::Object(Map::from_iter([
            ("components".to_owned(), Value::Object(self.components.clone())),
            ("functions".to_owned(), Value::Object(self.functions.clone())),
            ("$defs".to_owned(), Value::Object(self.defs.clone())),
        ]))
    }
}

/// A bundle whose every entry passed the expansion caps and compiled.
///
/// Only [`compile_entries`] makes one, so code taking it runs after the caps were enforced.
pub(super) struct Checked<'a>(&'a Bundle);

impl<'a> Checked<'a> {
    /// The checked bundle.
    pub(super) fn bundle(&self) -> &'a Bundle {
        self.0
    }
}
```

- `compile_entries` becomes
  `pub(super) fn compile_entries<'a>(bundle: &'a Bundle, components: …, functions: …) -> Result<(Definitions, Checked<'a>), Error>`:
  its first lines are `let document = bundle.document(); let root = validator_top(&document);`,
  `ComponentShape::of(&document, attrs.schema())` replaces `ComponentShape::of(bundle, …)`, and
  it ends with `Ok(((components, functions), Checked(bundle)))`. The document no longer carries
  `$schema`/`catalogId`/`protocolVersion`: nothing in compile reads them (`rooted` adds
  `$schema`); if a test shows otherwise, stop and report.

Replace `crates/a2ui-runtime/src/catalog/validation_schema.rs` with:

```rust
//! The validation schema: the checked bundle in upstream's flattened shape.
//!
//! [`build`] takes a [`Checked`] bundle, so it only runs after `compile_entries` enforced the
//! expansion caps on every `$ref` it follows (see `flatten`). The result keeps `$defs` no entry
//! reaches, which those caps never budgeted, so it is output only: nothing compiles it.

use std::collections::BTreeSet;

use serde_json::{Map, Value};

use super::compile::Checked;
use super::flatten::flattened;
use crate::bundle::{add_common_types, def_names, whole_def_name};
use crate::validator::DRAFT_2020_12;

/// Keywords whose value maps names to subschemas.
const SCHEMA_MAPS: [&str; 4] = ["properties", "patternProperties", "$defs", "dependentSchemas"];

/// Keywords whose value is a list of subschemas.
const SCHEMA_LISTS: [&str; 4] = ["allOf", "anyOf", "oneOf", "prefixItems"];

/// Keywords whose value is one subschema.
const SCHEMA_ONE: [&str; 10] = [
    "items",
    "not",
    "if",
    "then",
    "else",
    "additionalProperties",
    "unevaluatedProperties",
    "unevaluatedItems",
    "contains",
    "propertyNames",
];

/// Builds the validation schema of a checked bundle with the catalog's effective id and version.
pub(super) fn build(
    checked: Checked<'_>,
    id: &str,
    version: &str,
    instructions: Option<&str>,
    common: &Map<String, Value>,
) -> Value {
    let bundle = checked.bundle();
    let mut merged = BTreeSet::new();
    let mut components = Map::new();
    for (name, schema) in bundle.components() {
        let flat = flattened(name, schema, bundle.defs(), common);
        merged.extend(flat.merged);
        let mut flat = Value::Object(flat.schema);
        describe(&mut flat, common);
        components.insert(name.clone(), flat);
    }
    let mut functions = Map::new();
    for (name, schema) in bundle.functions() {
        let mut schema = schema.clone();
        close_args(&mut schema);
        describe(&mut schema, common);
        functions.insert(name.clone(), schema);
    }
    let mut defs: Map<String, Value> = bundle
        .defs()
        .iter()
        .filter(|(name, _)| !common.contains_key(*name))
        .map(|(name, definition)| {
            let mut definition = definition.clone();
            describe(&mut definition, common);
            (name.clone(), definition)
        })
        .collect();
    prune(&mut defs, &merged, &components, &functions);
    let mut reached = Map::new();
    let roots: Vec<&Value> = components
        .values()
        .chain(functions.values())
        .chain(defs.values())
        .collect();
    add_common_types(&mut reached, &roots, common);
    defs.extend(reached);
    let mut out = Map::from_iter([
        ("$schema".to_owned(), Value::from(DRAFT_2020_12)),
        ("catalogId".to_owned(), Value::from(id)),
        ("protocolVersion".to_owned(), Value::from(version)),
    ]);
    if let Some(text) = instructions.filter(|text| !text.is_empty()) {
        out.insert("instructions".to_owned(), Value::from(text));
    }
    out.insert("components".to_owned(), Value::Object(components));
    if !functions.is_empty() {
        out.insert("functions".to_owned(), Value::Object(functions));
    }
    out.insert("$defs".to_owned(), Value::Object(defs));
    Value::Object(out)
}

/// Gives each whole-entry `$ref` to a common type that type's description, unless it has one.
///
/// It walks schema positions only, never `const`, `enum`, `default` or `examples` data; the
/// draft-7 forms (`dependencies`, an array `items`) are not schema positions here.
fn describe(node: &mut Value, common: &Map<String, Value>) {
    let Value::Object(map) = node else {
        return;
    };
    for (key, value) in map.iter_mut() {
        let key = key.as_str();
        if SCHEMA_MAPS.contains(&key) {
            if let Value::Object(entries) = value {
                entries.values_mut().for_each(|schema| describe(schema, common));
            }
        } else if SCHEMA_LISTS.contains(&key) {
            if let Value::Array(items) = value {
                items.iter_mut().for_each(|schema| describe(schema, common));
            }
        } else if SCHEMA_ONE.contains(&key) {
            describe(value, common);
        }
    }
    if map.contains_key("description") {
        return;
    }
    let description = map
        .get("$ref")
        .and_then(Value::as_str)
        .and_then(whole_def_name)
        .and_then(|name| common.get(&name)?.get("description"))
        .filter(|description| description.is_string())
        .cloned();
    if let Some(description) = description {
        map.insert("description".to_owned(), description);
    }
}

/// Closes a function's inline object `properties.args`, as upstream does.
///
/// `additionalProperties` moves to `unevaluatedProperties`, which defaults to `false`. Upstream
/// closes only `properties.args`, not the `args`/`parameters` spellings catalogs may also use.
fn close_args(schema: &mut Value) {
    let Some(properties) = schema.get_mut("properties").and_then(Value::as_object_mut) else {
        return;
    };
    if !properties.contains_key("@call") && !properties.contains_key("call") {
        return;
    }
    let Some(Value::Object(args)) = properties.get_mut("args") else {
        return;
    };
    if args.get("type").and_then(Value::as_str) != Some("object") || args.contains_key("$ref") {
        return;
    }
    let open = args.remove("additionalProperties");
    if !args.contains_key("unevaluatedProperties") {
        args.insert(
            "unevaluatedProperties".to_owned(),
            open.unwrap_or(Value::Bool(false)),
        );
    }
}

/// Drops each merged mixin of `defs` that nothing kept still names.
///
/// Components, functions and every definition that is not a merged mixin are kept, so a
/// dropped mixin leaves no dangling `$ref`. Names are read from every `$ref` string, data
/// included, as `add_common_types` does: it may keep more, never less.
fn prune(
    defs: &mut Map<String, Value>,
    merged: &BTreeSet<String>,
    components: &Map<String, Value>,
    functions: &Map<String, Value>,
) {
    if merged.is_empty() {
        return;
    }
    let mut pending = Vec::new();
    for root in components.values().chain(functions.values()) {
        def_names(root, &mut pending);
    }
    for (name, definition) in defs.iter() {
        if !merged.contains(name) {
            def_names(definition, &mut pending);
        }
    }
    let mut reached = BTreeSet::new();
    while let Some(name) = pending.pop() {
        if reached.insert(name.clone())
            && let Some(definition) = defs.get(&name)
        {
            def_names(definition, &mut pending);
        }
    }
    defs.retain(|name, _| !merged.contains(name) || reached.contains(name));
}

#[cfg(test)]
#[path = "validation_schema_tests.rs"]
mod tests;
```

(`add_common_types` into an empty `reached` map, then `defs.extend(reached)`, is equivalent to
adding into `defs` because own `$defs` never shadow a common type, and saves cloning `defs`.)

In `crates/a2ui-runtime/src/catalog/load.rs`, the imports become
`use super::compile::{bundle, compile_entries};` and `use super::validation_schema;`, and the
end of `from_json` from `check_references(document)?;` on reads:

```rust
        check_references(document)?;
        let instructions = text(document, "instructions")?;
        let defs = section(document, "$defs")?;
        check_defs(defs)?;
        let (components, functions) = kept_entries(document, defs)?;
        let bundle = bundle(&components, &functions, defs)?;
        let ((components, functions), checked) =
            compile_entries(&bundle, components, functions)?;
        let schema =
            validation_schema::build(checked, id, version, instructions, common_type_defs()?);
        Ok(Self {
            id: id.to_owned(),
            protocol_version: version.to_owned(),
            document: document.clone(),
            components,
            functions,
            validation_schema: schema,
        })
```

In `crates/a2ui-runtime/src/catalog/mod.rs`:
- field `bundle: Value` becomes `validation_schema: Value`;
- the module doc's "bundles the self-contained [`Catalog::validation_schema`] and compiles one
  validator per entry" becomes "compiles one validator per entry and builds
  [`Catalog::validation_schema`]";
- the accessor becomes:

```rust
    /// The schema agents and tools validate against, in upstream's flattened v1.0 shape.
    ///
    /// Components are flattened (mixins merged, `id`/`component` envelope, no `catalogId`),
    /// function `args` closed, common-type descriptions copied onto their `$ref`s, and `$defs`
    /// holds the catalog's definitions plus the common types they reach. It can accept
    /// instances the per-entry validators reject (flattening replaces where `allOf` combines);
    /// the runtime's verdict is the validators'. It may hold definitions no load cap budgeted:
    /// compile it only under your own limits. The catalog theme is not written yet (t-1579),
    /// and a `$ref` into a flattened entry's interior can dangle (t-1580).
    pub fn validation_schema(&self) -> &Value {
        &self.validation_schema
    }
```

- `Catalog::instructions()` doc gains: "A loaded catalog's `instructions` is always a string."

In `crates/a2ui-runtime/src/catalog/flatten.rs`, delete both `cfg_attr(... expect(dead_code ...))`
lines.

In `crates/a2ui-runtime/ARCHITECTURE.md`, replace lines 29-39 (the whole `catalog` bullet) with
exactly:

```markdown
- `catalog`: `Catalog::from_json` checks a v1.0 catalog (depth, nodes, id, version, string
  `instructions`, `$ref` rules, UAX #31 names, callers, metadata, entry caps), keeps the entries
  its unions list and the authored document (`to_json`), compiles each entry against a bundle
  pruned to what it reaches (the validator lives in its `ComponentDef`/`FunctionDef`), then
  builds `validation_schema` in upstream's flattened shape, output only. `validate_component`
  checks an instance: depth, reserved `@` keys, undeclared envelope keys removed, its validator,
  then each nested call in its catalog (via `CatalogLookup`): exists, not `agentOnly`, matches
  the function's validator; `@index` as `IndexSystemFunction`; unknown catalogs only for
  identifiers. `derive`: a new-id catalog from a `Derivation` (source's version, theme, `$defs`;
  duplicate names rejected, changed unions regenerated). Submodules `load`, `entries`,
  `compile`, `document`, `shape`, `validate`, `derive`, `flatten`, `validation_schema`.
```

The file is then 50 lines (`wc -l`), every line within 100 columns.

In `docs/specs/2026-11-10-a2ui-rs-leptos-design.md`, the delivery item's "C2b the
`validationSchema` flattening matching upstream's goldens;" becomes "C2b the `validationSchema`
flattening matching upstream's goldens (theme left out, t-1579; a non-string `instructions` is
refused at load);" — rewrap that bullet so every line stays within 100 columns.

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo nextest run -p a2ui-runtime -p a2ui-catalog-basic`
Expected: all pass, including Plan 2's `validation_schema_every_ref_resolves_locally`,
`validation_schema_missing_unions_are_generated` and
`validation_schema_missing_function_union_matches_nothing` in `compile_tests.rs`. Then
`cargo nextest run -p a2ui-conformance --test core from_json`: the v1.0 `selfContained` cases
(basic, MCP and VegaChart catalogs) still pass on the flattened output.

- [ ] **Step 5: Gate and stage**

Run: `cargo fmt --all && just check && just test` (all green). Budget-rule check:
`grep -n "validator\|jsonschema\|compile(" crates/a2ui-runtime/src/catalog/flatten.rs crates/a2ui-runtime/src/catalog/validation_schema.rs`
shows only doc comments and the `DRAFT_2020_12` import. Then

```bash
git add crates/a2ui-runtime/src/catalog/validation_schema.rs crates/a2ui-runtime/src/catalog/validation_schema_tests.rs crates/a2ui-runtime/src/catalog/compile.rs crates/a2ui-runtime/src/catalog/compile_tests.rs crates/a2ui-runtime/src/catalog/test_support.rs crates/a2ui-runtime/src/catalog/load.rs crates/a2ui-runtime/src/catalog/load_tests.rs crates/a2ui-runtime/src/catalog/mod.rs crates/a2ui-runtime/src/catalog/flatten.rs crates/a2ui-runtime/src/bundle.rs crates/a2ui-catalog-basic/src/lib_tests.rs crates/a2ui-runtime/ARCHITECTURE.md docs/specs/2026-11-10-a2ui-rs-leptos-design.md
```

Stop for the review. After it, commit with subject
`feat(runtime): build Catalog::validation_schema in upstream's flattened shape (t-1577)`, a body
stating "from_json now refuses a non-string `instructions` (MalformedCatalog)", and the
`Reviewed-by:` trailer; then
`oku cli task done --project slider --id t-1577 --result "Catalog::validation_schema is built from the checked bundle in upstream's flattened v1.0 shape"`.

---

### Task 3: Run the `validation_schema` cases and close Plan 2b

**Oku:** t-1578 "Task 3: Run the validation_schema cases and close Plan 2b".

**Files:**
- Create: `crates/a2ui-conformance/src/handlers/validation_schema.rs`
- Modify: `crates/a2ui-conformance/src/paths.rs`, `crates/a2ui-conformance/src/handlers/mod.rs`,
  `crates/a2ui-conformance/src/handlers/schema_doc.rs`, `crates/a2ui-conformance/src/handlers/to_json.rs`,
  `crates/a2ui-conformance/skips.toml`, `crates/a2ui-conformance/ARCHITECTURE.md`, `README.md`

**Interfaces:**
- Consumes: `Catalog::validation_schema() -> &Value` (Task 2); `catalog_input::{document, load}`,
  `compare::schema_eq`, `keys::assert_case_keys`.
- Produces: `pub(crate) fn expect_file(case: &Case) -> Result<(String, Value), String>` in
  `paths`; `pub fn run(case: &Case) -> Result<(), String>` registered as action `validation_schema`.

- [ ] **Step 1: Make the cases run (red)**

Delete the line
`validation_schema = "Plan 2b: validationSchema flattening matching upstream goldens (t-1564)"`
from `[actions]` in `crates/a2ui-conformance/skips.toml`.

Run: `cargo nextest run -p a2ui-conformance --test core validation_schema`
Expected: `test_v10_basic_validation_schema` and `test_v10_custom_validation_schema_leaf_defs`
FAIL with `Unknown action 'validation_schema' in core/catalog.yaml#<case>`; the v0.8/v0.9 cases
are skipped by `[versions]`; `skips_toml_entries_all_match_a_case` passes.

- [ ] **Step 2: Implement the handler and share the `expectFile` reading**

In `crates/a2ui-conformance/src/paths.rs` add (with `use serde_json::Value;` and
`use crate::case::Case;`):

```rust
/// The `expectFile` a case names, and that file's JSON.
pub(crate) fn expect_file(case: &Case) -> Result<(String, Value), String> {
    let path = case
        .get("expectFile")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{}: `expectFile` must be a string", case.at()))?;
    Ok((path.to_owned(), read_json(&upstream_root()?.join(path))?))
}
```

In `handlers/schema_doc.rs` (`compare`) and `handlers/to_json.rs` (`run`), replace their
`expectFile` lookup and `read_json(&upstream_root()?.join(path))?` with
`let (path, expected) = expect_file(case)?;`, and drop the imports they no longer use.

`crates/a2ui-conformance/src/handlers/validation_schema.rs`:

```rust
//! The `validation_schema` action: a loaded catalog's validation schema equals the golden.
//!
//! Compared with `schema_eq`: key order and the order of `enum` and `required` are ignored.

use crate::case::Case;
use crate::compare::schema_eq;
use crate::handlers::catalog_input::{document, load};
use crate::keys::assert_case_keys;
use crate::paths::expect_file;

/// Loads the case's catalog and compares `validation_schema()` with `expectFile`.
pub fn run(case: &Case) -> Result<(), String> {
    assert_case_keys(
        case,
        &[
            "catalog",
            "catalogPath",
            "catalogId",
            "protocolVersion",
            "expectFile",
        ],
    )?;
    let at = case.at();
    let (path, expected) = expect_file(case)?;
    let input = document(case)?;
    let catalog = load(case, &input)?.map_err(|err| format!("{at}: {err}"))?;
    if schema_eq(catalog.validation_schema(), &expected) {
        Ok(())
    } else {
        Err(format!("{at}: validation_schema differs from {path}"))
    }
}
```

In `handlers/mod.rs` add `mod validation_schema;` (alphabetical, after `mod to_json;`) and the
match arm `"validation_schema" => Some(validation_schema::run),`.

- [ ] **Step 3: Run the cases (green)**

Run: `cargo nextest run -p a2ui-conformance --test core validation_schema to_json common_types agent_to_renderer`
Expected: both v1.0 `validation_schema` cases pass, and the `to_json` and schema-document cases
still pass through `expect_file`. If a `validation_schema` case fails, the fix goes in
`a2ui-runtime` (never the harness): write a failing runtime unit test reproducing the
difference first.

- [ ] **Step 4: Docs and consolidation**

- `crates/a2ui-conformance/ARCHITECTURE.md`: the `handlers` bullet's list
  "(schema_doc: the two schema-equality actions; common_type; round_trip; from_json; to_json;
  derive; catalog_checks: …)" gains "validation_schema (golden comparison with `schema_eq`);"
  after `derive;`, and the `paths` bullet gains "`expect_file`"; rewrap within 100 columns and
  stay at most 50 lines.
- `README.md` status: "agent-to-renderer schema) pass, except the `validationSchema` golden
  comparison (Plan 2b);\nlater stages are listed in crates/a2ui-conformance/skips.toml." becomes
  "agent-to-renderer schema) pass; later stages are listed in\ncrates/a2ui-conformance/skips.toml."
- Consolidation: `grep -rn "Plan 2b\|unflattened" crates docs/specs README.md` finds nothing
  stale; `grep -rn "dead_code\|allow(" crates/a2ui-runtime/src` finds no leftover;
  `wc -l crates/*/ARCHITECTURE.md` gives at most 50 each; the new source files stay under 500
  lines.
- Run `just ci` (check + package + test + audit) green, then
  `just conformance-manifest "$PWD/target/manifest-plan2b"` and
  `grep -h 'validation_schema' target/manifest-plan2b/*.jsonl` shows both `test_v10_*` cases
  passed and the three v0.8/v0.9 cases skipped by version.

- [ ] **Step 5: Gate, stage, close**

`cargo fmt --all && just check && just test`, then

```bash
git add crates/a2ui-conformance/src/handlers/validation_schema.rs crates/a2ui-conformance/src/paths.rs crates/a2ui-conformance/src/handlers/mod.rs crates/a2ui-conformance/src/handlers/schema_doc.rs crates/a2ui-conformance/src/handlers/to_json.rs crates/a2ui-conformance/skips.toml crates/a2ui-conformance/ARCHITECTURE.md README.md
```

Stop for the review. After it, commit with
`feat(conformance): run the v1.0 validation_schema cases (t-1578)` plus the `Reviewed-by:`
trailer. Then
`oku cli task done --project slider --id t-1578 --result "validation_schema handler added, both v1.0 cases pass, skip removed"`
and
`oku cli task done --project slider --id t-1564 --result "Catalog::validation_schema matches upstream's v1.0 goldens; validation_schema skip removed"`.
