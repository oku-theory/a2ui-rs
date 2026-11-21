//! Which catalog a component is checked against.

use serde_json::{Map, Value};

use crate::catalog::{Catalog, CatalogSet};
use crate::error::{ComponentShapeSnafu, Error, ProcessFailure};
use crate::evaluate::select_catalog;

/// The catalog `component`, whose id is `id`, names, else its surface's default.
///
/// # Errors
///
/// A `ValidationError` at `/catalogId` when that is not a string; a `NoDefaultCatalog` when
/// neither names a catalog; a `CatalogNotFound` when `set` does not hold the one named (`""` is
/// an id like any other).
pub(crate) fn component_catalog<'a>(
    component: &Map<String, Value>,
    id: &str,
    surface_id: &str,
    surface_default: Option<&str>,
    set: &'a CatalogSet,
) -> Result<&'a Catalog, Error> {
    let own = match component.get("catalogId") {
        Some(Value::String(id)) => Some(id.as_str()),
        Some(_) => {
            let reason = "catalogId must be a string";
            let error = Error::from(ComponentShapeSnafu { reason }.build());
            return Err(error.with_path("/catalogId".to_owned()));
        }
        None => None,
    };
    let catalog = select_catalog(own, surface_default, surface_id, "component", id)?;
    set.get(catalog)
        .ok_or_else(|| ProcessFailure::CatalogNotFound { id: catalog.into() }.into())
}

#[cfg(test)]
#[path = "resolve_tests.rs"]
mod tests;
