use rstest::rstest;
use serde_json::{Value, json};
use speculoos::prelude::*;

use super::*;

/// The tokens of `reference`'s pointer, or `None` when it has none.
fn tokens(reference: &str) -> Option<Vec<String>> {
    RefPointer::of(reference).map(|pointer| pointer.tokens().to_vec())
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
