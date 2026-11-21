//! A catalog with child slots, and surfaces built from it, for the graph unit tests.

use serde_json::{Map, Value, json};

use crate::catalog::CatalogSet;
use crate::catalog::test_support::load;
use crate::message::{AgentMessage, MessageLimits};
use crate::surface::{Staged, Surface};

/// The common type `name` as a `$ref`.
fn common(name: &str) -> Value {
    json!({"$ref": format!("common_types.json#/$defs/{name}")})
}

/// Catalog `t`: `Text`, `Column` and `List` (a `ChildList`), `Card` (a `Child`), `Stack` (an
/// array of `ComponentId`), `Tabs` (a `Child` in each tab) and `Header` (only under a `Card`).
pub(crate) fn slot_catalogs() -> CatalogSet {
    let tabs = json!({"type": "array", "items": {"type": "object",
        "properties": {"title": {"type": "string"}, "child": common("Child")}}});
    let catalog = load(json!({
        "catalogId": "t",
        "protocolVersion": "1.0",
        "components": {
            "Text": {"type": "object", "properties": {"text": {"type": "string"}}},
            "Column": {"type": "object", "properties": {"children": common("ChildList")}},
            "List": {"type": "object", "properties": {"children": common("ChildList")}},
            "Card": {"type": "object", "properties": {"child": common("Child")}},
            "Stack": {"type": "object", "properties": {
                "slots": {"type": "array", "items": common("ComponentId")}}},
            "Tabs": {"type": "object", "properties": {"tabs": tabs}},
            "Header": {"type": "object", "allowedParents": ["Card"]}
        }
    }))
    .unwrap();
    let mut set = CatalogSet::new();
    set.insert(catalog).unwrap();
    set
}

/// An empty surface `s1` on catalog `t`.
pub(crate) fn surface() -> Surface {
    let value = json!({"version": "1.0", "createSurface": {"surfaceId": "s1", "catalogId": "t"}});
    match AgentMessage::from_json(&value, &MessageLimits::default()).unwrap() {
        AgentMessage::CreateSurface(create) => Surface::new(&create),
        other => panic!("not a createSurface: {other:?}"),
    }
}

/// `components` staged as one message would stage them.
pub(crate) fn staged(components: &[Value]) -> Staged {
    let components = components
        .iter()
        .map(|component| {
            let map: Map<String, Value> = component.as_object().unwrap().clone();
            (component["id"].as_str().unwrap().to_owned(), map)
        })
        .collect();
    Staged { components }
}

/// A `Column` `id` listing `children`.
pub(crate) fn column(id: &str, children: &[&str]) -> Value {
    json!({"id": id, "component": "Column", "children": children})
}

/// A `Text` `id`.
pub(crate) fn text(id: &str) -> Value {
    json!({"id": id, "component": "Text", "text": id})
}

/// A chain `root` -> `c0` -> ... -> `c<n-1>` of `Card`s ending in a `Text`: `n` edges.
pub(crate) fn chain(n: usize) -> Vec<Value> {
    let name = |index: usize| match index {
        0 => "root".to_owned(),
        index => format!("c{}", index - 1),
    };
    let mut components: Vec<Value> = (0..n)
        .map(|index| json!({"id": name(index), "component": "Card", "child": name(index + 1)}))
        .collect();
    components.push(text(&name(n)));
    components
}
