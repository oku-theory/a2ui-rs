//! The state a processor keeps per surface.

use serde_json::{Map, Value};

use crate::data::DataModel;
use crate::message::CreateSurface;

/// One surface a processor holds: its settings and data model.
///
/// It names its default catalog by id; the processor's `CatalogSet` holds the catalogs.
#[derive(Debug)]
#[non_exhaustive]
pub struct Surface {
    id: String,
    catalog_id: Option<String>,
    sends_data_model: bool,
    metadata: Option<Map<String, Value>>,
    data_model: DataModel,
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

    pub(crate) fn data_model_mut(&mut self) -> &mut DataModel {
        &mut self.data_model
    }
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
