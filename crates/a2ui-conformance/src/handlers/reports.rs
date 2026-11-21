//! The renderer report actions: `get_renderer_capabilities` and `get_renderer_data_model`.

use a2ui_runtime::MessageProcessor;
use serde_json::{Map, Value};

use crate::case::Case;
use crate::compare::json_eq;
use crate::handlers::{catalogs, processing};
use crate::keys::{assert_case_keys, assert_known_keys};
use crate::manifest::record_aliased;

const CAPABILITY_KEYS: [&str; 6] = [
    "protocolVersion",
    "catalogPaths",
    "catalogs",
    "catalog",
    "args",
    "expect",
];

const DATA_MODEL_KEYS: [&str; 9] = [
    "protocolVersion",
    "catalogPaths",
    "catalogs",
    "catalog",
    "messages",
    "payload",
    "steps",
    "args",
    "expect",
];

/// The protocol version this runtime reports on, without a leading `v`.
const REPORTED_VERSION: &str = "1.0";

/// Runs a `get_renderer_capabilities` case on a processor holding the case's catalogs.
///
/// `args.includeInlineCatalogs` (default `false`) picks the report; `args.version` must be 1.0.
/// Catalog ids, inline catalogs (by `catalogId`) and listed component or function names (by
/// `name`) compare sorted, since registration order is not part of the report.
pub fn capabilities(case: &Case) -> Result<(), String> {
    assert_case_keys(case, &CAPABILITY_KEYS)?;
    let at = case.at();
    let args = args(case, &["includeInlineCatalogs", "version"])?;
    let version = version(args, &at)?;
    if version != REPORTED_VERSION {
        return Err(format!(
            "{at}: capabilities requested for '{version}'; a2ui-rs reports 1.0 only"
        ));
    }
    let inline = match args.get("includeInlineCatalogs") {
        None => false,
        Some(Value::Bool(inline)) => *inline,
        Some(other) => {
            return Err(format!(
                "{at}: `includeInlineCatalogs` must be a boolean, got {other}"
            ));
        }
    };
    let expected = case
        .get("expect")
        .ok_or_else(|| format!("{at}: a capabilities case needs an `expect`"))?;
    let loaded = catalogs::load(case)?;
    record_aliased(case, &loaded.aliased)?;
    let processor = MessageProcessor::new(loaded.set);
    let report = if inline {
        processor.capabilities_with_inline_catalogs()
    } else {
        processor.capabilities()
    };
    if json_eq(&sorted(&report)?, &sorted(expected)?) {
        Ok(())
    } else {
        Err(format!(
            "{at}: capabilities\n  expected: {expected}\n  reported: {report}"
        ))
    }
}

/// Runs a `get_renderer_data_model` case after replaying its messages.
///
/// `expect: null` matches no report. `args.version` 1.0 reports the v1.0 surfaces; an older
/// version (`0.<minor>`, optionally `.<patch>`) reports nothing, since every surface of this
/// runtime is a v1.0 one; any other version fails the case.
pub fn data_model(case: &Case) -> Result<(), String> {
    assert_case_keys(case, &DATA_MODEL_KEYS)?;
    let at = case.at();
    let version = version(args(case, &["version"])?, &at)?;
    let reported = if version == REPORTED_VERSION {
        true
    } else if is_older_version(&version) {
        false
    } else {
        return Err(format!(
            "{at}: data model requested for unknown protocol version '{version}'"
        ));
    };
    let expected = case
        .get("expect")
        .ok_or_else(|| format!("{at}: a data model case needs an `expect` (null for none)"))?;
    let processor = processing::replay(case)?;
    let report = if reported {
        processor.data_model_report()
    } else {
        None
    };
    match (&report, expected) {
        (None, Value::Null) => Ok(()),
        (Some(report), expected) if json_eq(report, expected) => Ok(()),
        _ => Err(format!(
            "{at}: data model\n  expected: {expected}\n  reported: {}",
            report.unwrap_or(Value::Null)
        )),
    }
}

/// The case's `args` mapping, holding only `allowed` keys.
fn args<'a>(case: &'a Case, allowed: &[&str]) -> Result<&'a Map<String, Value>, String> {
    let at = case.at();
    let args = case
        .get("args")
        .and_then(Value::as_object)
        .ok_or_else(|| format!("{at}: a report case needs an `args` mapping"))?;
    assert_known_keys(args, allowed, &format!("{at} args"))?;
    Ok(args)
}

/// The requested `args.version`, without a leading `v`.
fn version(args: &Map<String, Value>, at: &str) -> Result<String, String> {
    let version = args
        .get("version")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{at}: `args.version` must be a string"))?;
    Ok(version.strip_prefix('v').unwrap_or(version).to_owned())
}

/// Whether `version` (without its `v`) is a pre-1.0 one: `0.<digits>`, then `.<digits>` or
/// nothing.
fn is_older_version(version: &str) -> bool {
    let mut parts = version.split('.');
    let numbers = |part: &str| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit());
    parts.next() == Some("0")
        && parts.next().is_some_and(numbers)
        && parts.next().is_none_or(numbers)
        && parts.next().is_none()
}

/// A capabilities report with its lists sorted: ids, inline catalogs, component and function
/// lists.
fn sorted(report: &Value) -> Result<Value, String> {
    let mut report = report.clone();
    let Value::Object(versions) = &mut report else {
        return Ok(report);
    };
    for body in versions.values_mut() {
        if let Some(Value::Array(ids)) = body.get_mut("supportedCatalogIds") {
            ids.sort_by_key(|a| a.to_string());
        }
        if let Some(Value::Array(inline)) = body.get_mut("inlineCatalogs") {
            for catalog in inline.iter_mut() {
                for key in ["components", "functions"] {
                    if let Some(Value::Array(entries)) = catalog.get_mut(key) {
                        sort_by_field(entries, "name")?;
                    }
                }
            }
            sort_by_field(inline, "catalogId")?;
        }
    }
    Ok(report)
}

/// Sorts the report list `entries` by the text of each one's `field`, which each must hold.
fn sort_by_field(entries: &mut [Value], field: &str) -> Result<(), String> {
    if let Some(entry) = entries.iter().find(|entry| entry.get(field).is_none()) {
        return Err(format!(
            "capabilities: a report entry without `{field}`: {entry}"
        ));
    }
    entries.sort_by_key(|entry| entry.get(field).map(Value::to_string));
    Ok(())
}

#[cfg(test)]
#[path = "reports_tests.rs"]
mod tests;
