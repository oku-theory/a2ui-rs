//! The A2UI regex dialect: patterns checked, compiled and matched on a bounded machine.
//!
//! `parse` reads a pattern of at most [`MAX_PATTERN_CHARS`] code points into an [`ast::Node`]
//! tree with `nom`; `safety::check` refuses the shapes the protocol's static safety check
//! names; `compile` flattens the tree into at most [`MAX_PROGRAM`] instructions, which
//! `vm::Machine` runs over values of at most [`MAX_VALUE_CHARS`] code points. Every refusal is
//! an `EXPRESSION_ERROR` whose message starts with its [`Rule`]'s code.

mod ast;
mod charset;
mod compile;
mod escape;
mod parse;
mod safety;
mod vm;

use std::fmt;

use crate::error::{Error, EvalFailure};
use crate::quoted::Quoted;

/// The longest pattern accepted, in code points; it bounds the tree's depth.
pub const MAX_PATTERN_CHARS: usize = 256;

/// The longest value matched, in code points; a longer one does not match.
pub const MAX_VALUE_CHARS: usize = 4096;

/// The most instructions a compiled pattern may hold; beyond, it is too complex.
///
/// This is this library's limit, not the protocol's: it bounds expanded counted repeats.
pub const MAX_PROGRAM: usize = 4096;

/// A pattern of the A2UI regex dialect, ready to match values.
///
/// Matching searches the value, as ECMAScript's `test` does, in time bounded by
/// [`MAX_PROGRAM`] × ([`MAX_VALUE_CHARS`] + 1) states, without recursion.
#[derive(Clone)]
pub struct DialectRegex {
    pattern: String,
    machine: vm::Machine,
}

impl DialectRegex {
    /// Checks and compiles `pattern`.
    ///
    /// # Errors
    ///
    /// An `EXPRESSION_ERROR` whose message starts with `PATTERN_TOO_LONG`,
    /// `UNSUPPORTED_SYNTAX`, `NESTED_QUANTIFIER`, `OVERLAPPING_ALTERNATION`, `ADJACENT_OVERLAP`
    /// or `PATTERN_TOO_COMPLEX`.
    pub fn new(pattern: &str) -> Result<Self, Error> {
        let node = parse::parse(pattern)?;
        safety::check(&node)?;
        let program = compile::compile(&node)?;
        Ok(Self {
            pattern: pattern.to_owned(),
            machine: vm::Machine::new(program),
        })
    }

    /// Whether the pattern matches somewhere in `value`.
    ///
    /// A value longer than [`MAX_VALUE_CHARS`] code points does not match.
    #[must_use]
    pub fn is_match(&self, value: &str) -> bool {
        let value: Vec<char> = value.chars().take(MAX_VALUE_CHARS + 1).collect();
        value.len() <= MAX_VALUE_CHARS && self.machine.is_match(&value)
    }
}

impl fmt::Debug for DialectRegex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DialectRegex")
            .field("pattern", &self.pattern)
            .field("instructions", &self.machine.len())
            .finish()
    }
}

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
    /// The compiled pattern is longer than [`MAX_PROGRAM`] instructions.
    PatternTooComplex,
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
            Self::PatternTooComplex => "PATTERN_TOO_COMPLEX",
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
