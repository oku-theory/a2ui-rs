//! The skip list: unsupported actions, skipped cases and unsupported versions.

use std::collections::BTreeMap;

use serde::Deserialize;

use crate::case::Case;

/// The protocol versions this runtime implements.
const SUPPORTED_VERSIONS: [&str; 1] = ["1.0"];

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
}

impl Skips {
    /// Parses the TOML text of `skips.toml`.
    pub fn parse(text: &str) -> Result<Self, String> {
        toml::from_str(text).map_err(|err| format!("skips.toml: {err}"))
    }

    /// Why `case` is skipped, or `None` when it runs.
    pub fn reason(&self, case: &Case) -> Option<String> {
        if let Some(reason) = self.cases.get(&case.name) {
            return Some(reason.clone());
        }
        if let Some(reason) = self.actions.get(&case.action) {
            return Some(reason.clone());
        }
        let version = case.protocol_version()?;
        let neutral = VERSION_NEUTRAL_ACTIONS.contains(&case.action.as_str());
        (!neutral && !SUPPORTED_VERSIONS.contains(&version))
            .then(|| format!("protocol version {version} is not supported"))
    }

    /// Entries that match no case, as `actions.<name>` or `cases.<name>`.
    pub fn unused(&self, cases: &[Case]) -> Vec<String> {
        let actions = self
            .actions
            .keys()
            .filter(|action| !cases.iter().any(|case| &case.action == *action))
            .map(|action| format!("actions.{action}"));
        let named = self
            .cases
            .keys()
            .filter(|name| !cases.iter().any(|case| &case.name == *name))
            .map(|name| format!("cases.{name}"));
        actions.chain(named).collect()
    }
}

#[cfg(test)]
#[path = "skips_tests.rs"]
mod tests;
