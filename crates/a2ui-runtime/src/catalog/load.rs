//! Loading a catalog document (load rules 1 to 9 and 12) and writing it back.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Value};
use snafu::{OptionExt, ensure};

use super::Catalog;
use super::compile::{bundle, compile_entries};
use super::document::union_members;
use super::entries::{ComponentAttrs, FunctionAttrs};
use super::validation_schema::{self, Header};
use crate::bundle::check_references;
use crate::error::{
    CatalogJsonSnafu, CatalogLimitSnafu, DefsShadowCommonTypeSnafu, Error, MalformedCatalogSnafu,
    MissingCatalogIdSnafu, UnsupportedVersionSnafu,
};
use crate::limits::{
    MAX_CATALOG_BYTES, MAX_CATALOG_ENTRIES, MAX_CATALOG_NODES, MAX_JSON_DEPTH, MAX_KEY_BYTES,
    MAX_REF_BYTES, Oversize, deeper_than, oversize,
};
use crate::quoted::reason;
use crate::schema::common_type_defs;

/// The version a catalog targets when it declares none (`catalog_definition.json` default).
const DEFAULT_VERSION: &str = "0.9";

/// The kept components and functions, by name, before compiling.
type KeptEntries = (
    BTreeMap<String, ComponentAttrs>,
    BTreeMap<String, FunctionAttrs>,
);

impl Catalog {
    /// Loads a catalog document; `catalog_id` and `protocol_version` override its own.
    pub fn from_json(
        document: &Value,
        catalog_id: Option<&str>,
        protocol_version: Option<&str>,
    ) -> Result<Self, Error> {
        ensure!(
            !deeper_than(document, MAX_JSON_DEPTH),
            CatalogLimitSnafu {
                what: "levels of nesting",
                limit: MAX_JSON_DEPTH,
            }
        );
        if let Some(oversize) = oversize(document, MAX_CATALOG_NODES) {
            let (what, limit) = match oversize {
                Oversize::Nodes => ("JSON nodes", MAX_CATALOG_NODES),
                Oversize::Key => ("bytes in an object key", MAX_KEY_BYTES),
                Oversize::Ref => ("bytes in a $ref", MAX_REF_BYTES),
            };
            return Err(CatalogLimitSnafu { what, limit }.build().into());
        }
        let document = document.as_object().context(MalformedCatalogSnafu {
            reason: "a catalog document is a JSON object",
        })?;
        let id = match catalog_id {
            Some(id) => id,
            None => text(document, "catalogId")?.context(MissingCatalogIdSnafu)?,
        };
        ensure!(!id.is_empty(), MissingCatalogIdSnafu);
        let version = match protocol_version {
            Some(version) => version,
            None => text(document, "protocolVersion")?.unwrap_or(DEFAULT_VERSION),
        };
        ensure!(
            is_supported_version(version),
            UnsupportedVersionSnafu { version }
        );
        check_references(document)?;
        let instructions = text(document, "instructions")?;
        let defs = section(document, "$defs")?;
        check_defs(defs)?;
        let theme = document.get("theme");
        ensure!(
            theme.is_none_or(|theme| theme.is_object() || theme.is_boolean()),
            MalformedCatalogSnafu {
                reason: "`theme` must be a schema (an object or a boolean)",
            }
        );
        ensure!(
            theme.is_none() || !defs.is_some_and(|defs| defs.contains_key("theme")),
            MalformedCatalogSnafu {
                reason: "`theme` and `$defs.theme` both name the validation schema's theme",
            }
        );
        let (components, functions) = kept_entries(document, defs)?;
        let bundle = bundle(&components, &functions, defs)?;
        let ((components, functions), checked) = compile_entries(&bundle, components, functions)?;
        let header = Header {
            id,
            version,
            instructions,
            theme,
            defs,
        };
        let schema = validation_schema::build(checked, header, common_type_defs()?)?;
        Ok(Self {
            id: id.to_owned(),
            protocol_version: version.to_owned(),
            document: document.clone(),
            components,
            functions,
            validation_schema: schema,
        })
    }

    /// Parses JSON text of at most 4 MiB, then loads it as [`Catalog::from_json`] does.
    pub fn parse(
        text: &str,
        catalog_id: Option<&str>,
        protocol_version: Option<&str>,
    ) -> Result<Self, Error> {
        ensure!(
            text.len() <= MAX_CATALOG_BYTES,
            CatalogLimitSnafu {
                what: "bytes of catalog text",
                limit: MAX_CATALOG_BYTES,
            }
        );
        let document: Value = serde_json::from_str(text).map_err(|err| -> Error {
            CatalogJsonSnafu {
                message: err.to_string(),
            }
            .build()
            .into()
        })?;
        Self::from_json(&document, catalog_id, protocol_version)
    }

