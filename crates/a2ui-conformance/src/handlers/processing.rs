//! The `process_messages` action: batches through one `MessageProcessor`, step by step.

use a2ui_runtime::{Error, MessageProcessor, ValidationRules};
use serde_json::{Map, Value};

use crate::case::Case;
use crate::compare::expect_error;
use crate::handlers::{catalogs, surfaces};
use crate::keys::{assert_case_keys, assert_known_keys};
use crate::manifest::record_aliased;

const CASE_KEYS: [&str; 12] = [
    "protocolVersion",
    "catalogPaths",
    "catalogs",
    "catalog",
    "messages",
    "payload",
    "steps",
    "strictMode",
    "validationConfig",
    "expect",
    "expectError",
    "expect_error",
];

const STEP_KEYS: [&str; 6] = [
    "messages",
    "payload",
    "validationConfig",
    "expect",
    "expectError",
    "expect_error",
];

const RULE_KEYS: [&str; 4] = [
    "allowOrphanComponents",
    "allowDanglingReferences",
    "allowMissingRoot",
    "allowUnknownElements",
];

/// One call: a batch, the rules replacing the processor's for it, and its expectations.
struct Step<'a> {
    batch: &'a Value,
    rules: Option<ValidationRules>,
    expect: Option<&'a Value>,
    expect_error: Option<&'a Value>,
}

/// Runs a `process_messages` case on one processor built from the case's catalogs.
///
/// The case's `messages` or `payload` is one call, else each step is one. `strictMode: false`
/// makes the processor permissive; a `validationConfig` (case or step) sets each rule it names,
/// the others strict. A step without `expectError` must be accepted. An error no step expects
/// ends the run and is matched against the case's `expectError`; the case's `expect` is checked
/// last. A case where nothing is expected fails.
pub fn run(case: &Case) -> Result<(), String> {
    run_checked(case, true)
}

/// Runs a `validate` case: [`run`], where a case expecting nothing must have every step
/// accepted, since its acceptance is what the case checks.
pub fn validate(case: &Case) -> Result<(), String> {
    run_checked(case, false)
}

/// [`run`]; a case where nothing is expected fails only when `expectations_needed`.
fn run_checked(case: &Case, expectations_needed: bool) -> Result<(), String> {
    assert_case_keys(case, &CASE_KEYS)?;
    let at = case.at();
    let steps = steps(case, &at)?;
    let case_error = expected_error(&case.body, &at)?;
    let case_expect = case.get("expect");
    let anything_expected = case_error.is_some()
        || case_expect.is_some()
        || steps
            .iter()
            .any(|step| step.expect.is_some() || step.expect_error.is_some());
    if expectations_needed && !anything_expected {
        return Err(format!(
            "{at}: a case needs an `expect` or an `expectError`"
        ));
    }
    let (processor, unexpected) = drive(case, &steps, case_error.is_some())?;
    match (case_error, unexpected) {
        (Some(expected), Some(err)) => expect_error(&err, expected, &at)?,
        (Some(_), None) => return Err(format!("{at}: every step processed, expected an error")),
        (None, _) => {}
    }
    match case_expect {
        Some(expected) => surfaces::check(&processor, expected, &at),
        None => Ok(()),
    }
}

/// The processor after every call of `case`, each refusal matched against its step's.
///
/// For the report actions: the case's keys are checked by the caller; any refusal a step does
/// not expect fails.
pub fn replay(case: &Case) -> Result<MessageProcessor, String> {
    let steps = steps(case, &case.at())?;
    drive(case, &steps, false).map(|(processor, _)| processor)
}

