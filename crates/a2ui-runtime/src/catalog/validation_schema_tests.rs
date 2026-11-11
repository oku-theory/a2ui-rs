use rstest::rstest;
use serde_json::{Map, Value, json};
use speculoos::prelude::*;

use super::super::test_support::{load, mixin_copies, v10};
use crate::Catalog;
use crate::bundle::{localize_refs, resolve};
use crate::limits::{MAX_CATALOG_EXPANSION, oversize};
use crate::schema::common_type_defs;

/// The authored `$ref` to the `DynamicString` common type.
const DYNAMIC_STRING: &str = "common_types.json#/$defs/DynamicString";

/// The validation schema of a v1.0 catalog holding `extra`.
fn schema(extra: Value) -> Value {
    load(v10(extra)).unwrap().validation_schema().clone()
}

/// The validation schema of a catalog whose component `A` has property `label`.
fn with_label(label: Value) -> Value {
    schema(json!({"components": {"A": {"properties": {"label": label}}}}))
}

/// The flattened `label` property of [`with_label`]'s component.
fn label(label: Value) -> Value {
    with_label(label)["components"]["A"]["properties"]["label"].clone()
}

/// The `args` of function `f` declared with `properties`.
fn args_of(properties: Value) -> Value {
    let result = schema(json!({
        "$defs": {"Args": {"type": "object"}},
        "functions": {"f": {"properties": properties}}
    }));
    result["functions"]["f"]["properties"]["args"].clone()
}

#[rstest]
#[case::id("$id")]
#[case::title("title")]
#[case::description("description")]
#[case::metadata("metadata")]
fn validation_schema_header_omits_document_key(#[case] key: &str) {
    let result = schema(json!({key: "x", "components": {"A": {}}}));
    assert_that!(result.get(key)).is_none();
}

#[test]
fn validation_schema_header_names_schema_id_and_version() {
    let result = schema(json!({"components": {"A": {}}}));
    let header = (
        result["$schema"].clone(),
        result["catalogId"].clone(),
        result["protocolVersion"].clone(),
    );
    assert_that!(header).is_equal_to((
        json!("https://json-schema.org/draft/2020-12/schema"),
        json!("c"),
        json!("1.0"),
    ));
}

#[test]
fn validation_schema_header_uses_the_effective_id() {
    let document = v10(json!({"components": {"A": {}}}));
    let catalog = Catalog::from_json(&document, Some("other"), None).unwrap();
    assert_that!(catalog.validation_schema()["catalogId"].clone()).is_equal_to(json!("other"));
}

#[rstest]
#[case::text("Use Rows.", Some(json!("Use Rows.")))]
#[case::empty("", None)]
fn validation_schema_instructions_kept_when_non_empty(
    #[case] text: &str,
    #[case] expected: Option<Value>,
) {
    let result = schema(json!({"instructions": text, "components": {"A": {}}}));
    assert_that!(result.get("instructions").cloned()).is_equal_to(expected);
}

#[test]
fn validation_schema_components_are_flattened() {
    let result = schema(json!({"components": {"A": {
        "allOf": [{"$ref": "common_types.json#/$defs/Checkable"}]
    }}}));
    assert_that!(result["components"]["A"]["properties"].get("checks")).is_some();
}

#[test]
fn validation_schema_without_functions_has_no_functions_key() {
    assert_that!(schema(json!({"components": {"A": {}}})).get("functions")).is_none();
}

#[test]
fn validation_schema_kept_function_is_listed() {
    let result = schema(json!({"functions": {"f": {"returnType": "string"}}}));
    assert_that!(result["functions"].get("f")).is_some();
}

#[rstest]
#[case::absent(
    json!({"type": "object"}),
    json!({"type": "object", "unevaluatedProperties": false})
)]
#[case::moved(
    json!({"type": "object", "additionalProperties": true}),
    json!({"type": "object", "unevaluatedProperties": true})
)]
#[case::unevaluated_kept(
    json!({"type": "object", "unevaluatedProperties": true, "additionalProperties": false}),
    json!({"type": "object", "unevaluatedProperties": true})
)]
#[case::by_ref(
    json!({"$ref": "#/$defs/Args", "type": "object"}),
    json!({"$ref": "#/$defs/Args", "type": "object"})
)]
#[case::not_an_object_schema(json!({"type": "array"}), json!({"type": "array"}))]
fn validation_schema_function_args_are_closed(#[case] args: Value, #[case] expected: Value) {
    let result = args_of(json!({"@call": {"const": "f"}, "args": args}));
    assert_that!(result).is_equal_to(expected);
}

