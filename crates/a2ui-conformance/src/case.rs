//! One conformance test case and suite loading.

use std::path::Path;

use serde_json::{Map, Value};

use crate::paths::read_json;

/// The version upstream assumes for a catalog that declares none (`catalog_definition.json`).
pub const UPSTREAM_DEFAULT_VERSION: &str = "0.9";

/// Actions whose behavior does not depend on the declared protocol version.
const VERSION_NEUTRAL_ACTIONS: [&str; 3] =
    ["data_model", "resolve_path", "parse_expression_template"];

/// Why a case's protocol version could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VersionError {
    /// No rule classifies the version of a version-dependent case (the `suite#name` location).
    Unclassified(String),
    /// A catalog file could not be read or a version is not a string.
    Failed(String),
}

impl std::fmt::Display for VersionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unclassified(at) => write!(f, "{at}: no protocol version can be classified"),
            Self::Failed(message) => f.write_str(message),
        }
    }
}

impl From<String> for VersionError {
    fn from(message: String) -> Self {
        Self::Failed(message)
    }
}

/// One YAML test case: its suite, name, action and full body.
#[derive(Debug, Clone)]
pub struct Case {
    /// Suite path relative to `conformance/`, such as `core/data_model.yaml`.
    pub suite: String,
    /// The case `name`.
    pub name: String,
    /// The `action` the case runs.
    pub action: String,
    /// Every key of the case, `name` and `action` included.
    pub body: Map<String, Value>,
}

impl Case {
    /// The case's location for messages, such as `core/catalog.yaml#test_x`.
    pub fn at(&self) -> String {
        format!("{}#{}", self.suite, self.name)
    }

    /// The protocol version the case runs under, without a leading `v`.
    ///
    /// The first rule that answers wins: top-level `protocolVersion`, `catalog.protocolVersion`,
    /// the first message's `version`, `args.version`, the first catalog (`catalogs`,
    /// `catalogPaths`, `catalogPath` or an inline `catalog`; one without a version is at
    /// [`UPSTREAM_DEFAULT_VERSION`]), a v0.8 envelope key, the suite name's `_vN_M` suffix.
    /// Only a version-neutral action may stay unclassified (`None`); any other is
    /// [`VersionError::Unclassified`].
    pub fn effective_version(&self, root: &Path) -> Result<Option<String>, VersionError> {
        let found = match self.declared_version()? {
            Some(version) => Some(version),
            None => match self.message_version()? {
                Some(version) => Some(version),
                None => match self.args_version()? {
                    Some(version) => Some(version),
                    None => match self.catalog_version(root)? {
                        Some(version) => Some(version),
                        None => self.envelope_version().or_else(|| self.suite_version()),
                    },
                },
            },
        };
        match found {
            Some(version) => Ok(Some(drop_v(&version))),
            None if self.is_version_neutral() => Ok(None),
            None => Err(VersionError::Unclassified(self.at())),
        }
    }

    /// Whether the action behaves the same under every protocol version.
    pub fn is_version_neutral(&self) -> bool {
        VERSION_NEUTRAL_ACTIONS.contains(&self.action.as_str())
    }

    fn declared_version(&self) -> Result<Option<String>, String> {
        let inline = self
            .get("catalog")
            .and_then(|catalog| catalog.get("protocolVersion"));
        self.string_of(self.get("protocolVersion").or(inline), "protocolVersion")
    }

    fn message_version(&self) -> Result<Option<String>, String> {
        let first_step = self
            .get("steps")
            .and_then(Value::as_array)
            .and_then(|steps| steps.first());
        let first = self
            .first_message(
                self.body
                    .get("messages")
                    .or_else(|| self.body.get("payload")),
            )
            .or_else(|| {
                let step = first_step?;
                self.first_message(step.get("messages").or_else(|| step.get("payload")))
            });
        self.string_of(first.and_then(|message| message.get("version")), "version")
    }

    fn args_version(&self) -> Result<Option<String>, String> {
        self.string_of(
            self.get("args").and_then(|args| args.get("version")),
            "args.version",
        )
    }

