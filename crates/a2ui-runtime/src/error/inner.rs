//! The variants behind [`Error`](super::Error), each with its message and class.

use snafu::{Backtrace, Snafu};

use super::process::ProcessFailure;
use super::{Category, ErrorCode};
use crate::quoted::{Quoted, Reason};

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

    #[snafu(display("Global recursion limit exceeded: more than {limit} nested containers"))]
    ValueDepth { limit: usize, backtrace: Backtrace },

    #[snafu(display("functionCall depth {depth} exceeds the limit of {limit}"))]
    CallDepth {
        depth: usize,
        limit: usize,
        backtrace: Backtrace,
    },

    #[snafu(display("{failure}"))]
    Process {
        failure: Box<ProcessFailure>,
        backtrace: Backtrace,
    },
}

impl Inner {
    /// The category and code of this error, decided per variant.
    pub(super) fn class(&self) -> (Category, ErrorCode) {
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
            Self::Process { failure, .. } => (failure.category(), failure.code()),
        }
    }
}

#[cfg(test)]
#[path = "inner_tests.rs"]
mod tests;
