//! The `data_model` action: pointer reads, writes, deletes and observers.

use a2ui_runtime::{DataModel, Pointer, Watchers, Write};
use serde_json::{Map, Value};

use crate::case::Case;
use crate::compare::{expect_error, json_eq};
use crate::keys::{assert_case_keys, assert_known_keys};

/// Keys each op may carry; any other key would be silently ignored, so it fails.
fn op_keys(op: &str) -> Option<&'static [&'static str]> {
    match op {
        "get" => Some(&[
            "op",
            "path",
            "expect",
            "expect_absent",
            "expect_type",
            "expect_error",
        ]),
        "set" => Some(&[
            "op",
            "path",
            "value",
            "expect_error",
            "expect_notified",
            "expect_values",
        ]),
        "delete" => Some(&[
            "op",
            "path",
            "expect_error",
            "expect_notified",
            "expect_values",
        ]),
        "mutate_input" => Some(&["op", "path", "value", "expect_notified"]),
        "dispose" => Some(&["op", "expect_notified", "expect_values"]),
        _ => None,
    }
}

/// Observers keyed by the path as written in `watch`.
type Watched = Watchers<String>;

/// Runs a `data_model` case: build from `initial`, watch `watch`, run `steps`.
pub fn run(case: &Case) -> Result<(), String> {
    assert_case_keys(case, &["catalog", "initial", "watch", "steps"])?;
    // An absent `initial` is the documented empty model, not a fallback.
    let mut model = DataModel::new(case.get("initial").cloned().unwrap_or(Value::Null));
    let mut watched = watched(case.get("watch"))?;
    let mut last_input: Option<Value> = None;
    let steps = case
        .get("steps")
        .and_then(Value::as_array)
        .ok_or("a case needs `steps`")?;
    for (index, step) in steps.iter().enumerate() {
        let at = format!("{}#{} step {index}", case.suite, case.name);
        let step = step
            .as_object()
            .ok_or_else(|| format!("{at}: a step is a mapping"))?;
        run_step(&mut model, &mut watched, &mut last_input, step, &at)?;
    }
    Ok(())
}

fn run_step(
    model: &mut DataModel,
    watched: &mut Watched,
    last_input: &mut Option<Value>,
    step: &Map<String, Value>,
    at: &str,
) -> Result<(), String> {
    let op = step
        .get("op")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{at}: missing op"))?;
    let allowed = op_keys(op).ok_or_else(|| format!("{at}: Unknown op '{op}'"))?;
    assert_known_keys(step, allowed, &format!("{at} op '{op}'"))?;
    // Only `dispose` takes no path; every other op must name one.
    let path = if op == "dispose" {
        ""
    } else {
        step.get("path")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("{at}: op '{op}' needs a string `path`"))?
    };
    match op {
        "get" => {
            let read = Pointer::parse(path).map(|pointer| model.get(&pointer).cloned());
            match (read, step.get("expect_error")) {
                (Err(err), Some(expected)) => expect_error(&err, expected, at),
                (Err(err), None) => Err(format!("{at}: {err}")),
                (Ok(value), Some(_)) => Err(format!("{at}: expected an error, read {value:?}")),
                (Ok(value), None) => check_read(value.as_ref(), step, at),
            }
        }
        "set" | "delete" => {
            let value = match (op, step.get("value")) {
                ("set", Some(value)) => value.clone(),
                ("set", None) => return Err(format!("{at}: `set` needs a `value`")),
                _ => Value::Null,
            };
            let written = Pointer::parse(path).and_then(|pointer| {
                if op == "delete" {
                    return model.delete(&pointer);
                }
                *last_input = Some(value.clone());
                model.set(&pointer, value)
            });
            match (written, step.get("expect_error")) {
                (Err(err), Some(expected)) => expect_error(&err, expected, at),
                (Err(err), None) => Err(format!("{at}: {err}")),
                (Ok(_), Some(_)) => Err(format!("{at}: expected an error")),
                (Ok(write), None) => {
                    check_step(&fired(write.as_ref(), watched, model), model, step, at)
                }
            }
        }
        "mutate_input" => {
            // The model owns a copy, so changing the caller's value must notify nobody.
            let input = last_input
                .as_mut()
                .ok_or_else(|| format!("{at}: `mutate_input` needs a prior `set`"))?;
            let value = step
                .get("value")
                .ok_or_else(|| format!("{at}: `mutate_input` needs a `value`"))?;
            let target = input
                .pointer_mut(path)
                .ok_or_else(|| format!("{at}: `{path}` is not inside the last set value"))?;
            *target = value.clone();
            check_step(&[], model, step, at)
        }
        "dispose" => {
            watched.clear();
            check_step(&[], model, step, at)
        }
        _ => Err(format!("{at}: Unknown op '{op}'")),
    }
}

