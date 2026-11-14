use serde_json::{Map, Value, json};
use speculoos::prelude::*;

use super::super::test_support::{catalog_error, fan_out, load, mixin_copies, v10};
use super::{function_call_envelope, pruned, reach};
use crate::bundle::{for_each_ref, resolve};
use crate::limits::MAX_SCHEMA_DEPTH;

fn refs(node: &Value, out: &mut Vec<String>) {
    for_each_ref(node, &mut |target| out.push(target.to_owned()));
}

/// A catalog whose `section` holds `entry`, with the `$defs` of a fan-out `levels` deep.
fn fanned(section: &str, entry: &str, levels: usize, unevaluated: bool) -> Value {
    let mut extra = Map::new();
    extra.insert(
        "$defs".to_owned(),
        Value::Object(fan_out(levels, unevaluated)),
    );
    let mut entries = Map::new();
    entries.insert(
        entry.to_owned(),
        json!({"$ref": format!("#/$defs/L{levels}")}),
    );
    extra.insert(section.to_owned(), Value::Object(entries));
    v10(Value::Object(extra))
}

/// A root whose entry `X` reaches `$defs/D`, function `f` and component `C`, which reaches
/// `$defs/E`; `Unreached`, `g` and `$defs/F` are reached by nothing.
fn reaching_root() -> Value {
    json!({
        "components": {
            "X": {"properties": {
                "label": {"$ref": "#/$defs/D"},
                "call": {"$ref": "#/functions/f"},
                "child": {"$ref": "#/components/C"}
            }},
            "C": {"properties": {"n": {"$ref": "#/$defs/E"}}},
            "Unreached": {"type": "object"}
        },
        "functions": {"f": {"returnType": "string"}, "g": {"returnType": "number"}},
        "$defs": {"D": {"type": "string"}, "E": {"type": "number"}, "F": {"type": "null"}}
    })
}

#[test]
fn pruned_entry_reaching_defs_and_a_component_holds_exactly_those_keys() {
    let root = reaching_root();
    // As authored: `localize_refs` must turn the catalog-relative `$ref` into a local one.
    let entry = json!({"properties": {
        "label": {"$ref": "catalog.json#/$defs/D"},
        "call": {"$ref": "#/functions/f"},
        "child": {"$ref": "#/components/C"}
    }});
    let targets = reach(&root, "X", &entry).unwrap().targets;
    let document = pruned(&root, "components", "X", &entry, targets);
    // A literal, not `DRAFT_2020_12`: the pin is the document the build sees.
    assert_that!(document).is_equal_to(json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$ref": "#/components/X",
        "components": {
            "X": {"properties": {
                "label": {"$ref": "#/$defs/D"},
                "call": {"$ref": "#/functions/f"},
                "child": {"$ref": "#/components/C"}
            }},
            "C": {"properties": {"n": {"$ref": "#/$defs/E"}}}
        },
        "functions": {"f": {"returnType": "string"}},
        "$defs": {"D": {"type": "string"}, "E": {"type": "number"}}
    }));
}

#[test]
fn bundle_unions_keep_only_listed_components() {
    let document = v10(json!({
        "components": {"Listed": {"type": "object"}, "Unlisted": {"type": "object"}},
        "$defs": {"anyComponent": {
            "oneOf": [{"$ref": "#/components/Listed"}],
            "discriminator": {"propertyName": "component"}
        }}
    }));
    let catalog = load(document).unwrap();
    let names: Vec<&str> = catalog.components().map(|def| def.name()).collect();
    assert_that!(names).is_equal_to(vec!["Listed"]);
}

#[test]
fn bundle_unions_keep_only_listed_functions() {
    let document = v10(json!({
        "functions": {"listed": {"returnType": "string"}, "unlisted": {"returnType": "string"}},
        "$defs": {"anyFunction": {"oneOf": [{"$ref": "#/functions/listed"}]}}
    }));
    let catalog = load(document).unwrap();
    let names: Vec<&str> = catalog.functions().map(|def| def.name()).collect();
    assert_that!(names).is_equal_to(vec!["listed"]);
}

#[test]
fn validation_schema_every_ref_resolves_locally() {
    let document = v10(json!({"components": {"Label_ℓ": {
        "allOf": [{"$ref": "common_types.json#/$defs/Checkable"}],
        "properties": {"text": {
            "$ref": "https://a2ui.org/specification/v1_0/common_types.json#/$defs/DynamicString"
        }}
    }}}));
    let catalog = load(document).unwrap();
    let schema = catalog.validation_schema();
    let mut found = Vec::new();
    refs(schema, &mut found);
    assert_that!(found.is_empty()).is_false();
    for target in found {
        assert_that!(target.starts_with('#')).is_true();
        assert_that!(resolve(schema, &target)).is_some();
    }
}

