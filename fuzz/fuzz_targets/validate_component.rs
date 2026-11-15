//! Fuzzes `Catalog::validate_component` against the basic catalog: no panic, no hang.
//!
//! Input: a component instance as JSON, then optionally a NUL and the surface's default catalog
//! id (the basic catalog's when absent), which may name no loaded catalog. Determinism across
//! two loads of a catalog is `parse_then_validate`'s oracle: the basic catalog is loaded once.

#![no_main]

use a2ui_catalog_basic::CATALOG_ID;
use a2ui_fuzz::{basic, fields};
use libfuzzer_sys::fuzz_target;
use serde_json::Value;

fuzz_target!(|data: &[u8]| {
    let Some(fields) = fields(data) else {
        return;
    };
    let Ok(component) = serde_json::from_str::<Value>(fields[0]) else {
        return;
    };
    let default_catalog = fields.get(1).copied().unwrap_or(CATALOG_ID);
    // Ok or Err are both fine; only a panic, a timeout or an out-of-memory is a finding.
    let _ = basic().validate_component(&component, default_catalog, basic());
});
