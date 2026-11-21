//! Catalogs, implementations and assertions shared by the evaluation unit tests.

use serde_json::{Value, json};
use speculoos::prelude::*;

use super::{Evaluation, Evaluator, Scope};
use crate::catalog::test_support::{load, v10};
use crate::{Catalog, CatalogSet, Error, FunctionRegistry, coerce};

/// Catalog `id` declaring `echo` (called by `echo_callers`), `site`, `fmt`, the `agentOnly`
/// `secret`, the `requiresUserActivation` `open` and the schema-only `ghost`.
fn declaring(id: &str, echo_callers: &str) -> Catalog {
    let plain = json!({"type": "object"});
    load(v10(json!({"catalogId": id, "functions": {
        "echo": {"type": "object", "allowedCallers": echo_callers},
        "site": plain, "fmt": plain, "ghost": plain,
        "secret": {"type": "object", "allowedCallers": "agentOnly"},
        "open": {"type": "object", "requiresUserActivation": true}
    }})))
    .unwrap()
}

/// [`declaring`] `id` with a renderer-callable `echo`.
pub(crate) fn catalog(id: &str) -> Catalog {
    declaring(id, "rendererOnly")
}

/// [`declaring`] `id` with an `agentOnly` `echo`.
pub(crate) fn agent_only_echo(id: &str) -> Catalog {
    declaring(id, "agentOnly")
}

/// A set holding catalogs `c` and `d`.
pub(crate) fn catalogs() -> CatalogSet {
    let mut set = CatalogSet::new();
    set.insert(catalog("c")).unwrap();
    set.insert(catalog("d")).unwrap();
    set
}

/// Every function but `ghost` bound on `catalog`.
///
/// `echo` gives `{"catalog": <id it ran in>, "value": <its value argument>}`, `site` the id it
/// ran in, `fmt` its `value` interpolated, `open` `"opened"` and `secret` `"secret"`.
pub(crate) fn bind(registry: &mut FunctionRegistry, catalog: &Catalog) {
    registry
        .register(catalog, "echo", |args, context| {
            let value = args.get("value").cloned().unwrap_or(Value::Null);
            Ok(json!({"catalog": context.catalog_id(), "value": value}))
        })
        .unwrap();
    registry
        .register(catalog, "site", |_, context| {
            Ok(Value::from(context.catalog_id()))
        })
        .unwrap();
    registry
        .register(catalog, "fmt", |args, context| {
            let template = coerce::text(args.get("value").unwrap_or(&Value::Null));
            Ok(Value::String(context.interpolate(&template)?))
        })
        .unwrap();
    registry
        .register(catalog, "open", |_, _| Ok(json!("opened")))
        .unwrap();
    registry
        .register(catalog, "secret", |_, _| Ok(json!("secret")))
        .unwrap();
}

/// A registry binding [`bind`]'s functions on catalogs `c` and `d`.
pub(crate) fn registry() -> FunctionRegistry {
    let mut registry = FunctionRegistry::new();
    bind(&mut registry, &catalog("c"));
    bind(&mut registry, &catalog("d"));
    registry
}

/// `value` evaluated in `scope` against [`catalogs`] and [`registry`].
pub(crate) fn evaluate(value: &Value, scope: &Scope<'_>) -> Evaluation {
    let catalogs = catalogs();
    let functions = registry();
    Evaluator::new(&catalogs, &functions).evaluate(value, scope)
}

/// The value of an evaluation that must raise no error.
pub(crate) fn clean(evaluation: Evaluation) -> Value {
    assert_that!(evaluation.errors).is_empty();
    evaluation.value
}

/// The one error of an evaluation whose value must be `null`.
pub(crate) fn failed(evaluation: Evaluation) -> Error {
    assert_that!(evaluation.value).is_equal_to(Value::Null);
    only_error(evaluation)
}

/// The one error an evaluation raised.
pub(crate) fn only_error(evaluation: Evaluation) -> Error {
    let mut errors = evaluation.errors;
    assert_that!(errors).has_length(1);
    errors.remove(0)
}
