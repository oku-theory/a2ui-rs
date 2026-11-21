//! The catalogs a `process_messages` case registers, stubs opened and missing ids aliased.

use std::collections::BTreeMap;

use a2ui_runtime::{Catalog, CatalogSet, Derivation};
use serde_json::{Map, Value, json};

use crate::case::Case;
use crate::paths::{read_json, upstream_root};

/// A case's catalogs, and the ids aliased to its first registered catalog.
#[derive(Debug)]
pub struct Loaded {
    /// Every registered catalog, aliases included.
    pub set: CatalogSet,
    /// The ids registered as aliases, in the order the messages name them.
    pub aliased: Vec<String>,
}

/// Registers the case's catalogs: `catalogPaths` files, `catalogs` entries, then `catalog`.
///
/// Only v1.0 documents are registered (a document without a version takes the case's
/// `protocolVersion`); one with neither `catalogId` nor `components` is
/// ignored. A stub (`catalogId` and `protocolVersion` only) becomes an open catalog holding
/// each component type the messages place in it. Unless the case or a step expects a
/// `CatalogError`, each non-empty id the messages name and no catalog has is registered as a
/// derive of the first registered catalog (none: the basic catalog) under that id.
pub fn load(case: &Case) -> Result<Loaded, String> {
    let at = case.at();
    let named = Named::read(&batches(case));
    let mut set = CatalogSet::new();
    let mut first: Option<String> = None;
    for document in documents(case)? {
        let Some(document) = registrable(document, &named) else {
            continue;
        };
        let catalog = Catalog::from_json(&document, None, None)
            .map_err(|err| format!("{at}: catalog failed to load: {err}"))?;
        let id = catalog.id().to_owned();
        set.insert(catalog)
            .map_err(|err| format!("{at}: catalog not registered: {err}"))?;
        first.get_or_insert(id);
    }
    let mut aliased = Vec::new();
    if expects_catalog_error(case) {
        return Ok(Loaded { set, aliased });
    }
    for id in &named.ids {
        if id.is_empty() || set.get(id).is_some() {
            continue;
        }
        let source = match first.as_deref().and_then(|first| set.get(first)) {
            Some(source) => source,
            None => a2ui_catalog_basic::catalog()
                .map_err(|err| format!("{at}: basic catalog: {err}"))?,
        };
        let alias = source
            .derive(&Derivation::new(id.as_str()))
            .map_err(|err| format!("{at}: alias '{id}' failed: {err}"))?;
        set.insert(alias)
            .map_err(|err| format!("{at}: alias '{id}' not registered: {err}"))?;
        aliased.push(id.clone());
    }
    Ok(Loaded { set, aliased })
}

/// The case's catalog documents in registration order; `catalog.catalogSchema` is unwrapped.
///
/// A document declaring no `protocolVersion` takes the case's top-level one, if any.
fn documents(case: &Case) -> Result<Vec<Value>, String> {
    let at = case.at();
    let mut documents = Vec::new();
    if let Some(paths) = case.get("catalogPaths") {
        let paths = paths
            .as_array()
            .ok_or_else(|| format!("{at}: `catalogPaths` must be a list"))?;
        let root = upstream_root()?;
        for path in paths {
            let path = path
                .as_str()
                .ok_or_else(|| format!("{at}: `catalogPaths` entries must be strings"))?;
            documents.push(read_json(&root.join(path))?);
        }
    }
    if let Some(entries) = case.get("catalogs") {
        let entries = entries
            .as_array()
            .ok_or_else(|| format!("{at}: `catalogs` must be a list"))?;
        documents.extend(entries.iter().cloned());
    }
    if let Some(catalog) = case.get("catalog") {
        documents.push(unwrapped(catalog, &at)?);
    }
    if let Some(version) = case.get("protocolVersion") {
        for document in &mut documents {
            if let Value::Object(map) = document
                && !map.contains_key("protocolVersion")
            {
                map.insert("protocolVersion".to_owned(), version.clone());
            }
        }
    }
    Ok(documents)
}

/// An inline `catalog`, or its `catalogSchema`, which takes the outer `protocolVersion`.
fn unwrapped(catalog: &Value, at: &str) -> Result<Value, String> {
    let schema = match catalog.get("catalogSchema") {
        None => return Ok(catalog.clone()),
        Some(Value::Object(schema)) => schema,
        Some(_) => return Err(format!("{at}: `catalog.catalogSchema` must be a mapping")),
    };
    let mut document = schema.clone();
    if let Some(version) = catalog.get("protocolVersion")
        && !document.contains_key("protocolVersion")
    {
        document.insert("protocolVersion".to_owned(), version.clone());
    }
    Ok(Value::Object(document))
}

