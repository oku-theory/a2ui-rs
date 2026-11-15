//! Fuzzes `parse_template` with arbitrary text: it must return, never panic or hang.

#![no_main]

use a2ui_runtime::parse_template;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(text) = std::str::from_utf8(data) {
        // Ok or Err are both fine. A pure parse over text has no state a second run could
        // reveal, and no printer to round-trip through yet: a panic or a hang is the finding.
        let _ = parse_template(text);
    }
});
