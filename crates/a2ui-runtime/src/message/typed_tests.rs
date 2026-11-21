use serde_json::json;
use speculoos::prelude::*;

use super::*;

#[test]
fn read_a_body_the_schema_never_saw_names_the_missing_field() {
    let error = read(Action::DeleteSurface, &json!({})).err();
    assert_that!(error.map(|e| e.to_string())).is_equal_to(Some(
        "Validated message lacks a usable 'surfaceId'".to_owned(),
    ));
}

#[test]
fn read_a_body_that_is_not_an_object_names_the_action() {
    let error = read(Action::CreateSurface, &json!(1)).err();
    assert_that!(error.map(|e| e.to_string())).is_equal_to(Some(
        "Validated message lacks a usable 'createSurface'".to_owned(),
    ));
}
