use a2ui_runtime::FunctionRegistry;
use speculoos::prelude::*;

use super::*;
use crate::CATALOG_ID;

/// The functions Task 5 implements.
const IMPLEMENTED: [&str; 10] = [
    "required",
    "regex",
    "length",
    "numeric",
    "email",
    "formatString",
    "openUrl",
    "and",
    "or",
    "not",
];

/// The formatting functions registered by Task 6 (t-1716).
const FORMATTING: [&str; 4] = ["formatNumber", "formatCurrency", "formatDate", "pluralize"];

#[test]
fn functions_registers_all_fourteen_but_format_four() {
    let mut registry = FunctionRegistry::new();

    functions(&mut registry, BasicOptions::default()).unwrap();

    for name in IMPLEMENTED {
        assert_that!(registry.implementation(CATALOG_ID, name).is_some()).is_true();
    }
    for name in FORMATTING {
        assert_that!(registry.implementation(CATALOG_ID, name).is_none()).is_true();
    }
}
