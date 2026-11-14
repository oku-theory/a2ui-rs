use rstest::rstest;
use serde_json::{Value, json};
use speculoos::prelude::*;

use super::super::test_support::{load, nested};
use crate::limits::MAX_JSON_DEPTH;
use crate::{Catalog, Category, Error};

const ID: &str = "https://example.com/catalog";
const SECOND: &str = "https://example.com/second";

fn catalog() -> Catalog {
    let string = json!({"$ref": "common_types.json#/$defs/DynamicString"});
    load(json!({
        "catalogId": ID,
        "protocolVersion": "1.0",
        "components": {
            "Text": {
                "type": "object",
                "properties": {"component": {"const": "Text"}, "text": string},
                "required": ["component", "text"]
            },
            "Closed": {
                "type": "object",
                "properties": {"component": {"const": "Closed"}, "label": {"type": "string"}},
                "unevaluatedProperties": false
            },
            "Field": {
                "type": "object",
                "allOf": [{"$ref": "common_types.json#/$defs/ComponentCommon"}],
                "properties": {"component": {"const": "Field"}, "value": string},
                "required": ["component", "value"],
                "unevaluatedProperties": false
            },
            "Any": {"type": "object", "properties": {"component": {"const": "Any"}}}
        },
        "functions": {
            "upper": {
                "type": "object",
                "returnType": "string",
                "properties": {
                    "@call": {"const": "upper"},
                    "args": {
                        "type": "object",
                        "properties": {"value": string},
                        "required": ["value"],
                        "unevaluatedProperties": false
                    }
                },
                "required": ["@call", "args"]
            },
            "fetch": {"type": "object", "returnType": "string", "allowedCallers": "agentOnly"},
            "both": {
                "type": "object",
                "returnType": "string",
                "allowedCallers": "rendererOrAgent"
            }
        }
    }))
    .unwrap()
}

fn second() -> Catalog {
    load(json!({
        "catalogId": SECOND,
        "protocolVersion": "1.0",
        "functions": {
            "shout": {
                "type": "object",
                "returnType": "string",
                "properties": {
                    "@call": {"const": "shout"},
                    "args": {
                        "type": "object",
                        "properties": {
                            "value": {"$ref": "common_types.json#/$defs/DynamicString"}
                        },
                        "required": ["value"],
                        "unevaluatedProperties": false
                    }
                },
                "required": ["@call", "args"]
            },
            "secret": {"type": "object", "returnType": "string", "allowedCallers": "agentOnly"}
        }
    }))
    .unwrap()
}

fn text(value: Value) -> Value {
    json!({"id": "t1", "component": "Text", "text": value})
}

/// A `Text` whose `text` calls `f` in a catalog that is not loaded, with `args`.
fn foreign(args: Value) -> Value {
    text(json!({"@call": "f", "catalogId": "elsewhere", "args": args}))
}

fn any(key: &str, value: Value) -> Value {
    let mut component = json!({"id": "a1", "component": "Any"});
    component[key] = value;
    component
}

fn validate(component: &Value) -> Result<(), Error> {
    let catalog = catalog();
    catalog.validate_component(component, ID, &catalog)
}

fn validate_in_both(component: &Value) -> Result<(), Error> {
    let catalogs = [catalog(), second()];
    catalogs[0].validate_component(component, ID, &catalogs[..])
}

fn validation_message(result: Result<(), Error>) -> String {
    let err = result.unwrap_err();
    assert_that!(err.category()).is_equal_to(Category::Validation);
    err.to_string()
}

#[rstest]
#[case::literal(text(json!("hello")))]
#[case::binding(text(json!({"@path": "/a"})))]
#[case::call_here(text(json!({"@call": "upper", "args": {"value": "hi"}})))]
#[case::call_naming_this_catalog(
    text(json!({"@call": "upper", "catalogId": ID, "args": {"value": "hi"}}))
)]
#[case::call_in_unknown_catalog(
    text(json!({"@call": "anything", "catalogId": "elsewhere", "args": {"x": 1, "loud": true}}))
)]
#[case::renderer_or_agent(text(json!({"@call": "both"})))]
#[case::index_in_args(
    text(json!({"@call": "upper", "args": {"value": {"@call": "@index", "args": {"offset": 1}}}}))
)]
#[case::escaped_at_key(
    foreign(json!({"v": {"@@path": 1}}))
)]
#[case::call_in_array(any("items", json!([{"@call": "upper", "args": {"value": "hi"}}])))]
#[case::closed_declared_key(json!({"id": "c1", "component": "Closed", "label": "x"}))]
#[case::closed_undeclared_catalog_id(json!({"id": "c1", "component": "Closed", "catalogId": ID}))]
#[case::closed_undeclared_id(json!({"id": "my-button", "component": "Closed"}))]
#[case::declared_envelope(json!({
    "id": "f1",
    "component": "Field",
    "value": {"@path": "/v"},
    "metadata": {"extensions": {"com_example": 1}}
}))]
fn validate_component_valid_component_is_accepted(#[case] component: Value) {
    assert_that!(validate(&component)).is_ok();
}

