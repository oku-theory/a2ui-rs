use serde_json::{Map, Value, json};
use speculoos::prelude::*;

use super::*;
use crate::catalog::test_support::{catalog_error, load, load_on_a_small_stack, v10};
use crate::{Derivation, DeriveEntry};

/// The common type `name` as a `$ref`.
fn common(name: &str) -> Value {
    json!({"$ref": format!("common_types.json#/$defs/{name}")})
}

/// The step naming the property `name`.
fn key(name: &str) -> SlotStep {
    SlotStep::Key(name.to_owned())
}

/// A slot of `kind` at `steps`.
fn slot(steps: Vec<SlotStep>, kind: SlotKind) -> ChildSlot {
    ChildSlot { steps, kind }
}

/// The slots of component `C`, whose schema has `properties`, in a catalog with `defs`.
fn slots_with(properties: Value, defs: Value) -> Vec<ChildSlot> {
    let catalog = load(v10(json!({
        "components": {"C": {"type": "object", "properties": properties}},
        "$defs": defs
    })))
    .unwrap();
    catalog.component("C").unwrap().child_slots().to_vec()
}

/// The slots of component `C`, whose schema has `properties`.
fn slots(properties: Value) -> Vec<ChildSlot> {
    slots_with(properties, json!({}))
}

#[test]
fn column_children_is_a_list_slot() {
    let found = slots(json!({"component": {"const": "Column"}, "children": common("ChildList")}));
    assert_that!(found).is_equal_to(vec![slot(vec![key("children")], SlotKind::List)]);
}

#[test]
fn card_child_is_a_single_slot() {
    let found = slots(json!({"component": {"const": "Card"}, "child": common("Child")}));
    assert_that!(found).is_equal_to(vec![slot(vec![key("child")], SlotKind::Single)]);
}

#[test]
fn list_template_component_id_is_the_only_child() {
    let catalog = load(v10(json!({"components": {
        "List": {"type": "object", "properties": {"children": common("ChildList")}}
    }})))
    .unwrap();
    let component = json!({"id": "l", "component": "List",
        "children": {"componentId": "item", "path": "/items"}});
    let slots = catalog.component("List").unwrap().child_slots();

    let found = crate::processor::children::slot_children(component.as_object().unwrap(), slots);

    let ids: Vec<&str> = found.iter().map(|child| child.id).collect();
    assert_that!(ids).is_equal_to(vec!["item"]);
}

#[test]
fn tabs_child_inside_items_is_a_slot() {
    let found = slots(json!({"tabs": {"type": "array", "items": {
        "type": "object",
        "properties": {"title": {"type": "string"}, "child": common("Child")}
    }}}));
    let steps = vec![key("tabs"), SlotStep::Items, key("child")];
    assert_that!(found).is_equal_to(vec![slot(steps, SlotKind::Single)]);
}

#[test]
fn component_id_array_is_a_list_slot() {
    let found = slots(json!({"slots": {"type": "array", "items": common("ComponentId")}}));
    assert_that!(found).is_equal_to(vec![slot(vec![key("slots")], SlotKind::List)]);
}

#[test]
fn prefix_items_slot_is_at_its_index() {
    let found = slots(json!({"pair": {"type": "array",
        "prefixItems": [{"type": "string"}, common("ComponentId")]}}));
    let steps = vec![key("pair"), SlotStep::Index(1)];
    assert_that!(found).is_equal_to(vec![slot(steps, SlotKind::Single)]);
}

#[test]
fn plain_string_property_is_not_a_slot() {
    let found = slots(json!({"label": {"type": "string"}, "id": {"type": "string"}}));
    assert_that!(found).is_empty();
}

#[test]
fn envelope_id_is_not_a_slot() {
    let found = slots(json!({"id": common("ComponentId")}));
    assert_that!(found).is_empty();
}

