//! Fuzzes `Catalog::parse` and its overrides; a loaded catalog's document reloads unchanged.
//!
//! Input: optionally a `catalog_id` override and a NUL, then optionally a `protocol_version`
//! override and a NUL, then the catalog text (an empty override field is an empty override).

#![no_main]

use a2ui_fuzz::fields;
use a2ui_runtime::Catalog;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Some(fields) = fields(data) else {
        return;
    };
    let (text, overrides) = fields.split_last().unwrap_or((&"", &[]));
    let (catalog_id, version) = (overrides.first().copied(), overrides.get(1).copied());
    // An Err is fine; only a panic, a timeout, an out-of-memory or a broken oracle is a finding.
    let Ok(catalog) = Catalog::parse(text, catalog_id, version) else {
        return;
    };
    // Oracle: the document a catalog returns holds its effective id and version, so it loads
    // again, with no override, to the same catalog.
    let document = catalog.to_json();
    let again = match Catalog::from_json(&document, None, None) {
        Ok(again) => again,
        Err(error) => panic!("to_json of a loaded catalog does not reload: {error}"),
    };
    assert_eq!(again.id(), catalog.id());
    assert_eq!(again.protocol_version(), catalog.protocol_version());
    assert_eq!(again.to_json(), document);
});
