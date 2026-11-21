//! The dialect's grammar, read with `nom` combinators into a [`Node`] tree.
//!
//! The grammar is ECMAScript's `Pattern` in Unicode mode, less what the protocol leaves out.
//! `nom` reads tokens; groups nest on an explicit stack, not through recursion, so a debug
//! build's large parser frames cannot overflow the stack on deeply nested input.

use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::character::complete::{char, digit1, satisfy};
use nom::combinator::{map, opt, value};
use nom::error::{ErrorKind, ParseError};
use nom::sequence::{delimited, preceded};
use nom::{IResult, Parser};

use super::ast::Node;
use super::escape::{class, escape};
use super::{MAX_PATTERN_CHARS, Rule};
use crate::error::Error;

/// The largest bound a counted quantifier may hold.
const MAX_BOUND: u32 = 65_535;
/// The characters that must be escaped to stand for themselves.
const SYNTAX_CHARS: &str = "^$\\.*+?()[]{}|";

const DANGLING_QUANTIFIER: &str = "a quantifier must follow an atom it can repeat";
const LOOKBEHIND: &str = "lookbehind assertions (?<=...) and (?<!...) are not allowed";
const BOUND_TOO_LARGE: &str = "a quantifier bound above 65535 is not allowed";
const BOUNDS_REVERSED: &str = "a quantifier {n,m} needs n <= m";
const GROUP_UNCLOSED: &str = "a group is not closed by ')'";

/// Why a parser stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Fault {
    /// This parser does not apply here; another may.
    Mismatch,
    /// A construct the dialect refuses, in fixed English; raised as a `nom` failure.
    Refused(&'static str),
}

impl ParseError<&str> for Fault {
    fn from_error_kind(_: &str, _: ErrorKind) -> Self {
        Self::Mismatch
    }

    fn append(_: &str, _: ErrorKind, other: Self) -> Self {
        other
    }
}

/// A parser's result over the pattern text.
pub(super) type Res<'a, O> = IResult<&'a str, O, Fault>;

/// Reads `pattern` into a tree, refusing what the dialect does not allow.
///
/// A pattern past [`MAX_PATTERN_CHARS`] code points is `PATTERN_TOO_LONG`; anything else
/// refused is `UNSUPPORTED_SYNTAX`, its message naming the construct.
pub(crate) fn parse(pattern: &str) -> Result<Node, Error> {
    if pattern.chars().count() > MAX_PATTERN_CHARS {
        return Err(Rule::PatternTooLong.refuse("the pattern is longer than 256 code points"));
    }
    build(pattern).map_err(|detail| Rule::UnsupportedSyntax.refuse(detail))
}

/// Assembles the tokens of `pattern` into a tree on an explicit stack of open groups.
///
/// Nesting costs a heap frame, not a call: a pattern of 256 `(` cannot exhaust the stack.
fn build(pattern: &str) -> Result<Node, &'static str> {
    let mut open: Vec<Frame> = Vec::new();
    let mut current = Frame::new(None);
    let mut rest = pattern;
    while !rest.is_empty() {
        let (after, token) = match token(rest) {
            Ok(read) => read,
            Err(nom::Err::Failure(Fault::Refused(detail))) => return Err(detail),
            // Unreachable by construction: `literal` and `refused_escape` refuse every leading
            // character no other token takes, so `token` only fails with `Refused`.
            Err(_) => return Err("the pattern does not parse"),
        };
        match token {
            Token::Atom(node) => current.push(node, true),
            Token::Assertion(node) => current.push(node, false),
            Token::Quantifier { min, max, lazy } => {
                let Some(atom) = current.take_quantifiable() else {
                    return Err(DANGLING_QUANTIFIER);
                };
                let node = Box::new(atom);
                current.push(
                    Node::Repeat {
                        node,
                        min,
                        max,
                        lazy,
                    },
                    false,
                );
            }
            Token::Bar => current.end_branch(),
            Token::Open(opener) => {
                open.push(std::mem::replace(&mut current, Frame::new(Some(opener))))
            }
            Token::Close => {
                let (Some(opener), Some(parent)) = (current.opener, open.pop()) else {
                    return Err("a ')' closes no group");
                };
                let inner = Box::new(std::mem::replace(&mut current, parent).finish());
                match opener {
                    Opener::Group => current.push(Node::Group(inner), true),
                    Opener::Look { negated } => {
                        current.push(
                            Node::Look {
                                negated,
                                node: inner,
                            },
                            false,
                        );
                    }
                }
            }
        }
        rest = after;
    }
    if current.opener.is_some() {
        return Err(GROUP_UNCLOSED);
    }
    Ok(current.finish())
}