#[test]
fn any_of_branch_slot_counts() {
    let found = slots(json!({"child": {"anyOf": [{"type": "number"}, common("Child")]}}));
    assert_that!(found).is_equal_to(vec![slot(vec![key("child")], SlotKind::Single)]);
}

#[test]
fn undefined_local_def_reads_as_common_type() {
    let found = slots(json!({"child": {"$ref": "#/$defs/Child"}}));
    assert_that!(found).is_equal_to(vec![slot(vec![key("child")], SlotKind::Single)]);
}

#[test]
fn shared_def_slots_found_at_each_position() {
    let found = slots_with(
        json!({"a": {"$ref": "#/$defs/Pair"}, "b": {"$ref": "#/$defs/Pair"}}),
        json!({"Pair": {"type": "object",
            "properties": {"left": common("Child"), "right": common("ChildList")}}}),
    );
    assert_that!(found).is_equal_to(vec![
        slot(vec![key("a"), key("left")], SlotKind::Single),
        slot(vec![key("a"), key("right")], SlotKind::List),
        slot(vec![key("b"), key("left")], SlotKind::Single),
        slot(vec![key("b"), key("right")], SlotKind::List),
    ]);
}

#[test]
fn recursive_def_with_a_slot_is_a_catalog_error() {
    let document = v10(json!({
        "components": {"C": {"type": "object", "properties": {"tree": {"$ref": "#/$defs/T"}}}},
        "$defs": {"T": {"type": "object",
            "properties": {"child": common("Child"), "sub": {"$ref": "#/$defs/T"}}}}
    }));
    let expected = "Malformed catalog: Component 'C' has a recursive child slot through 'T'";
    assert_that!(catalog_error(load(document))).is_equal_to(expected.to_owned());
}

#[test]
fn recursive_def_without_a_slot_loads() {
    // Only a $def no kept entry reaches can recur: load rule 11 refuses any reached cycle.
    let document = v10(json!({
        "components": {"C": {"type": "object", "properties": {"child": common("Child")}}},
        "$defs": {"T": {"type": "object", "properties": {"sub": {"$ref": "#/$defs/T"}}}}
    }));
    let found = load(document)
        .unwrap()
        .component("C")
        .unwrap()
        .child_slots()
        .to_vec();
    assert_that!(found).is_equal_to(vec![slot(vec![key("child")], SlotKind::Single)]);
}

#[test]
fn reached_recursive_def_without_a_slot_is_a_ref_cycle() {
    let document = v10(json!({
        "components": {"C": {"type": "object", "properties": {"tree": {"$ref": "#/$defs/T"}}}},
        "$defs": {"T": {"type": "object", "properties": {"sub": {"$ref": "#/$defs/T"}}}}
    }));
    let expected = "Malformed catalog: $ref cycle through '#/$defs/T'";
    assert_that!(catalog_error(load(document))).is_equal_to(expected.to_owned());
}

#[test]
fn slot_under_additional_properties_is_a_catalog_error() {
    let document = v10(json!({"components": {"C": {"type": "object",
        "properties": {"named": {"type": "object", "additionalProperties": common("Child")}}}}}));
    let expected = "Malformed catalog: Component 'C' has a child slot under unsupported keyword \
                    'additionalProperties'";
    assert_that!(catalog_error(load(document))).is_equal_to(expected.to_owned());
}

#[test]
fn slot_in_a_def_under_not_is_a_catalog_error() {
    let document = v10(json!({
        "components": {"C": {"type": "object", "not": {"$ref": "#/$defs/Held"}}},
        "$defs": {"Held": {"type": "object", "properties": {"child": common("Child")}}}
    }));
    let expected = "Malformed catalog: Component 'C' has a child slot under unsupported keyword \
                    'not'";
    assert_that!(catalog_error(load(document))).is_equal_to(expected.to_owned());
}

