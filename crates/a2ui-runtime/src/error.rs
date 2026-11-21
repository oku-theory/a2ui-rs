//! Runtime errors, each tagged with a conformance category and an A2UI code.
//!
//! Input a message quotes is held as [`Quoted`] (escaped, capped) from the moment the error is
//! built, alone or as a piece of runtime wording, a [`Reason`] that only `reason!` composes.

use std::fmt;

use snafu::{Backtrace, ErrorCompat, Snafu};

use crate::detail::Context;
use crate::quoted::{Quoted, Reason};

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
}

#[derive(Debug, Snafu)]
#[snafu(visibility(pub(crate)))]
pub(crate) enum Inner {
    #[snafu(display("Forbidden path segment '{segment}' in '{path}'"))]
    ForbiddenSegment {
        path: Quoted,
        segment: Quoted,
        backtrace: Backtrace,
    },

    #[snafu(display("Invalid path syntax '{path}': {reason}"))]
    InvalidPointer {
        path: Quoted,
        reason: &'static str,
        backtrace: Backtrace,
    },

    #[snafu(display("Cannot set path '{path}': segment '{segment}' holds a primitive value"))]
    TraversePrimitive {
        path: Quoted,
        segment: Quoted,
        backtrace: Backtrace,
    },

    #[snafu(display("Cannot set path '{path}': non-numeric segment '{segment}' in a list"))]
    NonNumericSegment {
        path: Quoted,
        segment: Quoted,
        backtrace: Backtrace,
    },

    #[snafu(display("Cannot set path '{path}': list index '{segment}' exceeds the limit"))]
    IndexOutOfRange {
        path: Quoted,
        segment: Quoted,
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
        message: Quoted,
        backtrace: Backtrace,
    },

    #[snafu(display("Schema does not compile: {message}"))]
    SchemaBuild {
        message: Quoted,
        backtrace: Backtrace,
    },

    #[snafu(display("{message} at '{at}'"))]
    SchemaViolation {
        at: Quoted,
        message: Quoted,
        backtrace: Backtrace,
    },

    #[snafu(display("Unknown common type '{definition}'"))]
    UnknownDefinition {
        definition: Quoted,
        backtrace: Backtrace,
    },

    #[snafu(display("Value nesting exceeds {limit} levels"))]
    ValueLimit { limit: usize, backtrace: Backtrace },

    #[snafu(display("Component exceeds {limit} {what}"))]
    ComponentLimit {
        what: &'static str,
        limit: usize,
        backtrace: Backtrace,
    },

    #[snafu(display("Catalog document is not valid JSON: {message}"))]
    CatalogJson {
        message: Quoted,
        backtrace: Backtrace,
    },

    #[snafu(display("Malformed catalog: {reason}"))]
    MalformedCatalog {
        reason: Reason,
        backtrace: Backtrace,
    },

    #[snafu(display("catalog_id must be provided or exist in catalog schema"))]
    MissingCatalogId { backtrace: Backtrace },

