# a2ui-runtime

Renderer-side A2UI v1.0 runtime: protocol state and evaluation, no rendering, no I/O, no async.
It builds for `wasm32-unknown-unknown`.

## Modules

- `error`: one opaque `Error` (Snafu, backtrace per variant) with `category()`, the upstream
  conformance category (`DataError`, …). Messages are English and stable; callers localize.
- `pointer`: `Pointer`, an absolute RFC 6901 pointer with empty segments dropped, `~0`/`~1`
  unescaped and `__proto__`/`constructor`/`prototype` refused; list indices are canonical digits
  up to `MAX_LIST_INDEX` (10 000).
- `data`: `DataModel`, one JSON document per surface; stored `null` reads as absent and `set`
  of `null` deletes. `set` creates missing containers and validates the whole path first; each
  change returns a `Write` (path, previous value) whose `changes(watched)` compares values.
- `subscribe`: `Watchers<K>`, observers keyed by `K` on pointers; `notified(write, model)` lists
  the keys whose value changed (exact path, ancestors, descendants).
- `context`: `resolve_path(path, scope)`; `""`/`"."` is the scope, `/…` is absolute, anything
  else is appended to the scope.
- `expression`: `parse_template` turns `${…}` templates into `Expr` parts (literal, path, call
  with named arguments); `\${` is literal text. Limits: 10 000 characters, 1 000 parts, nesting
  100 (interpolations and call arguments together). Grammar notes: Plan 1, Task 7.
- `schema`: the embedded v1.0 `common_types.json` and `agent_to_renderer.json` (copied into
  `schemas/v1_0/` by `vendor/sync.sh`, Apache-2.0 `LICENSE` beside them), returned as is;
  `validate_common_type` (cached validators; `FunctionCall` takes any function but `@index`).
- `ident`: UAX #31 `is_identifier` (`_` allowed first), shared by parser and catalogs.
- `validator`: `jsonschema` 2020-12, `regex` engine, offline; build errors `CatalogError`,
  the first violation a `ValidationError`.
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
- `bundle`: v1.0 `$ref` rules (self, `catalog.json`, `common_types.json`; no anchors, dynamic
  refs or nested `$id`), percent-encoded `local_ref`/`localize_refs`, common-type closure.
- `limits`: caps on agent JSON (64 levels, 128 KiB per pattern, catalog nodes, 256-byte keys,
  4 KiB `$ref`s), and `expand`, which follows `$ref`s (each site resolved once), charges each
  reached entry whole, weighs strings and keys one more node per 64 bytes (so the caps bound
  retained string bytes), and rejects cycles, depth past 128 and oversized or pattern-heavy
  entries before any compile.

## Dependencies
`snafu`, `serde_json`, `unicode-ident`, `percent-encoding`, `jsonschema` (default features off;
brings `wasm-bindgen` through `getrandom` on wasm32, checked by `just deps`).
