use std::borrow::Cow;

use rstest::rstest;
use serde_json::{Value, json};
use speculoos::prelude::*;

use super::*;

/// The tokens of `reference`'s pointer, or `None` when it has none.
fn tokens(reference: &str) -> Option<Vec<String>> {
    RefPointer::of(reference).map(|pointer| {
        pointer
            .tokens()
            .iter()
            .map(|token| token.to_string())
            .collect()
    })
}

/// Whether token `index` of `reference`'s pointer borrows from `reference`.
fn borrows(reference: &str, index: usize) -> bool {
    let pointer = RefPointer::of(reference).unwrap();
    matches!(pointer.tokens()[index], Cow::Borrowed(_))
}

#[rstest]
#[case::entry("#/$defs/Child", Some(vec!["$defs", "Child"]))]
#[case::escapes_once("#/$defs/a~1b~0c~01", Some(vec!["$defs", "a/b~c~1"]))]
#[case::percent_decoded_first("#/$defs/%C2%B5~1", Some(vec!["$defs", "µ/"]))]
#[case::common_types("common_types.json#/$defs/Child", Some(vec!["$defs", "Child"]))]
#[case::whole_document("#", Some(vec![]))]
#[case::empty_token("#/", Some(vec![""]))]
#[case::undefined_escape("#/$defs/a~2b", None)]
#[case::undefined_escape_below_the_entry("#/$defs/A/x~", None)]
#[case::encoded_undefined_escape("#/$defs/a%7E2b", None)]
#[case::unsupported("other.json#/x", None)]
#[case::anchor("#A", None)]
#[case::not_utf8("#/components/%FF", None)]
fn ref_pointer_of_reference_decodes_each_token_once(
    #[case] reference: &str,
    #[case] expected: Option<Vec<&str>>,
) {
    let expected = expected.map(|tokens| tokens.into_iter().map(str::to_owned).collect());
    assert_that!(tokens(reference)).is_equal_to(expected);
}

#[rstest]
#[case::plain("#/$defs/Child", 1, true)]
#[case::plain_from_common_types("common_types.json#/$defs/Child", 1, true)]
#[case::tilde_escaped("#/$defs/a~1b", 1, false)]
#[case::tilde_zero_escaped("#/$defs/a~0b", 1, false)]
#[case::beside_a_tilde_escape("#/$defs/a~1b", 0, true)]
#[case::percent_escaped("#/$defs/%C2%B5", 1, false)]
#[case::empty_token("#/", 0, true)]
fn ref_pointer_of_token_borrows_unless_it_was_decoded(
    #[case] reference: &str,
    #[case] index: usize,
    #[case] expected: bool,
) {
    assert_that!(borrows(reference, index)).is_equal_to(expected);
}

#[rstest]
#[case("#/$defs/Child", Some(("$defs", "Child")))]
#[case("#/components/Card/properties/child", Some(("components", "Card")))]
#[case("#/functions/a~1b", Some(("functions", "a/b")))]
#[case("#", None)]
#[case("#/theme/x", None)]
#[case("#/$defs", None)]
fn ref_pointer_entry_names_the_entry_it_falls_in(
    #[case] reference: &str,
    #[case] expected: Option<(&str, &str)>,
) {
    let pointer = RefPointer::of(reference).unwrap();
    assert_that!(pointer.entry()).is_equal_to(expected);
}

#[rstest]
#[case::whole_document("#", Some(json!({"a/b": [1, {"~": 2}], "": 3})))]
#[case::escaped_key("#/a~1b/1/~0", Some(json!(2)))]
#[case::empty_key("#/", Some(json!(3)))]
#[case::index("#/a~1b/0", Some(json!(1)))]
#[case::index_past_the_end("#/a~1b/2", None)]
#[case::leading_zero("#/a~1b/01", None)]
#[case::plus_sign("#/a~1b/+1", None)]
#[case::digits_on_an_object_are_a_key("#/0", None)]
#[case::key_into_a_scalar("#/a~1b/0/x", None)]
#[case::missing_key("#/x", None)]
fn ref_pointer_resolve_names_the_node_rfc_6901_names(
    #[case] reference: &str,
    #[case] expected: Option<Value>,
) {
    let root = json!({"a/b": [1, {"~": 2}], "": 3});
    let pointer = RefPointer::of(reference).unwrap();
    assert_that!(pointer.resolve(&root).cloned()).is_equal_to(expected);
}