/// Runs `steps` on a processor built from the case's catalogs and rules.
///
/// A step's refusal is matched against its `expectError` and its `expect` checked. A refusal
/// no step expects fails, unless `tolerated`: then it ends the run and is returned.
fn drive(
    case: &Case,
    steps: &[Step<'_>],
    tolerated: bool,
) -> Result<(MessageProcessor, Option<Error>), String> {
    let at = case.at();
    let loaded = catalogs::load(case)?;
    record_aliased(case, &loaded.aliased)?;
    let mut processor = MessageProcessor::new(loaded.set).with_rules(case_rules(case, &at)?);
    for (index, step) in steps.iter().enumerate() {
        let context = format!("{at} step {index}");
        let result = match step.rules {
            Some(rules) => processor.handle_batch_with(step.batch, rules),
            None => processor.handle_batch(step.batch),
        };
        match (result, step.expect_error) {
            (Err(err), Some(expected)) => expect_error(&err, expected, &context)?,
            (Ok(_), Some(_)) => return Err(format!("{context}: processed, expected an error")),
            (Err(err), None) if tolerated => return Ok((processor, Some(err))),
            (Err(err), None) => return Err(format!("{context}: refused: {err}")),
            (Ok(_), None) => {}
        }
        if let Some(expected) = step.expect {
            surfaces::check(&processor, expected, &context)?;
        }
    }
    Ok((processor, None))
}

/// The case's calls: its own batch, or one per step.
fn steps<'a>(case: &'a Case, at: &str) -> Result<Vec<Step<'a>>, String> {
    let Some(steps) = case.get("steps") else {
        let batch = batch(&case.body, at)?;
        return Ok(vec![Step {
            batch,
            rules: None,
            expect: None,
            expect_error: None,
        }]);
    };
    if case.get("messages").is_some() || case.get("payload").is_some() {
        return Err(format!("{at}: a case has `steps` or messages, not both"));
    }
    let steps = steps
        .as_array()
        .ok_or_else(|| format!("{at}: `steps` must be a list"))?;
    steps
        .iter()
        .enumerate()
        .map(|(index, step)| {
            let here = format!("{at} step {index}");
            let step = step
                .as_object()
                .ok_or_else(|| format!("{here}: a step must be a mapping"))?;
            assert_known_keys(step, &STEP_KEYS, &here)?;
            Ok(Step {
                batch: batch(step, &here)?,
                rules: step
                    .get("validationConfig")
                    .map(|config| rules(config, &here))
                    .transpose()?,
                expect: step.get("expect"),
                expect_error: expected_error(step, &here)?,
            })
        })
        .collect()
}

/// The one batch of `holder`: `messages` or `payload`.
fn batch<'a>(holder: &'a Map<String, Value>, at: &str) -> Result<&'a Value, String> {
    match (holder.get("messages"), holder.get("payload")) {
        (Some(batch), None) | (None, Some(batch)) => Ok(batch),
        _ => Err(format!(
            "{at}: exactly one of `messages` and `payload` is needed"
        )),
    }
}

/// `expectError` or `expect_error`, at most one of them.
fn expected_error<'a>(
    holder: &'a Map<String, Value>,
    at: &str,
) -> Result<Option<&'a Value>, String> {
    match (holder.get("expectError"), holder.get("expect_error")) {
        (Some(_), Some(_)) => Err(format!("{at}: `expectError` and `expect_error` both set")),
        (one, other) => Ok(one.or(other)),
    }
}

/// The processor's rules: the case's `validationConfig`, else `strictMode` (strict by default).
fn case_rules(case: &Case, at: &str) -> Result<ValidationRules, String> {
    let strict = match case.get("strictMode") {
        None => None,
        Some(Value::Bool(strict)) => Some(*strict),
        Some(other) => return Err(format!("{at}: `strictMode` must be a boolean, got {other}")),
    };
    match (case.get("validationConfig"), strict) {
        (Some(_), Some(_)) => Err(format!(
            "{at}: `validationConfig` and `strictMode` both set"
        )),
        (Some(config), None) => rules(config, at),
        (None, Some(false)) => Ok(ValidationRules::permissive()),
        (None, Some(true) | None) => Ok(ValidationRules::strict()),
    }
}

/// A `validationConfig`: each rule it names, the others strict.
fn rules(config: &Value, at: &str) -> Result<ValidationRules, String> {
    let config = config
        .as_object()
        .ok_or_else(|| format!("{at}: `validationConfig` must be a mapping"))?;
    assert_known_keys(config, &RULE_KEYS, at)?;
    let flag = |key: &str| match config.get(key) {
        None => Ok(false),
        Some(Value::Bool(allow)) => Ok(*allow),
        Some(other) => Err(format!("{at}: `{key}` must be a boolean, got {other}")),
    };
    Ok(ValidationRules::strict()
        .allow_orphan_components(flag("allowOrphanComponents")?)
        .allow_dangling_references(flag("allowDanglingReferences")?)
        .allow_missing_root(flag("allowMissingRoot")?)
        .allow_unknown_elements(flag("allowUnknownElements")?))
}

#[cfg(test)]
#[path = "processing_tests.rs"]
mod tests;
