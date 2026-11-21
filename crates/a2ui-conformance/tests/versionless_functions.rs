//! Replays `functions.yaml`'s versionless `evaluate_function` cases against v1.0.
//!
//! The `core` run skips them as v0.9, upstream's default version. Here each one runs through
//! `handlers::functions::run` and must pass, unless a `versionless.toml` `[[wrap]]` group names
//! it: such a case must fail as written, then pass with `expect` rewritten as `{key: expect}`,
//! pinning the v1.0 result. A group entry naming no case fails. The results are not written to
//! the manifest.

use std::collections::BTreeMap;
use std::path::Path;

use a2ui_conformance::{Case, handlers, load_suite, parse_suite, upstream_root};
use libtest_mimic::{Arguments, Failed, Trial};
use serde::Deserialize;
use serde_json::{Map, Value};

/// `versionless.toml`: the groups of cases whose v1.0 result wraps the v0.9 one.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Expected {
    wrap: Vec<Wrap>,
}

/// Cases expected under v1.0 to return `{key: expect}` where v0.9 returns `expect`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Wrap {
    key: String,
    reason: String,
    cases: Vec<String>,
}

fn main() {
    let args = Arguments::from_args();
    let trials = match trials() {
        Ok(trials) => trials,
        Err(message) => vec![Trial::test("harness_setup", move || {
            Err(Failed::from(message.clone()))
        })],
    };
    libtest_mimic::run(&args, trials).exit();
}

fn trials() -> Result<Vec<Trial>, String> {
    let root = upstream_root()?;
    let list = Path::new(env!("CARGO_MANIFEST_DIR")).join("versionless.toml");
    let text =
        std::fs::read_to_string(&list).map_err(|err| format!("{}: {err}", list.display()))?;
    let expected: Expected =
        toml::from_str(&text).map_err(|err| format!("versionless.toml: {err}"))?;
    let mut wraps = BTreeMap::new();
    for group in &expected.wrap {
        for name in &group.cases {
            if wraps.insert(name.clone(), group.clone()).is_some() {
                return Err(format!("versionless.toml: {name} is listed twice"));
            }
        }
    }
    let suite = root.join("conformance/core/functions.yaml");
    let cases: Vec<Case> = load_suite(&suite, "core/functions.yaml")?
        .into_iter()
        .filter(versionless)
        .collect();
    let unused: Vec<String> = wraps
        .keys()
        .filter(|name| !cases.iter().any(|case| &case.name == *name))
        .cloned()
        .collect();
    let mut trials = vec![
        Trial::test("versionless_toml_entries_all_match_a_case", move || {
            if unused.is_empty() {
                Ok(())
            } else {
                Err(format!("unused versionless.toml entries: {unused:?}").into())
            }
        }),
        Trial::test("wrapped_case_wrong_boolean_fails", || {
            wrapped_case_wrong_boolean_fails().map_err(Failed::from)
        }),
    ];
    for case in cases {
        let wrap = wraps.get(&case.name).cloned();
        trials.push(trial(case, wrap));
    }
    Ok(trials)
}

/// Whether `case` is an `evaluate_function` case declaring no protocol version.
fn versionless(case: &Case) -> bool {
    case.action == "evaluate_function"
        && case.get("protocolVersion").is_none()
        && case.get("catalog").is_none()
}

/// The trial of `case`, checked under `wrap` when its group names it.
fn trial(case: Case, wrap: Option<Wrap>) -> Trial {
    let name = format!("versionless/functions::{}", case.name);
    Trial::test(name, move || {
        let outcome = match &wrap {
            None => handlers::functions::run(&case),
            Some(wrap) => wrapped(&case, wrap),
        };
        outcome.map_err(Failed::from)
    })
}

/// Checks that `case` fails as written and passes with `expect` wrapped in `wrap.key`.
fn wrapped(case: &Case, wrap: &Wrap) -> Result<(), String> {
    let at = case.at();
    if handlers::functions::run(case).is_ok() {
        return Err(format!(
            "{at}: passes, but versionless.toml expects a wrapped result ({})",
            wrap.reason
        ));
    }
    let expect = case
        .get("expect")
        .cloned()
        .ok_or_else(|| format!("{at}: a wrapped case needs `expect`"))?;
    let mut rewritten = case.clone();
    let mut value = Map::new();
    value.insert(wrap.key.clone(), expect);
    rewritten
        .body
        .insert("expect".to_owned(), Value::Object(value));
    handlers::functions::run(&rewritten)
        .map_err(|message| format!("{message} (wrapped in `{}`: {})", wrap.key, wrap.reason))
}

/// A wrapped case passes on the exact v1.0 value and fails on a wrong boolean.
fn wrapped_case_wrong_boolean_fails() -> Result<(), String> {
    let wrap = Wrap {
        key: "valid".to_owned(),
        reason: "self-check".to_owned(),
        cases: Vec::new(),
    };
    let case = |expect: bool| {
        parse_suite(
            "core/x.yaml",
            &format!(
                "- {{name: t, action: evaluate_function, function: required, \
                 args: {{value: x}}, expect: {expect}}}"
            ),
        )
        .map(|mut cases| cases.remove(0))
    };
    wrapped(&case(true)?, &wrap)?;
    match wrapped(&case(false)?, &wrap) {
        Ok(()) => Err("a wrong boolean passed the wrapped check".to_owned()),
        Err(_) => Ok(()),
    }
}
