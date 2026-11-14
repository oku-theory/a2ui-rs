use std::collections::BTreeSet;

use rstest::rstest;
use serde_json::{Map, Value, json};
use speculoos::prelude::*;

use super::{Flattened, flattened};
use crate::bundle::localize_refs;
use crate::schema::common_type_defs;

/// The bundle `$defs` a component resolves in: `own` plus every common type, localized.
fn defs(own: Value) -> Map<String, Value> {
    let mut defs = own.as_object().unwrap().clone();
    for (name, definition) in common_type_defs().unwrap() {
        let mut definition = definition.clone();
        localize_refs(&mut definition);
        defs.insert(name.clone(), definition);
    }
    defs
}

/// Flattens `schema` as component `A`, with `own` as the catalog's `$defs`.
fn flat(schema: Value, own: Value) -> Flattened {
    flattened("A", &schema, &defs(own), common_type_defs().unwrap())
}

/// The flattened schema of `schema` as component `A`, with `own` as the catalog's `$defs`.
fn flat_with(schema: Value, own: Value) -> Value {
    Value::Object(flat(schema, own).schema)
}

/// The flattened schema of `schema` as component `A`, with no `$defs` of its own.
fn schema_of(schema: Value) -> Value {
    flat_with(schema, json!({}))
}

/// The property names of a flattened schema.
fn property_names(schema: &Value) -> BTreeSet<String> {
    schema["properties"]
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect()
}

#[test]
fn flattened_empty_component_gets_the_envelope() {
    assert_that!(schema_of(json!({}))).is_equal_to(json!({
        "type": "object",
        "properties": {
            "id": {"$ref": "#/$defs/ComponentId"},
            "component": {"const": "A"}
        },
        "required": ["id", "component"]
    }));
}

#[test]
fn flattened_component_const_comes_from_its_property() {
    let schema = schema_of(json!({"properties": {"component": {"const": "Other"}}}));
    assert_that!(schema["properties"]["component"].clone()).is_equal_to(json!({"const": "Other"}));
}

#[test]
fn flattened_component_without_const_uses_entry_name() {
    let own = json!({"M": {"properties": {"component": {"const": "X"}}}});
    let schema = flat_with(
        json!({
            "allOf": [{"$ref": "#/$defs/M"}],
            "properties": {"component": {"description": "d"}}
        }),
        own,
    );
    assert_that!(schema["properties"]["component"].clone()).is_equal_to(json!({"const": "A"}));
}

#[test]
fn flattened_common_mixin_properties_are_merged() {
    let schema = schema_of(json!({"allOf": [{"$ref": "#/$defs/Checkable"}]}));
    assert_that!(schema["properties"]["checks"]["items"].clone())
        .is_equal_to(json!({"$ref": "#/$defs/CheckRule"}));
}

#[test]
fn flattened_merged_mixin_leaves_no_all_of() {
    let schema = schema_of(json!({"allOf": [{"$ref": "#/$defs/Checkable"}]}));
    assert_that!(schema.get("allOf")).is_none();
}

#[test]
fn flattened_common_mixin_description_is_not_copied() {
    let schema = schema_of(json!({"allOf": [{"$ref": "#/$defs/Checkable"}]}));
    assert_that!(schema.get("description")).is_none();
}

#[test]
fn flattened_component_common_drops_catalog_id() {
    let schema = schema_of(json!({"allOf": [{"$ref": "#/$defs/ComponentCommon"}]}));
    let expected: BTreeSet<String> = ["accessibility", "component", "id", "metadata"]
        .map(String::from)
        .into();
    assert_that!(property_names(&schema)).is_equal_to(expected);
}

#[test]
fn flattened_specific_piece_wins_over_local_mixin() {
    let own = json!({"M": {"properties": {"weight": {"type": "number"}}}});
    let schema = flat_with(
        json!({"allOf": [
            {"properties": {"component": {"const": "A"}, "weight": {"type": "integer"}}},
            {"$ref": "#/$defs/M"}
        ]}),
        own,
    );
    assert_that!(schema["properties"]["weight"].clone()).is_equal_to(json!({"type": "integer"}));
}

#[test]
fn flattened_local_mixin_with_component_property_stays_local() {
    let own = json!({"M": {"properties": {
        "component": {"const": "A"}, "w": {"type": "number"}
    }}});
    let schema = flat_with(
        json!({"properties": {"w": {"type": "string"}}, "allOf": [{"$ref": "#/$defs/M"}]}),
        own,
    );
    assert_that!(schema["properties"]["w"].clone()).is_equal_to(json!({"type": "string"}));
}

#[test]
fn flattened_local_mixin_is_reported_merged() {
    let own = json!({"M": {"properties": {"weight": {"type": "number"}}}});
    let merged = flat(json!({"allOf": [{"$ref": "#/$defs/M"}]}), own).merged;
    assert_that!(merged).is_equal_to(BTreeSet::from(["M".to_owned()]));
}

#[test]
fn flattened_mixin_cycle_merges_each_def_once() {
    let own = json!({
        "M": {"allOf": [{"$ref": "#/$defs/N"}], "properties": {"m": {}}},
        "N": {"allOf": [{"$ref": "#/$defs/M"}], "properties": {"n": {}}}
    });
    let merged = flat(json!({"allOf": [{"$ref": "#/$defs/M"}]}), own).merged;
    assert_that!(merged).is_equal_to(BTreeSet::from(["M".to_owned(), "N".to_owned()]));
}

