//! Path observers that a [`Write`] notifies when their value changes.

use crate::data::{DataModel, Write};
use crate::pointer::Pointer;

/// Observers keyed by `K`, each watching one pointer.
#[derive(Debug, Clone)]
pub struct Watchers<K> {
    entries: Vec<(K, Pointer)>,
}

impl<K> Default for Watchers<K> {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
        }
    }
}

impl<K: Clone> Watchers<K> {
    /// Registers `key` as an observer of `path`; keys may repeat.
    pub fn watch(&mut self, key: K, path: Pointer) {
        self.entries.push((key, path));
    }

    /// Removes every observer.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// The keys whose watched value `write` changed, one per observer, in order.
    pub fn notified(&self, write: &Write, model: &DataModel) -> Vec<K> {
        self.entries
            .iter()
            .filter(|(_, path)| write.changes(path, model))
            .map(|(key, _)| key.clone())
            .collect()
    }
}

#[cfg(test)]
#[path = "subscribe_tests.rs"]
mod tests;