fn fired(write: Option<&Write>, watched: &Watched, model: &DataModel) -> Vec<String> {
    match write {
        Some(write) => watched.notified(write, model),
        None => Vec::new(),
    }
}

fn watched(watch: Option<&Value>) -> Result<Watched, String> {
    let mut watchers = Watchers::default();
    let Some(list) = watch else {
        return Ok(watchers);
    };
    for raw in list.as_array().ok_or("`watch` is a list")? {
        let raw = raw.as_str().ok_or("`watch` holds strings")?;
        let pointer = Pointer::parse(raw).map_err(|err| err.to_string())?;
        watchers.watch(raw.to_owned(), pointer);
    }
    Ok(watchers)
}

fn check_read(value: Option<&Value>, step: &Map<String, Value>, at: &str) -> Result<(), String> {
    if let Some(expected) = step.get("expect")
        && !value.is_some_and(|value| json_eq(value, expected))
    {
        return Err(format!("{at}: expected {expected}, got {value:?}"));
    }
    // `expect_absent: false` asserts presence; a non-boolean is a malformed case.
    match step.get("expect_absent") {
        None => {}
        Some(Value::Bool(absent)) if *absent != value.is_some() => {}
        Some(Value::Bool(true)) => return Err(format!("{at}: expected absent, got {value:?}")),
        Some(Value::Bool(false)) => return Err(format!("{at}: expected a value, got none")),
        Some(other) => {
            return Err(format!(
                "{at}: expect_absent must be a boolean, got {other}"
            ));
        }
    }
    match (step.get("expect_type"), value) {
        (None, _) => Ok(()),
        (Some(Value::String(kind)), Some(Value::Array(_))) if kind == "list" => Ok(()),
        (Some(Value::String(kind)), Some(Value::Object(_))) if kind == "object" => Ok(()),
        (Some(Value::String(kind)), _) => Err(format!("{at}: expected a {kind}, got {value:?}")),
        (Some(other), _) => Err(format!("{at}: expect_type must be a string, got {other}")),
    }
}

fn check_step(
    fired: &[String],
    model: &DataModel,
    step: &Map<String, Value>,
    at: &str,
) -> Result<(), String> {
    if let Some(expected) = step.get("expect_notified") {
        let fired = Value::from(fired.to_vec());
        if !json_eq(&fired, expected) {
            return Err(format!("{at}: notified {fired}, expected {expected}"));
        }
    }
    let values = match step.get("expect_values") {
        None => return Ok(()),
        Some(Value::Object(values)) => values,
        Some(other) => {
            return Err(format!(
                "{at}: expect_values must be a mapping, got {other}"
            ));
        }
    };
    for (raw, expected) in values {
        let pointer = Pointer::parse(raw).map_err(|err| format!("{at}: {err}"))?;
        let actual = model.get(&pointer);
        if !actual.is_some_and(|actual| json_eq(actual, expected)) {
            return Err(format!("{at}: {raw} holds {actual:?}, expected {expected}"));
        }
    }
    Ok(())
}