#[test]
fn validation_schema_missing_unions_are_generated() {
    let catalog = load(v10(json!({"components": {"A": {}}}))).unwrap();
    let defs = &catalog.validation_schema()["$defs"];
    assert_that!(defs["anyComponent"].clone()).is_equal_to(json!({
        "oneOf": [{"$ref": "#/components/A"}],
        "discriminator": {"propertyName": "component"}
    }));
}

#[test]
fn validation_schema_missing_function_union_matches_nothing() {
    let catalog = load(v10(json!({"components": {"A": {}}}))).unwrap();
    let union = catalog.validation_schema()["$defs"]["anyFunction"].clone();
    assert_that!(union).is_equal_to(json!({"not": {}}));
}

#[test]
fn from_json_lookahead_pattern_is_catalog_error() {
    let document = v10(json!({"components": {"A": {"properties": {
        "p": {"type": "string", "pattern": "(?=a)"}
    }}}}));
    assert_that!(catalog_error(load(document))).contains("is not a \"regex\"");
}

#[test]
fn from_json_unreached_defs_are_not_compiled() {
    let document = v10(json!({
        "components": {"A": {"type": "object"}},
        "$defs": {"Unused": {"type": "string", "pattern": "(?=a)"}}
    }));
    assert_that!(load(document)).is_ok();
}

#[test]
fn from_json_unresolvable_local_ref_is_catalog_error() {
    let document = v10(json!({"components": {"A": {"$ref": "#/$defs/Missing"}}}));
    let message = catalog_error(load(document));
    let expected = "Malformed catalog: $ref '#/$defs/Missing' does not resolve";
    assert_that!(message).is_equal_to(expected.to_owned());
}

#[test]
fn from_json_same_instance_ref_cycle_is_catalog_error() {
    let document = v10(json!({
        "components": {"A": {"$ref": "#/$defs/X"}},
        "$defs": {"X": {"anyOf": [{"$ref": "#/$defs/Y"}]}, "Y": {"allOf": [{"$ref": "#/$defs/X"}]}}
    }));
    let message = catalog_error(load(document));
    let expected = "Malformed catalog: $ref cycle through '#/$defs/X'";
    assert_that!(message).is_equal_to(expected.to_owned());
}

#[test]
fn from_json_exponential_ref_fan_out_is_catalog_error() {
    let message = catalog_error(load(fanned("components", "A", 16, true)));
    let expected = "Catalog exceeds 16384 schema nodes in 'A' once its $refs are expanded";
    assert_that!(message).is_equal_to(expected.to_owned());
}

/// A catalog whose component `A` references `$defs.Big`, an `enum` of `values` zeros.
///
/// `A` expands to `2 * values + 6` nodes: its map and `$ref`, `Big` walked (`values + 2`),
/// and `Big` charged once, whole, as the entry it reaches (`values + 2`).
fn big_enum(values: usize) -> Value {
    v10(json!({
        "components": {"A": {"$ref": "#/$defs/Big"}},
        "$defs": {"Big": {"enum": vec![0; values]}}
    }))
}

#[test]
fn from_json_entry_at_the_expansion_cap_is_accepted() {
    assert_that!(load(big_enum(8189))).is_ok();
}

#[test]
fn from_json_entry_just_past_the_expansion_cap_is_catalog_error() {
    let message = catalog_error(load(big_enum(8190)));
    let expected = "Catalog exceeds 16384 schema nodes in 'A' once its $refs are expanded";
    assert_that!(message).is_equal_to(expected.to_owned());
}

#[test]
fn from_json_pointer_into_a_large_entry_is_catalog_error() {
    let properties: Map<String, Value> = (0..10_000)
        .map(|index| (format!("p{index}"), json!({"type": "string"})))
        .collect();
    let document = v10(json!({
        "components": {"A": {
            "type": "object",
            "allOf": [{"$ref": "#/$defs/Big/properties/p0"}]
        }},
        "$defs": {"Big": {"properties": properties}}
    }));
    let message = catalog_error(load(document));
    let expected = "Catalog exceeds 16384 schema nodes in 'A' once its $refs are expanded";
    assert_that!(message).is_equal_to(expected.to_owned());
}

#[test]
fn from_json_function_past_the_expansion_cap_is_catalog_error() {
    let message = catalog_error(load(fanned("functions", "f", 12, false)));
    let expected = "Catalog exceeds 16384 schema nodes in 'f' once its $refs are expanded";
    assert_that!(message).is_equal_to(expected.to_owned());
}

#[test]
fn from_json_entries_past_the_catalog_expansion_cap_are_catalog_error() {
    let components: Map<String, Value> = (0..32)
        .map(|index| (format!("C{index}"), json!({"$ref": "#/$defs/L10"})))
        .collect();
    let document = v10(json!({"components": components, "$defs": fan_out(10, false)}));
    let message = catalog_error(load(document));
    assert_that!(message).is_equal_to(
        "Catalog exceeds 262144 schema nodes in all entries once their $refs are expanded"
            .to_owned(),
    );
}

