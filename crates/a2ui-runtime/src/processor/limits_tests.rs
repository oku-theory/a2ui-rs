use speculoos::prelude::*;

use super::*;

#[test]
fn state_limits_default_reads_the_documented_caps() {
    let limits = StateLimits::default();
    assert_that!(limits.max_surfaces()).is_equal_to(64);
    assert_that!(limits.max_components_per_surface()).is_equal_to(10_000);
    assert_that!(limits.max_components_per_message()).is_equal_to(10_000);
    assert_that!(limits.max_data_model_nodes()).is_equal_to(100_000);
}

#[test]
fn state_limits_setters_read_back() {
    let limits = StateLimits::default()
        .with_max_surfaces(1)
        .with_max_components_per_surface(2)
        .with_max_components_per_message(3)
        .with_max_data_model_nodes(4);
    assert_that!(limits.max_surfaces()).is_equal_to(1);
    assert_that!(limits.max_components_per_surface()).is_equal_to(2);
    assert_that!(limits.max_components_per_message()).is_equal_to(3);
    assert_that!(limits.max_data_model_nodes()).is_equal_to(4);
}