#[rstest]
#[case::at_call("@call", true)]
#[case::call("call", true)]
#[case::no_call("name", false)]
fn validation_schema_args_closed_only_beside_a_call(#[case] key: &str, #[case] closed: bool) {
    let result = args_of(json!({key: {"const": "f"}, "args": {"type": "object"}}));
    assert_that!(result.get("unevaluatedProperties").is_some()).is_equal_to(closed);
}

#[test]
fn validation_schema_common_ref_gets_its_description() {
    let result = label(json!({"$ref": "common_types.json#/$defs/DynamicString"}));
    assert_that!(result["description"].clone()).is_equal_to(json!("Represents a string"));
}

#[test]
fn validation_schema_call_site_description_wins() {
    let result = label(json!({
        "$ref": "common_types.json#/$defs/DynamicString", "description": "Mine"
    }));
    assert_that!(result["description"].clone()).is_equal_to(json!("Mine"));
}

#[test]
fn validation_schema_own_def_ref_gets_no_description() {
    let result = schema(json!({
        "$defs": {"W": {"type": "number", "description": "Weight"}},
        "components": {"A": {"properties": {"w": {"$ref": "#/$defs/W"}}}}
    }));
    assert_that!(result["components"]["A"]["properties"]["w"].clone())
        .is_equal_to(json!({"$ref": "#/$defs/W"}));
}

#[test]
fn validation_schema_sub_pointer_common_ref_gets_no_description() {
    let reference = json!({"$ref": "common_types.json#/$defs/DynamicString/oneOf/0"});
    let result = label(reference);
    assert_that!(result.get("description")).is_none();
}

#[rstest]
#[case::const_data("const", json!({"$ref": "common_types.json#/$defs/DynamicString"}))]
#[case::enum_data("enum", json!([{"$ref": "common_types.json#/$defs/DynamicString"}]))]
#[case::default_data("default", json!({"$ref": "common_types.json#/$defs/DynamicString"}))]
#[case::examples_data("examples", json!([{"$ref": "common_types.json#/$defs/DynamicString"}]))]
fn validation_schema_ref_inside_data_is_not_described(#[case] keyword: &str, #[case] data: Value) {
    let result = label(json!({keyword: data}));
    assert_that!(result[keyword].to_string().contains("description")).is_false();
}

#[rstest]
#[case::items(json!({"items": {"$ref": DYNAMIC_STRING}}), "/items")]
#[case::any_of(json!({"anyOf": [{"$ref": DYNAMIC_STRING}]}), "/anyOf/0")]
#[case::prefix_items(json!({"prefixItems": [{"$ref": DYNAMIC_STRING}]}), "/prefixItems/0")]
#[case::additional(
    json!({"additionalProperties": {"$ref": DYNAMIC_STRING}}),
    "/additionalProperties"
)]
#[case::not(json!({"not": {"$ref": DYNAMIC_STRING}}), "/not")]
fn validation_schema_ref_in_schema_position_is_described(
    #[case] property: Value,
    #[case] at: &str,
) {
    let result = label(property);
    let described = result.pointer(&format!("{at}/description")).cloned();
    assert_that!(described).is_equal_to(Some(json!("Represents a string")));
}

#[test]
fn validation_schema_property_named_like_a_data_keyword_is_described() {
    let result = schema(json!({"components": {"A": {"properties": {
        "default": {"$ref": "common_types.json#/$defs/DynamicString"}
    }}}}));
    assert_that!(result["components"]["A"]["properties"]["default"]["description"].clone())
        .is_equal_to(json!("Represents a string"));
}

#[test]
fn validation_schema_own_defs_are_described() {
    let result = schema(json!({
        "$defs": {"Label": {"$ref": "common_types.json#/$defs/DynamicString"}},
        "components": {"A": {"properties": {"l": {"$ref": "#/$defs/Label"}}}}
    }));
    assert_that!(result["$defs"]["Label"]["description"].clone())
        .is_equal_to(json!("Represents a string"));
}

#[test]
fn validation_schema_functions_are_described() {
    let result = schema(json!({"functions": {"f": {"properties": {"args": {
        "type": "object",
        "properties": {"v": {"$ref": "common_types.json#/$defs/DynamicString"}}
    }}}}}));
    let argument = &result["functions"]["f"]["properties"]["args"]["properties"]["v"];
    assert_that!(argument["description"].clone()).is_equal_to(json!("Represents a string"));
}

#[test]
fn validation_schema_common_types_are_copied_verbatim() {
    let result = with_label(json!({"$ref": "common_types.json#/$defs/DynamicString"}));
    let mut expected = common_type_defs().unwrap()["DynamicString"].clone();
    localize_refs(&mut expected);
    assert_that!(result["$defs"]["DynamicString"].clone()).is_equal_to(expected);
}

