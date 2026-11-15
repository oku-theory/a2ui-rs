//! Fuzzes `Pointer::parse`: no forbidden segment passes, and written pointers parse back.

#![no_main]

use a2ui_runtime::Pointer;
use libfuzzer_sys::fuzz_target;

/// Segments `Pointer::parse` refuses, as prototype-pollution guards.
const FORBIDDEN: [&str; 3] = ["__proto__", "constructor", "prototype"];

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    let Ok(pointer) = Pointer::parse(text) else {
        return;
    };
    // Oracle: every pointer that parses holds no forbidden and no empty segment.
    for segment in pointer.segments() {
        assert!(
            !FORBIDDEN.contains(&segment.as_str()),
            "{text:?} kept {segment:?}"
        );
        assert!(!segment.is_empty(), "{text:?} kept an empty segment");
    }
    // Oracle: `Display` writes the normalized pointer, which parses to the same segments.
    let written = pointer.to_string();
    match Pointer::parse(&written) {
        Ok(again) => assert_eq!(again, pointer, "{text:?} wrote {written:?}"),
        Err(error) => panic!("{text:?} wrote {written:?}, which does not parse: {error}"),
    }
});
