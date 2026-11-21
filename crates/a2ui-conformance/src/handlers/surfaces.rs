//! Expectations on the surfaces a `MessageProcessor` holds (`expect.surfaces`).

use a2ui_runtime::{MessageProcessor, Pointer, Surface};
use serde_json::{Map, Value};

use crate::compare::json_eq;
use crate::keys::assert_known_keys;

const SURFACE_KEYS: [&str; 5] = [
    "exists",
    "sendDataModel",
    "metadata",
    "dataModel",
    "components",
];

/// Checks `expected`, an `expect` mapping holding `surfaces`, against `processor`.
///
/// An `expect` without a non-empty `surfaces` mapping checks nothing and fails.
/// Per surface: `exists`, `sendDataModel`, `metadata`, `dataModel` (the whole root) and
/// `components`, a list (the same count, each matched by `id`) or a map by id (the listed ids
/// only). A listed component key compares whole; an expected `null` matches an absent key.
/// Any other key fails. `context` names the step in failure text.
pub fn check(processor: &MessageProcessor, expected: &Value, context: &str) -> Result<(), String> {
    let expected = expected
        .as_object()
        .ok_or_else(|| format!("{context}: `expect` must be a mapping"))?;
    assert_known_keys(expected, &["surfaces"], context)?;
    let surfaces = expected
        .get("surfaces")
        .and_then(Value::as_object)
        .filter(|surfaces| !surfaces.is_empty())
        .ok_or_else(|| format!("{context}: `expect` checks nothing: needs a `surfaces` mapping"))?;
    for (id, want) in surfaces {
        let here = format!("{context}: surface '{id}'");
        let want = want
            .as_object()
            .ok_or_else(|| format!("{here}: the expectation must be a mapping"))?;
        assert_known_keys(want, &SURFACE_KEYS, &here)?;
        surface(processor.surface(id), want, &here)?;
    }
    Ok(())
}

fn surface(actual: Option<&Surface>, want: &Map<String, Value>, here: &str) -> Result<(), String> {
    let exists = match want.get("exists") {
        None => true,
        Some(Value::Bool(exists)) => *exists,
        Some(other) => return Err(format!("{here}: `exists` must be a boolean, got {other}")),
    };
    let actual = match (actual, exists) {
        (Some(actual), true) => actual,
        (None, false) if want.len() == 1 => return Ok(()),
        (None, false) => return Err(format!("{here}: an absent surface has nothing to check")),
        (Some(_), false) => return Err(format!("{here}: exists, expected absent")),
        (None, true) => return Err(format!("{here}: absent, expected to exist")),
    };
    if let Some(sends) = want.get("sendDataModel") {
        let have = Value::Bool(actual.sends_data_model());
        same(&have, sends, "sendDataModel", here)?;
    }
    if let Some(metadata) = want.get("metadata") {
        let have = actual
            .metadata()
            .cloned()
            .map_or(Value::Null, Value::Object);
        same(&have, metadata, "metadata", here)?;
    }
    if let Some(model) = want.get("dataModel") {
        let have = actual
            .data_model()
            .get(&Pointer::root())
            .ok_or_else(|| format!("{here}: the data model has no root"))?;
        same(have, model, "dataModel", here)?;
    }
    match want.get("components") {
        None => Ok(()),
        Some(Value::Array(list)) => component_list(actual, list, here),
        Some(Value::Object(by_id)) => {
            for (id, fields) in by_id {
                component(actual, id, fields, here)?;
            }
            Ok(())
        }
        Some(other) => Err(format!(
            "{here}: `components` is a list or a map, got {other}"
        )),
    }
}

fn component_list(actual: &Surface, list: &[Value], here: &str) -> Result<(), String> {
    let count = actual.components().len();
    if count != list.len() {
        return Err(format!(
            "{here}: {count} components, expected {}",
            list.len()
        ));
    }
    for (index, fields) in list.iter().enumerate() {
        let id = fields
            .get("id")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("{here}: components[{index}] needs a string `id`"))?;
        component(actual, id, fields, here)?;
    }
    Ok(())
}

fn component(actual: &Surface, id: &str, fields: &Value, here: &str) -> Result<(), String> {
    let fields = fields
        .as_object()
        .ok_or_else(|| format!("{here}: component '{id}' expectation must be a mapping"))?;
    let have = actual
        .unescaped_component(id)
        .ok_or_else(|| format!("{here}: no component '{id}'"))?;
    for (key, want) in fields {
        let matches = match have.get(key) {
            None => want.is_null(),
            Some(value) => json_eq(value, want),
        };
        if !matches {
            let value = have.get(key).map_or("absent".to_owned(), Value::to_string);
            return Err(format!(
                "{here}: component '{id}' `{key}` is {value}, expected {want}"
            ));
        }
    }
    Ok(())
}

fn same(have: &Value, want: &Value, key: &str, here: &str) -> Result<(), String> {
    if json_eq(have, want) {
        Ok(())
    } else {
        Err(format!("{here}: `{key}` is {have}, expected {want}"))
    }
}

#[cfg(test)]
#[path = "surfaces_tests.rs"]
mod tests;
