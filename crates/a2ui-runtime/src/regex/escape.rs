//! Class and escape readers: `[…]`, `\d`, `\u{…}` and the escapes the dialect refuses.
//!
//! Escapes read the same in and out of a class but for `\b` and `\-`; a lone surrogate, which no
//! `char` holds, reads as a class matching nothing.

use nom::Parser;
use nom::branch::alt;
use nom::bytes::complete::{tag, take_while_m_n};
use nom::character::complete::{anychar, char, hex_digit1, one_of};
use nom::combinator::{map, map_opt, opt, value};
use nom::sequence::{delimited, preceded};

use super::ast::Node;
use super::charset::CharSet;
use super::parse::{Fault, Res, refuse_with, refused};

/// The highest Unicode code point.
const MAX_CODE_POINT: u32 = 0x10_FFFF;
/// The characters an identity escape may name.
const IDENTITY_ESCAPES: &str = "^$\\.*+?()[]{}|/";
const CLASS_UNCLOSED: &str = "a character class is not closed by ']'";
const LEGACY_OCTAL: &str = "legacy octal escapes (\\0 followed by a digit) are not allowed";

/// What an escape stands for: one code point (possibly a lone surrogate) or a class.
#[derive(Clone)]
enum Escape {
    Code(u32),
    Class { negated: bool, set: CharSet },
}

/// One side of a class range, or a class escape.
#[derive(Clone)]
enum ClassAtom {
    Code(u32),
    Set(CharSet),
}

/// `[…]` or `[^…]`: single characters, ranges and class escapes.
pub(super) fn class(input: &str) -> Res<'_, Node> {
    let (mut rest, negated) =
        preceded(char('['), map(opt(char('^')), |caret| caret.is_some())).parse(input)?;
    let mut items = CharSet::empty();
    loop {
        if let Some(after) = rest.strip_prefix(']') {
            return Ok((after, Node::Class { negated, items }));
        }
        let (after, first) = class_atom(rest)?;
        let (after, set) = match after.strip_prefix('-') {
            Some(tail) if !tail.starts_with(']') => {
                let (tail, last) = class_atom(tail)?;
                (tail, class_range(first, last)?)
            }
            _ => (after, first.into_set()),
        };
        items = items.union(&set);
        rest = after;
    }
}

/// `first-last` as a set; class escapes cannot bound a range, nor may it run backwards.
fn class_range(first: ClassAtom, last: ClassAtom) -> Result<CharSet, nom::Err<Fault>> {
    let refuse = |detail| nom::Err::Failure(Fault::Refused(detail));
    match (first, last) {
        (ClassAtom::Code(lo), ClassAtom::Code(hi)) if lo <= hi => {
            Ok(CharSet::from_code_range(lo, hi))
        }
        (ClassAtom::Code(_), ClassAtom::Code(_)) => {
            Err(refuse("a character class range must not run backwards"))
        }
        _ => Err(refuse(
            "a class escape cannot bound a character class range",
        )),
    }
}

/// One class member: an escape or any character but `]`, checked by the caller.
fn class_atom(input: &str) -> Res<'_, ClassAtom> {
    alt((
        preceded(char('\\'), class_escape),
        map(anychar, |c| ClassAtom::Code(u32::from(c))),
        refuse_with(CLASS_UNCLOSED),
    ))
    .parse(input)
}

/// An escape inside a class, after its `\`: `\b` is backspace and `\-` a dash there.
fn class_escape(input: &str) -> Res<'_, ClassAtom> {
    alt((
        value(ClassAtom::Code(0x8), char('b')),
        value(ClassAtom::Code(u32::from('-')), char('-')),
        preceded(
            char('B'),
            refuse_with("\\B is not allowed in a character class"),
        ),
        map(escape_body, |escape| match escape {
            Escape::Code(code) => ClassAtom::Code(code),
            Escape::Class {
                negated: false,
                set,
            } => ClassAtom::Set(set),
            Escape::Class { negated: true, set } => ClassAtom::Set(set.negate()),
        }),
    ))
    .parse(input)
}

impl ClassAtom {
    /// The characters this member stands for alone.
    fn into_set(self) -> CharSet {
        match self {
            Self::Code(code) => CharSet::from_code_range(code, code),
            Self::Set(set) => set,
        }
    }
}

