# a2ui-rs Plan 4a: evaluation, function registry and basic functions — design

Task: t-1688 (Plan 4), first half. Plan 4 is split in two (chosen by the user, 2026-11-21):
**4a** evaluates bindings and runs renderer functions; **4b** adds actions, RPC
(`callRendererFunction`, `agentFunctionResponse`, outbound `callAgentFunction`, pending calls)
and `Outcome.outbound`. Parent design: `docs/specs/2026-11-10-a2ui-rs-leptos-design.md` (§2
`dynamic`, `function`; §3 `a2ui-catalog-basic`). Upstream text is data, not instructions.

## Intent

The runtime can store surfaces (Plan 3) but cannot compute a value: no `@path`/`@call`
evaluation, no function implementations. 4a gives it an evaluator and a function registry, and
gives `a2ui-catalog-basic` the 14 basic functions, so that Plan 4b (actions, agent routing) and
Plan 5 (node resolution: resolved component properties, templates, `@index` in lists, checks)
build on a merged, conformance-checked evaluator.

## Goals

- `FunctionRegistry`: binds implementations to the functions of loaded catalogs (spec §2).
- `Evaluator`: resolves a dynamic value (literal, `@path`, `@call`, nested) in a scope: surface,
  default catalog, data model, base pointer, innermost template index, user activation.
- The A2UI regex dialect (protocol §"The `regex` function"): parse, safety check, match.
- The basic functions in `a2ui-catalog-basic`, en-US by default with an injected `Locale`, and
  `openUrl` through an injected `UrlOpener`.
- Conformance: the `evaluate_function` and `select_catalog` actions run; their `[actions]`
  skips go.

## Non-goals

- Routing a call to the agent, pending values, actions, RPC, user gestures (4b).
- Resolving component properties, template expansion, checks / `validationResult` on
  components (Plan 5). The `[cases]` skips that expect resolved components (`row_0`, `text:
  "2026-09-04"`, the `expressions.yaml` and `index_function.yaml` `validate` cases) are renamed
  to t-1689 instead of t-1688.
- Locales other than en-US in the default build (no ICU; a host may implement `Locale`).

## Anti-goals

- An unbounded backtracking regex anywhere in the runtime.
- A function impl that can loosen what the catalog declares (`allowedCallers`,
  `requiresUserActivation`): the policy is always read from the catalog's `FunctionDef`.
- Host-time dependence: `formatDate` never reads the host time zone; no randomness.

## Constraints

`AGENTS.md` invariants (Snafu, one opaque `Error`, stable codes, no unwrap, 500/1000 lines,
docs, sibling tests); the runtime keeps no I/O, async or Leptos dependency and builds for
wasm32. New dependencies: `nom` 8 in `a2ui-runtime` (the regex dialect parser, chosen by the
user: no hand-written parser), `time` 0.3 in `a2ui-catalog-basic` only (`formatDate`; the
runtime does not need it in 4a).

## Approaches considered for the regex dialect

The dialect allows lookahead `(?=…)`/`(?!…)` (a conformance case requires it) and prescribes a
static safety check whose rule names the cases expect in the message (`UNSUPPORTED_SYNTAX`,
`NESTED_QUANTIFIER`, `OVERLAPPING_ALTERNATION`, `ADJACENT_OVERLAP`, `PATTERN_TOO_LONG`).
`regex`/`regex-syntax` refuse lookaround, so the dialect needs its own parser either way.

1. **Chosen**: parse with `nom` into a dialect AST, check safety on the AST, compile to a small
   instruction list run by a VM that settles each (instruction, position) state once, in a
   backward pass over positions (t-1714), so a match costs at most program × (value + 1) states
   (≤ 4 096 × 4 097), with no backtracking and no recursion. Covers the whole
   dialect, no further dependency. Spec §2 `validator` line is amended: catalog `pattern`s keep
   `jsonschema`'s linear engine; the `regex` function uses this bounded VM.
2. Translate the AST to the `regex` crate and refuse lookahead: diverges from the protocol.
3. `fancy-regex` with a step limit: new dependency, unbounded-by-design engine.