    fn catalog_version(&self, root: &Path) -> Result<Option<String>, String> {
        let first = |key: &str| {
            self.get(key)
                .and_then(Value::as_array)
                .and_then(|a| a.first())
        };
        if let Some(catalog) = first("catalogs") {
            if !catalog.is_object() {
                return Err(format!("{}: `catalogs` entries must be objects", self.at()));
            }
            return self
                .string_of(catalog.get("protocolVersion"), "protocolVersion")
                .map(|v| Some(v.unwrap_or_else(|| UPSTREAM_DEFAULT_VERSION.to_owned())));
        }
        let (key, path) = match (first("catalogPaths"), self.get("catalogPath")) {
            (Some(path), _) => ("catalogPaths", path),
            (None, Some(path)) => ("catalogPath", path),
            (None, None) => {
                return Ok(self
                    .get("catalog")
                    .map(|_| UPSTREAM_DEFAULT_VERSION.to_owned()));
            }
        };
        let Value::String(path) = path else {
            return Err(format!("{}: `{key}` must be a string", self.at()));
        };
        let document = read_json(&root.join(path))?;
        let version = self.string_of(document.get("protocolVersion"), "protocolVersion")?;
        Ok(Some(
            version.unwrap_or_else(|| UPSTREAM_DEFAULT_VERSION.to_owned()),
        ))
    }

    fn envelope_version(&self) -> Option<String> {
        const KEYS: [&str; 3] = ["beginRendering", "surfaceUpdate", "dataModelUpdate"];
        let first = self.first_message(
            self.body
                .get("messages")
                .or_else(|| self.body.get("payload")),
        )?;
        KEYS.iter()
            .any(|key| first.get(key).is_some())
            .then(|| "0.8".to_owned())
    }

    fn suite_version(&self) -> Option<String> {
        let stem = self.suite.strip_suffix(".yaml").unwrap_or(&self.suite);
        ["_v1_0", "_v0_9", "_v0_8"].iter().find_map(|suffix| {
            let version = stem.ends_with(suffix).then_some(&suffix[2..])?;
            Some(version.replace('_', "."))
        })
    }

    /// The first message of `list`, a message array or `{"messages": [...]}`.
    fn first_message<'a>(&self, list: Option<&'a Value>) -> Option<&'a Value> {
        match list? {
            Value::Array(items) => items.first(),
            Value::Object(map) => map.get("messages")?.as_array()?.first(),
            _ => None,
        }
    }

    /// `value` as a string; a present non-string is an error naming `what`.
    fn string_of(&self, value: Option<&Value>, what: &str) -> Result<Option<String>, String> {
        match value {
            None => Ok(None),
            Some(Value::String(text)) => Ok(Some(text.clone())),
            Some(other) => Err(format!("{}: `{what}` is {other}, not a string", self.at())),
        }
    }

    /// The body value under `key`.
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.body.get(key)
    }
}

fn drop_v(version: &str) -> String {
    version.strip_prefix('v').unwrap_or(version).to_owned()
}

/// Parses a suite's YAML text into its cases; an empty suite is an error.
pub fn parse_suite(suite: &str, text: &str) -> Result<Vec<Case>, String> {
    let value: Value = serde_yaml_ng::from_str(text).map_err(|err| format!("{suite}: {err}"))?;
    let Value::Array(items) = value else {
        return Err(format!("{suite}: a suite is a list of cases"));
    };
    if items.is_empty() {
        return Err(format!("{suite}: the suite has no cases"));
    }
    items.into_iter().map(|item| case(suite, item)).collect()
}

/// Reads and parses the suite file at `path`, named `suite`.
pub fn load_suite(path: &Path, suite: &str) -> Result<Vec<Case>, String> {
    let text = std::fs::read_to_string(path).map_err(|err| format!("{suite}: {err}"))?;
    parse_suite(suite, &text)
}

fn case(suite: &str, item: Value) -> Result<Case, String> {
    let Value::Object(body) = item else {
        return Err(format!("{suite}: a case is a mapping"));
    };
    let text = |key: &str| {
        body.get(key)
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| format!("{suite}: a case needs a string `{key}`"))
    };
    let name = text("name")?;
    let action = text("action").map_err(|err| format!("{err} ({name})"))?;
    Ok(Case {
        suite: suite.to_owned(),
        name,
        action,
        body,
    })
}

#[cfg(test)]
#[path = "case_tests.rs"]
mod tests;
