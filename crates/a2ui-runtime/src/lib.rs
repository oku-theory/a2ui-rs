//! Renderer-side runtime for the A2UI v1.0 protocol.
//!
//! The crate holds surface state and evaluates bindings without rendering,
//! async runtime or I/O, so any UI framework (and the browser) can embed it.
//!
//! Key items: [`Pointer`] (normalized RFC 6901 pointers), [`DataModel`]
//! (pointer-addressed JSON), [`Write`] (one committed change), [`Error`]
//! (classified by [`Category`]), [`Watchers`] (observers a write notifies),
//! [`resolve_path`] (scoped relative paths) and [`parse_template`] (`${...}`
//! expression templates).

mod context;
mod data;
mod error;
mod expression;
mod pointer;
mod subscribe;
pub use subscribe::Watchers;

pub use context::resolve_path;
pub use data::{DataModel, Write};
pub use error::{Category, Error};
pub use expression::{Expr, MAX_DEPTH, MAX_TEMPLATE_CHARS, MAX_TEMPLATE_PARTS, parse_template};
pub use pointer::{MAX_LIST_INDEX, Pointer};
