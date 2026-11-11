//! Renderer-side runtime for the A2UI v1.0 protocol.
//!
//! The crate holds surface state and evaluates bindings without rendering,
//! async runtime or I/O, so any UI framework (and the browser) can embed it.
//!
//! Key items: [`Pointer`] (normalized RFC 6901 pointers), [`DataModel`]
//! (pointer-addressed JSON), [`Write`] (one committed change) and [`Error`]
//! (classified by [`Category`]).

mod data;
mod error;
mod pointer;

pub use data::{DataModel, Write};
pub use error::{Category, Error};
pub use pointer::{MAX_LIST_INDEX, Pointer};
