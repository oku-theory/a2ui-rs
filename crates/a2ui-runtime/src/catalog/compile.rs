//! The validation schema and the per-entry validators (load rules 10 and 11).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use jsonschema::Validator;
use serde_json::{Map, Value, json};
use snafu::ensure;

use super::document::union;
use super::entries::{ComponentAttrs, FunctionAttrs};
use super::{ComponentDef, ComponentShape, FunctionDef};
use crate::bundle::{add_common_types, local_ref, localize_refs};
use crate::error::{CatalogLimitSnafu, Error, MalformedCatalogSnafu};
use crate::limits::{
    Expansion, MAX_CATALOG_EXPANSION, MAX_CATALOG_PATTERNS, MAX_ENTRY_EXPANSION, MAX_SCHEMA_DEPTH,
    expand,
};
use crate::quoted::{Quoted, reason};
use crate::schema::common_type_defs;
use crate::validator::{compile, rooted};

/// Bundles the kept entries, own `$defs`, unions and the common types they reach.
pub(super) fn bundle(
    components: &BTreeMap<String, ComponentAttrs>,
    functions: &BTreeMap<String, FunctionAttrs>,
    defs: Option<&Map<String, Value>>,
) -> Result<Bundle, Error> {
    let mut defs = defs.cloned().unwrap_or_default();
    if !defs.contains_key("anyComponent") {
        let names: Vec<&str> = components.keys().map(String::as_str).collect();
        defs.insert(
            "anyComponent".to_owned(),
            union("components", &names, Some("component")),
        );
    }
    if !defs.contains_key("anyFunction") {
        let names: Vec<&str> = functions.keys().map(String::as_str).collect();
        defs.insert("anyFunction".to_owned(), union("functions", &names, None));
    }
    defs.values_mut().for_each(localize_refs);
    let components = localized(
        components
            .values()
            .map(|attrs| (attrs.name(), attrs.schema())),
    );
    let functions = localized(
        functions
            .values()
            .map(|attrs| (attrs.name(), attrs.schema())),
    );
    let own_defs = Value::Object(defs.clone());
    let roots: Vec<&Value> = components
        .values()
        .chain(functions.values())
        .chain([&own_defs])
        .collect();
    add_common_types(&mut defs, &roots, common_type_defs()?);
    Ok(Bundle {
        components,
        functions,
        defs,
    })
}

/// The bundled catalog: kept entries and `$defs`, every `$ref` local.
///
/// `$defs` holds the catalog's own definitions, the unions and the common types they reach.
pub(super) struct Bundle {
    components: Map<String, Value>,
    functions: Map<String, Value>,
    defs: Map<String, Value>,
}

impl Bundle {
    /// The kept components, localized, by name.
    pub(super) fn components(&self) -> &Map<String, Value> {
        &self.components
    }

    /// The kept functions, localized, by name.
    pub(super) fn functions(&self) -> &Map<String, Value> {
        &self.functions
    }

    /// The `$defs`: own definitions, unions and reached common types.
    pub(super) fn defs(&self) -> &Map<String, Value> {
        &self.defs
    }

    /// The bundle as one document, each section under its catalog key.
    fn document(&self) -> Value {
        Value::Object(Map::from_iter([
            (
                "components".to_owned(),
                Value::Object(self.components.clone()),
            ),
            (
                "functions".to_owned(),
                Value::Object(self.functions.clone()),
            ),
            ("$defs".to_owned(), Value::Object(self.defs.clone())),
        ]))
    }
}

/// A bundle whose every entry passed the expansion caps and compiled.
///
/// Only [`compile_entries`] makes one, so code taking it runs after the caps were enforced.
pub(super) struct Checked<'a>(&'a Bundle);

impl<'a> Checked<'a> {
    /// The checked bundle.
    pub(super) fn bundle(&self) -> &'a Bundle {
        self.0
    }
}

fn localized<'a>(entries: impl Iterator<Item = (&'a str, &'a Value)>) -> Map<String, Value> {
    entries
        .map(|(name, schema)| {
            let mut schema = schema.clone();
            localize_refs(&mut schema);
            (name.to_owned(), schema)
        })
        .collect()
}

/// The kept definitions, by name, each holding its validator.
pub(super) type Definitions = (
    BTreeMap<String, ComponentDef>,
    BTreeMap<String, FunctionDef>,
);

/// Checks every kept entry against load rule 11, then compiles each one.
///
/// All entries pass before any compiles: compiling an oversized one could take minutes. Each
/// entry compiles against the targets its check found, so it is expanded once.
pub(super) fn compile_entries<'a>(
    bundle: &'a Bundle,
    components: BTreeMap<String, ComponentAttrs>,
    functions: BTreeMap<String, FunctionAttrs>,
) -> Result<(Definitions, Checked<'a>), Error> {
    let document = bundle.document();
    let root = validator_top(&document);
    let mut totals = Totals::default();
    let component_targets = components
        .values()
        .map(|attrs| totals.add(reach(&root, attrs.name(), attrs.schema())?))
        .collect::<Result<Vec<_>, Error>>()?;
    let function_targets = functions
        .values()
        .map(|attrs| totals.add(reach(&root, attrs.name(), attrs.schema())?))
        .collect::<Result<Vec<_>, Error>>()?;
    let components = components
        .into_iter()
        .zip(component_targets)
        .map(|((name, attrs), targets)| {
            let validator = compile_entry(&root, "components", &name, attrs.schema(), targets)?;
            let shape = ComponentShape::of(&document, attrs.schema());
            Ok((name, attrs.into_def(shape, validator)))
        })
        .collect::<Result<BTreeMap<_, _>, Error>>()?;
    let functions = functions
        .into_iter()
        .zip(function_targets)
        .map(|((name, attrs), targets)| {
            let validator = compile_entry(&root, "functions", &name, attrs.schema(), targets)?;
            Ok((name, attrs.into_def(validator)))
        })
        .collect::<Result<BTreeMap<_, _>, Error>>()?;
    Ok(((components, functions), Checked(bundle)))
}

