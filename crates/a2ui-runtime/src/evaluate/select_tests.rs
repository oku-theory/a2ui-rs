use rstest::rstest;
use speculoos::prelude::*;

use super::*;
use crate::{Category, ErrorCode};

/// A selected id, or an error's class and message.
#[derive(Debug, PartialEq)]
enum Selected {
    Catalog(String),
    Refused(Category, ErrorCode, String),
}

impl From<Result<&str, Error>> for Selected {
    fn from(result: Result<&str, Error>) -> Self {
        match result {
            Ok(id) => Self::Catalog(id.to_owned()),
            Err(error) => Self::Refused(error.category(), error.code(), error.to_string()),
        }
    }
}

/// The selection of `id`.
fn selected(id: &str) -> Selected {
    Selected::Catalog(id.to_owned())
}

/// A refusal of class `(category, code)` reading `text`.
fn refused(category: Category, code: ErrorCode, text: &str) -> Selected {
    Selected::Refused(category, code, text.to_owned())
}

/// `main_surface`'s missing default, for function `add`.
fn no_default() -> Selected {
    refused(
        Category::Catalog,
        ErrorCode::CatalogNotFound,
        "Surface 'main_surface' has no default catalogId for function 'add'",
    )
}

#[rstest]
#[case::own_wins(Some("chart"), Some("basic"), selected("chart"))]
#[case::default_fallback(None, Some("basic"), selected("basic"))]
#[case::own_without_default(Some("chart"), None, selected("chart"))]
#[case::empty_own_is_an_id(Some(""), Some("basic"), selected(""))]
#[case::neither(None, None, no_default())]
fn select_catalog_rows(
    #[case] own: Option<&str>,
    #[case] default: Option<&str>,
    #[case] expected: Selected,
) {
    let result = select_catalog(own, default, "main_surface", "function", "add");
    assert_that!(Selected::from(result)).is_equal_to(expected);
}

#[rstest]
#[case::own_listed(Some("chart"), Some("basic"), selected("chart"))]
#[case::default_listed(None, Some("basic"), selected("basic"))]
#[case::unlisted_default(
    None,
    Some("other"),
    refused(
        Category::Catalog,
        ErrorCode::CatalogError,
        "Catalog 'other' is not supported by surface 'main_surface'."
    )
)]
#[case::neither(None, None, no_default())]
fn select_supported_rows(
    #[case] own: Option<&str>,
    #[case] default: Option<&str>,
    #[case] expected: Selected,
) {
    let supported = ["basic".to_owned(), "chart".to_owned()];
    let result = select_supported(own, default, &supported, "main_surface", "function", "add");
    assert_that!(Selected::from(result)).is_equal_to(expected);
}

#[test]
fn select_supported_unlisted_is_unsupported_catalog() {
    let supported = ["basic".to_owned()];

    let result = select_supported(
        Some("unregistered_vendor_catalog"),
        Some("basic"),
        &supported,
        "main_surface",
        "component",
        "c1",
    );

    assert_that!(Selected::from(result)).is_equal_to(refused(
        Category::Catalog,
        ErrorCode::CatalogError,
        "Catalog 'unregistered_vendor_catalog' is not supported by surface 'main_surface'.",
    ));
}
