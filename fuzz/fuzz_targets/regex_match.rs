//! Fuzzes `DialectRegex` with a pattern, then a value: it must return, never panic or hang.

#![no_main]

use a2ui_fuzz::fields;
use a2ui_runtime::DialectRegex;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Some(parts) = fields(data) else {
        return;
    };
    // A refused pattern is fine; an accepted one must match in bounded time, the same twice.
    let Ok(regex) = DialectRegex::new(parts[0]) else {
        return;
    };
    let value = parts.get(1).copied().unwrap_or_default();
    assert_eq!(
        regex.is_match(value),
        regex.is_match(value),
        "matching is deterministic"
    );
});
