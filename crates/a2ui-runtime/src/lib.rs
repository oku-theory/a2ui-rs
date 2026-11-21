//! Renderer-side runtime for the A2UI v1.0 protocol.
//!
//! The crate holds surface state and evaluates bindings without rendering,
//! async runtime or I/O, so any UI framework (and the browser) can embed it.
//!
//! Key items: [`Pointer`] (normalized RFC 6901 pointers), [`DataModel`]
//! (pointer-addressed JSON), [`Write`] (one committed change), [`Error`]
//! (classified by [`Category`] and [`ErrorCode`]), [`Watchers`] (observers a write notifies),
//! [`resolve_path`] (scoped relative paths) and [`parse_template`] (`${...}`
//! expression templates), [`common_types_schema`] and [`agent_to_renderer_schema`] (the
//! embedded v1.0 schemas), [`validate_common_type`] (a value against one common type) and
//! [`Catalog`] (a loaded v1.0 catalog, with [`ComponentDef`], [`FunctionDef`] and
//! [`AllowedCallers`] and [`ComponentShape`]) and [`CatalogLookup`] (finds the catalogs nested
//! calls run in), derived with [`Catalog::derive`] from [`DeriveEntry`] values into a
//! [`Derivation`], and [`AgentMessage`] (one message read from JSON, with its six bodies, whose
//! faults carry a [`Detail`]), and [`MessageProcessor`] (applies messages to each [`Surface`],
//! checked against a [`CatalogSet`] within [`StateLimits`], returning an [`Outcome`]).

mod bundle;
mod catalog;
mod context;
mod data;
mod detail;
mod error;
mod expression;
mod ident;
mod keys;
mod limits;
mod message;
mod pointer;
mod processor;
mod quoted;
mod schema;
mod subscribe;
mod surface;
mod validator;
pub use subscribe::Watchers;

pub use catalog::{
    AllowedCallers, Catalog, CatalogLookup, CatalogSet, ComponentDef, ComponentShape, Derivation,
    DeriveEntry, FunctionDef,
};
pub use context::resolve_path;
pub use data::{DataModel, Write};
pub use detail::{Detail, DetailCode};
pub use error::{Category, Error, ErrorCode};
pub use expression::{Expr, MAX_DEPTH, MAX_TEMPLATE_CHARS, MAX_TEMPLATE_PARTS, parse_template};
pub use message::{
    AgentFunctionResponse, AgentMessage, Batch, CallRendererFunction, CreateSurface, DeleteSurface,
    MessageLimits, RendererMessage, UpdateComponents, UpdateDataModel,
};
pub use pointer::{MAX_LIST_INDEX, Pointer};
pub use processor::{MessageProcessor, Outcome, StateLimits};
pub use schema::{agent_to_renderer_schema, common_types_schema, validate_common_type};
pub use surface::Surface;
