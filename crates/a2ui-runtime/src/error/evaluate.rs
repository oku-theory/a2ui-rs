//! Why evaluating a binding or calling a function fails, boxed in one variant of `Error`.

use std::fmt;

use super::{Category, ErrorCode};
use crate::quoted::Quoted;

/// A function or binding that cannot be evaluated.
#[derive(Debug)]
pub(crate) enum EvalFailure {
    /// An implementation bound to a name its catalog does not declare.
    UndeclaredFunction { catalog: Quoted, name: Quoted },
    /// A call naming a function its catalog does not define.
    #[cfg_attr(not(test), expect(dead_code, reason = "used by Task 2 (t-1712)"))]
    UnknownFunction { catalog: Quoted, name: Quoted },
    /// A call from the renderer to an `agentOnly` function.
    #[cfg_attr(not(test), expect(dead_code, reason = "used by Task 2 (t-1712)"))]
    RendererForbidden { name: Quoted },
    /// A call to a function that needs a user activation the scope does not hold.
    #[cfg_attr(not(test), expect(dead_code, reason = "used by Task 2 (t-1712)"))]
    ActivationRequired { name: Quoted },
    /// A declared function with no implementation bound.
    #[cfg_attr(not(test), expect(dead_code, reason = "used by Task 2 (t-1712)"))]
    NotImplemented { catalog: Quoted, name: Quoted },
    /// An `@index` outside a template iteration.
    #[cfg_attr(not(test), expect(dead_code, reason = "used by Task 2 (t-1712)"))]
    IndexOutsideTemplate,
    /// A value nested past the evaluation depth limit.
    #[cfg_attr(not(test), expect(dead_code, reason = "used by Task 2 (t-1712)"))]
    Depth { limit: usize },
    /// A catalog the surface does not support.
    #[cfg_attr(not(test), expect(dead_code, reason = "used by Task 2 (t-1712)"))]
    UnsupportedCatalog { catalog: Quoted, surface: Quoted },
    /// An implementation refusing its arguments or input.
    Function { name: Quoted, reason: Quoted },
    /// A regex pattern the A2UI dialect refuses.
    #[cfg_attr(not(test), expect(dead_code, reason = "used by Task 3 (t-1713)"))]
    Regex { rule: &'static str, detail: Quoted },
}

impl EvalFailure {
    /// The conformance category of this failure.
    pub(crate) fn category(&self) -> Category {
        match self {
            Self::UndeclaredFunction { .. } | Self::UnsupportedCatalog { .. } => Category::Catalog,
            Self::Depth { .. } => Category::Recursion,
            Self::UnknownFunction { .. }
            | Self::RendererForbidden { .. }
            | Self::ActivationRequired { .. }
            | Self::NotImplemented { .. }
            | Self::IndexOutsideTemplate
            | Self::Function { .. }
            | Self::Regex { .. } => Category::Expression,
        }
    }

    /// The A2UI code a renderer reports for this failure.
    pub(crate) fn code(&self) -> ErrorCode {
        match self {
            Self::UndeclaredFunction { .. } | Self::UnsupportedCatalog { .. } => {
                ErrorCode::CatalogError
            }
            Self::Depth { .. } => ErrorCode::RecursionLimitExceeded,
            Self::UnknownFunction { .. }
            | Self::RendererForbidden { .. }
            | Self::ActivationRequired { .. }
            | Self::NotImplemented { .. }
            | Self::IndexOutsideTemplate
            | Self::Function { .. }
            | Self::Regex { .. } => ErrorCode::ExpressionError,
        }
    }
}

impl fmt::Display for EvalFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UndeclaredFunction { catalog, name } => {
                write!(
                    f,
                    "Function '{name}' is not declared by catalog '{catalog}'"
                )
            }
            Self::UnknownFunction { catalog, name } => {
                write!(f, "Function '{name}' is not defined in catalog '{catalog}'")
            }
            Self::RendererForbidden { name } => {
                write!(f, "Function '{name}' cannot be called by renderer")
            }
            Self::ActivationRequired { name } => {
                write!(f, "Function '{name}' requires user activation")
            }
            Self::NotImplemented { catalog, name } => write!(
                f,
                "Function '{name}' of catalog '{catalog}' has no renderer implementation"
            ),
            Self::IndexOutsideTemplate => f.write_str(
                "@index function can only be evaluated inside a collection template iteration \
                 scope.",
            ),
            Self::Depth { limit } => write!(f, "Evaluation nests deeper than {limit} levels"),
            Self::UnsupportedCatalog { catalog, surface } => {
                write!(
                    f,
                    "Catalog '{catalog}' is not supported by surface '{surface}'."
                )
            }
            Self::Function { name, reason } => write!(f, "Function '{name}': {reason}"),
            Self::Regex { rule, detail } => write!(f, "{rule}: {detail}"),
        }
    }
}

#[cfg(test)]
#[path = "evaluate_tests.rs"]
mod tests;
