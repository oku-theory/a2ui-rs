//! The basic catalog, registered and evaluated, shared by the function unit tests.

use a2ui_runtime::{
    Catalog, CatalogSet, DataModel, Error, Evaluation, Evaluator, FunctionRegistry, Scope,
};
use serde_json::{Value, json};

use crate::{BasicOptions, CATALOG_ID, CATALOG_JSON, functions};

/// `{"@call": name, "args": args}` evaluated on a surface whose default is the basic catalog.
///
/// The call runs inside a user activation, with `options` registered.
pub(crate) fn evaluate_with(options: BasicOptions, name: &str, args: Value) -> Evaluation {
    evaluate_on(options, Some(CATALOG_ID), name, None, args)
}

/// Call `name`, naming catalog `own` if any, on a surface whose default is `default`.
pub(crate) fn evaluate_in(
    default: Option<&str>,
    name: &str,
    own: Option<&str>,
    args: Value,
) -> Evaluation {
    evaluate_on(BasicOptions::default(), default, name, own, args)
}

/// Call `name` with `options` registered, in catalog `own` or the surface's `default`.
fn evaluate_on(
    options: BasicOptions,
    default: Option<&str>,
    name: &str,
    own: Option<&str>,
    args: Value,
) -> Evaluation {
    let mut catalogs = CatalogSet::new();
    catalogs
        .insert(Catalog::parse(CATALOG_JSON, None, None).unwrap())
        .unwrap();
    let mut registry = FunctionRegistry::new();
    functions(&mut registry, options).unwrap();
    let data = DataModel::default();
    let scope = Scope::new("s1", default, &data).with_user_activation(true);
    let mut call = json!({"@call": name, "args": args});
    if let Some(own) = own {
        call["catalogId"] = json!(own);
    }
    Evaluator::new(&catalogs, &registry).evaluate(&call, &scope)
}

/// The value of basic function `name` on `args`, or the first error it raised.
pub(crate) fn call(name: &str, args: Value) -> Result<Value, Error> {
    first_error(evaluate_with(BasicOptions::default(), name, args))
}

/// `evaluation`'s value, or its first error.
pub(crate) fn first_error(evaluation: Evaluation) -> Result<Value, Error> {
    match evaluation.errors.into_iter().next() {
        Some(error) => Err(error),
        None => Ok(evaluation.value),
    }
}
