//! The text functions `formatString` and `openUrl`.

use a2ui_runtime::{Args, CallContext, Error, coerce};
use serde_json::Value;

use super::arg;
use crate::UrlOpener;

/// The URL schemes `openUrl` hands to the opener, compared ignoring ASCII case.
const ALLOWED_SCHEMES: [&str; 4] = ["http", "https", "mailto", "tel"];

/// `formatString(value)`: the `${…}` template evaluated in the call's scope, as text.
///
/// A string value, including one read from the data model, is interpolated as a template; a
/// value that is not a string is returned as its text, uninterpreted.
pub(super) fn format_string(args: &Args, context: &mut CallContext<'_>) -> Result<Value, Error> {
    let text = match arg(args, "value") {
        Value::String(template) => context.interpolate(template)?,
        other => coerce::text(other),
    };
    Ok(Value::String(text))
}

/// `openUrl(url)`: hands `url` to `opener` when its scheme is allowed, and returns `null`.
///
/// The scheme is the text before the first `:`; a URL without one is refused.
pub(super) fn open_url(args: &Args, opener: &dyn UrlOpener) -> Result<Value, Error> {
    let url = coerce::text(arg(args, "url"));
    let Some((scheme, _)) = url.split_once(':') else {
        return Err(Error::function("openUrl", "the URL has no scheme"));
    };
    if !ALLOWED_SCHEMES
        .iter()
        .any(|allowed| scheme.eq_ignore_ascii_case(allowed))
    {
        let reason = format!("scheme '{scheme}' is not allowed");
        return Err(Error::function("openUrl", &reason));
    }
    opener.open(&url);
    Ok(Value::Null)
}

#[cfg(test)]
#[path = "text_tests.rs"]
mod tests;
