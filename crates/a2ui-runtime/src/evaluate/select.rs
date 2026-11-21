//! Which catalog a component or function call resolves in.

use crate::error::{Error, EvalFailure, ProcessFailure};

/// The catalog id `name`, a `what` such as `"function"`, resolves in on surface `surface_id`.
///
/// It is `own`, the id the component or call names, else `surface_default`. A nested call never
/// inherits the enclosing call's catalog. The caller looks the id up in its `CatalogSet`.
///
/// # Errors
///
/// A [`Category::Catalog`](crate::Category) error, code `CATALOG_NOT_FOUND`, when neither names
/// a catalog.
pub fn select_catalog<'a>(
    own: Option<&'a str>,
    surface_default: Option<&'a str>,
    surface_id: &str,
    what: &'static str,
    name: &str,
) -> Result<&'a str, Error> {
    own.or(surface_default).ok_or_else(|| {
        ProcessFailure::NoDefaultCatalog {
            surface: surface_id.into(),
            what,
            name: name.into(),
        }
        .into()
    })
}

/// [`select_catalog`], refusing an id the surface's `supported` list does not hold.
///
/// # Errors
///
/// As [`select_catalog`]; else a [`Category::Catalog`](crate::Category) error, code
/// `CATALOG_ERROR`, when `supported` does not list the selected id.
pub fn select_supported<'a>(
    own: Option<&'a str>,
    surface_default: Option<&'a str>,
    supported: &[String],
    surface_id: &str,
    what: &'static str,
    name: &str,
) -> Result<&'a str, Error> {
    let selected = select_catalog(own, surface_default, surface_id, what, name)?;
    if supported.iter().any(|id| id == selected) {
        Ok(selected)
    } else {
        Err(EvalFailure::UnsupportedCatalog {
            catalog: selected.into(),
            surface: surface_id.into(),
        }
        .into())
    }
}

#[cfg(test)]
#[path = "select_tests.rs"]
mod tests;
