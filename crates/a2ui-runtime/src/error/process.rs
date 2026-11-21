//! Why a message processor refuses a message, boxed in one variant to keep `Error` small.

use std::fmt;

use super::{Category, ErrorCode};
use crate::quoted::Quoted;

/// A message refused by the processor, or a catalog refused by a `CatalogSet`.
#[derive(Debug)]
pub(crate) enum ProcessFailure {
    /// A catalog whose id the set already holds.
    DuplicateCatalog { id: Quoted },
    /// A message naming a catalog the set does not hold.
    CatalogNotFound { id: Quoted },
    /// A `createSurface` for a surface that exists.
    SurfaceExists { id: Quoted },
    /// An update for a surface that does not exist.
    SurfaceNotFound { id: Quoted },
    /// An action the processor reads but does not apply yet.
    NotYetSupported { action: &'static str },
    /// A message that would take the processor past one of its `StateLimits`.
    StateLimit { what: &'static str, limit: usize },
}

impl ProcessFailure {
    /// The conformance category of this failure.
    pub(crate) fn category(&self) -> Category {
        match self {
            Self::DuplicateCatalog { .. } | Self::CatalogNotFound { .. } => Category::Catalog,
            Self::SurfaceExists { .. } | Self::SurfaceNotFound { .. } => Category::Integrity,
            Self::NotYetSupported { .. } | Self::StateLimit { .. } => Category::Validation,
        }
    }

    /// The A2UI code a renderer reports for this failure.
    pub(crate) fn code(&self) -> ErrorCode {
        match self {
            Self::DuplicateCatalog { .. } => ErrorCode::CatalogError,
            Self::CatalogNotFound { .. } => ErrorCode::CatalogNotFound,
            Self::SurfaceExists { .. } | Self::SurfaceNotFound { .. } => ErrorCode::IntegrityError,
            Self::NotYetSupported { .. } | Self::StateLimit { .. } => ErrorCode::ValidationFailed,
        }
    }
}

impl fmt::Display for ProcessFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateCatalog { id } => write!(f, "Catalog '{id}' is already registered"),
            // Upstream's exact wording: no quotes around the id.
            Self::CatalogNotFound { id } => write!(f, "Catalog not found: {id}"),
            Self::SurfaceExists { id } => write!(f, "Surface '{id}' already exists"),
            Self::SurfaceNotFound { id } => write!(f, "Surface not found for message: {id}"),
            Self::NotYetSupported { action } => write!(f, "Action '{action}' is not handled yet"),
            Self::StateLimit { what, limit } => write!(f, "{what} limit of {limit} exceeded"),
        }
    }
}

#[cfg(test)]
#[path = "process_tests.rs"]
mod tests;
