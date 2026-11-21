use serde_json::json;
use speculoos::prelude::*;

use super::*;
use crate::catalog::test_support::{load, v10};
use crate::{Category, ErrorCode};

/// A loaded empty v1.0 catalog whose id is `id`.
fn catalog(id: &str) -> Catalog {
    load(v10(json!({"catalogId": id}))).unwrap()
}

#[test]
fn insert_new_id_is_found_by_get() {
    let mut set = CatalogSet::new();
    set.insert(catalog("a")).unwrap();

    assert_that!(set.get("a").map(Catalog::id)).is_equal_to(Some("a"));
    assert_that!(set.catalog("a").map(Catalog::id)).is_equal_to(Some("a"));
    assert_that!(set.get("b").is_none()).is_true();
}

#[test]
fn insert_duplicate_id_is_a_catalog_error() {
    let mut set = CatalogSet::new();
    set.insert(catalog("a")).unwrap();

    let error = set.insert(catalog("a")).unwrap_err();

    assert_that!(error.category()).is_equal_to(Category::Catalog);
    assert_that!(error.code()).is_equal_to(ErrorCode::CatalogError);
    assert_that!(error.to_string()).is_equal_to("Catalog 'a' is already registered".to_owned());
    assert_that!(set.iter().count()).is_equal_to(1);
}

#[test]
fn iter_inserted_out_of_order_lists_ids_sorted() {
    let mut set = CatalogSet::default();
    for id in ["c", "a", "b"] {
        set.insert(catalog(id)).unwrap();
    }

    let ids: Vec<&str> = set.iter().map(Catalog::id).collect();

    assert_that!(ids).is_equal_to(vec!["a", "b", "c"]);
}
