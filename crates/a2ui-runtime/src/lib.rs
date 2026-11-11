//! Renderer-side runtime for the A2UI v1.0 protocol.
//!
//! The crate holds surface state and evaluates bindings without rendering,
//! async runtime or I/O, so any UI framework (and the browser) can embed it.
