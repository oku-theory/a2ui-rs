use rstest::rstest;
use serde_json::{Map, Value, json};
use speculoos::prelude::*;

use super::*;
use crate::pointer::is_escaped_pointer;

#[rstest]
#[case("#/$defs/Weight", true)]
#[case("#", true)]
#[case("catalog.json#/$defs/anyFunction", true)]
#[case("common_types.json#/$defs/DynamicString", true)]
#[case("./common_types.json#/$defs/DynamicString", true)]
#[case(
    "https://a2ui.org/specification/v1_0/common_types.json#/$defs/DynamicString",
    true
)]
#[case("common_types.json", false)]
#[case("evil/common_types.json#/$defs/X", false)]
#[case("https://evil.example/common_types.json#/$defs/X", false)]
#[case("basic_catalog.json#/components/Text", false)]
#[case(
    "https://a2ui.org/specification/v1_0/catalogs/basic/catalog.json#/components/Button",
    false
)]
fn is_supported_ref_v1_0_targets_are_classified(#[case] reference: &str, #[case] expected: bool) {
    assert_that!(is_supported_ref(reference)).is_equal_to(expected);
}

#[rstest]
#[case("/$defs/Child", "#/$defs/Child")]
#[case("/components/Component_ℓ", "#/components/Component_%E2%84%93")]
#[case("/$defs/a b", "#/$defs/a%20b")]
#[case("/$defs/a~1b", "#/$defs/a~1b")]
fn local_ref_pointer_is_percent_encoded(#[case] pointer: &str, #[case] expected: &str) {
    assert_that!(local_ref(pointer)).is_equal_to(expected.to_owned());
}

#[rstest]
#[case("common_types.json#/$defs/Child", "#/$defs/Child")]
#[case(
    "https://a2ui.org/specification/v1_0/common_types.json#/$defs/Child",
    "#/$defs/Child"
)]
#[case("catalog.json#/$defs/anyFunction", "#/$defs/anyFunction")]
#[case("#/components/Text", "#/components/Text")]
#[case("#/components/Component_ℓ", "#/components/Component_%E2%84%93")]
#[case("#/components/Component_%E2%84%93", "#/components/Component_%E2%84%93")]
#[case("other.json#/x", "other.json#/x")]
fn localize_ref_supported_targets_become_local(#[case] reference: &str, #[case] expected: &str) {
    assert_that!(localize_ref(reference)).is_equal_to(expected.to_owned());
}

#[test]
fn localize_refs_rewrites_every_ref_string() {
    let mut node = json!({
        "a": {"$ref": "common_types.json#/$defs/X"},
        "b": [{"$ref": "catalog.json#/c"}]
    });
    localize_refs(&mut node);
    assert_that!(node).is_equal_to(json!({"a": {"$ref": "#/$defs/X"}, "b": [{"$ref": "#/c"}]}));
}

#[rstest]
#[case::const_data("const", json!({"$ref": "catalog.json#/$defs/X"}), json!({"$ref": "#/$defs/X"}))]
#[case::enum_data(
    "enum",
    json!([{"$ref": "catalog.json#/$defs/X"}]),
    json!([{"$ref": "#/$defs/X"}])
)]
#[case::local_form_kept("const", json!({"$ref": "#/$defs/X"}), json!({"$ref": "#/$defs/X"}))]
fn localize_refs_ref_in_data_is_rewritten_like_any_ref(
    #[case] keyword: &str,
    #[case] data: Value,
    #[case] expected: Value,
) {
    let mut node = json!({keyword: data});
    localize_refs(&mut node);
    assert_that!(node).is_equal_to(json!({keyword: expected}));
}

#[test]
fn localize_refs_property_named_ref_holding_a_schema_is_walked() {
    let mut node = json!({"properties": {"$ref": {"$ref": "common_types.json#/$defs/X"}}});
    localize_refs(&mut node);
    assert_that!(node).is_equal_to(json!({"properties": {"$ref": {"$ref": "#/$defs/X"}}}));
}

#[rstest]
#[case("#/$defs/Child", Some("/$defs/Child"))]
#[case("common_types.json#/$defs/Child", Some("/$defs/Child"))]
#[case("#/components/Component_%E2%84%93", Some("/components/Component_ℓ"))]
#[case("#", Some(""))]
#[case("catalog.json#", Some(""))]
#[case("other.json#/x", None)]
#[case("#A", None)]
#[case("#/components/%FF", None)]
fn pointer_of_supported_refs_are_decoded(#[case] reference: &str, #[case] expected: Option<&str>) {
    assert_that!(pointer_of(reference).as_deref()).is_equal_to(expected);
}

#[test]
fn local_ref_percent_sign_is_encoded() {
    assert_that!(local_ref("/$defs/a%b")).is_equal_to("#/$defs/a%25b".to_owned());
}

#[test]
fn pointer_of_encoded_percent_sign_is_decoded() {
    assert_that!(pointer_of("#/$defs/a%25b").as_deref()).is_equal_to(Some("/$defs/a%b"));
}

#[rstest]
#[case("/$defs/Child", Some(("$defs", "Child")))]
#[case("/components/Card/properties/child", Some(("components", "Card")))]
#[case("/functions/a~1b", Some(("functions", "a/b")))]
#[case("", None)]
#[case("/theme/x", None)]
#[case("/$defs", None)]
#[case("/$defs/a~2b", None)]
fn entry_of_pointer_names_its_entry(#[case] pointer: &str, #[case] expected: Option<(&str, &str)>) {
    let entry = entry_of(pointer);
    let entry = entry
        .as_ref()
        .map(|(section, name)| (*section, name.as_str()));
    assert_that!(entry).is_equal_to(expected);
}

