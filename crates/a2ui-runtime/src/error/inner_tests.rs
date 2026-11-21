use snafu::ErrorCompat;
use speculoos::prelude::*;

use super::*;
use crate::error::Error;
use crate::error::process::ProcessFailure;

/// A boxed processor failure as an [`Error`].
fn surface_exists() -> Error {
    Error::from(ProcessFailure::SurfaceExists { id: "main".into() })
}

#[test]
fn process_display_is_the_failure_message() {
    assert_that!(surface_exists().to_string())
        .is_equal_to("Surface 'main' already exists".to_owned());
}

#[test]
fn process_has_a_backtrace() {
    assert_that!(ErrorCompat::backtrace(&surface_exists()).is_some()).is_true();
}

#[test]
fn process_class_is_the_failure_class() {
    let inner = ProcessSnafu {
        failure: Box::new(ProcessFailure::StateLimit {
            what: "Surface",
            limit: 64,
        }),
    }
    .build();
    assert_that!(inner.class()).is_equal_to((Category::Validation, ErrorCode::ValidationFailed));
}
