# catalog

v1.0 catalogs: the components and functions an agent may use on a surface.

- `load`: `Catalog::from_json` checks a catalog (depth, nodes, id, version, string
  `instructions`, `$ref` rules, UAX #31 names, callers, metadata, entry caps). `entries` keeps the
  entries its unions list; `document` keeps the authored document (`to_json`).
- `compile`: each entry against a bundle pruned to what it reaches; the validator lives in its
  `ComponentDef`/`FunctionDef`. `shape`: `ComponentShape`.
- `validation_schema`, `flatten`: the output-only schema in upstream's flattened shape.
- `validate`: `validate_located` checks an instance: depth, nodes, calls, reserved `@` keys,
  envelope keys removed, its validator, then each nested call in its own catalog else the
  surface default (via `CatalogLookup`): exists, not `agentOnly`, matches the function's
  validator; `@index` as `IndexSystemFunction`. `Located` sets the report (first or all, up to
  16 details), unknown catalogs (identifiers only or `CatalogNotFound`) and unknown elements;
  error paths are JSON Pointers into the component. `validate_component`: first, identifiers.
- `derive`: a new-id catalog from a `Derivation` (duplicate names rejected, changed unions
  regenerated).
- `set`: `CatalogSet`, catalogs by id in id order; a duplicate id is a `CatalogError`; it is the
  processor's `CatalogLookup`.
