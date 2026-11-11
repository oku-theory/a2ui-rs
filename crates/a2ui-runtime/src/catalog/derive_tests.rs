use rstest::rstest;
use serde_json::json;
use speculoos::prelude::*;

use crate::{Catalog, Category, Derivation, DeriveEntry};

fn source() -> Catalog {
    let document = json!({
        "$id": "https://example.com/source.json",
        "title": "Source",
        "description": "The source.",
        "instructions": "Write components of source.",
        "catalogId": "source",
        "protocolVersion": "1.0",
        "components": {
            "Text": {"type": "object", "properties": {"text": {"type": "string"}}},
            "Image": {"type": "object", "properties": {"url": {"type": "string"}}},
            "Button": {"type": "object", "properties": {"label": {"type": "string"}}}
        },
        "functions": {
            "upper": {"type": "object", "properties": {"returnType": {"const": "string"}}},
            "lower": {"type": "object", "properties": {"returnType": {"const": "string"}}}
        },
        "theme": {"type": "object", "properties": {"primaryColor": {"type": "string"}}},
        "$defs": {
            "anyComponent": {
                "oneOf": [
                    {"$ref": "#/components/Text"},
                    {"$ref": "#/components/Image"},
                    {"$ref": "#/components/Button"}
                ],
                "discriminator": {"propertyName": "component"}
            }
        }
    });
    Catalog::from_json(&document, None, None).unwrap()
}

fn keep(name: &str) -> DeriveEntry {
    DeriveEntry::Keep(name.to_owned())
}

fn component_names(catalog: &Catalog) -> Vec<&str> {
    catalog.components().map(|def| def.name()).collect()
}

#[test]
fn derive_named_entries_are_kept_and_others_dropped() {
    let derived = source()
        .derive(&Derivation::new("derived").components(vec![keep("Text"), keep("Button")]))
        .unwrap();
    assert_that!(component_names(&derived)).is_equal_to(vec!["Button", "Text"]);
}

#[test]
fn derive_kept_entry_keeps_its_schema() {
    let derived = source()
        .derive(&Derivation::new("derived").components(vec![keep("Text")]))
        .unwrap();
    assert_that!(derived.component("Text").unwrap().schema().clone())
        .is_equal_to(json!({"type": "object", "properties": {"text": {"type": "string"}}}));
}

#[test]
fn derive_new_entry_is_added() {
    let card = json!({"type": "object", "properties": {"title": {"type": "string"}}});
    let derived = source()
        .derive(&Derivation::new("derived").components(vec![
            keep("Text"),
            DeriveEntry::Define("Card".to_owned(), card),
        ]))
        .unwrap();
    assert_that!(component_names(&derived)).is_equal_to(vec!["Card", "Text"]);
}

#[test]
fn derive_new_entry_replaces_a_source_entry() {
    let button = json!({"type": "object", "properties": {"primary": {"type": "boolean"}}});
    let derived = source()
        .derive(
            &Derivation::new("derived").components(vec![DeriveEntry::Define(
                "Button".to_owned(),
                button.clone(),
            )]),
        )
        .unwrap();
    assert_that!(derived.component("Button").unwrap().schema().clone()).is_equal_to(button);
}

#[test]
fn derive_omitted_list_keeps_every_entry() {
    let derived = source()
        .derive(&Derivation::new("derived").components(vec![keep("Text")]))
        .unwrap();
    let names: Vec<&str> = derived.functions().map(|def| def.name()).collect();
    assert_that!(names).is_equal_to(vec!["lower", "upper"]);
}

#[test]
fn derive_source_description_keys_are_dropped() {
    let json = source()
        .derive(&Derivation::new("derived"))
        .unwrap()
        .to_json();
    let present: Vec<&str> = ["$id", "title", "description", "instructions"]
        .into_iter()
        .filter(|key| json.get(*key).is_some())
        .collect();
    assert_that!(present).is_empty();
}

#[test]
fn derive_new_id_is_used() {
    let derived = source().derive(&Derivation::new("derived")).unwrap();
    assert_that!(derived.id()).is_equal_to("derived");
}

#[test]
fn derive_source_version_is_kept() {
    let derived = source().derive(&Derivation::new("derived")).unwrap();
    assert_that!(derived.protocol_version()).is_equal_to("1.0");
}

#[test]
fn derive_source_theme_is_kept() {
    let derived = source().derive(&Derivation::new("derived")).unwrap();
    assert_that!(derived.theme()).is_equal_to(Some(&json!({
        "type": "object",
        "properties": {"primaryColor": {"type": "string"}}
    })));
}

#[test]
fn derive_to_json_writes_the_new_catalog_id() {
    let json = source()
        .derive(&Derivation::new("derived"))
        .unwrap()
        .to_json();
    assert_that!(json["catalogId"].clone()).is_equal_to(json!("derived"));
}

