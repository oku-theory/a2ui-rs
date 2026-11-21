//! The syntax tree of a pattern in the A2UI regex dialect.

use super::charset::CharSet;

/// One node of a parsed pattern.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Node {
    /// Matches the empty string: an empty pattern, branch or group.
    Empty,
    /// One literal code point.
    Char(char),
    /// `.`: any code point but a line terminator.
    Any,
    /// A class or class escape: a code point in `items`, or not in it when `negated`.
    Class { negated: bool, items: CharSet },
    /// `^`: the start of the value.
    Start,
    /// `$`: the end of the value.
    End,
    /// `\b` (`true`) or `\B` (`false`), with ASCII word characters.
    WordBoundary(bool),
    /// A capturing or non-capturing group; captures are not exposed.
    Group(Box<Node>),
    /// A lookahead `(?=…)`, or `(?!…)` when `negated`.
    Look { negated: bool, node: Box<Node> },
    /// Nodes matched one after the other.
    Concat(Vec<Node>),
    /// Branches tried in order.
    Alt(Vec<Node>),
    /// `node` repeated `min` to `max` times (unbounded when `None`), fewest first when `lazy`.
    Repeat {
        node: Box<Node>,
        min: u32,
        max: Option<u32>,
        lazy: bool,
    },
}
