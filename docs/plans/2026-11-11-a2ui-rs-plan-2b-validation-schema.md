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
eagerly in every `from_json`, `derive` and `a2ui_catalog_basic::catalog()` call: a lazy build
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
    the rewrite changes a `const` or `enum` value holding such an object; data that must keep
    its meaning writes such a `$ref` in the local form the rewrite leaves alone (`#/$defs/X`).
    Telling data from schema in every walk needs the compiler's keyword table, a second copy
    that could drift and let a `$ref` dangle unseen. The runtime never reads `default` or
    `examples`, and none of the 995 `$ref`s under `vendor/a2ui` sits in data. What a bad
    `$ref` names is `NotASchema`: `Missing`, or one variant per JSON type that is not a schema.
    It carries no wording; `validation_schema` owns the message.
16. **RFC 6901 escapes only (t-1595):** `crate::pointer` holds the crate's one reference-token
    codec, `escape_token` and `unescape_token` (`None` when a `~` is not followed by `0` or `1`,
    RFC 6901 §3-4); `Pointer`, `bundle`'s `$ref` paths, `entry_of` and `whole_def_name` use it,
    and `bundle`'s lenient unescape is gone. Load rule 4 now refuses a `$ref` whose decoded
    pointer has such an escape in any token: `limits::resolve` pairs the strict `entry_of` with
    `serde_json`'s lenient `Value::pointer`, which must decode the whole pointer alike so the
    entry charged holds the target. Upstream has no such rule; RFC 6901 calls those pointers
    invalid. Percent-decoding comes first, so `%7E2` is refused too. None of the 995 `$ref`s
    under `vendor/a2ui` holds a `~`, and a test pins that every common-type `$ref` is escaped.
17. **One descent, one rewrite order (t-1592, t-1593, t-1596, t-1603):** `bundle::walk`
    generates its shared and `mut` member descents from one macro body, so `find_ref` and
    `find_ref_mut` (behind `rewrite_refs`) visit the same `$ref`s, in the same order, at the
    same paths; a table test pins the `$ref` filter the two add on top. The descent still hands
    only scalar members to its visitor (t-1593): every load rule today reads a scalar keyword,
    so widening it now would only make each visitor skip containers. A rule on a keyword
    holding an object or array (`$vocabulary`) widens that one body then.
    `validation_schema::written_ref` (localize, then point `#/theme` at `#/$defs/theme`) is the
    one rewrite `build` applies to every `$defs` and theme `$ref`, and the one `authored`
    rebuilds with, so a new rewrite cannot reach one only. Collecting pointers with `find_ref`
    and writing through `Value::pointer_mut` was set aside: its `None` could only be a bug,
    needing a silent fallback or an `expect`. `written_ref` is idempotent (`FRAGMENT` encodes
    `%`), so re-localizing bundle `$defs` changes nothing.

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
- Run `just ci` (check + test + audit) green, then
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
