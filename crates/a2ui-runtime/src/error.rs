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
    /// A value that breaks a schema, such as a component its catalog rejects.
    Validation,
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
        }
    }
}

/// An error raised by the runtime; [`Error::category`] classifies it.
#[derive(Debug, Snafu)]
pub struct Error(Inner);

// `Result<_, Error>` stays under clippy's `result_large_err` threshold of 128 bytes.
const _: () = assert!(size_of::<Error>() <= 128);

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

    #[snafu(display("Schema does not compile: {message}"))]
    SchemaBuild {
        message: String,
        backtrace: Backtrace,
    },

    #[snafu(display("{message} at '{at}'"))]
    SchemaViolation {
        at: String,
        message: String,
        backtrace: Backtrace,
    },

    #[snafu(display("Unknown common type '{definition}'"))]
    UnknownDefinition {
        definition: String,
        backtrace: Backtrace,
    },

    #[snafu(display("Value nesting exceeds {limit} levels"))]
    ValueLimit { limit: usize, backtrace: Backtrace },

    #[snafu(display("Catalog document is not valid JSON: {message}"))]
    CatalogJson {
        message: String,
        backtrace: Backtrace,
    },

    #[snafu(display("Malformed catalog: {reason}"))]
    MalformedCatalog {
        reason: String,
        backtrace: Backtrace,
    },

    #[snafu(display("catalog_id must be provided or exist in catalog schema"))]
    MissingCatalogId { backtrace: Backtrace },

    #[snafu(display(
        "Catalog protocol version '{version}' is not supported; this runtime implements 1.0"
    ))]
    UnsupportedVersion {
        version: String,
        backtrace: Backtrace,
    },

    #[snafu(display(
        "Unsupported external $ref '{reference}' at {pointer}: a v1.0 catalog may reference \
         only itself and common_types.json"
    ))]
    ExternalRef {
        reference: String,
        pointer: String,
        backtrace: Backtrace,
    },

    #[snafu(display("Unsupported '{keyword}' at {pointer}: {reason}"))]
    ForbiddenKeyword {
        keyword: &'static str,
        pointer: String,
        reason: &'static str,
        backtrace: Backtrace,
    },

    #[snafu(display("Catalog $defs '{name}' shadows the common type of the same name"))]
    DefsShadowCommonType { name: String, backtrace: Backtrace },

    #[snafu(display("Catalog exceeds {limit} {what}"))]
    CatalogLimit {
        what: String,
        limit: usize,
        backtrace: Backtrace,
    },

    #[snafu(display("Component name 'Surface' is reserved for the surface root"))]
    ReservedComponent { backtrace: Backtrace },

    #[snafu(display("Invalid UAX #31 {kind} identifier: '{name}'{context}"))]
    InvalidIdentifier {
        kind: &'static str,
        name: String,
        context: String,
        backtrace: Backtrace,
    },

    #[snafu(display(
        "Function '{function}' sets requiresUserActivation, so its allowedCallers must be \
         rendererOnly"
    ))]
    UserActivation {
        function: String,
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
            Self::EmbeddedSchema { .. }
            | Self::SchemaBuild { .. }
            | Self::CatalogJson { .. }
            | Self::MalformedCatalog { .. }
            | Self::MissingCatalogId { .. }
            | Self::UnsupportedVersion { .. }
            | Self::ExternalRef { .. }
            | Self::ForbiddenKeyword { .. }
            | Self::DefsShadowCommonType { .. }
            | Self::CatalogLimit { .. }
            | Self::ReservedComponent { .. }
            | Self::InvalidIdentifier { .. }
            | Self::UserActivation { .. } => Category::Catalog,
            Self::SchemaViolation { .. }
            | Self::UnknownDefinition { .. }
            | Self::ValueLimit { .. } => Category::Validation,
        }
    }
}
