# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.0](https://github.com/oku-theory/a2ui-rs/compare/a2ui-runtime-v0.1.0...a2ui-runtime-v0.2.0) - 2026-11-14

### Added

- *(runtime)* [**breaking**] refuse a $ref in data that the runtime would rewrite (t-1602, t-1606)
- *(runtime)* let only capped input into composed error wording (t-1590)
- *(runtime)* refuse validation-schema $refs that name no schema (t-1586)
- *(runtime)* escape and cap input quoted in error messages (t-1584, t-1571)
- *(runtime)* name dangling validation-schema $refs where the author wrote them (t-1585)
- *(runtime)* refuse $refs that do not resolve in the validation schema (t-1582)
- *(runtime)* refuse $refs to catalog parts the validation schema drops (t-1581)
- *(runtime)* refuse $refs inside entries, point theme refs at $defs.theme (t-1580)
- *(runtime)* write the catalog theme into the validation schema (t-1579)
- *(conformance)* run the v1.0 validation_schema cases (t-1578)
- *(runtime)* build Catalog::validation_schema in upstream's flattened shape (t-1577)
- *(runtime)* flatten component schemas as upstream's validation schema does (t-1576)
- *(runtime)* derive a catalog from named and new entries (t-1560)
- *(catalog-basic)* embed the v1.0 basic catalog and run the from_json cases (t-1559)
- *(runtime)* validate components and their nested calls against catalogs (t-1558)
- *(runtime)* load, check and serialize v1.0 catalogs (t-1557)
- *(runtime)* validate common types with jsonschema on the regex engine (t-1555)
- *(runtime)* embed the v1.0 common types and agent_to_renderer schemas (t-1554)

### Fixed

- *(runtime)* bound the node walk before queuing and pin final-review gaps (t-1544)

### Other

- Merge pull request #2 from oku-theory/ci-hardening
- *(runtime)* pin that a target absent from root fails the pruned entry's build (t-1631)
- build the packaged crates and publish what was built (t-1629)
- *(runtime)* pin compile_entry's pruned document (t-1624)
- check trybuild cases and doctests on the MSRV (t-1620)
- *(runtime)* find a $ref's entry and target in one descent (t-1618)
- *(runtime)* borrow reached entry names through Expansion and Reach (t-1617)
- *(runtime)* borrow RefPointer tokens instead of allocating each (t-1614)
- *(runtime)* parse $ref pointers into decoded tokens once (t-1609)
- *(runtime)* move the function envelopes next to their callers (t-1607, t-1608)
- *(runtime)* one $ref filter for find_ref and find_ref_mut (t-1611)
- *(runtime)* pin that Pointer::parse errors name the whole path (t-1610)
- *(runtime)* give quoted its own ARCHITECTURE.md bullet and say $ref walks rewrite data (t-1605)
- *(runtime)* pin what reason! refuses and the worst-case Quoted size (t-1597, t-1598, t-1599, t-1600, t-1601)
- *(runtime)* one member descent and one $ref rewrite for build and authored (t-1592, t-1603, t-1596)
- *(runtime)* one RFC 6901 token codec, and refuse undefined ~ escapes in $refs (t-1595)
- *(runtime)* split bundle.rs and validation_schema_tests.rs by concept (t-1604, t-1594)
- *(runtime)* rule on $refs inside data and drop wording from NotASchema (t-1591)
- *(runtime)* decode the bad $ref's path once and verify its authored origin (t-1589)
- *(runtime)* one member walk under rule 4 and the $ref walk (t-1588)
- *(runtime)* pin catalog load at the depth limits on a 1 MiB stack (t-1587)
- *(runtime)* one $ref walk for the bundle passes (t-1583)
- *(runtime)* correct two Plan 2b doc comments (t-1564)
- *(conformance)* close stage C2, leaving validationSchema goldens to Plan 2b (t-1561)

## [0.1.0](https://github.com/oku-theory/a2ui-rs/releases/tag/v0.1.0) - 2026-11-11

### Added

- *(runtime)* parse expression templates (t-1551)
- *(runtime)* resolve scoped relative data paths (t-1550)
- *(runtime)* notify path observers whose value a write changed (t-1549)
- *(runtime)* add the pointer-addressed data model (t-1548)
- *(runtime)* add normalized JSON pointers and the error type (t-1547)

### Other

- publish crates to crates.io with release-plz (t-1544)
- record the Plan 1 conformance status (t-1552)
- bootstrap the a2ui-rs workspace and vendor upstream v1.0 suites (t-1545)
