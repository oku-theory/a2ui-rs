use a2ui_runtime::Category;
use serde_json::{Value, json};
use speculoos::prelude::*;

use super::*;

#[test]
fn catalog_published_document_loads_with_its_id() {
    assert_that!(catalog().unwrap().id()).is_equal_to(CATALOG_ID);
}

#[test]
fn catalog_eighteen_basic_components_are_kept() {
    assert_that!(catalog().unwrap().components().count()).is_equal_to(18);
}

#[test]
fn catalog_fourteen_basic_functions_are_kept() {
    assert_that!(catalog().unwrap().functions().count()).is_equal_to(14);
}

fn open_url_button(args: Value) -> Value {
    json!({
        "id": "b1", "component": "Button", "child": "t1",
        "action": {"functionCall": {"@call": "openUrl", "args": args}}
    })
}

#[test]
fn catalog_button_calling_open_url_is_valid() {
    let basic = catalog().unwrap();
    let button = open_url_button(json!({"url": "https://example.com"}));
    assert_that!(basic.validate_component(&button, CATALOG_ID, &basic)).is_ok();
}

#[test]
fn catalog_open_url_extra_argument_is_invalid() {
    let basic = catalog().unwrap();
    let button = open_url_button(json!({"url": "https://example.com", "target": "_blank"}));
    let err = basic
        .validate_component(&button, CATALOG_ID, &basic)
        .unwrap_err();
    assert_that!(err.category()).is_equal_to(Category::Validation);
    assert_that!(err.to_string()).contains("'target' was unexpected");
}

#[test]
fn catalog_text_field_without_value_is_invalid() {
    let basic = catalog().unwrap();
    let field = json!({"id": "f1", "component": "TextField", "label": "Email"});
    let err = basic
        .validate_component(&field, CATALOG_ID, &basic)
        .unwrap_err();
    assert_that!(err.to_string()).contains("\"value\" is a required property");
}

#[test]
fn catalog_validation_schema_properties_are_within_shapes() {
    let basic = catalog().unwrap();
    let outside: Vec<String> = basic
        .components()
        .flat_map(|def| {
            let flat = &basic.validation_schema()["components"][def.name()]["properties"];
            let names: Vec<String> = flat.as_object().unwrap().keys().cloned().collect();
            names
                .into_iter()
                .filter(|name| name != "id" && name != "component")
                .filter(|name| !def.shape().properties().contains(name))
                .map(|name| format!("{}.{name}", def.name()))
                .collect::<Vec<_>>()
        })
        .collect();
    assert_that!(outside).is_empty();
}
