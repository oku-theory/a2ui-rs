//! The renderer-side implementations of the basic catalog's functions.
//!
//! `validators` return `{"valid": bool}`; `logic` combines booleans with `coerce::boolean`;
//! `text` holds `formatString` and `openUrl`. Each implementation is a plain function, or a
//! closure over [`BasicOptions`], bound by [`functions`].

mod logic;
#[cfg(test)]
mod test_support;
mod text;
mod validators;

use a2ui_runtime::{Args, Error, FunctionRegistry};
use serde_json::Value;

use crate::{BasicOptions, catalog};

/// Binds the basic catalog's functions in `registry`, following `options`.
///
/// # Errors
///
/// The error of loading [`catalog`]; registering a name it declares cannot fail.
pub fn functions(registry: &mut FunctionRegistry, options: BasicOptions) -> Result<(), Error> {
    let basic = catalog()?;
    registry.register(basic, "required", validators::required)?;
    registry.register(basic, "regex", validators::regex)?;
    registry.register(basic, "length", validators::length)?;
    registry.register(basic, "numeric", validators::numeric)?;
    registry.register(basic, "email", validators::email)?;
    registry.register(basic, "and", logic::and)?;
    registry.register(basic, "or", logic::or)?;
    registry.register(basic, "not", logic::not)?;
    registry.register(basic, "formatString", text::format_string)?;
    let opener = options.url_opener;
    registry.register(basic, "openUrl", move |args, _context| {
        text::open_url(args, opener.as_ref())
    })?;
    Ok(())
}

/// Argument `name` of `args`; an absent one reads as `null`, as an unset binding does.
fn arg<'a>(args: &'a Args, name: &str) -> &'a Value {
    static NULL: Value = Value::Null;
    args.get(name).unwrap_or(&NULL)
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
