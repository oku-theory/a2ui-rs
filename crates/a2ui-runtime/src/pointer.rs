//! Normalized JSON Pointers (RFC 6901) with A2UI's segment rules, and the token codec.
//!
//! [`escape_token`] and [`unescape_token`] are the crate's one RFC 6901 reference-token codec:
//! `Pointer` adds its own rules on top, and `bundle` uses them for `$ref` pointers.

use std::borrow::Cow;
use std::fmt;

use crate::error::{Error, ForbiddenSegmentSnafu, InvalidPointerSnafu};

/// The largest list index a pointer may address.
pub const MAX_LIST_INDEX: usize = 10_000;

const FORBIDDEN_SEGMENTS: [&str; 3] = ["__proto__", "constructor", "prototype"];

/// An absolute JSON Pointer with its empty segments removed.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct Pointer {
    segments: Vec<String>,
}

/// How a segment reads as a list index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ListIndex {
    /// A canonical index no larger than [`MAX_LIST_INDEX`].
    At(usize),
    /// Canonical digits naming an index past [`MAX_LIST_INDEX`].
    TooLarge,
    /// Not a canonical non-negative integer, such as `-1`, `01` or `foo`.
    NotIndex,
}

impl Pointer {
    /// The pointer to the document root.
    pub fn root() -> Self {
        Self::default()
    }

    /// Parses an absolute pointer; `""` and `"/"` name the root.
    ///
    /// Empty segments are dropped (`//a///b//` is `/a/b`), `~1` and `~0` are
    /// unescaped, and `__proto__`, `constructor` and `prototype` are refused.
    pub fn parse(path: &str) -> Result<Self, Error> {
        if !path.is_empty() && !path.starts_with('/') {
            let reason = "an absolute pointer starts with '/'";
            return Err(InvalidPointerSnafu { path, reason }.build().into());
        }
        Self::from_segments(path)
    }

    /// Parses a relative path such as `address/city` into segments.
    pub(crate) fn parse_relative(path: &str) -> Result<Self, Error> {
        Self::from_segments(path)
    }

    fn from_segments(path: &str) -> Result<Self, Error> {
        let mut segments = Vec::new();
        for part in path.split('/').filter(|part| !part.is_empty()) {
            let segment = unescape(path, part)?;
            if FORBIDDEN_SEGMENTS.contains(&segment.as_str()) {
                return Err(ForbiddenSegmentSnafu { path, segment }.build().into());
            }
            segments.push(segment);
        }
        Ok(Self { segments })
    }

    /// The unescaped segments, outermost first.
    pub fn segments(&self) -> &[String] {
        &self.segments
    }

    /// Whether this pointer names the root.
    pub fn is_root(&self) -> bool {
        self.segments.is_empty()
    }

    /// Whether `prefix` equals this pointer or is one of its ancestors.
    pub fn starts_with(&self, prefix: &Pointer) -> bool {
        self.segments.starts_with(&prefix.segments)
    }

    /// This pointer followed by the segments of `tail`.
    pub fn join(&self, tail: &Pointer) -> Pointer {
        let mut segments = self.segments.clone();
        segments.extend(tail.segments.iter().cloned());
        Pointer { segments }
    }
}

impl fmt::Display for Pointer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.segments.is_empty() {
            return f.write_str("/");
        }
        for segment in &self.segments {
            write!(f, "/{}", escape_token(segment))?;
        }
        Ok(())
    }
}

/// Reads `segment` as a list index under A2UI's canonical-digits rule.
pub(crate) fn list_index(segment: &str) -> ListIndex {
    let digits = !segment.is_empty() && segment.bytes().all(|byte| byte.is_ascii_digit());
    if !digits || (segment.len() > 1 && segment.starts_with('0')) {
        return ListIndex::NotIndex;
    }
    match segment.parse::<usize>() {
        Ok(index) if index <= MAX_LIST_INDEX => ListIndex::At(index),
        _ => ListIndex::TooLarge,
    }
}

/// The RFC 6901 escape rule a token breaks when [`unescape_token`] returns `None`.
pub(crate) const BAD_ESCAPE: &str = "'~' must be followed by '0' or '1'";

fn unescape(path: &str, part: &str) -> Result<String, Error> {
    unescape_token(part).map(Cow::into_owned).ok_or_else(|| {
        let reason = BAD_ESCAPE;
        InvalidPointerSnafu { path, reason }.build().into()
    })
}

/// One reference token escaped for a JSON Pointer: `~` as `~0`, then `/` as `~1`.
///
/// RFC 6901, section 3; borrowed when the token holds neither character.
pub(crate) fn escape_token(token: &str) -> Cow<'_, str> {
    if token.contains(['~', '/']) {
        Cow::Owned(token.replace('~', "~0").replace('/', "~1"))
    } else {
        Cow::Borrowed(token)
    }
}

/// One JSON Pointer reference token with `~1` and `~0` unescaped, never twice.
///
/// RFC 6901, section 4: `~01` is `~1`, and `None` when a `~` is not followed by `0` or `1`.
/// Borrowed when the token holds no `~`.
pub(crate) fn unescape_token(token: &str) -> Option<Cow<'_, str>> {
    if !token.contains('~') {
        return Some(Cow::Borrowed(token));
    }
    let mut out = String::with_capacity(token.len());
    let mut chars = token.chars();
    while let Some(c) = chars.next() {
        if c != '~' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('0') => out.push('~'),
            Some('1') => out.push('/'),
            _ => return None,
        }
    }
    Some(Cow::Owned(out))
}

#[cfg(test)]
#[path = "pointer_tests.rs"]
mod tests;
