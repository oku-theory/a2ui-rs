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
    /// Two components of one message sharing an id.
    DuplicateComponent { id: Quoted },
    /// A component or call with no catalogId on a surface with no default.
    NoDefaultCatalog {
        surface: Quoted,
        what: &'static str,
        name: Quoted,
    },
    /// A component breaking its schema in several places; `first` is the first one's message.
    SchemaViolations {
        id: Quoted,
        kind: Quoted,
        count: usize,
        first: Quoted,
    },
    /// An object holding both `@@<k>` and `@<k>`, equal once unescaped.
    EscapedKeyCollision { escaped: Quoted, plain: Quoted },
    /// A component listing itself as a child.
    SelfReference { id: Quoted },
    /// A component naming a child the surface does not hold.
    DanglingReference {
        parent: Quoted,
        child: Quoted,
        field: Quoted,
    },
    /// Components reaching themselves; `ids` is the loop, cut short when `more` holds.
    CircularReference { ids: Vec<Quoted>, more: bool },
    /// A surface with components but no `root`.
    MissingRoot,
    /// A component `root` does not reach.
    Unreachable { id: Quoted },
    /// A component more than `MAX_MESSAGE_DEPTH` levels below `root`.
    ComponentDepth { id: Quoted, depth: usize },
    /// A component under a parent type its `allowedParents` does not list.
    UnallowedParent {
        child: Quoted,
        child_type: Quoted,
        parent: Quoted,
        parent_type: Quoted,
        allowed: Allowed,
    },
    /// A container holding a child type its `allowedChildren` does not list.
    UnallowedChild {
        parent: Quoted,
        parent_type: Quoted,
        child: Quoted,
        child_type: Quoted,
        allowed: Allowed,
    },
}

/// How many type names of an `allowedParents` or `allowedChildren` list an error shows.
const MAX_ALLOWED_NAMES: usize = 8;

/// The type names a composition error lists, cut short past `MAX_ALLOWED_NAMES`.
#[derive(Debug)]
pub(crate) struct Allowed {
    names: Vec<Quoted>,
    more: bool,
}

impl Allowed {
    /// The first `MAX_ALLOWED_NAMES` of `names`, noting whether more were left out.
    pub(crate) fn new(names: &[String]) -> Self {
        Self {
            names: names
                .iter()
                .take(MAX_ALLOWED_NAMES)
                .map(Quoted::from)
                .collect(),
            more: names.len() > MAX_ALLOWED_NAMES,
        }
    }
}

impl fmt::Display for Allowed {
    /// Upstream's Python list spelling, such as `['Text', 'Button']`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[")?;
        for (at, name) in self.names.iter().enumerate() {
            let comma = if at == 0 { "" } else { ", " };
            write!(f, "{comma}'{name}'")?;
        }
        if self.more {
            f.write_str(", …")?;
        }
        f.write_str("]")
    }
}

/// How many schema violations of one component an error lists as details.
pub(crate) const MAX_VIOLATION_DETAILS: usize = 16;

impl ProcessFailure {
    /// The conformance category of this failure.
    pub(crate) fn category(&self) -> Category {
        match self {
            Self::DuplicateCatalog { .. }
            | Self::CatalogNotFound { .. }
            | Self::NoDefaultCatalog { .. } => Category::Catalog,
            Self::SurfaceExists { .. }
            | Self::SurfaceNotFound { .. }
            | Self::DuplicateComponent { .. }
            | Self::DanglingReference { .. }
            | Self::Unreachable { .. } => Category::Integrity,
            Self::NotYetSupported { .. }
            | Self::StateLimit { .. }
            | Self::SchemaViolations { .. }
            | Self::EscapedKeyCollision { .. }
            | Self::MissingRoot
            | Self::UnallowedParent { .. }
            | Self::UnallowedChild { .. } => Category::Validation,
            Self::SelfReference { .. }
            | Self::CircularReference { .. }
            | Self::ComponentDepth { .. } => Category::Recursion,
        }
    }