#[rstest]
#[case::checkable("Checkable", false)]
#[case::component_common("ComponentCommon", false)]
#[case::check_rule("CheckRule", true)]
fn validation_schema_bundles_only_reached_common_types(#[case] name: &str, #[case] kept: bool) {
    let result = schema(json!({"components": {"A": {"allOf": [
        {"$ref": "common_types.json#/$defs/ComponentCommon"},
        {"$ref": "common_types.json#/$defs/Checkable"}
    ]}}}));
    assert_that!(result["$defs"].get(name).is_some()).is_equal_to(kept);
}

#[rstest]
#[case::merged_only(json!({"M": {"properties": {"m": {}}}}), json!({}), false)]
#[case::named_by_own_def(
    json!({"M": {"properties": {"m": {}}}, "U": {"$ref": "#/$defs/M"}}),
    json!({}),
    true
)]
#[case::named_by_a_property(
    json!({"M": {"properties": {"m": {}}}}),
    json!({"x": {"$ref": "#/$defs/M"}}),
    true
)]
fn validation_schema_merged_mixin_pruned_unless_named(
    #[case] defs: Value,
    #[case] properties: Value,
    #[case] kept: bool,
) {
    let result = schema(json!({
        "$defs": defs,
        "components": {"A": {"allOf": [{"$ref": "#/$defs/M"}], "properties": properties}}
    }));
    assert_that!(result["$defs"].get("M").is_some()).is_equal_to(kept);
}

/// `anyFunction`, not `anyComponent`: naming `anyComponent` cycles if the union lists it.
#[test]
fn validation_schema_union_named_as_a_member_stays_defined() {
    let result = schema(json!({"components": {"A": {
        "allOf": [{"$ref": "#/$defs/anyFunction"}]
    }}}));
    assert_that!(result["$defs"].get("anyFunction")).is_some();
}

/// Guard: the budget rule; a lookahead pattern fails any compile, so this def was never built.
#[test]
fn validation_schema_unreached_def_is_copied_never_compiled() {
    let unused = json!({"type": "string", "pattern": "(?=a)"});
    let result = schema(json!({"$defs": {"Unused": unused.clone()}, "components": {"A": {}}}));
    assert_that!(result["$defs"]["Unused"].clone()).is_equal_to(unused);
}

#[test]
fn validation_schema_mixin_copies_stay_within_catalog_expansion() {
    let catalog = load(mixin_copies(21)).unwrap();
    assert_that!(oversize(catalog.validation_schema(), MAX_CATALOG_EXPANSION)).is_none();
}

#[test]
fn validation_schema_sub_pointer_mixin_chain_is_not_followed() {
    let result = schema(json!({
        "$defs": {
            "M": {"properties": {"m": {}}, "allOf": [{"$ref": "#/$defs/N"}]},
            "N": {"properties": {"n": {}}}
        },
        "components": {"A": {"allOf": [{"$ref": "#/$defs/M/properties"}]}}
    }));
    assert_that!(result["components"]["A"]["properties"].get("n")).is_none();
}

#[test]
fn from_json_long_sub_pointer_chain_loads_on_a_small_stack() {
    let links = 20_000;
    let mut defs: Map<String, Value> = (0..links - 1)
        .map(|index| {
            let next = format!("#/$defs/M{}", index + 1);
            (
                format!("M{index}"),
                json!({"properties": {"p": {}}, "allOf": [{"$ref": next}]}),
            )
        })
        .collect();
    defs.insert(format!("M{}", links - 1), json!({"properties": {"p": {}}}));
    let document = v10(json!({
        "$defs": defs,
        "components": {"A": {"allOf": [{"$ref": "#/$defs/M0/properties"}]}}
    }));
    let loaded = std::thread::Builder::new()
        .stack_size(1 << 20)
        .spawn(move || load(document).map(|catalog| catalog.validation_schema().clone()))
        .unwrap()
        .join()
        .unwrap();
    assert_that!(loaded).is_ok();
}

/// The validation schema of a catalog with component `A` and `theme`.
fn with_theme(theme: Value) -> Value {
    schema(json!({"theme": theme, "components": {"A": {}}}))
}

#[rstest]
#[case::open(
    json!({"type": "object", "properties": {"primaryColor": {"type": "string"}}}),
    json!({
        "type": "object",
        "properties": {"primaryColor": {"type": "string"}},
        "additionalProperties": true
    })
)]
#[case::closed(
    json!({"type": "object", "additionalProperties": false}),
    json!({"type": "object", "additionalProperties": false})
)]
#[case::unevaluated(
    json!({"type": "object", "unevaluatedProperties": false}),
    json!({"type": "object", "unevaluatedProperties": false})
)]
#[case::boolean(json!(true), json!(true))]
fn validation_schema_theme_is_written_under_defs(#[case] theme: Value, #[case] expected: Value) {
    assert_that!(with_theme(theme)["$defs"]["theme"].clone()).is_equal_to(expected);
}

