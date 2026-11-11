use rstest::rstest;
use serde_json::json;
use speculoos::prelude::*;

use super::*;

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

#[test]
fn localize_refs_property_named_ref_holding_a_schema_is_walked() {
    let mut node = json!({"properties": {"$ref": {"$ref": "common_types.json#/$defs/X"}}});
    localize_refs(&mut node);
    assert_that!(node).is_equal_to(json!({"properties": {"$ref": {"$ref": "#/$defs/X"}}}));
}