    /// The A2UI code a renderer reports for this failure.
    pub(crate) fn code(&self) -> ErrorCode {
        match self {
            Self::DuplicateCatalog { .. } => ErrorCode::CatalogError,
            Self::CatalogNotFound { .. } | Self::NoDefaultCatalog { .. } => {
                ErrorCode::CatalogNotFound
            }
            Self::SurfaceExists { .. }
            | Self::SurfaceNotFound { .. }
            | Self::DuplicateComponent { .. }
            | Self::DanglingReference { .. }
            | Self::Unreachable { .. } => ErrorCode::IntegrityError,
            Self::NotYetSupported { .. }
            | Self::StateLimit { .. }
            | Self::SchemaViolations { .. }
            | Self::EscapedKeyCollision { .. }
            | Self::MissingRoot => ErrorCode::ValidationFailed,
            Self::UnallowedParent { .. } => ErrorCode::UnallowedParent,
            Self::UnallowedChild { .. } => ErrorCode::UnallowedChild,
            Self::SelfReference { .. }
            | Self::CircularReference { .. }
            | Self::ComponentDepth { .. } => ErrorCode::RecursionLimitExceeded,
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
            Self::DuplicateComponent { id } => {
                write!(f, "Duplicate component ID '{id}' in one message")
            }
            Self::NoDefaultCatalog {
                surface,
                what,
                name,
            } => write!(
                f,
                "Surface '{surface}' has no default catalogId for {what} '{name}'"
            ),
            Self::SchemaViolations {
                id,
                kind,
                count,
                first,
            } => {
                write!(f, "Component '{id}' ({kind}) has ")?;
                if *count > MAX_VIOLATION_DETAILS {
                    write!(f, "more than {MAX_VIOLATION_DETAILS}")?;
                } else {
                    write!(f, "{count}")?;
                }
                write!(f, " schema violations; first: {first}")
            }
            Self::EscapedKeyCollision { escaped, plain } => {
                write!(
                    f,
                    "Key '{escaped}' collides with '{plain}' after unescaping"
                )
            }
            Self::SelfReference { id } => write!(
                f,
                "Self-reference detected: component '{id}' lists itself as a child"
            ),
            Self::DanglingReference {
                parent,
                child,
                field,
            } => write!(
                f,
                "Dangling reference: component '{parent}' references non-existent component \
                 '{child}' in field '{field}'"
            ),
            Self::CircularReference { ids, more } => {
                write!(
                    f,
                    "Circular reference detected: circular component reference"
                )?;
                for (at, id) in ids.iter().enumerate() {
                    let arrow = if at == 0 { " " } else { " -> " };
                    write!(f, "{arrow}'{id}'")?;
                }
                match ids.first() {
                    Some(_) if *more => write!(f, " -> …"),
                    Some(first) => write!(f, " -> '{first}'"),
                    None => Ok(()),
                }
            }
            Self::MissingRoot => write!(f, "Missing root component 'root'"),
            Self::Unreachable { id } => write!(f, "Component '{id}' is not reachable from 'root'"),
            Self::ComponentDepth { id, depth } => write!(
                f,
                "Global recursion limit exceeded: component '{id}' is {depth} levels below 'root'"
            ),
            Self::UnallowedParent {
                child,
                child_type,
                parent,
                parent_type,
                allowed,
            } => write!(
                f,
                "Component '{child}' ({child_type}) cannot be placed under parent '{parent}' \
                 ({parent_type}). Allowed parents: {allowed}."
            ),
            Self::UnallowedChild {
                parent,
                parent_type,
                child,
                child_type,
                allowed,
            } => write!(
                f,
                "Container '{parent}' ({parent_type}) cannot contain child '{child}' \
                 ({child_type}). Allowed children: {allowed}."
            ),
        }
    }
}

#[cfg(test)]
#[path = "process_tests.rs"]
mod tests;