#[test]
fn chain_of_2000_defs_loads_within_expansion() {
    // 2 000 $defs reached in turn through one `allOf`, the last holding the slot: rule 11
    // bounds $ref depth at 128, so a nested chain this long never loads.
    let mut defs: Map<String, Value> = (0..1999)
        .map(|index| (format!("D{index}"), json!({"type": "object"})))
        .collect();
    defs.insert(
        "D1999".to_owned(),
        json!({"properties": {"child": common("Child")}}),
    );
    let all_of: Vec<Value> = (0..2000)
        .map(|index| json!({"$ref": format!("#/$defs/D{index}")}))
        .collect();
    let document = v10(json!({"components": {"C": {"allOf": all_of}}, "$defs": defs}));

    let found = load(document)
        .unwrap()
        .component("C")
        .unwrap()
        .child_slots()
        .to_vec();

    assert_that!(found).is_equal_to(vec![slot(vec![key("child")], SlotKind::Single)]);
}

#[test]
fn nested_chain_of_2000_defs_is_rule_11_depth_limit() {
    // D0 names D1 under `n`, and so on down to D1999, which holds the slot: the slot walk
    // must neither overflow a small stack nor fail before rule 11 bounds the depth.
    let mut defs: Map<String, Value> = (0..1999)
        .map(|index| {
            let next = format!("#/$defs/D{}", index + 1);
            (
                format!("D{index}"),
                json!({"properties": {"n": {"$ref": next}}}),
            )
        })
        .collect();
    defs.insert(
        "D1999".to_owned(),
        json!({"properties": {"child": common("Child")}}),
    );
    let document = v10(json!({"components": {"C": {"$ref": "#/$defs/D0"}}, "$defs": defs}));
    let expected = "Catalog exceeds 128 levels of 'C' once its $refs are expanded";
    assert_that!(catalog_error(load_on_a_small_stack(document))).is_equal_to(expected.to_owned());
}

#[test]
fn local_def_named_like_a_slot_type_is_refused() {
    // `classify` reads `#/$defs/Child` as the common type: load refuses any own def
    // that would shadow it, so a catalog-defined `Child` never reaches the slot walk.
    let document = v10(json!({
        "components": {"C": {"properties": {"child": {"$ref": "#/$defs/Child"}}}},
        "$defs": {"Child": {"type": "integer"}}
    }));
    let expected = "Catalog $defs 'Child' shadows the common type of the same name";
    assert_that!(catalog_error(load(document))).is_equal_to(expected.to_owned());
}

#[test]
fn slot_fan_out_past_the_budget_is_a_catalog_limit() {
    // L0 holds a slot; each level names the one below twice: 2^14 slots at L14.
    let mut defs = Map::new();
    defs.insert(
        "L0".to_owned(),
        json!({"properties": {"c": common("Child")}}),
    );
    for level in 1..=14 {
        let below = format!("#/$defs/L{}", level - 1);
        defs.insert(
            format!("L{level}"),
            json!({"properties": {"a": {"$ref": below}, "b": {"$ref": below}}}),
        );
    }
    let document = v10(json!({"components": {"C": {"$ref": "#/$defs/L14"}}, "$defs": defs}));
    let expected = "Catalog exceeds 16384 schema nodes in 'C' once its $refs are expanded";
    assert_that!(catalog_error(load(document))).is_equal_to(expected.to_owned());
}

#[test]
fn derived_catalog_keeps_slots() {
    let source = load(v10(json!({"components": {
        "Card": {"type": "object", "properties": {"child": common("Child")}},
        "Text": {"type": "object", "properties": {"text": {"type": "string"}}}
    }})))
    .unwrap();

    let derived = source
        .derive(&Derivation::new("d").components(vec![DeriveEntry::Keep("Card".to_owned())]))
        .unwrap();

    let found = derived.component("Card").unwrap().child_slots().to_vec();
    assert_that!(found).is_equal_to(vec![slot(vec![key("child")], SlotKind::Single)]);
}
