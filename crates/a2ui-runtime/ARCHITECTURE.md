# a2ui-runtime

Renderer-side A2UI v1.0 runtime: state and evaluation; no rendering, I/O or async; wasm32-ready.

## Modules
- `error`: opaque `Error` (Snafu `Inner`, backtrace per variant; `process`: processor failures,
  boxed in one variant), `category()`, `code()`, boxed context (`detail`: `path`, ids,
  `message_index`, `Detail` list, each stamped with the surface id); stable English.
- `quoted`: `Quoted` input, escaped, capped; `reason!` joins fixed text and `Quoted` only.
- `pointer`: the one RFC 6901 token codec (`escape_token`, `unescape_token`); `Pointer`, absolute,
  empty segments dropped, `__proto__`/`constructor`/`prototype` refused; list indices are
  canonical digits up to `MAX_LIST_INDEX` (10 000).
- `data`: `DataModel`, one JSON document per surface; stored `null` reads as absent and `set` of
  `null` deletes. `set` creates missing containers, validating the whole path first; each change
  returns a `Write` (path, previous value) whose `changes(watched)` compares values;
  `set_undoable`/`undo` restore the first changed slot exactly (created containers, list length).
- `subscribe`: `Watchers<K>`; `notified(write, model)` lists the keys of observers whose pointer
  value changed (exact path, ancestors, descendants). `context`: `resolve_path(path, scope)`.
- `expression`: `parse_template` turns `${…}` templates into `Expr` parts (literal, path, call
  with named arguments); `\${` is literal; limits 10 000 characters, 1 000 parts, nesting 100.
- `schema`: the embedded v1.0 `common_types.json` and `agent_to_renderer.json` (copied into
  `schemas/v1_0/` by `vendor/sync.sh`, Apache-2.0 `LICENSE` beside them), returned as is;
  `validate_common_type` (cached; `FunctionCall` takes any function but `@index`).
- `keys`: `CALL_KEY`, `PATH_KEY`. `ident`: UAX #31. `validator`: `jsonschema` 2020-12, offline;
  build failures are `CatalogError`.
- `catalog`: `Catalog` (checked v1.0 catalog, per-entry validators, `validate_component`,
  `derive`) and `CatalogSet` (by id, duplicates refused); see `src/catalog/ARCHITECTURE.md`.
- `bundle`, `walk`, `ref_pointer`, `target`, `position`, `check`: `$ref` bundling, walking and
  the v1.0 `$ref` rules (self, `catalog.json`, `common_types.json`; no anchor or inner `$id`).
- `limits`: caps on agent JSON (64 levels, 128 KiB per pattern, nodes, calls, 256-byte keys,
  4 KiB `$ref`s), node weight (a node per 64 bytes of text), and `expand`, which follows `$ref`s
  and rejects cycles, depth past 128 and oversized or pattern-heavy entries before compile.
- `message`: `AgentMessage::parse`/`from_json` check `MessageLimits` (1 MiB; text by `len`, a value
  by an escape-free estimate), then `envelope` (object, one action, version "1.0"), `values`
  (`check_depth`: components, `dataModel`, `value`, call `args`, 50 containers), `schema` (the
  action's branch, components opaque; first violation one `Detail`; `callRendererFunction` faults
  `INVALID_FUNCTION_CALL`), `typed` (by hand), then `check_bindings` (`@call` chains up to 5,
  `@path` parses). `batch`: array or `{"messages": […]}`. `RendererMessage::error`: an `Error`.
- `surface`: `Surface` (id, catalog id, `sendDataModel`, metadata, `DataModel`); components later.
- `processor`: `MessageProcessor` applies messages; a refused one changes nothing (an oversized
  write is undone). `StateLimits`: surfaces, components, data model nodes. `handle_batch` stops at
  the first error and stamps its `message_index`. `Outcome` is empty until t-1688/t-1689.

## Dependencies
`snafu`, `serde_json`, `unicode-ident`, `percent-encoding`, `jsonschema` (default features off;
`wasm-bindgen` via `getrandom` on wasm32, checked by `just deps`).
