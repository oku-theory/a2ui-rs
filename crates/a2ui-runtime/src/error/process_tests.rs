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
#[case::duplicate_component(
    ProcessFailure::DuplicateComponent { id: "c1".into() },
    "Duplicate component ID 'c1' in one message",
    (Category::Integrity, ErrorCode::IntegrityError)
)]
#[case::no_default_catalog(
    ProcessFailure::NoDefaultCatalog { surface: "s".into(), what: "component", name: "c1".into() },
    "Surface 's' has no default catalogId for component 'c1'",
    (Category::Catalog, ErrorCode::CatalogNotFound)
)]
#[case::self_reference(
    ProcessFailure::SelfReference { id: "root".into() },
    "Self-reference detected: component 'root' lists itself as a child",
    (Category::Recursion, ErrorCode::RecursionLimitExceeded)
)]
#[case::dangling_reference(
    ProcessFailure::DanglingReference {
        parent: "stack".into(),
        child: "missing".into(),
        field: "slots[1]".into(),
    },
    "Dangling reference: component 'stack' references non-existent component 'missing' in \
     field 'slots[1]'",
    (Category::Integrity, ErrorCode::IntegrityError)
)]
#[case::circular_reference(
    ProcessFailure::CircularReference { ids: vec!["a".into(), "b".into()], more: false },
    "Circular reference detected: circular component reference 'a' -> 'b' -> 'a'",
    (Category::Recursion, ErrorCode::RecursionLimitExceeded)
)]
#[case::circular_reference_past_the_ids(
    ProcessFailure::CircularReference { ids: vec!["a".into(), "b".into()], more: true },
    "Circular reference detected: circular component reference 'a' -> 'b' -> …",
    (Category::Recursion, ErrorCode::RecursionLimitExceeded)
)]
#[case::missing_root(
    ProcessFailure::MissingRoot,
    "Missing root component 'root'",
    (Category::Validation, ErrorCode::ValidationFailed)
)]
#[case::unreachable(
    ProcessFailure::Unreachable { id: "o".into() },
    "Component 'o' is not reachable from 'root'",
    (Category::Integrity, ErrorCode::IntegrityError)
)]
#[case::component_depth(
    ProcessFailure::ComponentDepth { id: "c50".into(), depth: 51 },
    "Global recursion limit exceeded: component 'c50' is 51 levels below 'root'",
    (Category::Recursion, ErrorCode::RecursionLimitExceeded)
)]
#[case::schema_violations(
    ProcessFailure::SchemaViolations {
        id: "b1".into(),
        kind: "Button".into(),
        count: 2,
        first: "\"child\" is a required property at ''".into(),
    },
    "Component 'b1' (Button) has 2 schema violations; first: \"child\" is a required property \
     at ''",
    (Category::Validation, ErrorCode::ValidationFailed)
)]
#[case::schema_violations_past_the_details(
    ProcessFailure::SchemaViolations {
        id: "m1".into(),
        kind: "Many".into(),
        count: 17,
        first: "x".into(),
    },
    "Component 'm1' (Many) has more than 16 schema violations; first: x",
    (Category::Validation, ErrorCode::ValidationFailed)
)]
#[case::escaped_key_collision(
    ProcessFailure::EscapedKeyCollision { escaped: "@@path".into(), plain: "@path".into() },
    "Key '@@path' collides with '@path' after unescaping",
    (Category::Validation, ErrorCode::ValidationFailed)
)]
#[case::unallowed_parent(
    ProcessFailure::UnallowedParent {
        child: "h".into(),
        child_type: "Header".into(),
        parent: "root".into(),
        parent_type: "Row".into(),
        allowed: Allowed::new(&["Column".to_owned(), "Box".to_owned()]),
    },
    "Component 'h' (Header) cannot be placed under parent 'root' (Row). Allowed parents: \
     ['Column', 'Box'].",
    (Category::Validation, ErrorCode::UnallowedParent)
)]
#[case::unallowed_child(
    ProcessFailure::UnallowedChild {
        parent: "root".into(),
        parent_type: "Box".into(),
        child: "v1".into(),
        child_type: "Video".into(),
        allowed: Allowed::new(&["Text".to_owned()]),
    },
    "Container 'root' (Box) cannot contain child 'v1' (Video). Allowed children: ['Text'].",
    (Category::Validation, ErrorCode::UnallowedChild)
)]
#[case::unallowed_past_the_names(
    ProcessFailure::UnallowedChild {
        parent: "p".into(),
        parent_type: "Box".into(),
        child: "c".into(),
        child_type: "Video".into(),
        allowed: Allowed::new(&(0..9).map(|n| format!("T{n}")).collect::<Vec<_>>()),
    },
    "Container 'p' (Box) cannot contain child 'c' (Video). Allowed children: ['T0', 'T1', \
     'T2', 'T3', 'T4', 'T5', 'T6', 'T7', …].",
    (Category::Validation, ErrorCode::UnallowedChild)
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
