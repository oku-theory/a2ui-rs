//! The reserved `@` keys of a component instance, spelled once for every walk.
//!
//! A leaf module: the call cap (`limits`), the nested-call walk (`catalog::validate`) and the
//! schemas that name a call (`schema`, `catalog::validation_schema`) all import it.

/// The key naming a call's function, `FunctionCommon`'s one required property.
///
/// Every place that finds a call reads it, so the component call cap and the validator walk
/// cannot disagree on what a call is.
pub(crate) const CALL_KEY: &str = "@call";

/// The key naming a data binding's path, `DataBinding`'s one required property.
pub(crate) const PATH_KEY: &str = "@path";

#[cfg(test)]
#[path = "keys_tests.rs"]
mod tests;
