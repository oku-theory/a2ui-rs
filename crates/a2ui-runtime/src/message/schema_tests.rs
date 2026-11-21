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
