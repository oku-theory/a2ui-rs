//! UAX #31 identifiers, the names of components, functions, arguments and properties.

use unicode_ident::{is_xid_continue, is_xid_start};

/// Whether `name` is a UAX #31 identifier, with `_` also allowed first.
pub(crate) fn is_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|c| c == '_' || is_xid_start(c)) && chars.all(is_xid_continue)
}

#[cfg(test)]
#[path = "ident_tests.rs"]
mod tests;
