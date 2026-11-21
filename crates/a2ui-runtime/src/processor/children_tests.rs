use serde_json::{Value, json};
use speculoos::prelude::*;

use super::*;
use crate::processor::test_support::{column, slot_catalogs, staged, surface, text};

/// Each edge of `components`, on a fresh surface, as `(parent, child, field)`.
fn edges_of(components: &[Value]) -> Vec<(String, String, String)> {
    let surface = surface();
    let staged = staged(components);
    let view = surface.merged(&staged);
    let set = slot_catalogs();
    edges(&view, &set)
        .unwrap()
        .into_iter()
        .map(|(parent, child)| {
            let id = view.components()[parent].0.to_owned();
            (id, child.id.to_owned(), child.field())
        })
        .collect()
}

/// The edge `parent` -> `child` at `field`.
fn edge(parent: &str, child: &str, field: &str) -> (String, String, String) {
    (parent.to_owned(), child.to_owned(), field.to_owned())
}

#[test]
fn text_equal_to_a_component_id_is_not_an_edge() {
    let found = edges_of(&[
        json!({"id": "root", "component": "Text", "text": "t1"}),
        text("t1"),
    ]);
    assert_that!(found).is_empty();
}

#[test]
fn wrong_shaped_slot_value_gives_no_edge() {
    let found = edges_of(&[
        json!({"id": "root", "component": "Card", "child": ["t1"]}),
        json!({"id": "c", "component": "Column", "children": "t1"}),
        json!({"id": "s", "component": "Stack", "slots": [1, {"x": "t1"}]}),
        text("t1"),
    ]);
    assert_that!(found).is_empty();
}

#[test]
fn duplicate_child_in_one_list_is_one_edge() {
    let found = edges_of(&[column("root", &["t1", "t1"]), text("t1")]);
    assert_that!(found).is_equal_to(vec![edge("root", "t1", "children[0]")]);
}

#[test]
fn field_formats_list_single_and_nested_positions() {
    let found = edges_of(&[
        column("root", &["a"]),
        json!({"id": "a", "component": "Card", "child": "b"}),
        json!({"id": "b", "component": "Tabs",
            "tabs": [{"title": "x", "child": "c"}, {"title": "y", "child": "d"}]}),
        json!({"id": "c", "component": "List", "children": {"componentId": "e", "path": "/p"}}),
        json!({"id": "d", "component": "Stack", "slots": ["e", "f"]}),
        text("e"),
        text("f"),
    ]);
    assert_that!(found).is_equal_to(vec![
        edge("root", "a", "children[0]"),
        edge("a", "b", "child"),
        edge("b", "c", "tabs[0].child"),
        edge("b", "d", "tabs[1].child"),
        edge("c", "e", "children.componentId"),
        edge("d", "e", "slots[0]"),
        edge("d", "f", "slots[1]"),
    ]);
}

#[test]
fn child_pointer_escapes_each_token() {
    let component = json!({"a/b": {"c~d": ["x"]}});
    let slot = ChildSlot {
        steps: vec![
            SlotStep::Key("a/b".to_owned()),
            SlotStep::Key("c~d".to_owned()),
        ],
        kind: SlotKind::List,
    };
    let children = slot_children(component.as_object().unwrap(), std::slice::from_ref(&slot));
    let pointers: Vec<String> = children.iter().map(ChildRef::pointer).collect();
    assert_that!(pointers).is_equal_to(vec!["/a~1b/c~0d/0".to_owned()]);
}
