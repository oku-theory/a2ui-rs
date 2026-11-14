use rstest::rstest;
use serde_json::{Value, json};
use speculoos::prelude::*;

use super::*;

#[test]
fn resolve_percent_encoded_ref_finds_the_entry() {
    let root = json!({"components": {"Component_ℓ": {"type": "object"}}});
    let found = resolve(&root, "#/components/Component_%E2%84%93");
    assert_that!(found).is_equal_to(Some(&json!({"type": "object"})));
}

#[test]
fn first_not_a_schema_path_holds_raw_keys() {
    let root = json!({"$defs": {"a/b": {"$ref": "#/$defs/Missing"}}});
    let path = vec![
        Segment::Key("$defs"),
        Segment::Key("a/b"),
        Segment::Key("$ref"),
    ];
    assert_that!(first_not_a_schema(&root)).is_equal_to(Some((
        "#/$defs/Missing",
        path,
        NotASchema::Missing,
    )));
}

#[rstest]
#[case::missing(json!({}), NotASchema::Missing)]
#[case::string(json!({"t": "T"}), NotASchema::String)]
#[case::number(json!({"t": 1}), NotASchema::Number)]
#[case::array(json!({"t": []}), NotASchema::Array)]
#[case::null(json!({"t": null}), NotASchema::Null)]
fn first_not_a_schema_target_kind_is_reported(#[case] target: Value, #[case] expected: NotASchema) {
    let root = json!({"$defs": {"X": target, "R": {"$ref": "#/$defs/X/t"}}});
    let found = first_not_a_schema(&root).map(|(_, _, found)| found);
    assert_that!(found).is_equal_to(Some(expected));
}
