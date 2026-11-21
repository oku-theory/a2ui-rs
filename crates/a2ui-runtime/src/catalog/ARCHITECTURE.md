# catalog

v1.0 catalogs: the components and functions an agent may use on a surface.

- `load`: `Catalog::from_json` checks a catalog (depth, nodes, id, version, string
  `instructions`, `$ref` rules, UAX #31 names, callers, metadata, entry caps). `entries` keeps the
  entries its unions list; `document` keeps the authored document (`to_json`).
- `compile`: each entry against a bundle pruned to what it reaches; the validator lives in its
  `ComponentDef`/`FunctionDef`. A component's document is closed by a root
  `unevaluatedProperties: false` beside its `$ref`. `shape`: `ComponentShape`.
- `slots`: each component's `ChildSlot`s (Decision 70), paths to `ComponentId`/`Child` (one id)
  or `ChildList` (ids or a template) through `properties`, `items`, `prefixItems`, `allOf`/`anyOf`/
  `oneOf` and `$ref`s; iterative, memoized, within the expansion caps. A slot under another
  keyword or in a recursive `$def` is a `CatalogError`; the envelope `id` is never a slot.
- `validation_schema`, `flatten`: the output-only schema in upstream's flattened shape.
- `validate`: `validate_located` checks an instance: depth, nodes, calls, reserved `@` keys,
  envelope keys removed, its validator, then each nested call in its own catalog else the
  surface default (via `CatalogLookup`): exists, not `agentOnly`, matches the function's
  validator; `@index` as `IndexSystemFunction`. `Located` sets the report (first or all, up to
  16 details), unknown catalogs (identifiers only or `CatalogNotFound`) and unknown elements
  (under all, an unknown component also gets a `/component` `INVALID_VALUE` detail), and
  undeclared properties (the closing violation, counted only once the schema passes); error
  paths are JSON Pointers into the component. `validate_component`: first, identifiers, strict.
- `derive`: a new-id catalog from a `Derivation` (duplicate names rejected, changed unions
  regenerated).
- `set`: `CatalogSet`, catalogs by id in id order; a duplicate id is a `CatalogError`; it is the
  processor's `CatalogLookup`.
