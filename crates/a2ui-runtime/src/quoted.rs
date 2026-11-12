//! Input text as errors hold it: invisible characters escaped, length capped.
//!
//! Every text field of an error is caller or catalog input, or holds some. Hosts log, show and
//! hand messages back to agents, so input is escaped where it could forge, reorder or hide text,
//! and capped so that one large value cannot make a large error. Both happen when the error is
//! built, so `Display`, `Debug` and memory are bounded alike. Runtime wording is a [`Reason`],
//! which only [`reason!`] fills with input, and only through `Quoted`.

use std::fmt;

use crate::limits::MAX_QUOTED_CHARS;

/// One input value an error quotes, escaped and capped when built.
///
/// Past [`MAX_QUOTED_CHARS`] input characters it keeps the first and last halves around an
/// elision marker; an escape is never cut.
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct Quoted(String);

/// Runtime wording around [`Quoted`] pieces, never capped.
///
/// Only [`reason!`] builds one with input in it, from a fixed list of [`Piece`]s: fixed text
/// (`&'static str`: literals, never text read from a document) and `Quoted` input. A `String`,
/// a borrowed `&str` or a `Reason` is not a piece, and only this module can make one, so the
/// compiler guarantees that any input a reason holds went through `Quoted`'s cap, and that its
/// length is bounded by its call site's pieces. Capping never cuts the wording a reader or the
/// conformance suite matches.
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct Reason(String);

/// One piece of a [`Reason`]: fixed text or [`Quoted`] input.
///
/// It is sealed: only this module implements it, and only for types whose text is bounded.
/// Never implement it for `String`, a borrowed `&str` or `Reason`, or the cap is lost.
pub(crate) trait Piece: sealed::Sealed {
    /// Appends this piece's text to `out`.
    fn push_onto(&self, out: &mut String);
}

impl Piece for &'static str {
    fn push_onto(&self, out: &mut String) {
        self.chars().for_each(|c| push_escaped(out, c));
    }
}

impl Piece for Quoted {
    fn push_onto(&self, out: &mut String) {
        out.push_str(&self.0);
    }
}

/// Keeps [`Piece`] implementable in this module only.
mod sealed {
    /// The supertrait other modules cannot name, so cannot implement.
    pub trait Sealed {}

    impl Sealed for &'static str {}
    impl Sealed for super::Quoted {}
}

/// A [`Reason`] joining fixed text and [`Quoted`] input, in order.
///
/// `reason!["$ref '", Quoted::new(reference), "' does not resolve"]`; each piece must be a
/// [`Piece`], so input that skips `Quoted` does not compile.
macro_rules! reason {
    ($($piece:expr),+ $(,)?) => {
        $crate::quoted::Reason::of(&[$(&$piece as &dyn $crate::quoted::Piece),+])
    };
}
pub(crate) use reason;

impl Quoted {
    /// Escapes `text` and caps it at [`MAX_QUOTED_CHARS`] input characters.
    pub(crate) fn new(text: &str) -> Self {
        let total = text.chars().count();
        let mut out = String::new();
        if total <= MAX_QUOTED_CHARS {
            text.chars().for_each(|c| push_escaped(&mut out, c));
        } else {
            let half = MAX_QUOTED_CHARS / 2;
            text.chars()
                .take(half)
                .for_each(|c| push_escaped(&mut out, c));
            out.push_str(&elision_marker(total - 2 * half));
            text.chars()
                .skip(total - half)
                .for_each(|c| push_escaped(&mut out, c));
        }
        Self(out)
    }
}

impl Reason {
    /// Joins `pieces` in order; [`reason!`] calls it with a list fixed at its call site.
    ///
    /// The array length is a constant, so even a repeated `[piece; N]` stays bounded.
    pub(crate) fn of<const N: usize>(pieces: &[&dyn Piece; N]) -> Self {
        let mut out = String::new();
        pieces.iter().for_each(|piece| piece.push_onto(&mut out));
        Self(out)
    }
}

impl From<&str> for Quoted {
    fn from(text: &str) -> Self {
        Self::new(text)
    }
}

impl From<&String> for Quoted {
    fn from(text: &String) -> Self {
        Self::new(text)
    }
}

impl From<String> for Quoted {
    fn from(text: String) -> Self {
        Self::new(&text)
    }
}

impl From<&'static str> for Reason {
    fn from(text: &'static str) -> Self {
        Self::of(&[&text])
    }
}

impl fmt::Display for Quoted {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl fmt::Debug for Quoted {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.0, f)
    }
}

impl fmt::Display for Reason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl fmt::Debug for Reason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.0, f)
    }
}

/// The marker that stands for `elided` input characters.
fn elision_marker(elided: usize) -> String {
    format!("…[{elided} characters elided]…")
}

/// Pushes `c`, or its Rust escape when it could forge, reorder or hide text.
///
/// Controls (`Cc`) are written as Rust's `escape_debug` does (`\n`, `\u{1b}`), the others as
/// `\u{…}`. That covers every control, every format character (`Cf`: bidi marks, embeddings,
/// overrides and isolates, zero-width characters, the BOM, tags) and the line and paragraph
/// separators. Quotes and backslashes are kept: they can only mislead within one line.
fn push_escaped(out: &mut String, c: char) {
    if c.is_control() {
        out.extend(c.escape_debug());
    } else if is_format_or_separator(c) {
        out.extend(c.escape_unicode());
    } else {
        out.push(c);
    }
}

/// Whether `c` is a format character (`Cf`) or a line or paragraph separator (Unicode 16).
fn is_format_or_separator(c: char) -> bool {
    matches!(
        c,
        '\u{00ad}'
            | '\u{0600}'..='\u{0605}'
            | '\u{061c}'
            | '\u{06dd}'
            | '\u{070f}'
            | '\u{0890}'..='\u{0891}'
            | '\u{08e2}'
            | '\u{180e}'
            | '\u{200b}'..='\u{200f}'
            | '\u{2028}'..='\u{202e}'
            | '\u{2060}'..='\u{2064}'
            | '\u{2066}'..='\u{206f}'
            | '\u{feff}'
            | '\u{fff9}'..='\u{fffb}'
            | '\u{110bd}'
            | '\u{110cd}'
            | '\u{13430}'..='\u{1343f}'
            | '\u{1bca0}'..='\u{1bca3}'
            | '\u{1d173}'..='\u{1d17a}'
            | '\u{e0001}'
            | '\u{e0020}'..='\u{e007f}'
    )
}

#[cfg(test)]
#[path = "quoted_tests.rs"]
mod tests;