/// The document to register, a stub opened; `None` when it is not registered.
///
/// A non-object document is kept, for `Catalog::from_json` to refuse.
fn registrable(document: Value, named: &Named) -> Option<Value> {
    let Value::Object(mut map) = document else {
        return Some(document);
    };
    if map.get("protocolVersion").and_then(Value::as_str) != Some("1.0") {
        return None;
    }
    let id = map
        .get("catalogId")
        .and_then(Value::as_str)
        .map(str::to_owned);
    if id.is_none() && !map.contains_key("components") {
        return None;
    }
    if let Some(id) = id
        && map.len() == 2
    {
        let open: Map<String, Value> = named
            .placed
            .get(&id)
            .into_iter()
            .flatten()
            .map(|name| (name.clone(), open_schema()))
            .collect();
        map.insert("components".to_owned(), Value::Object(open));
    }
    Some(Value::Object(map))
}

/// The schema of a stub catalog's component: the basic catalog's slot names, and any other
/// property declared by `additionalProperties`, so strict rules accept it.
fn open_schema() -> Value {
    json!({"type": "object", "properties": {
        "child": {"$ref": "common_types.json#/$defs/ComponentId"},
        "children": {"$ref": "common_types.json#/$defs/ChildList"}},
        "additionalProperties": true})
}

/// Whether the case or one of its steps expects a `CatalogError`.
fn expects_catalog_error(case: &Case) -> bool {
    let steps = case.get("steps").and_then(Value::as_array);
    let mut holders =
        std::iter::once(&case.body).chain(steps.into_iter().flatten().filter_map(Value::as_object));
    holders.any(|holder| {
        ["expectError", "expect_error"].iter().any(|key| {
            holder
                .get(*key)
                .and_then(|expected| expected.get("category"))
                .and_then(Value::as_str)
                == Some("CatalogError")
        })
    })
}

/// Every message batch of the case: top-level `messages`/`payload`, then each step's.
fn batches(case: &Case) -> Vec<&Value> {
    let mut all = batches_of(&case.body);
    if let Some(steps) = case.get("steps").and_then(Value::as_array) {
        all.extend(
            steps
                .iter()
                .filter_map(Value::as_object)
                .flat_map(batches_of),
        );
    }
    all
}

/// The `messages` and `payload` values of one case or step.
fn batches_of(holder: &Map<String, Value>) -> Vec<&Value> {
    ["messages", "payload"]
        .into_iter()
        .filter_map(|key| holder.get(key))
        .collect()
}

/// The catalog ids the messages name, and the component types they place in each.
#[derive(Debug, Default)]
struct Named {
    ids: Vec<String>,
    placed: BTreeMap<String, Vec<String>>,
}

impl Named {
    /// Reads `createSurface` and `updateComponents` messages; a surface's default catalog is
    /// the one its `createSurface` names.
    fn read(batches: &[&Value]) -> Self {
        let mut named = Self::default();
        let mut defaults: BTreeMap<&str, &str> = BTreeMap::new();
        let messages = batches.iter().flat_map(|batch| match batch {
            Value::Array(items) => items.as_slice(),
            Value::Object(map) => map
                .get("messages")
                .and_then(Value::as_array)
                .map_or(&[][..], Vec::as_slice),
            _ => &[],
        });
        for message in messages {
            let (body, created) = match message.get("createSurface") {
                Some(body) => (body, true),
                None => match message.get("updateComponents") {
                    Some(body) => (body, false),
                    None => continue,
                },
            };
            let Some(surface) = body.get("surfaceId").and_then(Value::as_str) else {
                continue;
            };
            if created && let Some(id) = body.get("catalogId").and_then(Value::as_str) {
                named.name(id);
                defaults.insert(surface, id);
            }
            let components = body.get("components").and_then(Value::as_array);
            for component in components.into_iter().flatten() {
                let own = component.get("catalogId").and_then(Value::as_str);
                if let Some(id) = own {
                    named.name(id);
                }
                let catalog = own.or_else(|| defaults.get(surface).copied());
                let kind = component.get("component").and_then(Value::as_str);
                if let (Some(catalog), Some(kind)) = (catalog, kind) {
                    named.place(catalog, kind);
                }
            }
        }
        named
    }

    fn name(&mut self, id: &str) {
        if !self.ids.iter().any(|known| known == id) {
            self.ids.push(id.to_owned());
        }
    }

    fn place(&mut self, catalog: &str, kind: &str) {
        let types = self.placed.entry(catalog.to_owned()).or_default();
        if !types.iter().any(|known| known == kind) {
            types.push(kind.to_owned());
        }
    }
}

#[cfg(test)]
#[path = "catalogs_tests.rs"]
mod tests;