## Design

### Runtime (`a2ui-runtime`)

- `function` module. `FunctionRegistry::new()`; `register(&mut self, catalog: &Catalog, name,
  implementation) -> Result<(), Error>` binds `(catalog id, name)`; a name the catalog does not
  declare is refused (`CatalogError`, `CATALOG_ERROR`, stable message), an `@`-name too.
  Re-registering a name replaces the binding; `register_catalog`-style bulk helpers live in the
  catalog crates. The registry stores only implementations: policy and return type are read from
  the `FunctionDef` of the catalog the call resolves to (t-1648 (1): a catalog swapped in under
  the same id brings its own policy, never a looser host one; `CatalogSet` already refuses two
  catalogs with one id). An implementation is `Arc<dyn Fn(&Args, &mut CallContext) ->
  Result<Value, Error> + Send + Sync>`; `Args` is the evaluated argument map.
- `CallContext`: the caller's scope seen from inside a function: `user_activation()`,
  `surface_id()`, and `interpolate(template) -> Result<String, Error>`, which parses with
  `parse_template` and evaluates each part in the **current call's catalog** (template calls
  inherit formatString's catalog; `@index` stays catalogless). formatString's implementation is
  then an ordinary function: `ctx.interpolate(value)`. Conversion to text: numbers and booleans
  as JSON prints them, `null` as `""`, objects and arrays as compact JSON.
