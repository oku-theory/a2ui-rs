//! The runner for each conformance action.

mod catalog_input;
mod common_type;
mod data_model;
mod expression;
mod resolve_path;
mod round_trip;
mod schema_doc;

use crate::case::Case;

/// Runs one case; `Err` carries the failure message.
pub type Handler = fn(&Case) -> Result<(), String>;

/// The runner for `action`, or `None` when no task has implemented it yet.
pub fn handler(action: &str) -> Option<Handler> {
    match action {
        "data_model" => Some(data_model::run),
        "resolve_path" => Some(resolve_path::run),
        "parse_expression_template" => Some(expression::run),
        "validate_common_type" => Some(common_type::run),
        "common_types_schema" => Some(schema_doc::run_common_types),
        "agent_to_renderer_schema" => Some(schema_doc::run_agent_to_renderer),
        "round_trip" => Some(round_trip::run),
        _ => None,
    }
}