/// What opens a group.
#[derive(Debug, Clone, Copy)]
enum Opener {
    /// `(` or `(?:`.
    Group,
    /// `(?=` or, when `negated`, `(?!`.
    Look { negated: bool },
}

/// One lexical unit of a pattern.
#[derive(Clone)]
enum Token {
    /// A node a quantifier may follow: a literal, `.`, a class or an escape.
    Atom(Node),
    /// `^`, `$`, `\b` or `\B`, which no quantifier may follow.
    Assertion(Node),
    /// `*`, `+`, `?` or braces, with an optional lazy `?`.
    Quantifier {
        min: u32,
        max: Option<u32>,
        lazy: bool,
    },
    /// `|`.
    Bar,
    /// `(`, `(?:`, `(?=` or `(?!`.
    Open(Opener),
    /// `)`.
    Close,
}

/// A group being read: its finished branches and the items of the current one.
struct Frame {
    /// `None` for the whole pattern.
    opener: Option<Opener>,
    branches: Vec<Node>,
    items: Vec<Node>,
    /// Whether the last item may take a quantifier.
    quantifiable: bool,
}

impl Frame {
    /// An empty group opened by `opener`.
    fn new(opener: Option<Opener>) -> Self {
        Self {
            opener,
            branches: Vec::new(),
            items: Vec::new(),
            quantifiable: false,
        }
    }

    /// Appends `node` to the current branch.
    fn push(&mut self, node: Node, quantifiable: bool) {
        self.items.push(node);
        self.quantifiable = quantifiable;
    }

    /// Removes the last item when a quantifier may follow it.
    fn take_quantifiable(&mut self) -> Option<Node> {
        if !std::mem::take(&mut self.quantifiable) {
            return None;
        }
        self.items.pop()
    }

    /// Ends the current branch at a `|`.
    fn end_branch(&mut self) {
        let items = std::mem::take(&mut self.items);
        self.branches.push(sequence(items));
        self.quantifiable = false;
    }

    /// The group's content: one branch as itself, several as an `Alt`.
    fn finish(mut self) -> Node {
        let last = sequence(self.items);
        if self.branches.is_empty() {
            return last;
        }
        self.branches.push(last);
        Node::Alt(self.branches)
    }
}

/// The items of one branch: `Empty`, the one item, or a `Concat`.
fn sequence(mut items: Vec<Node>) -> Node {
    if items.len() > 1 {
        return Node::Concat(items);
    }
    items.pop().unwrap_or(Node::Empty)
}

/// A `nom` failure refusing the construct `detail` names.
pub(super) fn refused<'a, O>(detail: &'static str) -> Res<'a, O> {
    Err(nom::Err::Failure(Fault::Refused(detail)))
}

/// A parser that always refuses with `detail`.
pub(super) fn refuse_with<'a, O>(detail: &'static str) -> impl Fn(&'a str) -> Res<'a, O> {
    move |_| refused(detail)
}

/// The next token. Assertions come before escapes, so `\b` is a word boundary there.
fn token(input: &str) -> Res<'_, Token> {
    alt((
        value(Token::Close, char(')')),
        value(Token::Bar, char('|')),
        map(opener, Token::Open),
        map(quantifier, |(min, max, lazy)| Token::Quantifier {
            min,
            max,
            lazy,
        }),
        map(assertion, Token::Assertion),
        map(
            alt((value(Node::Any, char('.')), class, escape, literal)),
            Token::Atom,
        ),
    ))
    .parse(input)
}