    /// The authored document with only the kept entries, and the effective id and version.
    pub fn to_json(&self) -> Value {
        Value::Object(self.kept_document())
    }

    /// The map [`Catalog::to_json`] returns; `derive` starts from it.
    pub(super) fn kept_document(&self) -> Map<String, Value> {
        let mut document = self.document.clone();
        retain(&mut document, "components", |name| {
            self.components.contains_key(name)
        });
        retain(&mut document, "functions", |name| {
            self.functions.contains_key(name)
        });
        document.insert("catalogId".to_owned(), Value::from(self.id.as_str()));
        document.insert(
            "protocolVersion".to_owned(),
            Value::from(self.protocol_version.as_str()),
        );
        document
    }
}

fn text<'a>(document: &'a Map<String, Value>, key: &'static str) -> Result<Option<&'a str>, Error> {
    match document.get(key) {
        None => Ok(None),
        Some(Value::String(text)) => Ok(Some(text)),
        Some(_) => Err(MalformedCatalogSnafu {
            reason: reason!["`", key, "` must be a string"],
        }
        .build()
        .into()),
    }
}

fn is_supported_version(version: &str) -> bool {
    version == "1.0"
        || version.strip_prefix("1.0.").is_some_and(|patch| {
            !patch.is_empty() && patch.bytes().all(|byte| byte.is_ascii_digit())
        })
}

fn section<'a>(
    document: &'a Map<String, Value>,
    key: &'static str,
) -> Result<Option<&'a Map<String, Value>>, Error> {
    match document.get(key) {
        None => Ok(None),
        Some(Value::Object(map)) => Ok(Some(map)),
        Some(_) => Err(MalformedCatalogSnafu {
            reason: reason!["`", key, "` must be an object"],
        }
        .build()
        .into()),
    }
}

fn check_defs(defs: Option<&Map<String, Value>>) -> Result<(), Error> {
    let common = common_type_defs()?;
    match defs
        .into_iter()
        .flatten()
        .find(|(name, _)| common.contains_key(*name))
    {
        Some((name, _)) => Err(DefsShadowCommonTypeSnafu { name }.build().into()),
        None => Ok(()),
    }
}

/// Reads every component and function, then keeps those the unions list (rules 6 to 9).
fn kept_entries(
    document: &Map<String, Value>,
    defs: Option<&Map<String, Value>>,
) -> Result<KeptEntries, Error> {
    let components = section(document, "components")?;
    let functions = section(document, "functions")?;
    let count = components.map_or(0, Map::len) + functions.map_or(0, Map::len);
    ensure!(
        count <= MAX_CATALOG_ENTRIES,
        CatalogLimitSnafu {
            what: "components and functions entries",
            limit: MAX_CATALOG_ENTRIES,
        }
    );
    let components = components
        .into_iter()
        .flatten()
        .map(|(name, schema)| ComponentAttrs::read(name, schema))
        .collect::<Result<Vec<_>, _>>()?;
    let functions = functions
        .into_iter()
        .flatten()
        .map(|(name, schema)| FunctionAttrs::read(name, schema))
        .collect::<Result<Vec<_>, _>>()?;
    let no_defs = Map::new();
    let defs = defs.unwrap_or(&no_defs);
    Ok((
        kept(
            components,
            union_members(defs, "anyComponent", "components"),
            ComponentAttrs::name,
        ),
        kept(
            functions,
            union_members(defs, "anyFunction", "functions"),
            FunctionAttrs::name,
        ),
    ))
}

/// The entries a union lists (all when `members` is `None`), by name.
///
/// `name` is a method path such as `ComponentAttrs::name`: a fn item is generic over the
/// borrow, which a closure returning a reference is not.
fn kept<T>(
    entries: Vec<T>,
    members: Option<BTreeSet<String>>,
    name: fn(&T) -> &str,
) -> BTreeMap<String, T> {
    entries
        .into_iter()
        .filter(|entry| {
            members
                .as_ref()
                .is_none_or(|members| members.contains(name(entry)))
        })
        .map(|entry| (name(&entry).to_owned(), entry))
        .collect()
}

fn retain(document: &mut Map<String, Value>, key: &str, keep: impl Fn(&str) -> bool) {
    if let Some(Value::Object(entries)) = document.get_mut(key) {
        entries.retain(|name, _| keep(name));
    }
}

#[cfg(test)]
#[path = "load_tests.rs"]
mod tests;