#[rstest]
#[case("#")]
#[case("#/")]
#[case("#/x")]
#[case("#/0")]
#[case("#/a~1b/0")]
#[case("#/a~1b/1/~0")]
#[case("#/a~1b/01")]
#[case("#/a~1b/00")]
#[case("#/a~1b/+1")]
#[case("#/a~1b/-1")]
#[case("#/a~1b/1a")]
#[case("#/a~1b/1e0")]
#[case("#/a~1b/%D9%A1")]
#[case("#/a~1b/18446744073709551616")]
#[case("#/a~1b/0/x")]
fn ref_pointer_resolve_any_pointer_agrees_with_serde_json(#[case] reference: &str) {
    let root = json!({"a/b": [1, {"~": 2}], "": 3});
    let pointer = RefPointer::of(reference).unwrap();
    let decoded = pointer_of(reference).unwrap();
    assert_that!(pointer.resolve(&root)).is_equal_to(root.pointer(&decoded));
}

/// The catalog the `locate` tests resolve in.
fn located_root() -> Value {
    json!({
        "$defs": {"a/b": {"x": [1, {"y": 2}]}},
        "components": {"C": {"properties": {"a": {"type": "string"}}}},
        "functions": {"f": {"returnType": "string"}},
        "theme": {"t": 3}
    })
}

/// What `locate` finds for `reference` in `root`: section, name, entry and target.
fn located(root: &Value, reference: &str) -> Option<(&'static str, String, Value, Value)> {
    let pointer = RefPointer::of(reference).unwrap();
    pointer.locate(root).map(|found| {
        (
            found.section,
            found.name.to_owned(),
            found.entry.clone(),
            found.target.clone(),
        )
    })
}

#[rstest]
#[case::whole_entry("#/$defs/a~1b", "$defs", "a/b", json!({"x": [1, {"y": 2}]}))]
#[case::inside_entry("#/$defs/a%7E1b/x/1/y", "$defs", "a/b", json!(2))]
#[case::component_property(
    "#/components/C/properties/a",
    "components",
    "C",
    json!({"type": "string"})
)]
#[case::function("#/functions/f/returnType", "functions", "f", json!("string"))]
fn ref_pointer_locate_names_entry_and_target(
    #[case] reference: &str,
    #[case] section: &str,
    #[case] name: &str,
    #[case] target: Value,
) {
    let root = located_root();
    let entry = root[section][name].clone();
    let expected = Some((section, name.to_owned(), entry, target));
    assert_that!(located(&root, reference)).is_equal_to(expected);
}

#[rstest]
#[case::missing_entry("#/$defs/Missing")]
#[case::missing_node_inside("#/$defs/a~1b/z")]
#[case::index_past_the_end("#/$defs/a~1b/x/2")]
#[case::outside_any_entry("#/theme/t")]
#[case::section_only("#/$defs")]
#[case::other_section("#/functions/C")]
#[case::whole_document("#")]
fn ref_pointer_locate_unresolved_is_none(#[case] reference: &str) {
    assert_that!(located(&located_root(), reference)).is_none();
}

#[rstest]
#[case::array_section(json!({"$defs": [{}]}), "#/$defs/0")]
#[case::array_root(json!([{"$defs": {}}]), "#/$defs/S")]
fn ref_pointer_locate_non_object_container_is_none(#[case] root: Value, #[case] reference: &str) {
    assert_that!(located(&root, reference)).is_none();
}

/// `locate` agrees with `entry`, `resolve` and a key lookup, the three it replaces in `limits`.
#[rstest]
#[case("#/$defs/a~1b")]
#[case("#/$defs/a%7E1b/x/1/y")]
#[case("#/$defs/a~1b/x/01")]
#[case("#/$defs/a~1b/x/2")]
#[case("#/$defs/Missing")]
#[case("#/components/C/properties/a")]
#[case("#/functions/f/returnType")]
#[case("#/functions/C")]
#[case("#/theme/t")]
#[case("#/$defs")]
#[case("#")]
fn ref_pointer_locate_any_pointer_agrees_with_entry_and_resolve(#[case] reference: &str) {
    let root = located_root();
    let pointer = RefPointer::of(reference).unwrap();
    let separate = pointer.entry().and_then(|(section, name)| {
        let (name, entry) = root.get(section)?.as_object()?.get_key_value(name)?;
        Some((section, name.as_str(), entry, pointer.resolve(&root)?))
    });
    let located = pointer
        .locate(&root)
        .map(|found| (found.section, found.name, found.entry, found.target));
    assert_that!(located).is_equal_to(separate);
}

#[test]
fn ref_pointer_locate_name_borrows_the_catalog_key() {
    let root = located_root();
    let pointer = RefPointer::of("#/$defs/a~1b").unwrap();
    let name = pointer.locate(&root).map(|found| found.name.as_ptr());
    let key = root["$defs"]
        .as_object()
        .and_then(|defs| defs.get_key_value("a/b"))
        .map(|(key, _)| key);
    assert_that!(name).is_some();
    assert_that!(name).is_equal_to(key.map(|key| key.as_ptr()));
}
