# a2ui-runtime

Renderer-side A2UI v1.0 runtime: protocol state and evaluation, no rendering, no I/O, no async.
It builds for `wasm32-unknown-unknown`.

## Modules

- `error`: one opaque `Error` (Snafu, backtrace per variant) with `category()`, the upstream
  conformance category (`DataError`, …). Messages are English and stable; callers localize.
- `pointer`: `Pointer`, an absolute RFC 6901 pointer with empty segments dropped, `~0`/`~1`
  unescaped and `__proto__`/`constructor`/`prototype` refused; list indices are canonical digits
  up to `MAX_LIST_INDEX` (10 000).

## Dependencies

`snafu`.