#[test]
fn from_json_patterns_past_the_cap_are_catalog_error() {
    let properties: Map<String, Value> = (0..=256)
        .map(|index| {
            (
                format!("p{index}"),
                json!({"type": "string", "pattern": "^a"}),
            )
        })
        .collect();
    let document = v10(json!({"components": {"A": {"properties": properties}}}));
    let message = catalog_error(load(document));
    assert_that!(message).is_equal_to("Catalog exceeds 256 patterns".to_owned());
}

#[test]
fn from_json_ref_chain_past_the_depth_cap_is_catalog_error() {
    let mut defs = Map::from_iter([("D0".to_owned(), json!({}))]);
    for link in 1..=MAX_SCHEMA_DEPTH {
        defs.insert(
            format!("D{link}"),
            json!({"$ref": format!("#/$defs/D{}", link - 1)}),
        );
    }
    let start = format!("#/$defs/D{MAX_SCHEMA_DEPTH}");
    let document = v10(json!({"components": {"A": {"$ref": start}}, "$defs": defs}));
    let message = catalog_error(load(document));
    let expected = "Catalog exceeds 128 levels of 'A' once its $refs are expanded";
    assert_that!(message).is_equal_to(expected.to_owned());
}

#[test]
fn from_json_same_document_fails_with_the_same_message_twice() {
    let document = v10(json!({"components": {"A": {"properties": {
        "p": {"type": "string", "pattern": "(?=a)"},
        "q": {"type": "string", "pattern": "(?!b)"}
    }}}}));
    let first = catalog_error(load(document.clone()));
    assert_that!(catalog_error(load(document))).is_equal_to(first);
}

/// A catalog whose component `A` holds a `description` of `bytes` bytes.
///
/// `A` weighs `2 + bytes / 64` nodes: its map, and the description string.
fn long_description(bytes: usize) -> Value {
    v10(json!({"components": {"A": {"description": "d".repeat(bytes)}}}))
}

#[test]
fn from_json_string_at_the_entry_weight_cap_is_accepted() {
    assert_that!(load(long_description(16_382 * 64 + 63))).is_ok();
}

#[test]
fn from_json_string_past_the_entry_weight_cap_is_catalog_error() {
    let message = catalog_error(load(long_description(16_383 * 64)));
    let expected = "Catalog exceeds 16384 schema nodes in 'A' once its $refs are expanded";
    assert_that!(message).is_equal_to(expected.to_owned());
}

#[test]
fn from_json_shared_large_description_past_the_catalog_expansion_cap_is_catalog_error() {
    // Each component weighs 12 506 nodes (its map and `$ref`, `Shared` walked and then
    // charged whole, the 400 000-byte description 6 251 each time): 21 make 262 626.
    let components: Map<String, Value> = (0..21)
        .map(|index| (format!("C{index}"), json!({"$ref": "#/$defs/Shared"})))
        .collect();
    let document = v10(json!({
        "components": components,
        "$defs": {"Shared": {"description": "d".repeat(400_000)}}
    }));
    let message = catalog_error(load(document));
    assert_that!(message).is_equal_to(
        "Catalog exceeds 262144 schema nodes in all entries once their $refs are expanded"
            .to_owned(),
    );
}

#[test]
fn from_json_21_mixin_copies_fit_catalog_expansion() {
    assert_that!(load(mixin_copies(21))).is_ok();
}

#[test]
fn from_json_22_mixin_copies_past_catalog_expansion_is_catalog_error() {
    assert_that!(catalog_error(load(mixin_copies(22))))
        .contains("schema nodes in all entries once their $refs are expanded");
}

#[test]
fn from_json_mixin_chain_past_schema_depth_is_catalog_error() {
    let hops = 50;
    let mut defs: Map<String, Value> = (0..hops)
        .map(|index| {
            let next = format!("#/$defs/M{}", index + 1);
            (format!("M{index}"), json!({"allOf": [{"$ref": next}]}))
        })
        .collect();
    defs.insert(format!("M{hops}"), json!({"properties": {"p": {}}}));
    let document = v10(json!({"$defs": defs, "components": {"A": {
        "allOf": [{"$ref": "#/$defs/M0"}]
    }}}));
    assert_that!(catalog_error(load(document))).contains("levels of 'A'");
}

#[test]
fn function_call_envelope_closes_the_call_over_function_common() {
    assert_that!(function_call_envelope()).is_equal_to(json!({
        "type": "object",
        "allOf": [{"$ref": "#/$defs/FunctionCommon"}],
        "properties": {"args": {"type": "object"}},
        "unevaluatedProperties": false
    }));
}
