//! Caps on the state a processor keeps, whatever the agent sends.

/// How many surfaces, components and data model nodes a processor keeps.
///
/// A message that would pass one is refused whole and changes nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StateLimits {
    max_surfaces: usize,
    max_components_per_surface: usize,
    max_components_per_message: usize,
    max_data_model_nodes: usize,
}

impl Default for StateLimits {
    /// 64 surfaces, 10 000 components per surface and per message, 100 000 data model nodes.
    fn default() -> Self {
        Self {
            max_surfaces: 64,
            max_components_per_surface: 10_000,
            max_components_per_message: 10_000,
            max_data_model_nodes: 100_000,
        }
    }
}

impl StateLimits {
    /// These limits with at most `max` surfaces.
    pub fn with_max_surfaces(self, max: usize) -> Self {
        Self {
            max_surfaces: max,
            ..self
        }
    }

    /// These limits with at most `max` components per surface.
    pub fn with_max_components_per_surface(self, max: usize) -> Self {
        Self {
            max_components_per_surface: max,
            ..self
        }
    }

    /// These limits with at most `max` components per message.
    pub fn with_max_components_per_message(self, max: usize) -> Self {
        Self {
            max_components_per_message: max,
            ..self
        }
    }

    /// These limits with at most `max` data model nodes per surface.
    ///
    /// Each value, object or list is a node; a string or key weighs one more per 64 bytes.
    pub fn with_max_data_model_nodes(self, max: usize) -> Self {
        Self {
            max_data_model_nodes: max,
            ..self
        }
    }

    /// The most surfaces a processor holds at once.
    pub fn max_surfaces(&self) -> usize {
        self.max_surfaces
    }

    /// The most components one surface holds.
    pub fn max_components_per_surface(&self) -> usize {
        self.max_components_per_surface
    }

    /// The most components one message carries.
    pub fn max_components_per_message(&self) -> usize {
        self.max_components_per_message
    }

    /// The most data model nodes one surface holds, weighed after each write.
    pub fn max_data_model_nodes(&self) -> usize {
        self.max_data_model_nodes
    }
}

#[cfg(test)]
#[path = "limits_tests.rs"]
mod tests;
