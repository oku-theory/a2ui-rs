//! One conformance test case and suite loading.

use std::path::Path;

use serde_json::{Map, Value};

use crate::paths::read_json;

/// The version upstream assumes for a catalog that declares none (`catalog_definition.json`).
pub const UPSTREAM_DEFAULT_VERSION: &str = "0.9";

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

    /// The declared protocol version: top-level, else `catalog.protocolVersion`.
    pub fn protocol_version(&self) -> Option<&str> {
        self.body
            .get("protocolVersion")
            .or_else(|| self.body.get("catalog")?.get("protocolVersion"))
            .and_then(Value::as_str)
    }

    /// The protocol version the case runs under, without a leading `v`.
    ///
    /// The declared version (top-level, then inline catalog) wins; else the `catalogPath`
    /// file's own; a catalog without one is at [`UPSTREAM_DEFAULT_VERSION`].
    pub fn effective_version(&self, root: &Path) -> Result<Option<String>, String> {
        let version = match (self.protocol_version(), self.get("catalogPath")) {
            (Some(declared), _) => Some(declared.to_owned()),
            (None, Some(Value::String(path))) => {
                Some(catalog_version(&read_json(&root.join(path))?, &self.at())?)
            }
            (None, Some(_)) => {
                return Err(format!("{}: `catalogPath` must be a string", self.at()));
            }
            (None, None) => self
                .get("catalog")
                .map(|_| UPSTREAM_DEFAULT_VERSION.to_owned()),
        };
        Ok(version.map(|version| {
            version
                .strip_prefix('v')
                .map_or_else(|| version.clone(), str::to_owned)
        }))
    }

    /// The body value under `key`.
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.body.get(key)
    }
}

fn catalog_version(document: &Value, at: &str) -> Result<String, String> {
    match document.get("protocolVersion") {
        None => Ok(UPSTREAM_DEFAULT_VERSION.to_owned()),
        Some(Value::String(version)) => Ok(version.clone()),
        Some(other) => Err(format!(
            "{at}: the catalog's protocolVersion is {other}, not a string"
        )),
    }
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