#[test]
fn add_common_types_copies_the_reachable_closure() {
    let common = json!({
        "A": {"$ref": "#/$defs/B"},
        "B": {"type": "string"},
        "C": {"type": "number"}
    });
    let mut defs = Map::new();
    add_common_types(
        &mut defs,
        &[&json!({"$ref": "#/$defs/A"})],
        common.as_object().unwrap(),
    );
    let names: Vec<&str> = defs.keys().map(String::as_str).collect();
    assert_that!(names).is_equal_to(vec!["A", "B"]);
}

#[test]
fn add_common_types_cycle_between_common_types_terminates() {
    let common = json!({"A": {"$ref": "#/$defs/B"}, "B": {"$ref": "#/$defs/A"}});
    let mut defs = Map::new();
    add_common_types(
        &mut defs,
        &[&json!({"$ref": "#/$defs/A"})],
        common.as_object().unwrap(),
    );
    let names: Vec<&str> = defs.keys().map(String::as_str).collect();
    assert_that!(names).is_equal_to(vec!["A", "B"]);
}

#[test]
fn add_common_types_definition_already_present_is_kept() {
    let common = json!({"A": {"type": "string"}});
    let mut defs = Map::from_iter([("A".to_owned(), json!({"type": "number"}))]);
    add_common_types(
        &mut defs,
        &[&json!({"$ref": "#/$defs/A"})],
        common.as_object().unwrap(),
    );
    assert_that!(defs["A"].clone()).is_equal_to(json!({"type": "number"}));
}

#[test]
fn add_common_types_encoded_and_escaped_names_are_decoded() {
    let common = json!({"a/ℓ": {"type": "string"}});
    let mut defs = Map::new();
    let root = json!({"$ref": "#/$defs/a~1%E2%84%93"});
    add_common_types(&mut defs, &[&root], common.as_object().unwrap());
    assert_that!(defs.contains_key("a/ℓ")).is_true();
}

#[test]
fn add_common_types_copied_refs_are_localized() {
    let common = json!({"A": {"$ref": "common_types.json#/$defs/B"}, "B": {}});
    let mut defs = Map::new();
    add_common_types(
        &mut defs,
        &[&json!({"$ref": "#/$defs/A"})],
        common.as_object().unwrap(),
    );
    assert_that!(defs["A"].clone()).is_equal_to(json!({"$ref": "#/$defs/B"}));
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

#[rstest]
#[case::entry("#/$defs/Weight", Some("Weight"))]
#[case::escaped("#/$defs/a~1b~0c", Some("a/b~c"))]
#[case::percent_encoded("#/$defs/%C2%B5", Some("µ"))]
#[case::below_the_entry("#/$defs/Weight/description", None)]
#[case::other_section("#/components/Weight", None)]
#[case::section_only("#/$defs", None)]
#[case::bad_escape_in_the_name("#/$defs/a~2b", None)]
fn whole_def_name_names_only_a_whole_entry(#[case] reference: &str, #[case] name: Option<&str>) {
    assert_that!(whole_def_name(reference)).is_equal_to(name.map(str::to_owned));
}

#[test]
fn def_names_lists_only_defs_entries() {
    let node = json!({
        "a": {"$ref": "#/$defs/A"},
        "b": {"$ref": "#/components/B"},
        "c": {"$ref": "#/theme"},
        "d": {"$ref": "#/$defs/D/properties/x"}
    });
    let mut names = Vec::new();
    def_names(&node, &mut names);
    assert_that!(names).is_equal_to(vec!["A".to_owned(), "D".to_owned()]);
}

#[test]
fn common_types_every_ref_pointer_is_rfc_6901_escaped() {
    let mut pointers = Vec::new();
    for definition in crate::schema::common_type_defs().unwrap().values() {
        for_each_ref(definition, &mut |reference| {
            pointers.extend(pointer_of(reference))
        });
    }
    assert_that!(pointers.iter().all(|pointer| is_escaped_pointer(pointer))).is_true();
}

#[rstest]
#[case::localized("catalog.json#/$defs/X", false, "#/$defs/X")]
#[case::percent_encoded("#/$defs/µ", false, "#/$defs/%C2%B5")]
#[case::common_type("common_types.json#/$defs/Weight", true, "#/$defs/Weight")]
#[case::theme_node("catalog.json#/theme/properties/b", true, "#/$defs/theme/properties/b")]
#[case::whole_theme("#/theme", true, "#/$defs/theme")]
#[case::theme_absent("#/theme/properties/b", false, "#/theme/properties/b")]
#[case::longer_than_theme("#/themes", true, "#/themes")]
#[case::unsupported("other.json#/a", true, "other.json#/a")]
fn written_ref_authored_ref_is_the_validation_schema_ref(
    #[case] authored: &str,
    #[case] with_theme: bool,
    #[case] expected: &str,
) {
    assert_that!(written_ref(authored, with_theme)).is_equal_to(expected.to_owned());
}

#[rstest]
#[case::theme_with_space("catalog.json#/theme/a b")]
#[case::encoded("#/$defs/%C2%B5")]
#[case::literal_percent("#/$defs/a%2541")]
#[case::bad_escape("#/$defs/%zz")]
#[case::non_utf8("#/$defs/%E9")]
#[case::anchor("catalog.json#Name")]
#[case::encoded_theme_node("#/theme/%C2%B5")]
fn written_ref_is_idempotent_on_what_it_writes(
    #[case] authored: &str,
    #[values(false, true)] with_theme: bool,
) {
    let once = written_ref(authored, with_theme);
    assert_that!(written_ref(&once, with_theme)).is_equal_to(once);
}
