use serde_json::{Value, json};
use speculoos::prelude::*;

use super::*;
use crate::catalog::test_support::load;
use crate::message::{AgentMessage, MessageLimits};
use crate::processor::test_support::{staged, surface};
use crate::processor::{MessageProcessor, ValidationRules};
use crate::surface::Surface;
use crate::{Category, DetailCode, ErrorCode};

/// The common type `name` as a `$ref`.
fn common(name: &str) -> Value {
    json!({"$ref": format!("common_types.json#/$defs/{name}")})
}

/// Catalog `t` with composition constraints: open containers (`Box`, `Card`, `Column`, `Row`),
/// `RestrictedBox` (children `Text` or `Button`, in a list, a single and a nested slot), leaves
/// restricted to some parents (`ColumnHeader`, `Header`, `Surfaced`) and free leaves.
fn catalogs() -> CatalogSet {
    let list = || json!({"type": "object", "properties": {"children": common("ChildList")}});
    let tabs = json!({"type": "array", "items": {"type": "object",
        "properties": {"child": common("Child")}}});
    let catalog = load(json!({
        "catalogId": "t",
        "protocolVersion": "1.0",
        "components": {
            "Box": list(),
            "Card": list(),
            "Column": list(),
            "Row": list(),
            "RestrictedBox": {"type": "object", "allowedChildren": ["Text", "Button"],
                "properties": {"children": common("ChildList"), "child": common("Child"),
                    "tabs": tabs}},
            "ColumnHeader": {"type": "object", "allowedParents": ["Column"]},
            "Header": {"type": "object", "allowedParents": ["Box"]},
            "Surfaced": {"type": "object", "allowedParents": ["Surface", "Column"]},
            "Text": {"type": "object"},
            "Button": {"type": "object"},
            "Video": {"type": "object"}
        }
    }))
    .unwrap();
    let mut set = CatalogSet::new();
    set.insert(catalog).unwrap();
    set
}

/// A component `id` of type `kind` with `extra` entries.
fn of(id: &str, kind: &str, extra: Value) -> Value {
    let mut component = json!({"id": id, "component": kind});
    for (key, value) in extra.as_object().unwrap() {
        component[key] = value.clone();
    }
    component
}

/// A `kind` container `id` listing `children`.
fn holder(id: &str, kind: &str, children: &[&str]) -> Value {
    of(id, kind, json!({"children": children}))
}

/// A leaf `id` of type `kind`.
fn leaf(id: &str, kind: &str) -> Value {
    of(id, kind, json!({}))
}

/// Checks `message` sent to a surface already holding `stored`.
fn run_on(stored: &[Value], message: &[Value]) -> Result<(), Error> {
    let mut surface: Surface = surface();
    surface.commit(staged(stored));
    let staged = staged(message);
    check(&surface.merged(&staged), &staged, &catalogs())
}

/// Checks `message` sent to an empty surface.
fn run(message: &[Value]) -> Result<(), Error> {
    run_on(&[], message)
}

/// Asserts `error` is `code` with `text`, at `path`, with one detail there on `s1`.
fn assert_refused(error: &Error, code: ErrorCode, text: &str, path: &str) {
    let detail = match code {
        ErrorCode::UnallowedParent => DetailCode::UnallowedParent,
        _ => DetailCode::UnallowedChild,
    };
    assert_that!((error.category(), error.code())).is_equal_to((Category::Validation, code));
    assert_that!(error.to_string()).is_equal_to(text.to_owned());
    assert_that!(error.path()).is_equal_to(Some(path));
    assert_that!(error.surface_id()).is_equal_to(Some("s1"));
    let details: Vec<_> = error
        .details()
        .iter()
        .map(|d| {
            (
                d.path().to_owned(),
                d.code(),
                d.surface_id().map(str::to_owned),
            )
        })
        .collect();
    assert_that!(details).is_equal_to(vec![(path.to_owned(), detail, Some("s1".to_owned()))]);
}

const CHILD_FAULT: &str = "Container 'root' (RestrictedBox) cannot contain child 'v1' (Video). \
                           Allowed children: ['Text', 'Button'].";

const CARD_FAULT: &str = "Component 'h' (Header) cannot be placed under parent 'card' (Card). \
                          Allowed parents: ['Box'].";

