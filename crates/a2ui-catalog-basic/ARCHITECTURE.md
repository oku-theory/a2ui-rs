# a2ui-catalog-basic

The A2UI v1.0 basic catalog: 18 components and 14 functions, as published upstream.

- `catalog.json`: the published document, copied by `vendor/sync.sh` with upstream's
  `LICENSE-A2UI` (and `NOTICE-A2UI` when upstream has a `NOTICE`); never edit it by hand.
  The crate is licensed Apache-2.0 only, as that document is.
- `src/lib.rs`: `CATALOG_ID`, `CATALOG_JSON` and `catalog()`, which loads it with
  `Catalog::parse` on every call.

Plan 4 adds the renderer-side implementations of the basic functions here.

Depends on: `a2ui-runtime`.
