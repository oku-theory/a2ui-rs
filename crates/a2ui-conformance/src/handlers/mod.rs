//! The runner for each conformance action.

mod data_model;
mod expression;
mod resolve_path;

use crate::case::Case;

/// Runs one case; `Err` carries the failure message.
pub type Handler = fn(&Case) -> Result<(), String>;

/// The runner for `action`, or `None` when no task has implemented it yet.
pub fn handler(action: &str) -> Option<Handler> {
    match action {
        "data_model" => Some(data_model::run),
        "resolve_path" => Some(resolve_path::run),
        "parse_expression_template" => Some(expression::run),
        _ => None,
    }
}