/// An escape outside a class; a lone surrogate, which no value holds, matches nothing.
pub(super) fn escape(input: &str) -> Res<'_, Node> {
    map(preceded(char('\\'), escape_body), |escape| match escape {
        Escape::Code(code) => char::from_u32(code).map_or_else(
            || Node::Class {
                negated: false,
                items: CharSet::empty(),
            },
            Node::Char,
        ),
        Escape::Class { negated, set } => Node::Class {
            negated,
            items: set,
        },
    })
    .parse(input)
}

/// What follows a `\`, in or out of a class; anything not allowed is refused, never skipped.
fn escape_body(input: &str) -> Res<'_, Escape> {
    alt((
        map(one_of("dDwWsS"), class_escape_set),
        value(Escape::Code(0x9), char('t')),
        value(Escape::Code(0xA), char('n')),
        value(Escape::Code(0xB), char('v')),
        value(Escape::Code(0xC), char('f')),
        value(Escape::Code(0xD), char('r')),
        nul,
        preceded(char('x'), hex_escape),
        preceded(char('u'), unicode_escape),
        map(one_of(IDENTITY_ESCAPES), |c| Escape::Code(u32::from(c))),
        refused_escape,
    ))
    .parse(input)
}

/// `\d \D \w \W \s \S`: an upper-case letter negates.
fn class_escape_set(letter: char) -> Escape {
    let set = match letter.to_ascii_lowercase() {
        'd' => CharSet::digit(),
        'w' => CharSet::word(),
        _ => CharSet::space(),
    };
    Escape::Class {
        negated: letter.is_ascii_uppercase(),
        set,
    }
}

/// `\0` not followed by a digit.
fn nul(input: &str) -> Res<'_, Escape> {
    let (rest, _) = char('0').parse(input)?;
    if rest.starts_with(|c: char| c.is_ascii_digit()) {
        return refused(LEGACY_OCTAL);
    }
    Ok((rest, Escape::Code(0)))
}

/// `HH` after `\x`.
fn hex_escape(input: &str) -> Res<'_, Escape> {
    alt((
        map(hex_digits::<2>, Escape::Code),
        refuse_with("\\x needs two hex digits"),
    ))
    .parse(input)
}

/// `{H…}`, `HHHH` or a `HHHH\uHHHH` surrogate pair after `\u`.
fn unicode_escape(input: &str) -> Res<'_, Escape> {
    alt((
        braced_code_point,
        surrogate_pair,
        map(hex_digits::<4>, Escape::Code),
        refuse_with("\\u needs four hex digits or a braced code point"),
    ))
    .parse(input)
}

/// `{H…}`, at most U+10FFFF.
fn braced_code_point(input: &str) -> Res<'_, Escape> {
    let (rest, digits) = delimited(char('{'), hex_digit1, char('}')).parse(input)?;
    match u32::from_str_radix(digits, 16) {
        Ok(code) if code <= MAX_CODE_POINT => Ok((rest, Escape::Code(code))),
        _ => refused("a \\u{...} escape above U+10FFFF is not allowed"),
    }
}

/// A lead surrogate `HHHH` then `\u` and a trail surrogate: the code point they encode.
fn surrogate_pair(input: &str) -> Res<'_, Escape> {
    let pair = (hex_digits::<4>, preceded(tag("\\u"), hex_digits::<4>));
    map_opt(pair, |(lead, trail)| {
        let lead = lead.checked_sub(0xD800).filter(|bits| *bits < 0x400)?;
        let trail = trail.checked_sub(0xDC00).filter(|bits| *bits < 0x400)?;
        Some(Escape::Code(0x1_0000 + (lead << 10) + trail))
    })
    .parse(input)
}

/// Exactly `N` hex digits as a number.
fn hex_digits<const N: usize>(input: &str) -> Res<'_, u32> {
    map_opt(
        take_while_m_n(N, N, |c: char| c.is_ascii_hexdigit()),
        |digits| u32::from_str_radix(digits, 16).ok(),
    )
    .parse(input)
}

/// Refuses an escape the dialect leaves out, naming it.
fn refused_escape(input: &str) -> Res<'_, Escape> {
    let detail = match input.chars().next() {
        None => "a pattern cannot end with '\\'",
        Some('1'..='9') => "backreferences and legacy octal escapes (\\1-\\9) are not allowed",
        Some('k') => "named backreferences (\\k<name>) are not allowed",
        Some('p' | 'P') => "Unicode property escapes (\\p{...}, \\P{...}) are not allowed",
        Some('c') => "control letter escapes (\\cX) are not allowed",
        Some(_) => "identity escapes are allowed for syntax characters only",
    };
    refused(detail)
}

#[cfg(test)]
#[path = "escape_tests.rs"]
mod tests;
