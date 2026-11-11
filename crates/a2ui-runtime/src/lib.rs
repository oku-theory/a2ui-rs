//! Renderer-side runtime for the A2UI v1.0 protocol.
//!
//! The crate holds surface state and evaluates bindings without rendering,
//! async runtime or I/O, so any UI framework (and the browser) can embed it.
//!
//! Key items: [`Pointer`] (normalized RFC 6901 pointers), [`DataModel`]
//! (pointer-addressed JSON), [`Write`] (one committed change), [`Error`]
//! (classified by [`Category`]), [`Watchers`] (observers a write notifies),
//! [`resolve_path`] (scoped relative paths) and [`parse_template`] (`${...}`
//! expression templates), [`common_types_schema`] and [`agent_to_renderer_schema`] (the
//! embedded v1.0 schemas), [`validate_common_type`] (a value against one common type) and
//! [`Catalog`] (a loaded v1.0 catalog, with [`ComponentDef`], [`FunctionDef`] and
//! [`AllowedCallers`]).

mod bundle;
mod catalog;
mod context;
mod data;
mod error;
mod expression;
mod ident;
mod limits;
mod pointer;
mod schema;
mod subscribe;
mod validator;
pub use subscribe::Watchers;

pub use catalog::{AllowedCallers, Catalog, ComponentDef, ComponentShape, FunctionDef};
pub use context::resolve_path;
pub use data::{DataModel, Write};
pub use error::{Category, Error};
pub use expression::{Expr, MAX_DEPTH, MAX_TEMPLATE_CHARS, MAX_TEMPLATE_PARTS, parse_template};
pub use pointer::{MAX_LIST_INDEX, Pointer};
pub use schema::{agent_to_renderer_schema, common_types_schema, validate_common_type};
