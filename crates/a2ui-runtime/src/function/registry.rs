//! The bindings from catalog functions to their implementations.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use serde_json::{Map, Value};

use super::CallContext;
use crate::catalog::Catalog;
use crate::error::{Error, EvalFailure};

/// The named arguments of a call, already evaluated.
pub type Args = Map<String, Value>;

/// A function implementation: evaluated arguments and the call's context in, a value out.
pub type Implementation =
    Arc<dyn Fn(&Args, &mut CallContext<'_>) -> Result<Value, Error> + Send + Sync>;

/// Implementations keyed by catalog id and function name.
///
/// It stores code only: whether a call is allowed is always read from the `FunctionDef` of the
/// catalog the call resolves to, so data cannot loosen the host's policy.
#[derive(Default, Clone)]
pub struct FunctionRegistry {
    bound: BTreeMap<String, BTreeMap<String, Implementation>>,
}

impl FunctionRegistry {
    /// An empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Binds `implementation` to function `name` of `catalog`, replacing an earlier binding.
    ///
    /// A name `catalog` does not declare, or one starting with `@`, is a `CatalogError`.
    pub fn register<F>(
        &mut self,
        catalog: &Catalog,
        name: &str,
        implementation: F,
    ) -> Result<(), Error>
    where
        F: Fn(&Args, &mut CallContext<'_>) -> Result<Value, Error> + Send + Sync + 'static,
    {
        if name.starts_with('@') || catalog.function(name).is_none() {
            return Err(EvalFailure::UndeclaredFunction {
                catalog: catalog.id().into(),
                name: name.into(),
            }
            .into());
        }
        self.bound
            .entry(catalog.id().to_owned())
            .or_default()
            .insert(name.to_owned(), Arc::new(implementation));
        Ok(())
    }

    /// The implementation bound to `name` in the catalog with id `catalog_id`, if any.
    pub fn implementation(&self, catalog_id: &str, name: &str) -> Option<&Implementation> {
        self.bound.get(catalog_id)?.get(name)
    }
}

impl fmt::Debug for FunctionRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let pairs = self
            .bound
            .iter()
            .flat_map(|(id, names)| names.keys().map(move |name| (id, name)));
        f.debug_list().entries(pairs).finish()
    }
}

#[cfg(test)]
#[path = "registry_tests.rs"]
mod tests;
