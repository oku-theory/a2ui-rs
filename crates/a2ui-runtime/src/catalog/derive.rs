//! Deriving a catalog with a new id from named entries of a catalog and new entries.

use serde_json::{Map, Value};
use snafu::{OptionExt, ensure};

use super::Catalog;
use super::document::union;
use crate::error::{DeriveDuplicateEntrySnafu, DeriveUnknownEntrySnafu, Error};

/// Document keys that describe the source catalog, so a derived catalog drops them.
const SOURCE_ONLY: [&str; 4] = ["$id", "title", "description", "instructions"];

/// One entry of a derived catalog: kept from the source, or newly defined.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum DeriveEntry {
    /// Keep the source catalog's entry of this name.
    Keep(String),
    /// A new entry, replacing any source entry of that name.
    Define(String, Value),
}

/// What [`Catalog::derive`] builds: a new id, the entries to keep or add, instructions.
#[derive(Debug, Clone, PartialEq)]
pub struct Derivation {
    catalog_id: String,
    components: Option<Vec<DeriveEntry>>,
    functions: Option<Vec<DeriveEntry>>,
    instructions: Option<String>,
}

impl Derivation {
    /// A derivation to `catalog_id` that keeps every entry and sets no instructions.
    pub fn new(catalog_id: impl Into<String>) -> Self {
        Self {
            catalog_id: catalog_id.into(),
            components: None,
            functions: None,
            instructions: None,
        }
    }

    /// Only these components; without this call, every source component is kept.
    pub fn components(mut self, entries: Vec<DeriveEntry>) -> Self {
        self.components = Some(entries);
        self
    }

    /// Only these functions; without this call, every source function is kept.
    pub fn functions(mut self, entries: Vec<DeriveEntry>) -> Self {
        self.functions = Some(entries);
        self
    }

    /// The derived catalog's instructions for agents.
    pub fn instructions(mut self, text: impl Into<String>) -> Self {
        self.instructions = Some(text.into());
        self
    }
}

/// One kind of entry: its singular name, document section, union and discriminator.
struct Kind {
    name: &'static str,
    section: &'static str,
    union: &'static str,
    discriminator: Option<&'static str>,
}

const COMPONENTS: Kind = Kind {
    name: "component",
    section: "components",
    union: "anyComponent",
    discriminator: Some("component"),
};

const FUNCTIONS: Kind = Kind {
    name: "function",
    section: "functions",
    union: "anyFunction",
    discriminator: None,
};

impl Catalog {
    /// A catalog with a new id, built from entries of this one and new entries.
    ///
    /// It keeps this catalog's protocol version, theme and `$defs`, and drops `$id`, `title`,
    /// `description` and `instructions`, which describe this catalog. A name listed twice in
    /// one list is an error. The result is loaded with [`Catalog::from_json`], so new entries
    /// meet every catalog rule and every entry gets its own compiled validator.
    pub fn derive(&self, derivation: &Derivation) -> Result<Catalog, Error> {
        let mut document = self.kept_document();
        for key in SOURCE_ONLY {
            document.remove(key);
        }
        document.insert(
            "catalogId".to_owned(),
            Value::from(derivation.catalog_id.as_str()),
        );
        if let Some(entries) = &derivation.components {
            self.replace(&mut document, &COMPONENTS, entries)?;
        }
        if let Some(entries) = &derivation.functions {
            self.replace(&mut document, &FUNCTIONS, entries)?;
        }
        if let Some(instructions) = &derivation.instructions {
            document.insert(
                "instructions".to_owned(),
                Value::from(instructions.as_str()),
            );
        }
        Catalog::from_json(&Value::Object(document), None, None)
    }

    fn replace(
        &self,
        document: &mut Map<String, Value>,
        kind: &Kind,
        entries: &[DeriveEntry],
    ) -> Result<(), Error> {
        let source = document.get(kind.section).and_then(Value::as_object);
        let mut picked = Map::new();
        for entry in entries {
            let (name, schema) = match entry {
                DeriveEntry::Keep(name) => {
                    let schema = source
                        .and_then(|source| source.get(name))
                        .with_context(|| DeriveUnknownEntrySnafu {
                            catalog: self.id.as_str(),
                            kind: kind.name,
                            name: name.as_str(),
                        })?;
                    (name, schema.clone())
                }
                DeriveEntry::Define(name, schema) => (name, schema.clone()),
            };
            ensure!(
                !picked.contains_key(name),
                DeriveDuplicateEntrySnafu {
                    kind: kind.name,
                    name: name.as_str(),
                }
            );
            picked.insert(name.clone(), schema);
        }
        let names: Vec<&str> = picked.keys().map(String::as_str).collect();
        let regenerated = union(kind.section, &names, kind.discriminator);
        if let Some(Value::Object(defs)) = document.get_mut("$defs")
            && defs.contains_key(kind.union)
        {
            defs.insert(kind.union.to_owned(), regenerated);
        }
        document.insert(kind.section.to_owned(), Value::Object(picked));
        Ok(())
    }
}

#[cfg(test)]
#[path = "derive_tests.rs"]
mod tests;
