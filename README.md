# a2ui-rs

Rust implementation of the renderer side of [A2UI](https://github.com/a2ui-project/a2ui) v1.0,
checked against the upstream conformance suites, with a Leptos renderer.

| Crate | Role |
|---|---|
| `a2ui-runtime` | Protocol state: data model, expressions, surfaces, catalogs (pure Rust) |
| `a2ui-conformance` | Runs the vendored upstream suites (not published) |

`vendor/a2ui` is a copy of upstream files (Apache-2.0) at the commit in `vendor/UPSTREAM`;
refresh it with `just vendor-sync <commit>`.

Licensed under MIT or Apache-2.0, at your option.