#[rstest]
#[case::non_mixin_common_type(json!({"$ref": "#/$defs/DynamicString"}))]
#[case::ref_with_siblings(json!({"$ref": "#/$defs/Checkable", "description": "x"}))]
#[case::boolean_member(json!(true))]
#[case::sub_pointer_own_def(json!({"$ref": "#/$defs/M/properties"}))]
#[case::sub_pointer_common_type(json!({"$ref": "#/$defs/Checkable/properties"}))]
#[case::union(json!({"$ref": "#/$defs/anyComponent"}))]
fn flattened_unmergeable_member_is_kept_whole(#[case] member: Value) {
    let own = json!({
        "M": {"properties": {"m": {}}},
        "anyComponent": {"oneOf": [{"$ref": "#/components/A"}]}
    });
    let schema = flat_with(json!({"allOf": [member.clone()]}), own);
    assert_that!(schema["allOf"].clone()).is_equal_to(json!([member]));
}

#[test]
fn flattened_sub_pointer_member_merges_nothing() {
    let own = json!({"M": {
        "properties": {"m": {}},
        "allOf": [{"$ref": "#/$defs/N"}]
    }, "N": {"properties": {"n": {}}}});
    let found = flat(json!({"allOf": [{"$ref": "#/$defs/M/properties"}]}), own);
    assert_that!(found.merged.is_empty()).is_true();
}

#[test]
fn flattened_inline_member_extra_keys_stay_in_all_of() {
    let schema = schema_of(json!({"allOf": [
        {"properties": {"a": {"type": "string"}}, "minProperties": 1}
    ]}));
    assert_that!(schema["allOf"].clone()).is_equal_to(json!([{"minProperties": 1}]));
}

#[test]
fn flattened_inline_member_properties_are_merged() {
    let schema = schema_of(json!({"allOf": [
        {"properties": {"a": {"type": "string"}}, "minProperties": 1}
    ]}));
    assert_that!(schema["properties"]["a"].clone()).is_equal_to(json!({"type": "string"}));
}

#[test]
fn flattened_local_mixin_extra_keys_stay_in_all_of() {
    let own = json!({"M": {"properties": {"m": {}}, "minProperties": 1}});
    let schema = flat_with(json!({"allOf": [{"$ref": "#/$defs/M"}]}), own);
    assert_that!(schema["allOf"].clone()).is_equal_to(json!([{"minProperties": 1}]));
}

#[test]
fn flattened_member_closing_keyword_is_dropped() {
    let schema = schema_of(json!({"allOf": [
        {"properties": {"a": {}}, "additionalProperties": false}
    ]}));
    let kept = (schema.get("allOf"), schema.get("unevaluatedProperties"));
    assert_that!(kept).is_equal_to((None, None));
}

#[test]
fn flattened_required_drops_protocol_keys_and_sorts() {
    let schema = schema_of(json!({"required": ["b", "catalogId", "a", "b"]}));
    assert_that!(schema["required"].clone()).is_equal_to(json!(["id", "a", "b", "component"]));
}

#[test]
fn flattened_specific_description_wins_over_common() {
    let schema = schema_of(json!({
        "description": "Top",
        "allOf": [{"properties": {"component": {"const": "A"}}, "description": "Specific"}]
    }));
    assert_that!(schema["description"].clone()).is_equal_to(json!("Specific"));
}

#[test]
fn flattened_empty_description_keeps_the_earlier_one() {
    let schema = schema_of(json!({
        "description": "Top",
        "allOf": [{"properties": {"component": {"const": "A"}}, "description": ""}]
    }));
    assert_that!(schema["description"].clone()).is_equal_to(json!("Top"));
}

#[test]
fn flattened_additional_properties_is_removed() {
    let schema = schema_of(json!({"additionalProperties": false}));
    assert_that!(schema.get("additionalProperties")).is_none();
}

#[test]
fn flattened_additional_properties_becomes_unevaluated() {
    let schema = schema_of(json!({"additionalProperties": false}));
    assert_that!(schema["unevaluatedProperties"].clone()).is_equal_to(json!(false));
}

#[test]
fn flattened_unevaluated_properties_wins_over_additional() {
    let schema = schema_of(json!({
        "unevaluatedProperties": true, "additionalProperties": false
    }));
    assert_that!(schema["unevaluatedProperties"].clone()).is_equal_to(json!(true));
}

#[rstest]
#[case::parents_empty("allowedParents", json!([]), None)]
#[case::parents_listed("allowedParents", json!(["Row"]), Some(json!(["Row"])))]
#[case::children_empty("allowedChildren", json!([]), None)]
#[case::children_listed("allowedChildren", json!(["Row"]), Some(json!(["Row"])))]
fn flattened_allowed_list_kept_when_non_empty(
    #[case] key: &str,
    #[case] listed: Value,
    #[case] expected: Option<Value>,
) {
    let schema = schema_of(json!({key: listed}));
    assert_that!(schema.get(key).cloned()).is_equal_to(expected);
}

#[test]
fn flattened_other_top_level_key_is_kept() {
    let metadata = json!({"extensions": {"vendor": 1}});
    let schema = schema_of(json!({"metadata": metadata.clone()}));
    assert_that!(schema["metadata"].clone()).is_equal_to(metadata);
}

#[test]
fn flattened_own_id_property_is_replaced_by_the_envelope() {
    let schema = schema_of(json!({"properties": {"id": {"type": "integer"}}}));
    assert_that!(schema["properties"]["id"].clone())
        .is_equal_to(json!({"$ref": "#/$defs/ComponentId"}));
}
