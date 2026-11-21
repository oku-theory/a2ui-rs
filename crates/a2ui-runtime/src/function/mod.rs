//! Function implementations a host binds to the functions its catalogs declare.
//!
//! `registry` holds the bindings; `context` what an implementation sees of the call.

mod context;
mod registry;

pub use context::CallContext;
pub use registry::{Args, FunctionRegistry, Implementation};