/// The expansion of all entries so far, against the catalog-wide caps.
#[derive(Default)]
struct Totals {
    size: usize,
    patterns: usize,
}

impl Totals {
    /// Adds one entry's reach; `Ok` holds its targets.
    fn add(&mut self, reach: Reach) -> Result<Targets, Error> {
        self.size += reach.size;
        self.patterns += reach.patterns;
        ensure!(
            self.size <= MAX_CATALOG_EXPANSION,
            CatalogLimitSnafu {
                what: "schema nodes in all entries once their $refs are expanded",
                limit: MAX_CATALOG_EXPANSION,
            }
        );
        ensure!(
            self.patterns <= MAX_CATALOG_PATTERNS,
            CatalogLimitSnafu {
                what: "patterns",
                limit: MAX_CATALOG_PATTERNS,
            }
        );
        Ok(reach.targets)
    }
}

/// Compiles the entry `name` of `section` against a document pruned to what it reaches.
///
/// The pruned document holds `$schema`, the entry and each `$defs`, `components` or
/// `functions` entry of `targets`, so a build costs the entry, not the catalog. Every target
/// was resolved in `root` by the entry's expansion, so each lookup finds it; were one absent,
/// the pruned document would keep a dangling `$ref` and the build would report it.
fn compile_entry(
    root: &Value,
    section: &'static str,
    name: &str,
    schema: &Value,
    targets: Targets,
) -> Result<Arc<Validator>, Error> {
    let mut entry = schema.clone();
    localize_refs(&mut entry);
    let mut sections: BTreeMap<&str, Map<String, Value>> = BTreeMap::new();
    sections
        .entry(section)
        .or_default()
        .insert(name.to_owned(), entry);
    for (target_section, target) in targets {
        let definition = root
            .get(target_section)
            .and_then(|entries| entries.get(target.as_str()));
        if let Some(definition) = definition {
            sections
                .entry(target_section)
                .or_default()
                .insert(target, definition.clone());
        }
    }
    let top: Map<String, Value> = sections
        .into_iter()
        .map(|(key, entries)| (key.to_owned(), Value::Object(entries)))
        .collect();
    let reference = local_ref(&format!("/{section}/{name}"));
    Ok(Arc::new(compile(&rooted(&top, &reference))?))
}

/// `bundle` with `FunctionCall` reduced to its envelope, as the root `$ref`s resolve in.
///
/// `FunctionCall` is present only when an entry reaches it, and its own definition names
/// `FunctionCommon`, so the common-type closure already copied the envelope's one target.
fn validator_top(bundle: &Value) -> Value {
    let mut top = bundle.clone();
    if let Some(call) = top.pointer_mut("/$defs/FunctionCall") {
        *call = function_call_envelope();
    }
    top
}

/// A `FunctionCall` checking only the envelope: `@call`, `catalogId`, `args`, nothing else.
///
/// A call may run in another catalog, so a component check cannot judge its function;
/// `Catalog::validate_component`'s nested-call check judges it in the catalog it runs in. It
/// replaces `FunctionCall` itself, not only `anyFunction` as `schema::common_document` does:
/// `FunctionCall` → `IndexSystemFunction` → `DynamicNumber` → `FunctionCall` is a cycle of the
/// common types, which load rule 11 rejects.
fn function_call_envelope() -> Value {
    json!({
        "type": "object",
        "allOf": [{"$ref": local_ref("/$defs/FunctionCommon")}],
        "properties": {"args": {"type": "object"}},
        "unevaluatedProperties": false
    })
}

/// The `(section, name)` of every entry one entry's `$ref`s land in.
type Targets = BTreeSet<(&'static str, String)>;

/// What one entry reaches once its `$ref`s are followed.
struct Reach {
    size: usize,
    patterns: usize,
    targets: Targets,
}

/// Expands the entry `name` in `root`; each way it breaks load rule 11 is a `CatalogError`.
fn reach(root: &Value, name: &str, schema: &Value) -> Result<Reach, Error> {
    match expand(root, schema, MAX_ENTRY_EXPANSION) {
        Expansion::Within {
            size,
            patterns,
            reached,
        } => Ok(Reach {
            size,
            patterns,
            targets: reached,
        }),
        Expansion::TooLarge => Err(CatalogLimitSnafu {
            what: reason![
                "schema nodes in '",
                Quoted::new(name),
                "' once its $refs are expanded",
            ],
            limit: MAX_ENTRY_EXPANSION,
        }
        .build()
        .into()),
        Expansion::TooDeep => Err(CatalogLimitSnafu {
            what: reason![
                "levels of '",
                Quoted::new(name),
                "' once its $refs are expanded"
            ],
            limit: MAX_SCHEMA_DEPTH,
        }
        .build()
        .into()),
        Expansion::Cycle(reference) => Err(MalformedCatalogSnafu {
            reason: reason!["$ref cycle through '", Quoted::new(&reference), "'"],
        }
        .build()
        .into()),
        Expansion::Unresolved(reference) => Err(MalformedCatalogSnafu {
            reason: reason!["$ref '", Quoted::new(&reference), "' does not resolve"],
        }
        .build()
        .into()),
    }
}

#[cfg(test)]
#[path = "compile_tests.rs"]
mod tests;
