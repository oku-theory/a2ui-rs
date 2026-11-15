use rstest::rstest;
use serde_json::json;
use speculoos::prelude::*;

use super::*;
use crate::schema::common_types_schema;

#[rstest]
#[case::call("FunctionCommon", CALL_KEY)]
#[case::path("DataBinding", PATH_KEY)]
fn reserved_key_common_type_requires_it(#[case] common_type: &str, #[case] key: &str) {
    let schema = common_types_schema().unwrap();
    let required = schema["$defs"][common_type]["required"]
        .as_array()
        .unwrap()
        .clone();
    assert_that!(required).contains(json!(key));
}
