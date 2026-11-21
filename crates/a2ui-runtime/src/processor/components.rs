//! Checking the components of one message before any is stored.

use std::collections::BTreeSet;

use serde_json::{Map, Value};

use super::resolve::component_catalog;
use super::{StateLimits, ValidationRules, state_limit};
use crate::catalog::{CallCatalogs, CatalogSet, Located, Report, bounded, identifier};
use crate::error::{ComponentShapeSnafu, Error, ProcessFailure};
use crate::surface::{Staged, Surface};
use crate::validator::pointer;

/// Checks `components`, sent to `surface`, and stages them for [`Surface::commit`].
///
/// It checks the component counts against `limits`, then that no two share an id, then each
/// component in order: its size, id, catalog, escaped keys and schema. Errors past the counts
/// point at `/components/<i>`.
pub(crate) fn check(
    surface: &Surface,
    components: &[Value],
    rules: ValidationRules,
    set: &CatalogSet,
    limits: &StateLimits,
) -> Result<Staged, Error> {
    let max_message = limits.max_components_per_message();
    if components.len() > max_message {
        return Err(state_limit("Components per message", max_message));
    }
    let mut ids = BTreeSet::new();
    for id in components
        .iter()
        .filter_map(|c| c.get("id").and_then(Value::as_str))
    {
        if !ids.insert(id) {
            return Err(ProcessFailure::DuplicateComponent { id: id.into() }.into());
        }
    }
    let added = ids
        .iter()
        .filter(|id| surface.component(id).is_none())
        .count();
    let max_surface = limits.max_components_per_surface();
    if surface.components().len().saturating_add(added) > max_surface {
        return Err(state_limit("Components per surface", max_surface));
    }
    let mut staged = Vec::with_capacity(components.len());
    for (index, component) in components.iter().enumerate() {
        let map = one(surface, component, rules, set)
            .map_err(|error| error.prefix_path(&format!("/components/{index}")))?;
        staged.push(map);
    }
    Ok(Staged { components: staged })
}

/// Checks one component and returns its id and entries; error paths are relative to it.
fn one(
    surface: &Surface,
    component: &Value,
    rules: ValidationRules,
    set: &CatalogSet,
) -> Result<(String, Map<String, Value>), Error> {
    bounded(component)?;
    let map = component.as_object().ok_or_else(|| {
        let reason = "a component is a JSON object";
        Error::from(ComponentShapeSnafu { reason }.build()).with_path(String::new())
    })?;
    let id = map.get("id").and_then(Value::as_str).ok_or_else(|| {
        let reason = "id must be a string";
        Error::from(ComponentShapeSnafu { reason }.build()).with_path("/id".to_owned())
    })?;
    identifier("Component id", id).map_err(|error| error.with_path("/id".to_owned()))?;
    let catalog = component_catalog(map, id, surface.id(), surface.catalog_id(), set)?;
    collisions(component, &mut Vec::new())?;
    let at = Located {
        default_catalog: surface.catalog_id(),
        surface_id: surface.id(),
        catalogs: set,
        report: Report::All { id },
        calls: CallCatalogs::Resolve,
        allow_unknown: rules.allows_unknown_elements(),
    };
    catalog.validate_located(component, &at)?;
    Ok((id.to_owned(), map.clone()))
}

/// Refuses an object under `node`, at `tokens`, two of whose keys are equal once unescaped.
///
/// Only `@@<k>` beside a plain `@<k>` collide: `@@@<k>` beside `@@<k>` unescape to `@@<k>` and
/// `@<k>`. The component is depth-checked first, so this recurses.
fn collisions(node: &Value, tokens: &mut Vec<String>) -> Result<(), Error> {
    match node {
        Value::Object(map) => {
            let mut escaped = map.keys().filter(|key| key.starts_with("@@"));
            let collides = |key: &&String| {
                let plain = &key[1..];
                !plain.starts_with("@@") && map.contains_key(plain)
            };
            if let Some(key) = escaped.find(collides) {
                let path = pointer(tokens);
                let failure = ProcessFailure::EscapedKeyCollision {
                    escaped: key.into(),
                    plain: key[1..].into(),
                };
                return Err(Error::from(failure).with_path(path));
            }
            for (key, value) in map {
                tokens.push(key.clone());
                collisions(value, tokens)?;
                tokens.pop();
            }
        }
        Value::Array(items) => {
            for (index, item) in items.iter().enumerate() {
                tokens.push(index.to_string());
                collisions(item, tokens)?;
                tokens.pop();
            }
        }
        _ => {}
    }
    Ok(())
}

#[cfg(test)]
#[path = "components_tests.rs"]
mod tests;