#[test]
fn validation_schema_header_has_no_theme_key() {
    assert_that!(with_theme(json!({"type": "object"})).get("theme")).is_none();
}

#[test]
fn validation_schema_without_theme_has_no_theme_def() {
    assert_that!(schema(json!({"components": {"A": {}}}))["$defs"].get("theme")).is_none();
}

#[test]
fn validation_schema_theme_common_ref_is_localized() {
    let result = with_theme(json!({"properties": {"name": {"$ref": DYNAMIC_STRING}}}));
    assert_that!(result["$defs"]["theme"]["properties"]["name"]["$ref"].clone())
        .is_equal_to(json!("#/$defs/DynamicString"));
}

#[test]
fn validation_schema_theme_common_ref_is_described() {
    let result = with_theme(json!({"properties": {"name": {"$ref": DYNAMIC_STRING}}}));
    assert_that!(result["$defs"]["theme"]["properties"]["name"]["description"].clone())
        .is_equal_to(json!("Represents a string"));
}

#[test]
fn validation_schema_theme_common_ref_is_bundled() {
    let result = with_theme(json!({"properties": {"name": {"$ref": DYNAMIC_STRING}}}));
    assert_that!(result["$defs"].get("DynamicString")).is_some();
}

#[rstest]
#[case::whole("#/theme", "#/$defs/theme")]
#[case::interior("#/theme/$defs/Brand", "#/$defs/theme/$defs/Brand")]
#[case::encoded("#/theme/$defs/Brand_%E2%84%93", "#/$defs/theme/$defs/Brand_%E2%84%93")]
fn validation_schema_theme_self_ref_points_at_theme_def(
    #[case] reference: &str,
    #[case] expected: &str,
) {
    let result = with_theme(json!({"properties": {"brand": {"$ref": reference}}}));
    assert_that!(result["$defs"]["theme"]["properties"]["brand"]["$ref"].clone())
        .is_equal_to(json!(expected));
}

#[test]
fn validation_schema_def_ref_to_a_def_is_unchanged() {
    let reference = "#/$defs/Brand";
    let result = schema(json!({
        "$defs": {"Brand": {}, "Unreached": {"$ref": reference}},
        "theme": {},
        "components": {"A": {}}
    }));
    assert_that!(result["$defs"]["Unreached"]["$ref"].clone()).is_equal_to(json!(reference));
}

#[test]
fn validation_schema_def_ref_into_theme_points_at_theme_def() {
    let result = schema(json!({
        "$defs": {"Unreached": {"$ref": "#/theme/properties/brand"}},
        "theme": {"properties": {"brand": {"type": "string"}}},
        "components": {"A": {}}
    }));
    assert_that!(result["$defs"]["Unreached"]["$ref"].clone())
        .is_equal_to(json!("#/$defs/theme/properties/brand"));
}

#[test]
fn validation_schema_recursive_theme_has_no_dangling_ref() {
    let result = with_theme(json!({
        "$defs": {"Brand": {"properties": {"inner": {"$ref": "#/theme/$defs/Brand"}}}},
        "properties": {"brand": {"$ref": "#/theme/$defs/Brand"}}
    }));
    let mut references = Vec::new();
    collect_refs(&result, &mut references);
    let dangling: Vec<&String> = references
        .iter()
        .filter(|reference| resolve(&result, reference).is_none())
        .collect();
    assert_that!(dangling).is_empty();
}

/// Every `$ref` string under `node`.
fn collect_refs(node: &Value, out: &mut Vec<String>) {
    match node {
        Value::Object(map) => {
            for (key, value) in map {
                match value {
                    Value::String(reference) if key == "$ref" => out.push(reference.clone()),
                    _ => collect_refs(value, out),
                }
            }
        }
        Value::Array(items) => items.iter().for_each(|item| collect_refs(item, out)),
        _ => {}
    }
}

#[test]
fn validation_schema_mixin_named_by_theme_is_kept() {
    let result = schema(json!({
        "$defs": {"M": {"properties": {"m": {}}}},
        "theme": {"properties": {"brand": {"$ref": "#/$defs/M"}}},
        "components": {"A": {"allOf": [{"$ref": "#/$defs/M"}]}}
    }));
    assert_that!(result["$defs"].get("M")).is_some();
}
