//! Harness running the upstream A2UI conformance suites against `a2ui-runtime`.
//!
//! [`Case`] is one YAML test case, [`Skips`] the skip list, [`handlers::handler`]
//! maps an action to its runner, and [`Manifest`] writes upstream's executed-case records.

mod case;
mod compare;
pub mod handlers;
mod keys;
mod manifest;
mod paths;
mod skips;

pub use case::{Case, load_suite, parse_suite};
pub use compare::json_eq;
pub use keys::{assert_case_keys, assert_known_keys};
pub use manifest::{Manifest, Status};
pub use paths::upstream_root;
pub use skips::Skips;