    #[snafu(display(
        "Catalog protocol version '{version}' is not supported; this runtime implements 1.0"
    ))]
    UnsupportedVersion {
        version: Quoted,
        backtrace: Backtrace,
    },

    #[snafu(display(
        "Unsupported external $ref '{reference}' at {pointer}: a v1.0 catalog may reference \
         only itself and common_types.json"
    ))]
    ExternalRef {
        reference: Quoted,
        pointer: Quoted,
        backtrace: Backtrace,
    },

    #[snafu(display("Unsupported '{keyword}' at {pointer}: {reason}"))]
    ForbiddenKeyword {
        keyword: &'static str,
        pointer: Quoted,
        reason: &'static str,
        backtrace: Backtrace,
    },

    #[snafu(display("Catalog $defs '{name}' shadows the common type of the same name"))]
    DefsShadowCommonType { name: Quoted, backtrace: Backtrace },

    #[snafu(display("Catalog exceeds {limit} {what}"))]
    CatalogLimit {
        what: Reason,
        limit: usize,
        backtrace: Backtrace,
    },

    #[snafu(display("Catalog '{catalog}' has no {kind} '{name}' to derive from"))]
    DeriveUnknownEntry {
        catalog: Quoted,
        kind: &'static str,
        name: Quoted,
        backtrace: Backtrace,
    },

    #[snafu(display("Derivation lists {kind} '{name}' more than once"))]
    DeriveDuplicateEntry {
        kind: &'static str,
        name: Quoted,
        backtrace: Backtrace,
    },

    #[snafu(display("Component name 'Surface' is reserved for the surface root"))]
    ReservedComponent { backtrace: Backtrace },

    #[snafu(display("{reason}"))]
    InvalidIdentifier {
        reason: Reason,
        backtrace: Backtrace,
    },

    #[snafu(display(
        "Function '{function}' sets requiresUserActivation, so its allowedCallers must be \
         rendererOnly"
    ))]
    UserActivation {
        function: Quoted,
        backtrace: Backtrace,
    },

    #[snafu(display("Invalid component: {reason}"))]
    ComponentShape {
        reason: Reason,
        backtrace: Backtrace,
    },

    #[snafu(display("Component has no 'component' type name"))]
    MissingComponentType { backtrace: Backtrace },

    #[snafu(display("Unrecognized component type '{component}'"))]
    UnknownComponent {
        component: Quoted,
        backtrace: Backtrace,
    },

    #[snafu(display("{what} '{name}' must be a valid UAX #31 identifier"))]
    BadIdentifier {
        what: &'static str,
        name: Quoted,
        backtrace: Backtrace,
    },

    #[snafu(display("Key '{key}' is reserved: only '@path' and '@call' may start with one '@'"))]
    ReservedKey { key: Quoted, backtrace: Backtrace },

    #[snafu(display("Invalid function call: {reason}"))]
    CallShape {
        reason: Reason,
        backtrace: Backtrace,
    },

    #[snafu(display("System function '{function}' cannot name a catalogId"))]
    SystemFunctionCatalog {
        function: Quoted,
        backtrace: Backtrace,
    },

    #[snafu(display("Unrecognized function '{function}'"))]
    UnknownFunction {
        function: Quoted,
        backtrace: Backtrace,
    },

    #[snafu(display("Function '{function}' is agentOnly and cannot be called from a component"))]
    AgentOnlyCall {
        function: Quoted,
        backtrace: Backtrace,
    },

    #[snafu(display("{reason}"))]
    MessageEnvelope {
        reason: Reason,
        backtrace: Backtrace,
    },

    #[snafu(display("{reason}"))]
    MessageField {
        reason: Reason,
        backtrace: Backtrace,
    },

    #[snafu(display("A2UI message exceeds the {limit}-byte limit"))]
    MessageTooLarge { limit: usize, backtrace: Backtrace },

    #[snafu(display("Invalid JSON in A2UI message: {message}"))]
    MessageJson {
        message: Quoted,
        backtrace: Backtrace,
    },

    #[snafu(display("{reason}"))]
    FunctionCallShape {
        reason: Reason,
        backtrace: Backtrace,
    },

    #[snafu(display("Global recursion limit exceeded: more than 50 nested containers"))]
    ValueDepth { backtrace: Backtrace },

    #[snafu(display("functionCall depth {depth} exceeds the limit of 5"))]
    CallDepth { depth: usize, backtrace: Backtrace },
}

impl Inner {
    /// The category and code of this error, decided per variant.
    fn class(&self) -> (Category, ErrorCode) {
        match self {
            Self::ForbiddenSegment { .. }
            | Self::TraversePrimitive { .. }
            | Self::NonNumericSegment { .. }
            | Self::IndexOutOfRange { .. } => (Category::Data, ErrorCode::DataError),
            Self::ExpressionSyntax { .. } => (Category::Parse, ErrorCode::ExpressionError),
            Self::MessageJson { .. } => (Category::Parse, ErrorCode::ValidationFailed),
            Self::ExpressionLimit { .. } => (Category::Expression, ErrorCode::ExpressionError),
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
            | Self::DeriveUnknownEntry { .. }
            | Self::DeriveDuplicateEntry { .. }
            | Self::ReservedComponent { .. }
            | Self::InvalidIdentifier { .. }
            | Self::UserActivation { .. } => (Category::Catalog, ErrorCode::CatalogError),
            Self::InvalidPointer { .. }
            | Self::SchemaViolation { .. }
            | Self::UnknownDefinition { .. }
            | Self::ValueLimit { .. }
            | Self::ComponentLimit { .. }
            | Self::ComponentShape { .. }
            | Self::MissingComponentType { .. }
            | Self::UnknownComponent { .. }
            | Self::BadIdentifier { .. }
            | Self::ReservedKey { .. }
            | Self::CallShape { .. }
            | Self::SystemFunctionCatalog { .. }
            | Self::UnknownFunction { .. }
            | Self::AgentOnlyCall { .. }
            | Self::MessageEnvelope { .. }
            | Self::MessageTooLarge { .. }
            | Self::MessageField { .. } => (Category::Validation, ErrorCode::ValidationFailed),
            Self::FunctionCallShape { .. } => {
                (Category::Validation, ErrorCode::InvalidFunctionCall)
            }
            Self::ValueDepth { .. } | Self::CallDepth { .. } => {
                (Category::Recursion, ErrorCode::RecursionLimitExceeded)
            }
        }
    }
}

#[cfg(test)]
#[path = "error_tests.rs"]
mod tests;
