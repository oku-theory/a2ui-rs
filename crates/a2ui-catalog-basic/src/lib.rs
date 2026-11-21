//! The A2UI v1.0 basic catalog, embedded for `a2ui-runtime`.
//!
//! [`catalog`] returns the published `catalog.json` ([`CATALOG_JSON`], copied by
//! `vendor/sync.sh`, Apache-2.0: see `LICENSE-A2UI`), whose id is [`CATALOG_ID`], loaded once and
//! shared. [`functions`] binds the renderer-side implementations of its functions in a
//! `FunctionRegistry`, following the host's [`BasicOptions`] ([`Locale`], [`UrlOpener`]).

mod functions;
mod locale;
mod options;

use std::sync::OnceLock;

use a2ui_runtime::{Catalog, Error};

pub use functions::functions;
pub use locale::{CurrencyFormat, EnUs, Locale, PluralCategory};
pub use options::{BasicOptions, NoOpener, UrlOpener};

/// The basic catalog's id.
pub const CATALOG_ID: &str = "https://a2ui.org/specification/v1_0/catalogs/basic/catalog.json";

/// The published catalog document, as JSON text.
pub const CATALOG_JSON: &str = include_str!("../catalog.json");

/// The basic catalog, loaded once on first use and then shared.
///
/// The first successful call parses and compiles [`CATALOG_JSON`] (about 20 ms) and keeps the
/// result; later calls on any thread return the same catalog. Only success is kept: a load error
/// is built afresh on each call, owned, with that caller's backtrace, so `?` composes. Two
/// threads racing on the first load may both compile; one result is kept.
pub fn catalog() -> Result<&'static Catalog, Error> {
    static BASIC: OnceLock<Catalog> = OnceLock::new();
    if let Some(basic) = BASIC.get() {
        return Ok(basic);
    }
    let loaded = Catalog::parse(CATALOG_JSON, None, None)?;
    Ok(BASIC.get_or_init(|| loaded))
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
