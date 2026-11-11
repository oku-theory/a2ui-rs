//! The skip list: unsupported actions, skipped cases and unsupported versions.

use std::collections::BTreeMap;

use serde::Deserialize;

use crate::case::Case;

/// Actions whose behavior does not depend on the declared protocol version.
const VERSION_NEUTRAL_ACTIONS: [&str; 3] =
    ["data_model", "resolve_path", "parse_expression_template"];

/// Skip entries read from `skips.toml`, each with its reason.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Skips {
    #[serde(default)]
    actions: BTreeMap<String, String>,
    #[serde(default)]
    cases: BTreeMap<String, String>,
    #[serde(default)]
    versions: BTreeMap<String, String>,
}

impl Skips {
    /// Parses the TOML text of `skips.toml`.
    pub fn parse(text: &str) -> Result<Self, String> {
        toml::from_str(text).map_err(|err| format!("skips.toml: {err}"))
    }

    /// Why `case` is skipped under `version`, its effective version, or `None` when it runs.
    ///
    /// A version absent from `[versions]` runs, so an unexpected one fails loudly.
    pub fn reason(&self, case: &Case, version: Option<&str>) -> Option<String> {
        if let Some(reason) = self.cases.get(&case.name) {
            return Some(reason.clone());
        }
        if let Some(reason) = self.actions.get(&case.action) {
            return Some(reason.clone());
        }
        if is_version_neutral(case) {
            return None;
        }
        self.versions.get(version?).cloned()
    }

    /// Entries that match no case, as `actions.<name>`, `cases.<name>` or `versions.<version>`.
    ///
    /// Each case comes with its effective version; a version-neutral case uses no version entry.
    pub fn unused(&self, cases: &[(&Case, Option<&str>)]) -> Vec<String> {
        let actions = self
            .actions
            .keys()
            .filter(|action| !cases.iter().any(|(case, _)| &case.action == *action))
            .map(|action| format!("actions.{action}"));
        let named = self
            .cases
            .keys()
            .filter(|name| !cases.iter().any(|(case, _)| &case.name == *name))
            .map(|name| format!("cases.{name}"));
        let versions = self
            .versions
            .keys()
            .filter(|listed| {
                !cases.iter().any(|(case, version)| {
                    !is_version_neutral(case) && *version == Some(listed.as_str())
                })
            })
            .map(|version| format!("versions.{version}"));
        actions.chain(named).chain(versions).collect()
    }
}

fn is_version_neutral(case: &Case) -> bool {
    VERSION_NEUTRAL_ACTIONS.contains(&case.action.as_str())
}

#[cfg(test)]
#[path = "skips_tests.rs"]
mod tests;
