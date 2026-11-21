use serde_json::{Value, json};
use speculoos::prelude::*;

use super::*;
use crate::catalog::test_support::{load, v10};
use crate::{Category, ErrorCode};

/// Catalog `id` declaring functions `f` and `g`.
fn catalog(id: &str) -> Catalog {
    let schema = json!({"type": "object"});
    load(v10(json!({
        "catalogId": id,
        "functions": {"f": schema.clone(), "g": schema}
    })))
    .unwrap()
}

/// Calls the implementation bound to `name` in `catalog_id`.
fn call(registry: &FunctionRegistry, catalog_id: &str, name: &str) -> Option<Value> {
    let implementation = registry.implementation(catalog_id, name)?;
    let mut context = CallContext::new(false, "s", catalog_id);
    Some(implementation(&Args::new(), &mut context).unwrap())
}

#[test]
fn register_declared_function_is_found() {
    let mut registry = FunctionRegistry::new();
    registry
        .register(&catalog("c"), "f", |_, _| Ok(json!(1)))
        .unwrap();

    assert_that!(call(&registry, "c", "f")).is_equal_to(Some(json!(1)));
    assert_that!(call(&registry, "c", "g")).is_none();
}

#[test]
fn register_twice_replaces_binding() {
    let mut registry = FunctionRegistry::new();
    let catalog = catalog("c");
    registry
        .register(&catalog, "f", |_, _| Ok(json!(1)))
        .unwrap();
    registry
        .register(&catalog, "f", |_, _| Ok(json!(2)))
        .unwrap();

    assert_that!(call(&registry, "c", "f")).is_equal_to(Some(json!(2)));
}

#[test]
fn register_undeclared_name_is_a_catalog_error() {
    let mut registry = FunctionRegistry::new();

    let error = registry
        .register(&catalog("c"), "nope", |_, _| Ok(Value::Null))
        .unwrap_err();

    assert_that!(error.category()).is_equal_to(Category::Catalog);
    assert_that!(error.code()).is_equal_to(ErrorCode::CatalogError);
    assert_that!(error.to_string())
        .is_equal_to("Function 'nope' is not declared by catalog 'c'".to_owned());
    assert_that!(registry.implementation("c", "nope").is_none()).is_true();
}

#[test]
fn register_at_name_is_refused() {
    // The catalog loader already refuses `@` names, so this pins the undeclared-name path.
    let mut registry = FunctionRegistry::new();

    let error = registry
        .register(&catalog("c"), "@index", |_, _| Ok(Value::Null))
        .unwrap_err();

    assert_that!(error.code()).is_equal_to(ErrorCode::CatalogError);
    assert_that!(registry.implementation("c", "@index").is_none()).is_true();
}

#[test]
fn implementation_other_catalog_id_is_none() {
    let mut registry = FunctionRegistry::new();
    registry
        .register(&catalog("c"), "f", |_, _| Ok(Value::Null))
        .unwrap();

    assert_that!(registry.implementation("d", "f").is_none()).is_true();
}

#[test]
fn debug_lists_bound_pairs() {
    let mut registry = FunctionRegistry::new();
    registry
        .register(&catalog("c"), "g", |_, _| Ok(Value::Null))
        .unwrap();
    registry
        .register(&catalog("c"), "f", |_, _| Ok(Value::Null))
        .unwrap();

    assert_that!(format!("{registry:?}")).is_equal_to(r#"[("c", "f"), ("c", "g")]"#.to_owned());
}
