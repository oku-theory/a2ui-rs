# a2ui-runtime

Renderer-side A2UI v1.0 runtime: protocol state and evaluation, no rendering, no I/O, no async.
It builds for `wasm32-unknown-unknown`.

## Modules

- `error`: one opaque `Error` (Snafu, backtrace per variant) with `category()`, the upstream
  conformance category (`DataError`, …). Messages are English and stable; callers localize.
- `pointer`: `Pointer`, an absolute RFC 6901 pointer with empty segments dropped, `~0`/`~1`
  unescaped and `__proto__`/`constructor`/`prototype` refused; list indices are canonical digits
  up to `MAX_LIST_INDEX` (10 000).
- `data`: `DataModel`, one JSON document per surface. A stored `null` reads as absent; `set`
  creates missing containers (a list when the next segment is index-like), validates the whole
  path before mutating, and treats `null` as delete. Each change returns a `Write` (path plus the
  previous value), whose `changes(watched)` tells whether an observer's value differs.
- `subscribe`: `Watchers<K>`, observers keyed by `K` on pointers; `notified(write, model)` lists
  the keys whose value the write changed (exact path, ancestors, descendants; unchanged values
  stay silent). Renderers map keys to their own reactive cells.
- `context`: `resolve_path(path, scope)`; `""`/`"."` is the scope, `/…` is absolute, anything
  else is appended to the scope.
- `expression`: `parse_template` turns `${…}` templates into `Expr` parts (literal, path, call
  with named arguments); `\${` is literal text. Limits: 10 000 characters, 1 000 parts, nesting
  100 (interpolations and call arguments together). Grammar notes: Plan 1, Task 7.
- `schema`: the embedded v1.0 `common_types.json` and `agent_to_renderer.json` (copied into
  `schemas/v1_0/` by `vendor/sync.sh`, Apache-2.0 `LICENSE` beside them), returned as is;
  `validate_common_type` (cached validators; `FunctionCall` accepts any function but `@index`).
- `ident`: UAX #31 `is_identifier` (`_` allowed first), shared by the parser and catalogs.
- `validator`: `jsonschema` 2020-12, `regex` engine, offline; build errors are `CatalogError`,
  the first violation a `ValidationError`.
- `catalog`: `Catalog::from_json` checks a v1.0 catalog (depth, nodes, id, version, `$ref`
  rules, UAX #31 names, callers, metadata, entry caps), keeps the entries its unions list and
  the authored document (`to_json`), bundles `validation_schema` (unflattened until Plan 2b)
  and compiles each entry against a document pruned to what it reaches; the validator lives
  in its `ComponentDef`/`FunctionDef`. Submodules `load`, `entries`, `compile`, `document`,
  `shape`.
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
