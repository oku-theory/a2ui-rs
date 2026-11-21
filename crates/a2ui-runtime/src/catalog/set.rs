//! The catalogs a renderer has loaded, by id.

use std::collections::BTreeMap;
use std::collections::btree_map::Entry;

use super::{Catalog, CatalogLookup};
use crate::error::{Error, ProcessFailure};

/// The v1.0 catalogs a renderer has loaded, each under its id.
///
/// It holds what [`Catalog::from_json`] builds, so v1.0 catalogs only: a message naming any
/// other id finds no catalog.
#[derive(Debug, Default)]
pub struct CatalogSet {
    catalogs: BTreeMap<String, Catalog>,
}

impl CatalogSet {
    /// An empty set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds `catalog` under its id.
    ///
    /// # Errors
    ///
    /// A [`Category::Catalog`](crate::Category) error when the set already holds a catalog with
    /// that id; the refused catalog is dropped and the set is unchanged.
    pub fn insert(&mut self, catalog: Catalog) -> Result<(), Error> {
        match self.catalogs.entry(catalog.id().to_owned()) {
            Entry::Occupied(_) => Err(ProcessFailure::DuplicateCatalog {
                id: catalog.id().into(),
            }
            .into()),
            Entry::Vacant(slot) => {
                slot.insert(catalog);
                Ok(())
            }
        }
    }

    /// The catalog whose id is `id`, if the set holds it.
    pub fn get(&self, id: &str) -> Option<&Catalog> {
        self.catalogs.get(id)
    }

    /// Every catalog, in id order.
    pub fn iter(&self) -> impl Iterator<Item = &Catalog> {
        self.catalogs.values()
    }
}

impl CatalogLookup for CatalogSet {
    fn catalog(&self, id: &str) -> Option<&Catalog> {
        self.get(id)
    }
}

#[cfg(test)]
#[path = "set_tests.rs"]
mod tests;
