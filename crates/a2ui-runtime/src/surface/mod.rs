//! The state a processor keeps per surface.

use std::collections::HashMap;
use std::collections::hash_map::Entry;

use serde_json::{Map, Value};

use crate::data::DataModel;
use crate::message::CreateSurface;

/// One surface a processor holds: its settings, components and data model.
///
/// It names its default catalog by id; the processor's `CatalogSet` holds the catalogs. It
/// keeps each component as sent, in arrival order; one sent again replaces it in place.
#[derive(Debug)]
#[non_exhaustive]
pub struct Surface {
    id: String,
    catalog_id: Option<String>,
    sends_data_model: bool,
    metadata: Option<Map<String, Value>>,
    data_model: DataModel,
    components: Vec<Map<String, Value>>,
    /// The position in `components` of each component id.
    index: HashMap<String, usize>,
}

/// Components checked for a surface, stored by [`Surface::commit`].
#[derive(Debug)]
pub(crate) struct Staged {
    /// Each component's id and entries, in message order; ids are distinct.
    pub(crate) components: Vec<(String, Map<String, Value>)>,
}

impl Surface {
    /// The surface a `createSurface` message describes.
    pub(crate) fn new(create: &CreateSurface) -> Self {
        let data_model = create.data_model.clone().map(Value::Object);
        Self {
            id: create.surface_id.clone(),
            catalog_id: create.catalog_id.clone(),
            sends_data_model: create.send_data_model,
            metadata: create.metadata.clone(),
            data_model: data_model.map_or_else(DataModel::default, DataModel::new),
            components: Vec::new(),
            index: HashMap::new(),
        }
    }

    /// The surface id.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// The default catalog id; `Some("")` when the message named an empty one.
    pub fn catalog_id(&self) -> Option<&str> {
        self.catalog_id.as_deref()
    }

    /// Whether the renderer sends the data model back to the agent.
    pub fn sends_data_model(&self) -> bool {
        self.sends_data_model
    }

    /// The metadata `createSurface` gave, if any.
    pub fn metadata(&self) -> Option<&Map<String, Value>> {
        self.metadata.as_ref()
    }

    /// The surface's data model.
    pub fn data_model(&self) -> &DataModel {
        &self.data_model
    }

    /// The components, as sent, in arrival order.
    pub fn components(&self) -> impl ExactSizeIterator<Item = &Map<String, Value>> {
        self.components.iter()
    }

    /// The component whose id is `id`, as sent.
    pub fn component(&self, id: &str) -> Option<&Map<String, Value>> {
        self.index.get(id).and_then(|&at| self.components.get(at))
    }

    /// The component whose id is `id`, with one `@` stripped from each key starting `@@`.
    ///
    /// Keys at every depth are unescaped; values are untouched.
    pub fn unescaped_component(&self, id: &str) -> Option<Map<String, Value>> {
        let mut component = self.component(id)?.clone();
        unescape_keys(&mut component);
        let mut pending: Vec<&mut Value> = component.values_mut().collect();
        while let Some(node) = pending.pop() {
            match node {
                Value::Object(map) => {
                    unescape_keys(map);
                    pending.extend(map.values_mut());
                }
                Value::Array(items) => pending.extend(items.iter_mut()),
                _ => {}
            }
        }
        Some(component)
    }

    /// Stores `staged`: a component with a known id replaces it in place, others are appended.
    pub(crate) fn commit(&mut self, staged: Staged) {
        for (id, component) in staged.components {
            match self.index.entry(id) {
                // Only this match writes `index` and `components`, and a vacant id gets the
                // position its push fills, so every indexed position exists.
                Entry::Occupied(entry) => self.components[*entry.get()] = component,
                Entry::Vacant(entry) => {
                    entry.insert(self.components.len());
                    self.components.push(component);
                }
            }
        }
    }

    pub(crate) fn data_model_mut(&mut self) -> &mut DataModel {
        &mut self.data_model
    }
}

/// Strips one `@` from each key of `map` starting `@@`.
fn unescape_keys(map: &mut Map<String, Value>) {
    for (key, value) in std::mem::take(map) {
        let key = match key.strip_prefix('@') {
            Some(rest) if rest.starts_with('@') => rest.to_owned(),
            _ => key,
        };
        map.insert(key, value);
    }
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
