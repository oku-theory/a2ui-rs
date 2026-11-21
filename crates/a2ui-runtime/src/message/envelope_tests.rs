use speculoos::prelude::*;

use super::*;
use crate::schema::agent_to_renderer_schema;

#[test]
fn action_def_names_each_schema_branch() -> Result<(), Error> {
    let defs = agent_to_renderer_schema()?
        .get("$defs")
        .and_then(Value::as_object);
    for action in Action::ALL {
        assert_that!(defs.is_some_and(|defs| defs.contains_key(action.def()))).is_true();
    }
    Ok(())
}