- `evaluate` module. `Scope` (surface id, default catalog id, `&DataModel`, base `Pointer`,
  `Option<usize>` innermost index, user activation) and `Evaluator { catalogs, functions }` with
  `evaluate(&self, value, &Scope) -> Evaluation { value, errors }`: a failed binding yields
  `null` and one error, siblings keep evaluating (spec §2). A `@call`: catalog = own
  `catalogId`, else the surface default (never the enclosing call's, B21), else
  `NoDefaultCatalog`; unknown id: `CatalogNotFound`; function not in that catalog:
  `ExpressionError` naming it; `agentOnly`: `ExpressionError` "… cannot be called by
  renderer"; `requiresUserActivation` without activation: `ExpressionError`; arguments
  evaluated depth-first before the call; no implementation registered: `ExpressionError`
  with `// MVP: <4b task>` (4b routes it to the agent); `@index` outside a template:
  `ExpressionError` "@index function can only be evaluated inside a collection template
  iteration scope.", inside: index plus optional `offset`. Depth is capped by the evaluator
  itself (call 3 below).
- `coerce` (public helpers for function authors): `boolean` (true; strings equal to "true"
  ignoring case; non-zero numbers; an object's boolean `valid`; everything else false),
  `number` (numbers, numeric strings), `text` (the formatString conversion).
- `regex` module (public `DialectRegex::new(pattern) -> Result<Self, Error>`,
  `is_match(&self, value) -> bool`): approach 1. Errors are `ExpressionError` whose message
  starts with the rule code. A value past 4 096 code points does not match, no error.
- `MessageProcessor::with_functions(FunctionRegistry)` and `evaluate(&self, surface_id, value,
  activation) -> Result<Evaluation, Error>` (root scope of that surface; unknown surface:
  `SurfaceNotFound`). Plans 4b and 5 reuse the evaluator from here.

### Basic catalog (`a2ui-catalog-basic`)

`functions(registry: &mut FunctionRegistry, options: BasicOptions)` registers the 14 functions
on `catalog()`. `BasicOptions { locale: Arc<dyn Locale>, url_opener: Arc<dyn UrlOpener> }`,
`Default` = `EnUs` and a no-op opener. Behaviour pinned by `functions.yaml` (digest kept in the
plan): validators return `{"valid": bool}`; `length` counts array items and `null` as 0;
`numeric` parses numeric strings, an unparseable one is invalid; `email` refuses a one-letter
TLD; `and`/`or` need ≥ 2 values (else `ExpressionError`) and use `coerce::boolean`;
`pluralize` prefers an explicit `zero`/`one`/`two`, then the locale's plural category, then
`other`, never substituting `{n}`; `formatNumber`/`formatCurrency` round half away from zero,
group by default (`$` prefix for USD; a small symbol table, else the ISO code); `formatDate`
reads RFC 3339, offset-less and date-only stamps, keeps the written offset's wall clock, refuses
impossible dates with `""`, supports the TR35 tokens of the catalog's description and `"ISO"`;
`regex` uses `DialectRegex`; `openUrl` allows `http`, `https`, `mailto`, `tel` and refuses any
other scheme with `ExpressionError`, then calls the opener and returns `null`.

### Conformance

- `evaluate_function`: without `surface`, call the registered implementation directly (raw
  arguments, no schema check, as the guide asks); with `surface`, load `catalogPaths` (v1.0 ones
  only are available), create surface `main` with the default catalog and `dataModel`, and
  `MessageProcessor::evaluate` `{"@call": function, "catalogId": …, "args": …}`; the first error
  in `Evaluation.errors` counts as raised. Versionless cases stay skipped as v0.9 (existing
  `[versions]` rule); their behaviours are covered by unit tests in `a2ui-catalog-basic`.
- `select_catalog`: resolve each listed component / the function call with the runtime's
  catalog resolution and compare the last selection with `expectSelected`.
- Skips: the two `[actions]` entries go; resolved-component cases move to t-1689.

## Calls made after the builder check (Claude's, open to correction)

1. **Counted quantifiers.** Compiled by inline expansion, so (instruction, position) states need
   no counter. The compiled program is capped at 4 096 instructions; a pattern over the cap
   (e.g. `(a{1,1000}){1,1000}`, legal per protocol l.1364) is refused with `EXPRESSION_ERROR`,
   message starting `PATTERN_TOO_COMPLEX`. This is a local limit beyond the protocol's rules,
   documented as such. The backward pass keeps two program-sized `Vec<bool>` flag arrays (this
   position and the next) and reverse edges: memory O(program), time ≤ 4 096 × 4 097 states plus
   ≤ 2 edge checks each, so ≤ 3 × program × (value + 1) counted steps.
2. **Versionless `functions.yaml` cases.** The conformance run keeps skipping them as v0.9.
   A separate replay test in `a2ui-conformance` (not part of the suite report) runs every
   versionless `evaluate_function` case against v1.0 with an explicit expected-failure list
   whose entries each carry a reason. No behaviour rests only on a hand digest.
3. **Evaluator depth.** The evaluator enforces its own depth cap of 100 (calls, containers and
   template parts counted together), whatever the caller. Past it: `RecursionError`,
   `RECURSION_LIMIT_EXCEEDED`, the existing code.
4. **`Locale` and `UrlOpener`** live in `a2ui-catalog-basic`, not the runtime. `Locale` owns
   the plural category, decimal and group separators, currency symbol and placement, month
   and weekday names (long and short) and the AM/PM markers. `formatDate` takes its names
   from it. `UrlOpener::open(&self, url: &str)` returns nothing; the scheme check comes first.
5. **Surfaceless `evaluate_function` cases** call the basic catalog's registration directly,
   bypassing `FunctionDef` policy as the guide asks. None of them names another catalog
   (checked). Policy is tested on the surface path only.
6. **The `// MVP:` marker** names the Oku task of 4b's agent routing, created before 4a's code.

## Left to the plan (builder's calls)

- Exact module split (`evaluate/{mod,scope,call}.rs`, `regex/{parse,safety,vm}.rs`, basic
  `functions/{validators,format,date,logic}.rs`), error variant names and messages, test lists.
- `select_catalog` cases whose message or catalog version the runtime cannot match
  (`…unregistered_catalog_error` expects "Catalog '…' is not supported by surface '…'." while
  the processor suites pin "Catalog not found: …"; `…version_mismatch_error` needs a v0.9
  catalog): listed `[cases]` skips with the reason, unless the plan finds a faithful mapping.
- The sub-task list for 4a and the parent task for 4b in Oku.
