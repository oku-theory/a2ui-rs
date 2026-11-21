use rstest::rstest;
use serde_json::{Value, json};
use speculoos::prelude::*;

use super::*;
use crate::catalog::test_support::{load, v10};
use crate::{Category, DataModel, ErrorCode, Pointer};

const V: &str = "1.0";

/// A processor holding the empty catalog `c`.
fn processor() -> MessageProcessor {
    let mut catalogs = CatalogSet::new();
    catalogs.insert(load(v10(json!({}))).unwrap()).unwrap();
    MessageProcessor::new(catalogs)
}

/// The message `value` reads as.
fn message(value: Value) -> AgentMessage {
    AgentMessage::from_json(&value, &MessageLimits::default()).unwrap()
}

/// A `createSurface` for `id` on catalog `c`.
fn create(id: &str) -> AgentMessage {
    message(json!({"version": V, "createSurface": {"surfaceId": id, "catalogId": "c"}}))
}

/// An `updateDataModel` writing `value` at `path` on `main`.
fn write(path: &str, value: Value) -> AgentMessage {
    message(json!({"version": V,
        "updateDataModel": {"surfaceId": "main", "path": path, "value": value}}))
}

/// The processor's surface ids, in order.
fn ids(processor: &MessageProcessor) -> Vec<String> {
    (0..100)
        .map(|n| format!("s{n}"))
        .chain(["main".to_owned(), "missing_surface".to_owned()])
        .filter(|id| processor.surface(id).is_some())
        .collect()
}

/// Checks the class, message, surface id and path of `error`.
fn assert_error(
    error: &Error,
    class: (Category, ErrorCode),
    text: &str,
    surface: Option<&str>,
    path: Option<&str>,
) {
    assert_that!((error.category(), error.code())).is_equal_to(class);
    assert_that!(error.to_string()).is_equal_to(text.to_owned());
    assert_that!(error.surface_id()).is_equal_to(surface);
    assert_that!(error.path()).is_equal_to(path);
}

#[test]
fn create_surface_fields_read_back() {
    let mut processor = processor();
    let outcome = processor.handle(&message(json!({"version": V, "createSurface": {
        "surfaceId": "main", "catalogId": "c", "sendDataModel": true,
        "metadata": {"extensions": {}}, "dataModel": {"user": {"name": "Ada"}},
    }})));

    assert_that!(outcome.unwrap()).is_equal_to(Outcome::default());
    let surface = processor.surface("main").unwrap();
    assert_that!(surface.id()).is_equal_to("main");
    assert_that!(surface.catalog_id()).is_equal_to(Some("c"));
    assert_that!(surface.sends_data_model()).is_true();
    assert_that!(surface.metadata().cloned())
        .is_equal_to(json!({"extensions": {}}).as_object().cloned());
    assert_that!(surface.data_model())
        .is_equal_to(&DataModel::new(json!({"user": {"name": "Ada"}})));
}

#[test]
fn create_surface_without_catalog_id_has_no_default() {
    let mut processor = processor();
    processor
        .handle(&message(
            json!({"version": V, "createSurface": {"surfaceId": "main"}}),
        ))
        .unwrap();

    let surface = processor.surface("main").unwrap();
    assert_that!(surface.catalog_id()).is_none();
    assert_that!(surface.sends_data_model()).is_false();
    assert_that!(surface.metadata()).is_none();
    assert_that!(surface.data_model()).is_equal_to(&DataModel::default());
}

#[test]
fn create_surface_unknown_catalog_is_catalog_not_found() {
    let mut processor = processor();
    let error = processor
        .handle(&message(json!({"version": V,
            "createSurface": {"surfaceId": "main", "catalogId": "missing-catalog"}})))
        .unwrap_err();

    let class = (Category::Catalog, ErrorCode::CatalogNotFound);
    assert_error(
        &error,
        class,
        "Catalog not found: missing-catalog",
        Some("main"),
        None,
    );
    assert_that!(processor.surface("main").is_none()).is_true();
}

#[rstest]
#[case::known_catalog("c")]
#[case::unknown_catalog("missing-catalog")]
fn create_surface_twice_is_an_integrity_error(#[case] catalog: &str) {
    let mut processor = processor();
    processor.handle(&create("main")).unwrap();

    let error = processor
        .handle(&message(json!({"version": V,
            "createSurface": {"surfaceId": "main", "catalogId": catalog, "sendDataModel": true}})))
        .unwrap_err();

    let class = (Category::Integrity, ErrorCode::IntegrityError);
    assert_error(
        &error,
        class,
        "Surface 'main' already exists",
        Some("main"),
        None,
    );
    assert_that!(processor.surface("main").unwrap().sends_data_model()).is_false();
}