#[test]
fn composition_root_checked_against_surface() {
    let error = run(&[leaf("root", "ColumnHeader")]).unwrap_err();
    assert_refused(
        &error,
        ErrorCode::UnallowedParent,
        "Component 'root' (ColumnHeader) cannot be placed under parent 'Surface' (Surface). \
         Allowed parents: ['Column'].",
        "/components/0",
    );
}

#[test]
fn composition_unallowed_parent_points_at_the_child() {
    let error = run(&[holder("root", "Row", &["h"]), leaf("h", "ColumnHeader")]).unwrap_err();
    assert_refused(
        &error,
        ErrorCode::UnallowedParent,
        "Component 'h' (ColumnHeader) cannot be placed under parent 'root' (Row). Allowed \
         parents: ['Column'].",
        "/components/1",
    );
}

#[test]
fn composition_unallowed_child_points_at_the_slot() {
    let error = run(&[
        holder("root", "RestrictedBox", &["v1"]),
        leaf("v1", "Video"),
    ])
    .unwrap_err();
    assert_refused(
        &error,
        ErrorCode::UnallowedChild,
        CHILD_FAULT,
        "/components/0/children/0",
    );
}

#[test]
fn composition_unallowed_child_in_a_single_slot_points_at_it() {
    let root = of("root", "RestrictedBox", json!({"child": "v1"}));
    let error = run(&[root, leaf("v1", "Video")]).unwrap_err();
    assert_refused(
        &error,
        ErrorCode::UnallowedChild,
        CHILD_FAULT,
        "/components/0/child",
    );
}

#[test]
fn composition_unallowed_child_in_a_nested_slot_points_at_it() {
    let root = of(
        "root",
        "RestrictedBox",
        json!({"tabs": [{"child": "t1"}, {"child": "v1"}]}),
    );
    let error = run(&[root, leaf("t1", "Text"), leaf("v1", "Video")]).unwrap_err();
    assert_refused(
        &error,
        ErrorCode::UnallowedChild,
        CHILD_FAULT,
        "/components/0/tabs/1/child",
    );
}

#[test]
fn composition_unallowed_child_through_a_template_points_at_its_id() {
    let template = json!({"children": {"componentId": "v1", "path": "/items"}});
    let root = of("root", "RestrictedBox", template);
    let error = run(&[root, leaf("v1", "Video")]).unwrap_err();
    assert_refused(
        &error,
        ErrorCode::UnallowedChild,
        CHILD_FAULT,
        "/components/0/children/componentId",
    );
}

#[test]
fn composition_child_in_message_parent_stored_points_at_the_child() {
    let stored = [holder("root", "RestrictedBox", &["v1"]), leaf("v1", "Text")];
    let error = run_on(&stored, &[leaf("v1", "Video")]).unwrap_err();
    assert_refused(
        &error,
        ErrorCode::UnallowedChild,
        CHILD_FAULT,
        "/components/0",
    );
}

#[test]
fn composition_parent_in_message_child_stored_points_at_the_parent_slot() {
    let stored = [holder("root", "Box", &["h"]), leaf("h", "Header")];
    let message = [leaf("t1", "Text"), holder("root", "Card", &["t1", "h"])];
    let error = run_on(&stored, &message).unwrap_err();
    assert_refused(
        &error,
        ErrorCode::UnallowedParent,
        "Component 'h' (Header) cannot be placed under parent 'root' (Card). Allowed parents: \
         ['Box'].",
        "/components/1/children/1",
    );
}

#[test]
fn composition_parent_violation_reported_before_child_violation() {
    let error = run(&[holder("root", "RestrictedBox", &["h"]), leaf("h", "Header")]).unwrap_err();
    assert_refused(
        &error,
        ErrorCode::UnallowedParent,
        "Component 'h' (Header) cannot be placed under parent 'root' (RestrictedBox). Allowed \
         parents: ['Box'].",
        "/components/1",
    );
}

#[test]
fn composition_first_violation_in_arrival_order_when_first_reports_card() {
    let error = run(&[
        holder("root", "Box", &["card", "b"]),
        holder("card", "Card", &["h"]),
        holder("b", "Box", &["h"]),
        leaf("h", "Header"),
    ])
    .unwrap_err();
    assert_refused(
        &error,
        ErrorCode::UnallowedParent,
        CARD_FAULT,
        "/components/3",
    );
}

