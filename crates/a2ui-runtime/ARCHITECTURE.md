# a2ui-runtime

Renderer-side A2UI v1.0 runtime: state and evaluation; no rendering, I/O or async; wasm32-ready.

## Modules
- `error`: opaque `Error` (Snafu `Inner`, backtrace per variant), `category()`, `code()`, boxed
  context (`detail`: `path`, ids, `Detail` list); stable English.
- `quoted`: `Quoted` input, escaped, capped; `reason!` joins fixed text and `Quoted` only.
- `pointer`: the one RFC 6901 token codec (`escape_token`, `unescape_token`); `Pointer`, absolute,
  empty segments dropped, `__proto__`/`constructor`/`prototype` refused; list indices are
  canonical digits up to `MAX_LIST_INDEX` (10 000).
- `data`: `DataModel`, one JSON document per surface; stored `null` reads as absent and `set` of
  `null` deletes. `set` creates missing containers, validating the whole path first; each change
  returns a `Write` (path, previous value) whose `changes(watched)` compares values.
- `subscribe`: `Watchers<K>`; `notified(write, model)` lists the keys of observers whose pointer
  value changed (exact path, ancestors, descendants). `context`: `resolve_path(path, scope)`.
- `expression`: `parse_template` turns `${…}` templates into `Expr` parts (literal, path, call
  with named arguments); `\${` is literal; limits 10 000 characters, 1 000 parts, nesting 100.
- `schema`: the embedded v1.0 `common_types.json` and `agent_to_renderer.json` (copied into
  `schemas/v1_0/` by `vendor/sync.sh`, Apache-2.0 `LICENSE` beside them), returned as is;
  `validate_common_type` (cached; `FunctionCall` takes any function but `@index`).
- `keys`: `CALL_KEY`, `PATH_KEY`. `ident`: UAX #31. `validator`: `jsonschema` 2020-12, offline;
  build failures are `CatalogError`.
- `catalog`: `Catalog::from_json` checks a v1.0 catalog (depth, nodes, id, version, string
  `instructions`, `$ref` rules, UAX #31 names, callers, metadata, entry caps), keeps the entries its
  unions list and the authored document (`to_json`), compiles each entry against a bundle pruned to
  what it reaches (the validator lives in its `ComponentDef`/`FunctionDef`), then builds
  `validation_schema`, output only, in upstream's flattened shape. `validate_component` checks an
  instance: depth, nodes, calls, reserved `@` keys, envelope keys removed, its validator, then each
  nested call in its catalog (via `CatalogLookup`): exists, not `agentOnly`, matches the function's
  validator; `@index` as `IndexSystemFunction`; unknown catalogs only for identifiers. `derive`: a
  new-id catalog from a `Derivation` (duplicate names rejected, changed unions regenerated).
- `bundle`: `local_ref`s, common types, `written_ref`; `walk`: `$ref` walks, data included;
  `ref_pointer`: a `$ref`'s decoded tokens, `locate`; `target`; `position`: schema vs data keywords;
  `check`: v1.0 `$ref` rules (self, `catalog.json`, `common_types.json` naming common types,
  RFC 6901 escapes; no anchor, dynamic ref, inner `$id`, interior).
- `limits`: caps on agent JSON (64 levels, 128 KiB per pattern, catalog and component nodes,
  component calls, 256-byte keys, 4 KiB `$ref`s), and `expand`, which follows `$ref`s (each site
  once), charges each entry whole, weighs strings and keys (a node per 64 bytes), and rejects
  cycles, depth past 128 and oversized or pattern-heavy entries before compile.
- `message`: `AgentMessage::parse`/`from_json` check `MessageLimits` (1 MiB; text by `len`, a value
  by an escape-free estimate), then `envelope` (object, one action, version "1.0"), `values`
  (`check_depth`: components, `dataModel`, `value`, call `args`, 50 containers), `schema` (the
  action's branch, components opaque; first violation one `Detail`; `callRendererFunction` faults
  `INVALID_FUNCTION_CALL`), `typed` (by hand), then `check_bindings` (`@call` chains up to 5,
  `@path` parses). `batch`: array or `{"messages": […]}`. `RendererMessage::error`: an `Error`.

## Dependencies
`snafu`, `serde_json`, `unicode-ident`, `percent-encoding`, `jsonschema` (default features off;
`wasm-bindgen` via `getrandom` on wasm32, checked by `just deps`).
