//! The runner for each conformance action.

use crate::case::Case;

/// Runs one case; `Err` carries the failure message.
pub type Handler = fn(&Case) -> Result<(), String>;

/// The runner for `action`, or `None` when no task has implemented it yet.
pub fn handler(_action: &str) -> Option<Handler> {
    None
}
