//! The A2UI regex dialect: a pattern is length-checked, parsed and safety-checked.
//!
//! `parse` reads a pattern of at most [`MAX_PATTERN_CHARS`] code points into an [`ast::Node`]
//! tree with `nom`; `safety::check` refuses the shapes the protocol's static safety check
//! names. Every refusal is an `EXPRESSION_ERROR` whose message starts with its [`Rule`]'s code.

mod ast;
mod charset;
mod escape;
mod parse;
mod safety;

use crate::error::{Error, EvalFailure};
use crate::quoted::Quoted;

/// The longest pattern accepted, in code points; it bounds the tree's depth.
pub(crate) const MAX_PATTERN_CHARS: usize = 256;

/// A rule of the dialect a pattern breaks, named by its stable code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Rule {
    /// The pattern is longer than [`MAX_PATTERN_CHARS`] code points.
    PatternTooLong,
    /// The pattern does not parse, or uses a construct the dialect leaves out.
    UnsupportedSyntax,
    /// An unbounded quantifier inside a quantifier whose upper bound is above 1.
    NestedQuantifier,
    /// An unbounded quantifier over branches that can start with the same character.
    OverlappingAlternation,
    /// Two overlapping unbounded atoms with only nullable items between them.
    AdjacentOverlap,
}

impl Rule {
    /// The code a refusal's message starts with.
    pub(crate) fn code(self) -> &'static str {
        match self {
            Self::PatternTooLong => "PATTERN_TOO_LONG",
            Self::UnsupportedSyntax => "UNSUPPORTED_SYNTAX",
            Self::NestedQuantifier => "NESTED_QUANTIFIER",
            Self::OverlappingAlternation => "OVERLAPPING_ALTERNATION",
            Self::AdjacentOverlap => "ADJACENT_OVERLAP",
        }
    }

    /// The error refusing a pattern under this rule, `detail` being fixed English.
    pub(crate) fn refuse(self, detail: &'static str) -> Error {
        EvalFailure::Regex {
            rule: self.code(),
            detail: Quoted::new(detail),
        }
        .into()
    }
}
