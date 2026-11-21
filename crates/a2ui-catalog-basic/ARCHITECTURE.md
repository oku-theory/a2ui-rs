# a2ui-catalog-basic

The A2UI v1.0 basic catalog: 18 components and 14 functions, as published upstream.

- `catalog.json`: the published document, copied by `vendor/sync.sh` with upstream's
  `LICENSE-A2UI` (and `NOTICE-A2UI` when upstream has a `NOTICE`); never edit it by hand.
  The crate is licensed Apache-2.0 only, as that document is.
- `src/lib.rs`: `CATALOG_ID`, `CATALOG_JSON` and `catalog()`, which loads it with
  `Catalog::parse` once, keeps it in a `OnceLock`, and returns the shared `&'static Catalog`;
  only success is kept, so a load error is owned and fresh on each call.

- `src/options.rs`: `BasicOptions` (`Locale`, `UrlOpener`; default `EnUs`, `NoOpener`).
- `src/locale.rs`: `Locale` (plural category, separators, currency, default decimals, AM/PM),
  `PluralCategory`, `CurrencyFormat`, `EnUs` (CLDR English).
- `src/functions/`: `functions(registry, options)` binds each implementation on `catalog()`;
  an absent argument reads as `null`. `validators`: `required`, `length` (code points, array
  items), `numeric`, `email`, `regex` (`DialectRegex`, its errors returned), each
  `{"valid": bool}`. `logic`: `and`/`or` (at least 2 `values`), `not`, via `coerce::boolean`.
  `text`: `formatString` (`CallContext::interpolate`), `openUrl` (`http`, `https`, `mailto`,
  `tel` only, then the opener; returns `null`). Plan 4a Task 6 adds the formatting functions.

Depends on: `a2ui-runtime`, `serde_json`.
