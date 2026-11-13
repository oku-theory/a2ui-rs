//! A supported `$ref`'s JSON Pointer, parsed once into decoded reference tokens.
//!
//! Every reader of what a `$ref` names (load rule 4, `limits`' `$ref` resolve, the mixin
//! and union readers, `resolve`) takes its tokens from here, so an undefined `~` escape is
//! refused once, where the pointer is parsed, and no reader decodes a token its own way (plan
//! 2b, Decision 16). Rewriting a `$ref` (`localize_ref`, `written_ref`) keeps the decoded string.

use std::borrow::Cow;

use serde_json::Value;

use super::{ENTRY_SECTIONS, pointer_of};
use crate::pointer::unescape_token;

/// The decoded reference tokens of a supported `$ref`'s JSON Pointer.
#[derive(Debug)]
pub(crate) struct RefPointer {
    tokens: Vec<String>,
}

impl RefPointer {
    /// The pointer a supported `$ref` names, percent-decoded, then each token unescaped.
    ///
    /// `None` when [`pointer_of`] gives none (an unsupported reference, an anchor, a fragment
    /// whose escapes are not UTF-8), or when a token has a `~` that RFC 6901 does not define.
    pub(crate) fn of(reference: &str) -> Option<Self> {
        Self::parse(&pointer_of(reference)?)
    }

    /// Parses a decoded JSON Pointer into reference tokens.
    ///
    /// `""` names the whole document; any other pointer starts with `/`. `None` when it does
    /// not, or when a token has an undefined `~` escape.
    pub(crate) fn parse(pointer: &str) -> Option<Self> {
        if pointer.is_empty() {
            return Some(Self { tokens: Vec::new() });
        }
        let tokens = pointer
            .strip_prefix('/')?
            .split('/')
            .map(|token| unescape_token(token).map(Cow::into_owned))
            .collect::<Option<_>>()?;
        Some(Self { tokens })
    }

    /// The decoded tokens, outermost first.
    pub(crate) fn tokens(&self) -> &[String] {
        &self.tokens
    }

    /// The section and name of the entry the pointer names or falls in.
    ///
    /// `/components/Card/properties/child` falls in `("components", "Card")`. `None` when it
    /// names no `$defs`, `components` or `functions` entry nor a node inside one.
    pub(crate) fn entry(&self) -> Option<(&'static str, &str)> {
        let [section, name, ..] = self.tokens.as_slice() else {
            return None;
        };
        let section = ENTRY_SECTIONS.into_iter().find(|known| known == section)?;
        Some((section, name.as_str()))
    }

    /// The node of `root` the pointer names, as RFC 6901 and `Value::pointer` resolve it.
    ///
    /// An array index is canonical digits: no sign, no leading zero but `0` itself.
    pub(crate) fn resolve<'a>(&self, root: &'a Value) -> Option<&'a Value> {
        self.tokens.iter().try_fold(root, |node, token| match node {
            Value::Object(map) => map.get(token),
            Value::Array(items) => items.get(array_index(token)?),
            _ => None,
        })
    }
}

/// `token` as an array index: digits only, and no leading zero unless it is `0`.
fn array_index(token: &str) -> Option<usize> {
    let canonical = !token.is_empty()
        && token.bytes().all(|byte| byte.is_ascii_digit())
        && (token == "0" || !token.starts_with('0'));
    canonical.then(|| token.parse().ok()).flatten()
}

#[cfg(test)]
#[path = "ref_pointer_tests.rs"]
mod tests;
