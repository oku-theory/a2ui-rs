//! Renderer capabilities and data model reports.

use serde_json::{Map, Value, json};

use super::MessageProcessor;

impl MessageProcessor {
    /// The `a2uiRendererCapabilities` this renderer announces, without inline catalogs.
    pub fn capabilities(&self) -> Value {
        self.capabilities_report(false)
    }

    /// The capabilities of [`MessageProcessor::capabilities`] plus every catalog inline.
    ///
    /// Each inline catalog keeps its `catalogId` and `protocolVersion`; an empty `components`
    /// or `functions` section is left out.
    pub fn capabilities_with_inline_catalogs(&self) -> Value {
        self.capabilities_report(true)
    }

    /// The `a2uiRendererDataModel` of the surfaces that send their data model.
    ///
    /// `None` when no surface sends it. The processor keeps every root an object.
    pub fn data_model_report(&self) -> Option<Value> {
        let surfaces: Map<String, Value> = self
            .surfaces
            .values()
            .filter(|surface| surface.sends_data_model())
            .map(|surface| (surface.id().to_owned(), surface.data_model().root().clone()))
            .collect();
        (!surfaces.is_empty()).then(|| json!({"version": "1.0", "surfaces": surfaces}))
    }

    fn capabilities_report(&self, inline: bool) -> Value {
        let ids: Vec<&str> = self.catalogs.iter().map(|catalog| catalog.id()).collect();
        let mut report = Map::new();
        report.insert("supportedCatalogIds".to_owned(), json!(ids));
        if inline {
            let catalogs: Vec<Value> = self
                .catalogs
                .iter()
                .map(|catalog| {
                    let mut document = catalog.to_json();
                    if let Value::Object(map) = &mut document {
                        for key in ["components", "functions"] {
                            if map
                                .get(key)
                                .and_then(Value::as_object)
                                .is_some_and(Map::is_empty)
                            {
                                map.remove(key);
                            }
                        }
                    }
                    document
                })
                .collect();
            report.insert("inlineCatalogs".to_owned(), Value::Array(catalogs));
        }
        json!({"1.0": report})
    }
}

#[cfg(test)]
#[path = "reports_tests.rs"]
mod tests;