#[test]
fn composition_first_violation_in_arrival_order_when_second_reports_card() {
    let error = run(&[
        holder("root", "Box", &["b", "card"]),
        holder("b", "Box", &["h"]),
        holder("card", "Card", &["h"]),
        leaf("h", "Header"),
    ])
    .unwrap_err();
    assert_refused(
        &error,
        ErrorCode::UnallowedParent,
        CARD_FAULT,
        "/components/3",
    );
}

#[test]
fn composition_first_violation_follows_the_child_arrival_order() {
    let error = run(&[
        holder("root", "Box", &["x", "y"]),
        leaf("late", "Header"),
        holder("x", "Card", &["early"]),
        holder("y", "Card", &["late"]),
        leaf("early", "Header"),
    ])
    .unwrap_err();
    assert_refused(
        &error,
        ErrorCode::UnallowedParent,
        "Component 'late' (Header) cannot be placed under parent 'y' (Card). Allowed parents: \
         ['Box'].",
        "/components/1",
    );
}

#[test]
fn composition_unset_constraints_accept_anything() {
    let result = run(&[
        holder("root", "Box", &["c"]),
        holder("c", "Card", &["v"]),
        leaf("v", "Video"),
    ]);
    assert_that!(result).is_ok();
}

#[test]
fn composition_surface_in_allowed_parents_accepts_root() {
    assert_that!(run(&[leaf("root", "Surfaced")])).is_ok();
}

#[test]
fn composition_unreachable_component_is_not_checked() {
    let result = run(&[
        holder("root", "Box", &[]),
        holder("orphan", "Card", &["h"]),
        leaf("h", "Header"),
    ]);
    assert_that!(result).is_ok();
}

#[test]
fn composition_edge_without_an_end_in_the_message_is_not_rechecked() {
    let stored = [
        holder("root", "Card", &["h", "t1"]),
        leaf("h", "Header"),
        leaf("t1", "Text"),
    ];
    assert_that!(run_on(&stored, &[leaf("t1", "Button")])).is_ok();
}

#[test]
fn composition_unknown_types_are_skipped_when_permissive() {
    let result = run(&[
        holder("root", "RestrictedBox", &["m"]),
        leaf("m", "Mystery"),
    ]);
    assert_that!(result).is_ok();
}

#[test]
fn composition_dangling_reference_is_not_checked() {
    let result = run(&[holder("root", "RestrictedBox", &["missing"])]);
    assert_that!(result).is_ok();
}

/// `value` as a message.
fn message(value: Value) -> AgentMessage {
    AgentMessage::from_json(&value, &MessageLimits::default()).unwrap()
}

#[test]
fn composition_reattached_orphan_edge_is_refused_and_changes_nothing() {
    let rules = ValidationRules::strict().allow_orphan_components(true);
    let mut processor = MessageProcessor::new(catalogs()).with_rules(rules);
    let stored = vec![
        holder("root", "Box", &[]),
        holder("o", "Card", &["h"]),
        leaf("h", "Header"),
    ];
    processor
        .handle(&message(json!({"version": "1.0", "createSurface": {
            "surfaceId": "s1", "catalogId": "t", "components": stored}})))
        .unwrap();
    let before: Vec<_> = processor
        .surface("s1")
        .unwrap()
        .components()
        .cloned()
        .collect();

    let error = processor
        .handle(&message(json!({"version": "1.0", "updateComponents": {
            "surfaceId": "s1", "components": [holder("root", "Box", &["o"])]}})))
        .unwrap_err();

    assert_that!(error.code()).is_equal_to(ErrorCode::UnallowedParent);
    assert_that!(error.to_string()).is_equal_to(
        "Component 'h' (Header) cannot be placed under parent 'o' (Card). Allowed parents: \
         ['Box']."
            .to_owned(),
    );
    assert_that!(error.path()).is_none();
    assert_that!(error.details().len()).is_equal_to(0);
    assert_that!(error.surface_id()).is_equal_to(Some("s1"));
    let after: Vec<_> = processor
        .surface("s1")
        .unwrap()
        .components()
        .cloned()
        .collect();
    assert_that!(after).is_equal_to(before);
}