#[rstest]
#[case::not_an_object(json!("Text"), "Invalid component: a component is a JSON object")]
#[case::missing_type(json!({"id": "t1", "text": "x"}), "Component has no 'component' type name")]
#[case::unknown_type(
    json!({"id": "t1", "component": "Nope"}),
    "Unrecognized component type 'Nope'"
)]
#[case::bad_type_name(
    json!({"id": "t1", "component": "Te-xt"}),
    "component type 'Te-xt' must be a valid UAX #31 identifier"
)]
#[case::top_level_reserved_key(
    json!({"id": "t1", "component": "Text", "text": "x", "@x": 1}),
    "Key '@x' is reserved: only '@path' and '@call' may start with one '@'"
)]
#[case::top_level_bare_at(
    json!({"id": "t1", "component": "Text", "text": "x", "@": 1}),
    "Key '@' is reserved: only '@path' and '@call' may start with one '@'"
)]
#[case::schema_violation(
    json!({"id": "t1", "component": "Text"}),
    "\"text\" is a required property at ''"
)]
#[case::closed_extra_key(
    json!({"id": "c1", "component": "Closed", "bogus": 1}),
    "Unevaluated properties are not allowed ('bogus' was unexpected) at ''"
)]
#[case::envelope_id_not_string(
    json!({"id": 3, "component": "Closed"}),
    "Invalid component: id must be a string"
)]
#[case::envelope_catalog_id_not_string(
    json!({"id": "c1", "component": "Closed", "catalogId": 3}),
    "Invalid component: catalogId must be a string"
)]
#[case::envelope_metadata_bad_key(
    json!({"id": "c1", "component": "Closed", "metadata": {"extensions": {"a.b": 1}}}),
    "Additional properties are not allowed ('a.b' was unexpected)"
)]
#[case::unknown_function(
    text(json!({"@call": "notAFunction", "args": {}})),
    "Unrecognized function 'notAFunction'"
)]
#[case::unknown_function_named_here(
    text(json!({"@call": "notAFunction", "catalogId": ID, "args": {}})),
    "Unrecognized function 'notAFunction'"
)]
#[case::extra_arg(
    text(json!({"@call": "upper", "catalogId": ID, "args": {"value": "hi", "extra": 1}})),
    "Unevaluated properties are not allowed ('extra' was unexpected) at '/args'"
)]
#[case::extra_call_key(
    text(json!({"@call": "upper", "args": {"value": "hi"}, "bogus": 1})),
    "is not valid under any of the schemas listed in the 'oneOf' keyword at '/text'"
)]
#[case::agent_only(
    text(json!({"@call": "fetch", "args": {}})),
    "Function 'fetch' is agentOnly and cannot be called from a component"
)]
#[case::nested_unknown(
    text(json!({"@call": "upper", "args": {"value": {"@call": "notAFunction", "args": {}}}})),
    "Unrecognized function 'notAFunction'"
)]
#[case::nested_in_unknown_catalog(
    text(json!({
        "@call": "shout",
        "catalogId": "elsewhere",
        "args": {"value": {"@call": "notAFunction", "args": {}}}
    })),
    "Unrecognized function 'notAFunction'"
)]
#[case::nested_in_index(
    text(json!({"@call": "@index", "args": {"offset": {"@call": "notAFunction", "args": {}}}})),
    "Unrecognized function 'notAFunction'"
)]
#[case::call_in_array(
    any("items", json!(["x", {"@call": "notAFunction"}])),
    "Unrecognized function 'notAFunction'"
)]
#[case::bad_function_name_elsewhere(
    text(json!({"@call": "not-a-function", "catalogId": "elsewhere"})),
    "function 'not-a-function' must be a valid UAX #31 identifier"
)]
#[case::bad_arg_name_elsewhere(
    foreign(json!({"bad-arg": "x"})),
    "argument 'bad-arg' must be a valid UAX #31 identifier"
)]
#[case::args_not_object(
    foreign(json!(1)),
    "is not valid under any of the schemas listed in the 'oneOf' keyword at '/text'"
)]
#[case::foreign_args_call_not_string(
    foreign(json!({"v": {"@call": 5}})),
    "Invalid function call: @call must be a string"
)]
#[case::foreign_args_catalog_id_not_string(
    foreign(json!({"v": {"@call": "g", "catalogId": 5}})),
    "Invalid function call: catalogId of 'g' must be a string"
)]
#[case::foreign_args_args_not_object(
    foreign(json!({"v": {"@call": "g", "args": 1}})),
    "Invalid function call: args of 'g' must be an object"
)]
#[case::unknown_system_function(text(json!({"@call": "@foo"})), "Unrecognized function '@foo'")]
#[case::system_function_with_catalog(
    text(json!({"@call": "@index", "catalogId": "elsewhere"})),
    "System function '@index' cannot name a catalogId"
)]
#[case::index_extra_arg(
    text(json!({"@call": "@index", "args": {"offset": 1, "step": 2}})),
    "Unevaluated properties are not allowed ('step' was unexpected) at '/args'"
)]
#[case::reserved_key(
    text(json!({"@call": "shout", "catalogId": "elsewhere", "args": {"value": {"@unknown": 1}}})),
    "Key '@unknown' is reserved: only '@path' and '@call' may start with one '@'"
)]
fn validate_component_invalid_component_is_validation_error(
    #[case] component: Value,
    #[case] message: &str,
) {
    assert_that!(validation_message(validate(&component))).contains(message);
}

