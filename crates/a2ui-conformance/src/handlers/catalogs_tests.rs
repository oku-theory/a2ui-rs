use serde_json::json;
use speculoos::prelude::*;

use super::*;
use crate::case::parse_suite;

fn case(yaml: &str) -> Case {
    parse_suite("core/x.yaml", yaml).unwrap().remove(0)
}

fn ids(loaded: &Loaded) -> Vec<String> {
    loaded
        .set
        .iter()
        .map(|catalog| catalog.id().to_owned())
        .collect()
}

const BASE: &str = r#"
  catalog:
    catalogId: base
    protocolVersion: "1.0"
    components:
      Box: {type: object}
"#;

fn with_base(rest: &str) -> Case {
    case(&format!(
        "- name: t\n  action: process_messages\n{BASE}{rest}"
    ))
}

#[test]
fn stub_becomes_open_with_basic_slots() {
    let loaded = load(&case(
        r#"
- name: t
  action: process_messages
  catalogs:
    - {catalogId: c, protocolVersion: "1.0"}
  messages:
    - version: "1.0"
      createSurface:
        surfaceId: s1
        catalogId: c
        components:
          - {id: root, component: Card}
"#,
    ))
    .unwrap();
    let card = loaded.set.get("c").unwrap().component("Card").unwrap();
    assert_that!(card.schema()).is_equal_to(&json!({"type": "object", "properties": {
        "child": {"$ref": "common_types.json#/$defs/ComponentId"},
        "children": {"$ref": "common_types.json#/$defs/ChildList"}}}));
}

#[test]
fn alias_registers_missing_ids() {
    let loaded = load(&with_base(
        r#"
  messages:
    - version: "1.0"
      createSurface:
        surfaceId: s1
        catalogId: other
        components:
          - {id: root, component: Box, catalogId: third}
"#,
    ))
    .unwrap();
    assert_that!(loaded.aliased).is_equal_to(vec!["other".to_owned(), "third".to_owned()]);
    assert_that!(loaded.set.get("third").and_then(|c| c.component("Box"))).is_some();
}

#[test]
fn alias_keeps_a_registered_id() {
    let loaded = load(&with_base(
        r#"
  messages:
    - {version: "1.0", createSurface: {surfaceId: s1, catalogId: base}}
"#,
    ))
    .unwrap();
    assert_that!(loaded.aliased).is_empty();
}

#[test]
fn alias_never_registers_empty_id() {
    let loaded = load(&with_base(
        r#"
  messages:
    - {version: "1.0", createSurface: {surfaceId: s1, catalogId: ""}}
"#,
    ))
    .unwrap();
    assert_that!(loaded.aliased).is_empty();
    assert_that!(ids(&loaded)).is_equal_to(vec!["base".to_owned()]);
}

#[test]
fn no_alias_when_a_catalog_error_is_expected() {
    let loaded = load(&with_base(
        r#"
  steps:
    - messages:
        - {version: "1.0", createSurface: {surfaceId: s1, catalogId: other}}
      expectError: {category: CatalogError}
"#,
    ))
    .unwrap();
    assert_that!(loaded.aliased).is_empty();
}

#[test]
fn non_v10_document_is_not_registered() {
    let loaded = load(&case(
        r#"
- name: t
  action: process_messages
  catalogs:
    - {catalogId: old, protocolVersion: v0.9, components: {Box: {type: object}}}
    - {catalogId: unversioned, components: {Box: {type: object}}}
"#,
    ))
    .unwrap();
    assert_that!(ids(&loaded)).is_empty();
}

#[test]
fn idless_componentless_catalog_is_ignored() {
    let loaded = load(&case(
        r#"
- name: t
  action: process_messages
  catalog: {protocolVersion: "1.0"}
"#,
    ))
    .unwrap();
    assert_that!(ids(&loaded)).is_empty();
}

#[test]
fn catalog_schema_key_is_unwrapped() {
    let loaded = load(&case(
        r#"
- name: t
  action: process_messages
  catalog:
    protocolVersion: "1.0"
    catalogSchema:
      catalogId: custom
      components:
        Header: {allowedParents: [Column]}
"#,
    ))
    .unwrap();
    let header = loaded.set.get("custom").and_then(|c| c.component("Header"));
    assert_that!(
        header
            .and_then(|h| h.allowed_parents())
            .map(<[String]>::to_vec)
    )
    .is_equal_to(Some(vec!["Column".to_owned()]));
}

#[test]
fn alias_without_a_registered_catalog_derives_the_basic_one() {
    let loaded = load(&case(
        r#"
- name: t
  action: process_messages
  messages:
    - {version: "1.0", createSurface: {surfaceId: s1, catalogId: basic}}
"#,
    ))
    .unwrap();
    let basic = loaded.set.get("basic").unwrap();
    assert_that!(basic.component("Column")).is_some();
}
