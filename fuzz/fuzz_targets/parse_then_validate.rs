//! Fuzzes an agent's catalog, then a component checked against it and the basic one.
//!
//! Input: a component instance as JSON, a NUL, optionally the surface's default catalog id
//! (which may name no loaded catalog) and a NUL, then the catalog text. The catalog is loaded
//! twice; the oracle is that both loads give the same document and the same outcomes, so no
//! state built at load time (hash maps, compiled validators) leaks into a result. The component
//! is validated in the agent's catalog, finding the basic one through a `Catalog` lookup, and
//! in the basic one, finding the agent's through a `[Catalog]` lookup.

#![no_main]

use a2ui_fuzz::{basic, fields, outcome};
use a2ui_runtime::Catalog;
use libfuzzer_sys::fuzz_target;
use serde_json::Value;

fuzz_target!(|data: &[u8]| {
    let Some(fields) = fields(data) else {
        return;
    };
    let (Some(component), Some(text)) = (fields.first(), fields.last()) else {
        return;
    };
    let default_id = (fields.len() > 2).then(|| fields[1]);
    // The component first: a malformed one skips the costly catalog load.
    let Ok(component) = serde_json::from_str::<Value>(component) else {
        return;
    };
    let Ok(agent) = Catalog::parse(text, None, None) else {
        return;
    };
    let Ok(twin) = Catalog::parse(text, None, None) else {
        panic!("a catalog that loaded once does not load again");
    };
    assert_eq!(agent.to_json(), twin.to_json(), "two loads differ");
    let in_agent = |catalog: &Catalog| {
        outcome(catalog.validate_component(&component, default_id.unwrap_or(catalog.id()), basic()))
    };
    assert_eq!(
        in_agent(&agent),
        in_agent(&twin),
        "validation differs between two loads"
    );
    let in_basic = |catalog: &Catalog| {
        let default = default_id.unwrap_or(basic().id());
        outcome(basic().validate_component(&component, default, std::slice::from_ref(catalog)))
    };
    assert_eq!(
        in_basic(&agent),
        in_basic(&twin),
        "lookup differs between two loads"
    );
});