#[rstest]
#[case::data_model(json!({"updateDataModel": {"surfaceId": "missing_surface", "value": 1}}))]
#[case::components(json!({"updateComponents": {
    "surfaceId": "missing_surface", "components": [{"id": "root", "component": "Text"}],
}}))]
fn update_unknown_surface_is_an_integrity_error(#[case] mut body: Value) {
    body["version"] = json!(V);
    let mut processor = processor();

    let error = processor.handle(&message(body)).unwrap_err();

    let class = (Category::Integrity, ErrorCode::IntegrityError);
    let text = "Surface not found for message: missing_surface";
    assert_error(&error, class, text, Some("missing_surface"), None);
    assert_that!(ids(&processor).is_empty()).is_true();
}

#[test]
fn update_data_model_null_deletes() {
    let mut processor = processor();
    processor.handle(&create("main")).unwrap();
    processor
        .handle(&write("/a", json!({"b": 1, "c": 2})))
        .unwrap();

    processor.handle(&write("/a/b", Value::Null)).unwrap();

    assert_that!(processor.surface("main").unwrap().data_model())
        .is_equal_to(&DataModel::new(json!({"a": {"c": 2}})));
}

#[test]
fn update_data_model_root_replaces() {
    let mut processor = processor();
    processor.handle(&create("main")).unwrap();
    processor.handle(&write("/a", json!(1))).unwrap();

    processor
        .handle(&message(json!({"version": V,
            "updateDataModel": {"surfaceId": "main", "value": {"b": 2}}})))
        .unwrap();

    assert_that!(processor.surface("main").unwrap().data_model())
        .is_equal_to(&DataModel::new(json!({"b": 2})));
}

#[test]
fn update_data_model_root_null_resets_to_an_empty_object() {
    let mut processor = processor();
    processor.handle(&create("main")).unwrap();
    processor.handle(&write("/a", json!(1))).unwrap();

    processor.handle(&write("", Value::Null)).unwrap();

    assert_that!(processor.surface("main").unwrap().data_model())
        .is_equal_to(&DataModel::new(json!({})));
}

#[rstest]
#[case::scalar(json!(5))]
#[case::array(json!([1]))]
fn update_data_model_non_object_root_is_refused(#[case] value: Value) {
    let mut processor = processor();
    processor.handle(&create("main")).unwrap();
    processor.handle(&write("/a", json!(1))).unwrap();

    let error = processor.handle(&write("", value)).unwrap_err();

    let class = (Category::Validation, ErrorCode::ValidationFailed);
    assert_error(
        &error,
        class,
        "The data model root must be an object",
        Some("main"),
        None,
    );
    assert_that!(processor.surface("main").unwrap().data_model())
        .is_equal_to(&DataModel::new(json!({"a": 1})));
}

#[test]
fn update_data_model_bad_path_changes_nothing() {
    let mut processor = processor();
    processor.handle(&create("main")).unwrap();
    processor.handle(&write("/a", json!(1))).unwrap();

    let error = processor.handle(&write("/a/b/c", json!(2))).unwrap_err();

    let class = (Category::Data, ErrorCode::DataError);
    let text = "Cannot set path '/a/b/c': segment 'b' holds a primitive value";
    assert_error(&error, class, text, Some("main"), None);
    assert_that!(processor.surface("main").unwrap().data_model())
        .is_equal_to(&DataModel::new(json!({"a": 1})));
}

#[test]
fn delete_surface_removes_it() {
    let mut processor = processor();
    processor.handle(&create("main")).unwrap();

    let outcome = processor.handle(&message(
        json!({"version": V, "deleteSurface": {"surfaceId": "main"}}),
    ));

    assert_that!(outcome.unwrap()).is_equal_to(Outcome::default());
    assert_that!(processor.surface("main").is_none()).is_true();
}

#[test]
fn delete_unknown_surface_is_a_noop() {
    let mut processor = processor();
    processor.handle(&create("main")).unwrap();

    let outcome = processor.handle(&message(
        json!({"version": V, "deleteSurface": {"surfaceId": "s1"}}),
    ));

    assert_that!(outcome.unwrap()).is_equal_to(Outcome::default());
    assert_that!(ids(&processor)).is_equal_to(vec!["main".to_owned()]);
}

#[test]
fn handle_batch_stops_at_the_first_error() {
    let mut processor = processor();
    let create = json!({"version": V, "createSurface": {"surfaceId": "main", "catalogId": "c"}});
    let malformed = json!({"version": V, "deleteSurface": {}});

    let error = processor
        .handle_batch(&json!([create, create, malformed]))
        .unwrap_err();

    let class = (Category::Integrity, ErrorCode::IntegrityError);
    assert_error(
        &error,
        class,
        "Surface 'main' already exists",
        Some("main"),
        None,
    );
    assert_that!(error.message_index()).is_equal_to(Some(1));
    assert_that!(ids(&processor)).is_equal_to(vec!["main".to_owned()]);
}

#[test]
fn handle_batch_detail_less_error_names_its_index() {
    let mut processor = processor();
    let create = json!({"version": V, "createSurface": {"surfaceId": "main", "catalogId": "c"}});

    let error = processor
        .handle_batch(&json!({"messages": [create, 42]}))
        .unwrap_err();

    let class = (Category::Validation, ErrorCode::ValidationFailed);
    assert_error(
        &error,
        class,
        "A2UI message must be a JSON object",
        None,
        None,
    );
    assert_that!(error.details().is_empty()).is_true();
    assert_that!(error.message_index()).is_equal_to(Some(1));
    assert_that!(ids(&processor)).is_equal_to(vec!["main".to_owned()]);
}

