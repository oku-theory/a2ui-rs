//! Errors raised by the runtime, each tagged with a conformance category.

use snafu::{Backtrace, Snafu};

/// The conformance category of an [`Error`], as the upstream suites name it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Category {
    /// A data model read or write that cannot be satisfied at its path.
    Data,
}

impl Category {
    /// The name the conformance schema uses, such as `"DataError"`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Data => "DataError",
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
}

impl Inner {
    fn category(&self) -> Category {
        match self {
            Self::ForbiddenSegment { .. }
            | Self::InvalidPointer { .. }
            | Self::TraversePrimitive { .. }
            | Self::NonNumericSegment { .. }
            | Self::IndexOutOfRange { .. } => Category::Data,
        }
    }
}
