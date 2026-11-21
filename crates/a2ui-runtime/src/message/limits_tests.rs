use rstest::rstest;
use serde_json::{Value, json};
use speculoos::prelude::*;

use super::*;

#[test]
fn default_limit_is_one_mebibyte() {
    assert_that!(MessageLimits::default().max_bytes()).is_equal_to(1_048_576);
}

#[test]
fn new_keeps_the_given_cap() {
    assert_that!(MessageLimits::new(7).max_bytes()).is_equal_to(7);
}

#[rstest]
#[case::mixed(json!({"a": ["b", true, null, false], "c": {}}))]
#[case::accent(json!("é"))]
#[case::empty_containers(json!([[], {}]))]
fn encoded_len_exceeds_matches_compact_encoding(
    #[case] value: Value,
) -> Result<(), Box<dyn std::error::Error>> {
    let n = serde_json::to_string(&value)?.len();
    assert_that!(encoded_len_exceeds(&value, n)).is_false();
    assert_that!(encoded_len_exceeds(&value, n - 1)).is_true();
    Ok(())
}

#[test]
fn encoded_len_exceeds_counts_numbers_as_24() {
    let value = json!([1]);
    assert_that!(encoded_len_exceeds(&value, 26)).is_false();
    assert_that!(encoded_len_exceeds(&value, 25)).is_true();
}

#[test]
fn encoded_len_exceeds_stops_at_cap() {
    let value = Value::Array(vec![json!("a"); 100_000]);
    assert_that!(encoded_len_exceeds(&value, 64)).is_true();
}