#[test]
fn validate_component_unqualified_call_in_unknown_default_checks_identifiers_only() {
    let catalog = catalog();
    let component = text(json!({"@call": "anything", "args": {}}));
    assert_that!(catalog.validate_component(&component, "elsewhere", &catalog)).is_ok();
}

#[test]
fn validate_component_call_naming_this_catalog_under_foreign_default_is_checked_here() {
    let catalog = catalog();
    let component = text(json!({"@call": "notAFunction", "catalogId": ID, "args": {}}));
    let result = catalog.validate_component(&component, "elsewhere", &catalog);
    assert_that!(validation_message(result)).contains("Unrecognized function 'notAFunction'");
}

#[test]
fn validate_component_call_into_second_known_catalog_is_accepted() {
    let component = text(json!({"@call": "shout", "catalogId": SECOND, "args": {"value": "hi"}}));
    assert_that!(validate_in_both(&component)).is_ok();
}

#[rstest]
#[case::unknown_function(
    json!({"@call": "upper", "catalogId": SECOND, "args": {"value": "hi"}}),
    "Unrecognized function 'upper'"
)]
#[case::extra_arg(
    json!({"@call": "shout", "catalogId": SECOND, "args": {"value": "hi", "extra": 1}}),
    "Unevaluated properties are not allowed ('extra' was unexpected) at '/args'"
)]
#[case::agent_only(
    json!({"@call": "secret", "catalogId": SECOND}),
    "Function 'secret' is agentOnly and cannot be called from a component"
)]
fn validate_component_bad_call_into_second_known_catalog_is_validation_error(
    #[case] call: Value,
    #[case] message: &str,
) {
    assert_that!(validation_message(validate_in_both(&text(call)))).contains(message);
}

#[test]
fn validate_component_call_into_unlisted_catalog_checks_identifiers_only() {
    let call = json!({"@call": "shout", "catalogId": SECOND, "args": {"value": "hi", "extra": 1}});
    assert_that!(validate(&text(call))).is_ok();
}

#[test]
fn validate_component_value_at_the_depth_cap_is_accepted() {
    let component = any("data", nested(MAX_JSON_DEPTH - 1));
    assert_that!(validate(&component)).is_ok();
}

#[test]
fn validate_component_value_past_the_depth_cap_is_rejected() {
    let component = any("data", nested(MAX_JSON_DEPTH));
    let message = validation_message(validate(&component));
    assert_that!(message).is_equal_to("Value nesting exceeds 64 levels".to_owned());
}
