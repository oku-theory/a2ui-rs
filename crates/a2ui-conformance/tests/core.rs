//! Runs every case of `conformance/core/*.yaml` as one test.

use std::sync::Arc;

use a2ui_conformance::{Case, Manifest, Skips, Status, handlers, load_suite, upstream_root};
use libtest_mimic::{Arguments, Failed, Trial};

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
    let crate_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let skips = Skips::parse(
        &std::fs::read_to_string(crate_dir.join("skips.toml")).map_err(|err| err.to_string())?,
    )?;
    let manifest = Manifest::from_env()?.map(Arc::new);
    let cases = all_cases(&root)?;
    let unused = skips.unused(&cases);
    let mut trials = vec![Trial::test(
        "skips_toml_entries_all_match_a_case",
        move || {
            if unused.is_empty() {
                Ok(())
            } else {
                Err(format!("unused skip entries: {unused:?}").into())
            }
        },
    )];
    for case in cases {
        trials.push(trial(case, &skips, manifest.clone())?);
    }
    Ok(trials)
}

fn all_cases(root: &std::path::Path) -> Result<Vec<Case>, String> {
    let dir = root.join("conformance/core");
    let mut paths: Vec<_> = std::fs::read_dir(&dir)
        .map_err(|err| format!("{}: {err}", dir.display()))?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.extension().is_some_and(|ext| ext == "yaml"))
        .collect();
    paths.sort();
    let mut cases = Vec::new();
    for path in paths {
        let file = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| format!("non-UTF-8 file name {}", path.display()))?;
        cases.extend(load_suite(&path, &format!("core/{file}"))?);
    }
    Ok(cases)
}

fn trial(case: Case, skips: &Skips, manifest: Option<Arc<Manifest>>) -> Result<Trial, String> {
    let name = format!("{}::{}", case.suite.trim_end_matches(".yaml"), case.name);
    if let Some(reason) = skips.reason(&case) {
        if let Some(manifest) = &manifest {
            manifest.record(&case.suite, &case.name, Status::Skipped, Some(&reason))?;
        }
        return Ok(Trial::test(name, || Ok(())).with_ignored_flag(true));
    }
    Ok(Trial::test(name, move || {
        let outcome = match handlers::handler(&case.action) {
            Some(run) => run(&case),
            None => Err(format!(
                "Unknown action '{}' in {}#{}",
                case.action, case.suite, case.name
            )),
        };
        if let Some(manifest) = &manifest {
            let status = if outcome.is_ok() {
                Status::Passed
            } else {
                Status::Failed
            };
            manifest.record(&case.suite, &case.name, status, None)?;
        }
        outcome.map_err(Failed::from)
    }))
}
