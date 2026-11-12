use rstest::rstest;
use serde_json::{Value, json};
use speculoos::prelude::*;

use super::super::test_support::{catalog_error, load, v10};
use crate::{AllowedCallers, Category};

#[test]
fn component_def_surface_is_reserved() {
    let document = v10(json!({"components": {"Surface": {"type": "object"}}}));
    assert_that!(catalog_error(load(document))).contains("'Surface' is reserved");
}

#[rstest]
#[case(
    json!({"components": {"My-Component": {"type": "object"}}}),
    "Invalid UAX #31 component identifier: 'My-Component'"
)]
#[case(
    json!({"components": {"V": {"properties": {"prop—name": {}}}}}),
    "Invalid UAX #31 property identifier: 'prop—name' in component 'V'"
)]
#[case(
    json!({"components": {"V": {"allOf": [{"properties": {"bad-name": {}}}]}}}),
    "Invalid UAX #31 property identifier: 'bad-name'"
)]
#[case(
    json!({"functions": {"func֊name": {"returnType": "string"}}}),
    "Invalid UAX #31 function identifier: 'func֊name'"
)]
#[case(
    json!({"functions": {"@index": {"returnType": "number"}}}),
    "Invalid UAX #31 function identifier: '@index'"
)]
#[case(
    json!({"functions": {"f": {"parameters": {"123invalid_arg": {"type": "string"}}}}}),
    "Invalid UAX #31 argument identifier: '123invalid_arg' in function 'f'"
)]
#[case(
    json!({"functions": {"f": {"properties": {"args": {"properties": {"bad-arg": {}}}}}}}),
    "Invalid UAX #31 argument identifier: 'bad-arg'"
)]
fn entry_invalid_identifier_is_catalog_error(#[case] extra: Value, #[case] message: &str) {
    assert_that!(catalog_error(load(v10(extra)))).contains(message);
}

#[test]
fn entry_non_ascii_identifiers_are_accepted() {
    let document = v10(json!({
        "components": {"Component_ℓ": {
            "type": "object",
            "properties": {"property_ℓ": {"type": "string"}}
        }},
        "functions": {"func_ℓ": {
            "returnType": "string",
            "parameters": {"arg_ℓ": {"type": "string"}}
        }}
    }));
    assert_that!(load(document)).is_ok();
}

#[test]
fn entry_shorthand_arguments_named_like_keywords_are_accepted() {
    let document = v10(json!({"functions": {"f": {
        "returnType": "string",
        "parameters": {"type": {"type": "string"}, "properties": {"type": "object"}}
    }}}));
    assert_that!(load(document)).is_ok();
}

#[rstest]
#[case("agentOnly")]
#[case("rendererOrAgent")]
fn function_def_user_activation_needs_renderer_only(#[case] callers: &str) {
    let document = v10(json!({"functions": {"openUrl": {
        "returnType": "void", "allowedCallers": callers, "requiresUserActivation": true
    }}}));
    assert_that!(catalog_error(load(document))).contains("requiresUserActivation");
}

#[test]
fn function_def_user_activation_is_checked_outside_the_union() {
    let document = v10(json!({
        "functions": {
            "good": {"returnType": "void"},
            "bad": {
                "returnType": "void",
                "allowedCallers": "agentOnly",
                "requiresUserActivation": true
            }
        },
        "$defs": {"anyFunction": {"oneOf": [{"$ref": "#/functions/good"}]}}
    }));
    assert_that!(catalog_error(load(document))).contains("requiresUserActivation");
}

#[test]
fn function_def_omitted_callers_are_renderer_only() {
    let document = v10(json!({"functions": {"copy": {
        "returnType": "void", "requiresUserActivation": true
    }}}));
    let catalog = load(document).unwrap();
    assert_that!(catalog.function("copy").unwrap().allowed_callers())
        .is_equal_to(AllowedCallers::RendererOnly);
}

#[rstest]
#[case(
    json!({"functions": {"f": {"allowedCallers": "everyone"}}}),
    "Malformed catalog: function 'f' has an unknown allowedCallers"
)]
#[case(
    json!({"functions": {"f": {"requiresUserActivation": "yes"}}}),
    "Malformed catalog: requiresUserActivation of function 'f' must be a boolean"
)]
#[case(json!({"functions": {"f": 3}}), "function 'f' must be a schema object")]
#[case(
    json!({"components": {"A": {"allowedParents": "Row"}}}),
    "`allowedParents` of component 'A'"
)]
#[case(
    json!({"components": {"A": {"allowedChildren": [1]}}}),
    "`allowedChildren` of component 'A'"
)]
fn entry_malformed_attribute_is_catalog_error(#[case] extra: Value, #[case] fragment: &str) {
    assert_that!(catalog_error(load(v10(extra)))).contains(fragment);
}

#[test]
fn component_def_allowed_parents_are_exposed() {
    let document = v10(json!({"components": {"Card": {
        "type": "object", "allowedParents": ["Surface", "Row"]
    }}}));
    let catalog = load(document).unwrap();
    let parents = catalog.component("Card").unwrap().allowed_parents();
    assert_that!(parents).is_equal_to(Some(&["Surface".to_owned(), "Row".to_owned()][..]));
}

#[test]
fn component_def_allowed_children_are_exposed() {
    let document = v10(json!({"components": {"Card": {
        "type": "object", "allowedChildren": ["Button"]
    }}}));
    let catalog = load(document).unwrap();
    let children = catalog.component("Card").unwrap().allowed_children();
    assert_that!(children).is_equal_to(Some(&["Button".to_owned()][..]));
}

#[test]
fn function_def_return_type_is_exposed() {
    let document = v10(json!({"functions": {"fetchData": {
        "returnType": "object", "allowedCallers": "agentOnly"
    }}}));
    let catalog = load(document).unwrap();
    assert_that!(catalog.function("fetchData").unwrap().return_type()).is_equal_to(Some("object"));
}

#[test]
fn component_def_metadata_bad_key_is_validation_error() {
    let document = v10(json!({"components": {"Card": {
        "type": "object", "metadata": {"extensions": {"com.example": 1}}
    }}}));
    let err = load(document).unwrap_err();
    assert_that!(err.category()).is_equal_to(Category::Validation);
}

#[test]
fn component_def_metadata_is_exposed() {
    let metadata = json!({"extensions": {"com_example": {"track": true}}});
    let document = v10(json!({"components": {"Text": {
        "type": "object", "metadata": metadata.clone()
    }}}));
    let catalog = load(document).unwrap();
    assert_that!(catalog.component("Text").unwrap().metadata()).is_equal_to(Some(&metadata));
}

#[test]
fn entry_invalid_identifier_without_owner_has_no_context() {
    let extra = json!({"components": {"My-Component": {"type": "object"}}});
    assert_that!(catalog_error(load(v10(extra))))
        .is_equal_to("Invalid UAX #31 component identifier: 'My-Component'".to_owned());
}