#[test]
fn handle_batch_all_valid_applies_every_message() {
    let mut processor = processor();
    let batch = json!([
        {"version": V, "createSurface": {"surfaceId": "s1"}},
        {"version": V, "createSurface": {"surfaceId": "s2"}},
        {"version": V, "deleteSurface": {"surfaceId": "s1"}},
    ]);

    assert_that!(processor.handle_batch(&batch).unwrap()).is_equal_to(Outcome::default());
    assert_that!(ids(&processor)).is_equal_to(vec!["s2".to_owned()]);
}

#[test]
fn state_limits_refuse_the_65th_surface_and_change_nothing() {
    let mut processor = processor();
    for n in 0..64 {
        processor.handle(&create(&format!("s{n}"))).unwrap();
    }

    let error = processor.handle(&create("s64")).unwrap_err();

    let class = (Category::Validation, ErrorCode::ValidationFailed);
    assert_error(
        &error,
        class,
        "Surface limit of 64 exceeded",
        Some("s64"),
        None,
    );
    assert_that!(ids(&processor).len()).is_equal_to(64);
    assert_that!(processor.surface("s64").is_none()).is_true();
}

#[test]
fn state_limits_refuse_an_oversized_data_model_and_undo_the_write() {
    let limits = StateLimits::default().with_max_data_model_nodes(4);
    let mut processor = processor().with_state_limits(limits);
    processor.handle(&create("main")).unwrap();
    processor
        .handle(&write("/a", json!({"k": Value::Null})))
        .unwrap();
    let before = processor.surface("main").unwrap().data_model().clone();

    let error = processor
        .handle(&write("/a/k/x/y", json!([1, 2])))
        .unwrap_err();

    let class = (Category::Validation, ErrorCode::ValidationFailed);
    let text = "Data model node limit of 4 exceeded";
    assert_error(&error, class, text, Some("main"), None);
    assert_that!(processor.surface("main").unwrap().data_model()).is_equal_to(&before);
}

#[test]
fn state_limits_refuse_an_oversized_initial_data_model() {
    let limits = StateLimits::default().with_max_data_model_nodes(2);
    let mut processor = processor().with_state_limits(limits);

    let error = processor
        .handle(&message(json!({"version": V,
            "createSurface": {"surfaceId": "main", "dataModel": {"a": [1, 2]}}})))
        .unwrap_err();

    let class = (Category::Validation, ErrorCode::ValidationFailed);
    let text = "Data model node limit of 2 exceeded";
    assert_error(&error, class, text, Some("main"), None);
    assert_that!(processor.surface("main").is_none()).is_true();
}

#[rstest]
#[case::call_renderer_function(
    json!({"callRendererFunction": {"functionCallId": "f1",
        "callFunction": {"@call": "f", "catalogId": "c"}}}),
    "Action 'callRendererFunction' is not handled yet"
)]
#[case::agent_function_response(
    json!({"agentFunctionResponse": {"functionCallId": "f1", "value": 1}}),
    "Action 'agentFunctionResponse' is not handled yet"
)]
fn function_messages_are_not_handled_yet(#[case] mut body: Value, #[case] text: &str) {
    body["version"] = json!(V);
    let mut processor = processor();
    processor.handle(&create("main")).unwrap();

    let error = processor.handle(&message(body)).unwrap_err();

    let class = (Category::Validation, ErrorCode::ValidationFailed);
    assert_error(&error, class, text, None, None);
    assert_that!(ids(&processor)).is_equal_to(vec!["main".to_owned()]);
}

#[test]
fn catalogs_reads_back_the_set() {
    let processor = processor();
    let ids: Vec<&str> = processor.catalogs().iter().map(|c| c.id()).collect();
    assert_that!(ids).is_equal_to(vec!["c"]);
}

#[test]
fn with_limits_applies_to_handle_batch() {
    let mut processor = processor().with_limits(MessageLimits::new(16));
    let create = json!({"version": V, "createSurface": {"surfaceId": "main"}});

    let error = processor.handle_batch(&json!([create])).unwrap_err();

    assert_that!(error.to_string())
        .is_equal_to("A2UI message exceeds the 16-byte limit".to_owned());
    assert_that!(error.message_index()).is_equal_to(Some(0));
    assert_that!(processor.surface("main").is_none()).is_true();
}

#[test]
fn surface_data_model_pointer_reads_written_value() {
    let mut processor = processor();
    processor.handle(&create("main")).unwrap();
    processor.handle(&write("/n", json!(3))).unwrap();

    let model = processor.surface("main").unwrap().data_model();
    assert_that!(model.get(&Pointer::parse("/n").unwrap())).is_equal_to(Some(&json!(3)));
}
