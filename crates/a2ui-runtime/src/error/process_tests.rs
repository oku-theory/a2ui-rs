use rstest::rstest;
use speculoos::prelude::*;

use super::*;

#[rstest]
#[case::duplicate_catalog(
    ProcessFailure::DuplicateCatalog { id: "c".into() },
    "Catalog 'c' is already registered",
    (Category::Catalog, ErrorCode::CatalogError)
)]
#[case::catalog_not_found(
    ProcessFailure::CatalogNotFound { id: "missing-catalog".into() },
    "Catalog not found: missing-catalog",
    (Category::Catalog, ErrorCode::CatalogNotFound)
)]
#[case::surface_exists(
    ProcessFailure::SurfaceExists { id: "main".into() },
    "Surface 'main' already exists",
    (Category::Integrity, ErrorCode::IntegrityError)
)]
#[case::surface_not_found(
    ProcessFailure::SurfaceNotFound { id: "s".into() },
    "Surface not found for message: s",
    (Category::Integrity, ErrorCode::IntegrityError)
)]
#[case::not_yet_supported(
    ProcessFailure::NotYetSupported { action: "callRendererFunction" },
    "Action 'callRendererFunction' is not handled yet",
    (Category::Validation, ErrorCode::ValidationFailed)
)]
#[case::state_limit(
    ProcessFailure::StateLimit { what: "Surface", limit: 64 },
    "Surface limit of 64 exceeded",
    (Category::Validation, ErrorCode::ValidationFailed)
)]
fn process_failure_display_and_class_match_upstream(
    #[case] failure: ProcessFailure,
    #[case] text: &str,
    #[case] class: (Category, ErrorCode),
) {
    assert_that!(failure.to_string()).is_equal_to(text.to_owned());
    assert_that!((failure.category(), failure.code())).is_equal_to(class);
}

#[test]
fn process_failure_quotes_agent_ids() {
    let failure = ProcessFailure::SurfaceExists { id: "a\nb".into() };
    assert_that!(failure.to_string()).is_equal_to("Surface 'a\\nb' already exists".to_owned());
}
