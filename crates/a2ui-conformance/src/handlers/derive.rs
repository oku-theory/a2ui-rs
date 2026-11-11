//! The `derive` action: derive a catalog, then check its entries and its `to_json` document.

use a2ui_runtime::{Catalog, Derivation, DeriveEntry};
use serde_json::{Map, Value};

use crate::case::Case;
use crate::compare::expect_error;
use crate::handlers::catalog_checks::entries;
use crate::handlers::catalog_input::{document, load};
use crate::keys::{assert_case_keys, assert_known_keys};

const EXPECT_KEYS: [&str; 7] = [
    "catalogId",
    "protocolVersion",
    "components",
    "functions",
    "theme",
    "instructions",
    "absentKeys",
];

/// Loads the case's catalog, derives from it with `args`, and checks the result.
pub fn run(case: &Case) -> Result<(), String> {
    assert_case_keys(
        case,
        &[
            "catalog",
            "catalogPath",
            "catalogId",
            "protocolVersion",
            "args",
            "expect",
            "expectError",
        ],
    )?;
    let at = case.at();
    let input = document(case)?;
    let source = load(case, &input)?.map_err(|err| format!("{at}: load failed: {err}"))?;
    let derivation = derivation(case.get("args"), &at)?;
    match (case.get("expectError"), source.derive(&derivation)) {
        (Some(expected), Err(err)) => expect_error(&err, expected, &at),
        (Some(_), Ok(_)) => Err(format!("{at}: derived, expected an error")),
        (None, Err(err)) => Err(format!("{at}: derive failed: {err}")),
        (None, Ok(derived)) => match case.get("expect") {
            Some(Value::Object(expect)) => check(expect, &derived, &at),
            _ => Err(format!("{at}: a derive case needs an `expect` mapping")),
        },
    }
}

fn derivation(args: Option<&Value>, at: &str) -> Result<Derivation, String> {
    let args = args
        .and_then(Value::as_object)
        .ok_or_else(|| format!("{at}: `args` must be a mapping"))?;
    assert_known_keys(
        args,
        &["catalogId", "components", "functions", "instructions"],
        at,
    )?;
    let id = args
        .get("catalogId")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{at}: `args.catalogId` must be a string"))?;
    let mut derivation = Derivation::new(id);
    if let Some(list) = args.get("components") {
        derivation = derivation.components(derive_entries(list, at)?);
    }
    if let Some(list) = args.get("functions") {
        derivation = derivation.functions(derive_entries(list, at)?);
    }
    match args.get("instructions") {
        None => {}
        Some(Value::String(text)) => derivation = derivation.instructions(text.as_str()),
        Some(_) => return Err(format!("{at}: `args.instructions` must be a string")),
    }
    Ok(derivation)
}

fn derive_entries(list: &Value, at: &str) -> Result<Vec<DeriveEntry>, String> {
    let items = list
        .as_array()
        .ok_or_else(|| format!("{at}: a derive list must be a list"))?;
    items
        .iter()
        .map(|item| match item {
            Value::String(name) => Ok(DeriveEntry::Keep(name.clone())),
            Value::Object(entry) => match entry.iter().collect::<Vec<_>>().as_slice() {
                [(name, schema)] => Ok(DeriveEntry::Define((*name).clone(), (*schema).clone())),
                _ => Err(format!(
                    "{at}: a derive entry map has exactly one key, got {item}"
                )),
            },
            other => Err(format!(
                "{at}: a derive entry is a name or a one-key map, got {other}"
            )),
        })
        .collect()
}

fn check(expect: &Map<String, Value>, derived: &Catalog, at: &str) -> Result<(), String> {
    assert_known_keys(expect, &EXPECT_KEYS, at)?;
    entries(expect, derived, at)?;
    if let Some(expected) = expect.get("instructions")
        && expected.as_str() != derived.instructions()
    {
        let actual = derived.instructions();
        return Err(format!(
            "{at}: instructions {actual:?}, expected {expected}"
        ));
    }
    let Some(absent) = expect.get("absentKeys") else {
        return Ok(());
    };
    let json = derived.to_json();
    let keys = absent
        .as_array()
        .ok_or_else(|| format!("{at}: `absentKeys` must be a list"))?;
    match keys
        .iter()
        .find(|key| key.as_str().is_none_or(|key| json.get(key).is_some()))
    {
        Some(key) => Err(format!("{at}: key {key} is present in to_json")),
        None => Ok(()),
    }
}
