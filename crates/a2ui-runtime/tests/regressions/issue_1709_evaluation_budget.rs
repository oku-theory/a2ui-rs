//! t-1709: a `formatString` template read from the data model could fan out exponentially.
//!
//! Entry `/t{i}` interpolates `formatString` twice on `/t{i+1}`; 30 levels took about twenty
//! minutes and 2 GiB before evaluation had a step budget.

use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use a2ui_runtime::{
    Catalog, CatalogSet, DataModel, Error, ErrorCode, Evaluation, Evaluator, FunctionRegistry,
    Scope, coerce,
};
use serde_json::{Map, Value, json};
use speculoos::prelude::*;

/// Levels of the fan-out: 2^30 `formatString` calls without a budget.
const LEVELS: usize = 30;

/// How long the bounded evaluation may take, far above its few milliseconds.
const PROMPTLY: Duration = Duration::from_secs(10);

/// Catalog `c` declaring `formatString`, bound to an implementation that interpolates `value`.
fn catalog_and_registry() -> Result<(CatalogSet, FunctionRegistry), Error> {
    let document = json!({
        "catalogId": "c", "protocolVersion": "1.0",
        "functions": {"formatString": {"type": "object"}}
    });
    let catalog = Catalog::from_json(&document, None, None)?;
    let mut registry = FunctionRegistry::new();
    registry.register(&catalog, "formatString", |args, context| {
        let template = coerce::text(args.get("value").unwrap_or(&Value::Null));
        Ok(Value::String(context.interpolate(&template)?))
    })?;
    let mut set = CatalogSet::new();
    set.insert(catalog)?;
    Ok((set, registry))
}

/// `/t0` to `/t{LEVELS - 1}`, each calling `formatString` twice on the next; the last is "xy".
fn fan_out() -> DataModel {
    let mut entries = Map::new();
    for level in 0..LEVELS {
        let next = level + 1;
        let call = format!("${{formatString(value: ${{/t{next}}})}}");
        entries.insert(format!("t{level}"), Value::String(format!("{call}{call}")));
    }
    entries.insert(format!("t{LEVELS}"), json!("xy"));
    DataModel::new(Value::Object(entries))
}

/// `formatString` of `/t0`, evaluated on another thread.
fn evaluate_fan_out() -> mpsc::Receiver<Result<Evaluation, Error>> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let evaluation = catalog_and_registry().map(|(catalogs, functions)| {
            let data = fan_out();
            let scope = Scope::new("s1", Some("c"), &data);
            let call = json!({"@call": "formatString", "args": {"value": {"@path": "/t0"}}});
            Evaluator::new(&catalogs, &functions).evaluate(&call, &scope)
        });
        // The receiver is gone only when the test already failed on its timeout.
        let _ = sender.send(evaluation);
    });
    receiver
}

#[test]
fn evaluation_budget_fan_out_stops_with_recursion_limit() {
    let evaluation = evaluate_fan_out().recv_timeout(PROMPTLY).unwrap().unwrap();

    assert_that!(evaluation.value).is_equal_to(Value::Null);
    let codes: Vec<ErrorCode> = evaluation.errors.iter().map(|error| error.code()).collect();
    assert_that!(codes).is_equal_to(vec![ErrorCode::RecursionLimitExceeded]);
}
