//! Fuzzes `Catalog::validate_component` against the basic catalog with arbitrary JSON.

#![no_main]

use a2ui_catalog_basic::{CATALOG_ID, catalog};
use libfuzzer_sys::fuzz_target;
use serde_json::Value;

fuzz_target!(|data: &[u8]| {
    let Ok(component) = serde_json::from_slice::<Value>(data) else {
        return;
    };
    #[expect(
        clippy::expect_used,
        reason = "the embedded catalog always loads (its tests guard it): a failure is a finding"
    )]
    let basic = catalog().expect("the basic catalog loads");
    // Ok or Err are both fine; only a panic, a timeout or an out-of-memory is a finding.
    let _ = basic.validate_component(&component, CATALOG_ID, basic);
});
