//! Runtime errors, each tagged with a conformance category and an A2UI code.
//!
//! Input a message quotes is held as `Quoted` (escaped, capped) from the moment the error is
//! built, alone or as a piece of runtime wording, a `Reason` that only `reason!` composes.
//! `inner` holds the variants; `process` the processor's failures and `evaluate` the evaluator's,
//! each boxed in one variant.

pub(crate) mod evaluate;
mod inner;
pub(crate) mod process;

use std::fmt;

use snafu::{Backtrace, ErrorCompat};

use crate::detail::Context;

pub(crate) use evaluate::EvalFailure;
pub(crate) use inner::*;
pub(crate) use process::{Allowed, MAX_VIOLATION_DETAILS, ProcessFailure};

/// The conformance category of an [`Error`], as the upstream suites name it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Category {
    /// A data model read or write that cannot be satisfied at its path.
    Data,
    /// Malformed text, such as an expression template that does not parse.
    Parse,
    /// An expression template past the length or part limits.
    Expression,
    /// A catalog that cannot be loaded, or an embedded schema that is broken.
    Catalog,
    /// A value that breaks a schema, such as a rejected component or a malformed pointer.
    Validation,
    /// A message value nested past the recursion limits.
    Recursion,
    /// A message that contradicts the surfaces held, such as a duplicate surface.
    Integrity,
}

impl Category {
    /// The name the conformance schema uses, such as `"DataError"`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Data => "DataError",
            Self::Parse => "ParseError",
            Self::Expression => "ExpressionError",
            Self::Catalog => "CatalogError",
            Self::Validation => "ValidationError",
            Self::Recursion => "RecursionError",
            Self::Integrity => "IntegrityError",
        }
    }
}

/// The A2UI error code a renderer reports for an [`Error`].
///
/// It is the `code` of a renderer `error` message (`renderer_to_agent.json`) or of a surface
/// diagnostic, for the stage that raises the error: upstream gives one fault different codes at
/// different stages (an unknown component is `VALIDATION_FAILED` when a component is validated,
/// `UNKNOWN_COMPONENT_TYPE` when a surface renders it), so each stage raises its own variants.
/// [`ErrorCode::as_str`] is the text to send; [`Error`]'s `Display`, English and quoting
/// (escaped, capped) input, is for logs, and applications localize from the code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ErrorCode {
    /// A data model path that cannot be read or written, such as `/__proto__`.
    DataError,
    /// An expression that does not parse or exceeds its limits.
    ExpressionError,
    /// A catalog that cannot be loaded, or an embedded schema that is broken.
    CatalogError,
    /// A rejected component or value, or a malformed JSON Pointer.
    ValidationFailed,
    /// A `callRendererFunction` message that breaks its schema.
    InvalidFunctionCall,
    /// A message value nested past the recursion limits.
    RecursionLimitExceeded,
    /// A message that contradicts the surfaces held, such as an unknown surface.
    IntegrityError,
    /// A message naming a catalog the renderer has not loaded.
    CatalogNotFound,
    /// A component placed under a parent type its `allowedParents` does not list.
    UnallowedParent,
    /// A container holding a child type its `allowedChildren` does not list.
    UnallowedChild,
}

impl ErrorCode {
    /// The code as the protocol spells it, such as `"VALIDATION_FAILED"`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DataError => "DATA_ERROR",
            Self::ExpressionError => "EXPRESSION_ERROR",
            Self::CatalogError => "CATALOG_ERROR",
            Self::ValidationFailed => "VALIDATION_FAILED",
            Self::InvalidFunctionCall => "INVALID_FUNCTION_CALL",
            Self::RecursionLimitExceeded => "RECURSION_LIMIT_EXCEEDED",
            Self::IntegrityError => "INTEGRITY_ERROR",
            Self::CatalogNotFound => "CATALOG_NOT_FOUND",
            Self::UnallowedParent => "UNALLOWED_PARENT",
            Self::UnallowedChild => "UNALLOWED_CHILD",
        }
    }
}

/// An error raised by the runtime; [`Error::category`] and [`Error::code`] classify it.
///
/// An error from reading a message also carries where it happened: [`Error::path`],
/// [`Error::surface_id`], [`Error::function_call_id`] and [`Error::details`].
#[derive(Debug)]
pub struct Error {
    inner: Inner,
    pub(crate) context: Option<Box<Context>>,
}

// Clippy's `result_large_err` trips at 128 bytes, so `Result<_, Error>` must stay below it.
const _: () = assert!(size_of::<Error>() < 128);

// A loaded catalog is shared across threads and reactive scopes.
const _: () = {
    const fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<crate::Catalog>();
};

impl From<Inner> for Error {
    fn from(inner: Inner) -> Self {
        Self {
            inner,
            context: None,
        }
    }
}

impl From<ProcessFailure> for Error {
    fn from(failure: ProcessFailure) -> Self {
        ProcessSnafu {
            failure: Box::new(failure),
        }
        .build()
        .into()
    }
}

impl From<EvalFailure> for Error {
    fn from(failure: EvalFailure) -> Self {
        EvaluateSnafu {
            failure: Box::new(failure),
        }
        .build()
        .into()
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.fmt(f)
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.inner.source()
    }
}

impl ErrorCompat for Error {
    fn backtrace(&self) -> Option<&Backtrace> {
        ErrorCompat::backtrace(&self.inner)
    }
}

impl Error {
    /// The conformance category this error belongs to.
    pub fn category(&self) -> Category {
        let (category, _) = self.inner.class();
        category
    }

    /// The A2UI code a renderer reports for this error.
    pub fn code(&self) -> ErrorCode {
        let (_, code) = self.inner.class();
        code
    }

    /// The error a function implementation returns when it refuses its arguments or input.
    ///
    /// `name` and `reason` are quoted and capped like any agent string; the error is an
    /// `ExpressionError`.
    pub fn function(name: &str, reason: &str) -> Self {
        EvalFailure::Function {
            name: name.into(),
            reason: reason.into(),
        }
        .into()
    }
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
