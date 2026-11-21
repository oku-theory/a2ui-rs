use rstest::rstest;
use serde_json::json;
use speculoos::prelude::*;

use super::*;

#[rstest]
#[case::create(Action::CreateSurface)]
#[case::update_components(Action::UpdateComponents)]
#[case::update_data(Action::UpdateDataModel)]
#[case::delete(Action::DeleteSurface)]
#[case::call(Action::CallRendererFunction)]
#[case::response(Action::AgentFunctionResponse)]
fn message_schema_builds_for_each_action(#[case] action: Action) {
    assert_that!(validator_for(action).as_ref().is_ok()).is_true();
}

#[test]
fn message_schema_component_placeholder_accepts_any_object() {
    let message = json!({
        "version": "1.0",
        "updateComponents": {"surfaceId": "s", "components": [{"anything": 1}]}
    });
    assert_that!(check(Action::UpdateComponents, &message).is_ok()).is_true();
}

#[rstest]
#[case::delete(Action::DeleteSurface, json!({"version": "1.0", "deleteSurface": {}}))]
#[case::create(
    Action::CreateSurface,
    json!({"version": "1.0", "createSurface": {"surfaceId": 123}})
)]
fn message_schema_fault_without_a_surface_id_reports_an_empty_one(
    #[case] action: Action,
    #[case] message: serde_json::Value,
) {
    let error = check(action, &message).unwrap_err();

    assert_that!(error.surface_id()).is_equal_to(Some(""));
    assert_that!(error.path()).is_equal_to(Some("/surfaceId"));
}

#[test]
fn message_schema_agent_function_response_fault_reports_no_surface() {
    let message = json!({"version": "1.0", "agentFunctionResponse": {}});

    let error = check(Action::AgentFunctionResponse, &message).unwrap_err();

    assert_that!(error.surface_id()).is_none();
}
