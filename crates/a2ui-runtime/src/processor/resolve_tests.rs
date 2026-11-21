use rstest::rstest;
use serde_json::{Value, json};
use speculoos::prelude::*;

use super::*;
use crate::catalog::test_support::catalog_set;
use crate::{Category, ErrorCode};

/// What resolving a component gives: a catalog id, or an error's class, text and path.
#[derive(Debug, PartialEq)]
enum Resolved {
    Catalog(String),
    Refused((Category, ErrorCode), String, Option<String>),
}

/// A refusal of class `class` with message `text` at `path`.
fn refused(class: (Category, ErrorCode), text: &str, path: Option<&str>) -> Resolved {
    Resolved::Refused(class, text.to_owned(), path.map(str::to_owned))
}

const NOT_FOUND: (Category, ErrorCode) = (Category::Catalog, ErrorCode::CatalogNotFound);
const INVALID: (Category, ErrorCode) = (Category::Validation, ErrorCode::ValidationFailed);

#[rstest]
#[case::own_id(json!({"catalogId": "b"}), Some("a"), Resolved::Catalog("b".to_owned()))]
#[case::surface_default(json!({}), Some("a"), Resolved::Catalog("a".to_owned()))]
#[case::neither(
    json!({}),
    None,
    refused(NOT_FOUND, "Surface 's1' has no default catalogId for component 'c1'", None)
)]
#[case::unknown(
    json!({"catalogId": "missing-catalog"}),
    Some("a"),
    refused(NOT_FOUND, "Catalog not found: missing-catalog", None)
)]
#[case::empty(json!({"catalogId": ""}), Some("a"), refused(NOT_FOUND, "Catalog not found: ", None))]
#[case::number(
    json!({"catalogId": 3}),
    Some("a"),
    refused(INVALID, "Invalid component: catalogId must be a string", Some("/catalogId"))
)]
fn component_catalog_resolves_or_errors(
    #[case] mut component: Value,
    #[case] surface_default: Option<&str>,
    #[case] expected: Resolved,
) {
    component["id"] = json!("c1");
    component["component"] = json!("Text");
    let set = catalog_set();
    let map = component.as_object().unwrap();

    let resolved = match component_catalog(map, "c1", "s1", surface_default, &set) {
        Ok(catalog) => Resolved::Catalog(catalog.id().to_owned()),
        Err(error) => refused(
            (error.category(), error.code()),
            &error.to_string(),
            error.path(),
        ),
    };

    assert_that!(resolved).is_equal_to(expected);
}