#[test]
fn derive_new_entry_is_checked_by_its_own_validator() {
    let card = json!({"type": "object", "properties": {"title": {"type": "string"}}});
    let derived = source()
        .derive(
            &Derivation::new("derived")
                .components(vec![DeriveEntry::Define("Card".to_owned(), card)]),
        )
        .unwrap();
    let component = json!({"id": "c1", "component": "Card", "title": 5});
    let err = derived
        .validate_component(&component, "derived", &derived)
        .unwrap_err();
    assert_that!(err.to_string()).contains("5 is not of type \"string\"");
}

#[test]
fn derive_instructions_replace_the_source_ones() {
    let derived = source()
        .derive(&Derivation::new("derived").instructions("Write components of derived."))
        .unwrap();
    assert_that!(derived.instructions()).is_equal_to(Some("Write components of derived."));
}

#[test]
fn derive_given_list_regenerates_the_source_union() {
    let derived = source()
        .derive(&Derivation::new("derived").components(vec![
            keep("Text"),
            DeriveEntry::Define("Card_ℓ".to_owned(), json!({"type": "object"})),
        ]))
        .unwrap();
    assert_that!(derived.to_json()["$defs"]["anyComponent"].clone()).is_equal_to(json!({
        "oneOf": [{"$ref": "#/components/Card_%E2%84%93"}, {"$ref": "#/components/Text"}],
        "discriminator": {"propertyName": "component"}
    }));
}

#[test]
fn derive_function_restriction_regenerates_any_function_with_the_picked_names() {
    let mut document = source().to_json();
    document["$defs"]["anyFunction"] = json!({
        "oneOf": [{"$ref": "#/functions/lower"}, {"$ref": "#/functions/upper"}]
    });
    let source = Catalog::from_json(&document, None, None).unwrap();
    let derived = source
        .derive(&Derivation::new("derived").functions(vec![keep("upper")]))
        .unwrap();
    assert_that!(derived.to_json()["$defs"]["anyFunction"].clone())
        .is_equal_to(json!({"oneOf": [{"$ref": "#/functions/upper"}]}));
}

#[test]
fn derive_unknown_component_is_catalog_error_naming_it() {
    let err = source()
        .derive(&Derivation::new("derived").components(vec![keep("Text"), keep("Slider")]))
        .unwrap_err();
    assert_that!(err.category()).is_equal_to(Category::Catalog);
    assert_that!(err.to_string()).contains("has no component 'Slider'");
}

#[test]
fn derive_unknown_function_is_catalog_error_naming_it() {
    let err = source()
        .derive(&Derivation::new("derived").functions(vec![keep("capitalize")]))
        .unwrap_err();
    assert_that!(err.to_string()).contains("has no function 'capitalize'");
}

#[rstest]
#[case::kept_twice(vec![keep("Text"), keep("Text")])]
#[case::kept_and_defined(vec![keep("Text"), DeriveEntry::Define("Text".to_owned(), json!({}))])]
#[case::defined_twice(vec![
    DeriveEntry::Define("Text".to_owned(), json!({})),
    DeriveEntry::Define("Text".to_owned(), json!({"type": "object"})),
])]
fn derive_duplicate_entry_is_catalog_error_naming_it(#[case] entries: Vec<DeriveEntry>) {
    let err = source()
        .derive(&Derivation::new("derived").components(entries))
        .unwrap_err();
    assert_that!(err.category()).is_equal_to(Category::Catalog);
    assert_that!(err.to_string())
        .is_equal_to("Derivation lists component 'Text' more than once".to_owned());
}

#[test]
fn derive_invalid_new_entry_name_is_catalog_error() {
    let err = source()
        .derive(
            &Derivation::new("derived")
                .components(vec![DeriveEntry::Define("Bad-Name".to_owned(), json!({}))]),
        )
        .unwrap_err();
    assert_that!(err.to_string()).contains("Invalid UAX #31 component identifier: 'Bad-Name'");
}

#[test]
fn derive_new_entry_with_an_external_ref_is_catalog_error() {
    let entry = json!({"$ref": "https://evil.example/x.json"});
    let err = source()
        .derive(
            &Derivation::new("derived")
                .components(vec![DeriveEntry::Define("Evil".to_owned(), entry)]),
        )
        .unwrap_err();
    assert_that!(err.category()).is_equal_to(Category::Catalog);
    assert_that!(err.to_string()).contains("Unsupported external $ref");
}

#[test]
fn derive_dropping_a_component_the_theme_names_is_catalog_error() {
    let document = json!({
        "catalogId": "source",
        "protocolVersion": "1.0",
        "components": {"Text": {}, "Image": {}},
        "theme": {"properties": {"hero": {"$ref": "#/components/Image"}}}
    });
    let source = Catalog::from_json(&document, None, None).unwrap();
    let err = source
        .derive(&Derivation::new("derived").components(vec![keep("Text")]))
        .unwrap_err();
    assert_that!(err.category()).is_equal_to(Category::Catalog);
}
