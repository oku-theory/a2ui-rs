//! The renderer-side implementations of the basic catalog's functions.
//!
//! `validators` return `{"valid": bool}`; `logic` combines booleans with `coerce::boolean`;
//! `text` holds `formatString` and `openUrl`; `number`, `plural` and `date` format through the
//! [`Locale`](crate::Locale). Each implementation is a plain function, or a closure over
//! [`BasicOptions`], bound by [`functions`].

mod date;
mod logic;
mod number;
mod plural;
#[cfg(test)]
mod test_support;
mod text;
mod validators;

use std::sync::Arc;

use a2ui_runtime::{Args, CallContext, Error, FunctionRegistry};
use serde_json::Value;

use crate::{BasicOptions, Locale, catalog};

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
    let locale = &options.locale;
    registry.register(
        basic,
        "formatNumber",
        localized(locale, number::format_number),
    )?;
    registry.register(
        basic,
        "formatCurrency",
        localized(locale, number::format_currency),
    )?;
    registry.register(
        basic,
        "formatDate",
        localized(locale, |args, locale| Ok(date::format_date(args, locale))),
    )?;
    registry.register(basic, "pluralize", localized(locale, plural::pluralize))?;
    let opener = options.url_opener;
    registry.register(basic, "openUrl", move |args, _context| {
        text::open_url(args, opener.as_ref())
    })?;
    Ok(())
}

/// An implementation running `format` with its own handle on `locale`.
fn localized<F>(
    locale: &Arc<dyn Locale>,
    format: F,
) -> impl Fn(&Args, &mut CallContext<'_>) -> Result<Value, Error> + Send + Sync + 'static
where
    F: Fn(&Args, &dyn Locale) -> Result<Value, Error> + Send + Sync + 'static,
{
    let locale = Arc::clone(locale);
    move |args, _context| format(args, locale.as_ref())
}

/// Argument `name` of `args`; an absent one reads as `null`, as an unset binding does.
fn arg<'a>(args: &'a Args, name: &str) -> &'a Value {
    static NULL: Value = Value::Null;
    args.get(name).unwrap_or(&NULL)
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
