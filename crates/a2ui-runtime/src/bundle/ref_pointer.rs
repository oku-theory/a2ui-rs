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
///
/// A token borrows from the `$ref` (or the pointer given to [`RefPointer::parse`]) unless it
/// had a `~` escape, or the `$ref`'s fragment had a percent escape: then it owns its text.
#[derive(Debug)]
pub(crate) struct RefPointer<'a> {
    tokens: Vec<Cow<'a, str>>,
}

impl<'a> RefPointer<'a> {
    /// The pointer a supported `$ref` names, percent-decoded, then each token unescaped.
    ///
    /// `None` when [`pointer_of`] gives none (an unsupported reference, an anchor, a fragment
    /// whose escapes are not UTF-8), or when a token has a `~` that RFC 6901 does not define.
    pub(crate) fn of(reference: &'a str) -> Option<Self> {
        match pointer_of(reference)? {
            Cow::Borrowed(pointer) => Self::parse(pointer),
            // The decoded pointer ends with this call, so its tokens are copied out of it. The
            // whole fragment is decoded before it is split: `%2F` decodes to a separator.
            Cow::Owned(pointer) => RefPointer::parse(&pointer).map(RefPointer::into_owned),
        }
    }

    /// The same tokens, each owning its text.
    fn into_owned(self) -> RefPointer<'static> {
        let tokens = self
            .tokens
            .into_iter()
            .map(|token| Cow::Owned(token.into_owned()))
            .collect();
        RefPointer { tokens }
    }

    /// Parses a decoded JSON Pointer into reference tokens.
    ///
    /// `""` names the whole document; any other pointer starts with `/`. `None` when it does
    /// not, or when a token has an undefined `~` escape.
    pub(crate) fn parse(pointer: &'a str) -> Option<Self> {
        if pointer.is_empty() {
            return Some(Self { tokens: Vec::new() });
        }
        let tokens = pointer
            .strip_prefix('/')?
            .split('/')
            .map(unescape_token)
            .collect::<Option<_>>()?;
        Some(Self { tokens })
    }

    /// The decoded tokens, outermost first.
    pub(crate) fn tokens(&self) -> &[Cow<'a, str>] {
        &self.tokens
    }

    /// The section and name of the entry the pointer names or falls in.
    ///
    /// `/components/Card/properties/child` falls in `("components", "Card")`. `None` when it
    /// names no `$defs`, `components` or `functions` entry nor a node inside one.
    pub(crate) fn entry(&self) -> Option<(&'static str, &str)> {
        self.split_entry().map(|(section, name, _)| (section, name))
    }

    /// The entry's section and name, then the tokens below the entry.
    fn split_entry(&self) -> Option<EntrySplit<'_, 'a>> {
        let [section, name, rest @ ..] = self.tokens.as_slice() else {
            return None;
        };
        let section = ENTRY_SECTIONS.into_iter().find(|known| known == section)?;
        Some((section, name.as_ref(), rest))
    }

    /// The node of `root` the pointer names, as RFC 6901 and `Value::pointer` resolve it.
    ///
    /// An array index is canonical digits: no sign, no leading zero but `0` itself.
    pub(crate) fn resolve<'v>(&self, root: &'v Value) -> Option<&'v Value> {
        descend(root, &self.tokens)
    }

    /// The entry the pointer falls in and the node it names.
    ///
    /// One descent of `root` finds both. `None` when [`RefPointer::entry`] is `None`, when
    /// `root`'s section is not an object holding that entry, or when no node lies there. The
    /// name is `root`'s own key, whatever escapes the `$ref` spelled it with.
    pub(crate) fn locate<'v>(&self, root: &'v Value) -> Option<Located<'v>> {
        let (section, name, rest) = self.split_entry()?;
        let (name, entry) = root.get(section)?.as_object()?.get_key_value(name)?;
        let target = descend(entry, rest)?;
        Some(Located {
            section,
            name,
            entry,
            target,
        })
    }
}

/// An entry pointer's section, name, and the tokens below the entry.
type EntrySplit<'p, 'a> = (&'static str, &'p str, &'p [Cow<'a, str>]);

/// Where a `$ref` lands: the `$defs`, `components` or `functions` entry and the node named.
#[derive(Debug)]
pub(crate) struct Located<'v> {
    /// The entry's section: `$defs`, `components` or `functions`.
    pub(crate) section: &'static str,
    /// The entry's name, borrowed from the section's own key.
    pub(crate) name: &'v str,
    /// The whole entry.
    pub(crate) entry: &'v Value,
    /// The node the pointer names: the entry itself or a node inside it.
    pub(crate) target: &'v Value,
}

/// The node `tokens` name below `node`.
///
/// Each token is an object key or, in an array, a canonical index.
fn descend<'v>(node: &'v Value, tokens: &[Cow<'_, str>]) -> Option<&'v Value> {
    tokens.iter().try_fold(node, |node, token| match node {
        Value::Object(map) => map.get(token.as_ref()),
        Value::Array(items) => items.get(array_index(token)?),
        _ => None,
    })
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
