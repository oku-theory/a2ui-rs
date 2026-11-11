//! Errors raised by the runtime, each tagged with a conformance category.

use snafu::{Backtrace, Snafu};

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
}

impl Category {
    /// The name the conformance schema uses, such as `"DataError"`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Data => "DataError",
            Self::Parse => "ParseError",
            Self::Expression => "ExpressionError",
            Self::Catalog => "CatalogError",
        }
    }
}

/// An error raised by the runtime; [`Error::category`] classifies it.
#[derive(Debug, Snafu)]
pub struct Error(Inner);

impl Error {
    /// The conformance category this error belongs to.
    pub fn category(&self) -> Category {
        self.0.category()
    }
}

#[derive(Debug, Snafu)]
#[snafu(visibility(pub(crate)))]
pub(crate) enum Inner {
    #[snafu(display("Forbidden path segment '{segment}' in '{path}'"))]
    ForbiddenSegment {
        path: String,
        segment: String,
        backtrace: Backtrace,
    },

    #[snafu(display("Invalid JSON Pointer '{path}': {reason}"))]
    InvalidPointer {
        path: String,
        reason: &'static str,
        backtrace: Backtrace,
    },

    #[snafu(display("Cannot set path '{path}': segment '{segment}' holds a primitive value"))]
    TraversePrimitive {
        path: String,
        segment: String,
        backtrace: Backtrace,
    },

    #[snafu(display("Cannot set path '{path}': non-numeric segment '{segment}' in a list"))]
    NonNumericSegment {
        path: String,
        segment: String,
        backtrace: Backtrace,
    },

    #[snafu(display("Cannot set path '{path}': list index '{segment}' exceeds the limit"))]
    IndexOutOfRange {
        path: String,
        segment: String,
        backtrace: Backtrace,
    },

    #[snafu(display("{message} at offset {offset}"))]
    ExpressionSyntax {
        message: &'static str,
        offset: usize,
        backtrace: Backtrace,
    },

    #[snafu(display("Expression template exceeds {limit}"))]
    ExpressionLimit {
        limit: &'static str,
        backtrace: Backtrace,
    },

    #[snafu(display("Embedded schema {name} is broken: {message}"))]
    EmbeddedSchema {
        name: &'static str,
        message: String,
        backtrace: Backtrace,
    },
}

impl Inner {
    fn category(&self) -> Category {
        match self {
            Self::ForbiddenSegment { .. }
            | Self::InvalidPointer { .. }
            | Self::TraversePrimitive { .. }
            | Self::NonNumericSegment { .. }
            | Self::IndexOutOfRange { .. } => Category::Data,
            Self::ExpressionSyntax { .. } => Category::Parse,
            Self::ExpressionLimit { .. } => Category::Expression,
            Self::EmbeddedSchema { .. } => Category::Catalog,
        }
    }
}
