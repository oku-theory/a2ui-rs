//! Normalized JSON Pointers (RFC 6901) with A2UI's segment rules.

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
            write!(f, "/{}", segment.replace('~', "~0").replace('/', "~1"))?;
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

fn unescape(path: &str, part: &str) -> Result<String, Error> {
    let mut out = String::with_capacity(part.len());
    let mut chars = part.chars();
    while let Some(c) = chars.next() {
        if c != '~' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('0') => out.push('~'),
            Some('1') => out.push('/'),
            _ => {
                let reason = "'~' must be followed by '0' or '1'";
                return Err(InvalidPointerSnafu { path, reason }.build().into());
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
#[path = "pointer_tests.rs"]
mod tests;