/// `^`, `$`, `\b` or `\B`.
fn assertion(input: &str) -> Res<'_, Node> {
    alt((
        value(Node::Start, char('^')),
        value(Node::End, char('$')),
        value(Node::WordBoundary(true), tag("\\b")),
        value(Node::WordBoundary(false), tag("\\B")),
    ))
    .parse(input)
}

/// `(?:`, `(?=`, `(?!` or `(`; any other `(?` is refused.
fn opener(input: &str) -> Res<'_, Opener> {
    alt((
        value(Opener::Group, tag("(?:")),
        value(Opener::Look { negated: false }, tag("(?=")),
        value(Opener::Look { negated: true }, tag("(?!")),
        preceded(tag("(?<="), refuse_with(LOOKBEHIND)),
        preceded(tag("(?<!"), refuse_with(LOOKBEHIND)),
        preceded(
            tag("(?<"),
            refuse_with("named groups (?<name>...) are not allowed"),
        ),
        preceded(
            tag("(?"),
            refuse_with("inline flags and group modifiers (?...) are not allowed"),
        ),
        value(Opener::Group, char('(')),
    ))
    .parse(input)
}

/// A quantifier: `(min, max, lazy)`, `max` `None` when unbounded.
fn quantifier(input: &str) -> Res<'_, (u32, Option<u32>, bool)> {
    let (rest, (min, max)) = alt((
        value((0, None), char('*')),
        value((1, None), char('+')),
        value((0, Some(1)), char('?')),
        braces,
    ))
    .parse(input)?;
    let (rest, lazy) = map(opt(char('?')), |mark| mark.is_some()).parse(rest)?;
    Ok((rest, (min, max, lazy)))
}

/// `{n}`, `{n,}` or `{n,m}`, bounds at most [`MAX_BOUND`] and `n <= m`.
fn braces(input: &str) -> Res<'_, (u32, Option<u32>)> {
    let inner = (bound, opt(preceded(char(','), opt(bound))));
    let (rest, (min, upper)) = delimited(char('{'), inner, char('}')).parse(input)?;
    let Some(min) = min else {
        return refused(BOUND_TOO_LARGE);
    };
    let max = match upper {
        None => Some(min),
        Some(None) => None,
        Some(Some(None)) => return refused(BOUND_TOO_LARGE),
        Some(Some(Some(max))) => Some(max),
    };
    if max.is_some_and(|max| max < min) {
        return refused(BOUNDS_REVERSED);
    }
    Ok((rest, (min, max)))
}

/// Decimal digits as a bound, `None` when above [`MAX_BOUND`].
fn bound(input: &str) -> Res<'_, Option<u32>> {
    map(digit1, |digits: &str| {
        digits
            .parse::<u32>()
            .ok()
            .filter(|bound| *bound <= MAX_BOUND)
    })
    .parse(input)
}

/// A character standing for itself; a lone `{`, `}` or `]` is refused.
///
/// Quantifiers, `(`, `)` and `|` are tokens of their own and never reach it.
fn literal(input: &str) -> Res<'_, Node> {
    let detail = match input.chars().next() {
        Some('{') => "a '{' that starts no quantifier must be escaped",
        Some('}') => "a lone '}' must be escaped",
        Some(']') => "a lone ']' must be escaped",
        _ => {
            let pattern_char = satisfy(|c| !SYNTAX_CHARS.contains(c));
            return map(pattern_char, Node::Char).parse(input);
        }
    };
    refused(detail)
}

#[cfg(test)]
#[path = "parse_tests.rs"]
mod tests;
