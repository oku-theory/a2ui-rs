//! Fuzzes `Catalog::parse` with arbitrary text: it must return, never panic or hang.

#![no_main]

use a2ui_runtime::Catalog;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(text) = std::str::from_utf8(data) {
        // Ok or Err are both fine; only a panic, a timeout or an out-of-memory is a finding.
        let _ = Catalog::parse(text, None, None);
    }
});
