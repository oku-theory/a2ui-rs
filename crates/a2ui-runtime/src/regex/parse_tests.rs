use rstest::rstest;
use speculoos::prelude::*;

use super::*;
use crate::error::{Category, ErrorCode};
use crate::regex::charset::CharSet;

fn ch(c: char) -> Node {
    Node::Char(c)
}

fn class(negated: bool, items: CharSet) -> Node {
    Node::Class { negated, items }
}

fn repeat(node: Node, min: u32, max: Option<u32>, lazy: bool) -> Node {
    Node::Repeat {
        node: Box::new(node),
        min,
        max,
        lazy,
    }
}

fn group(node: Node) -> Node {
    Node::Group(Box::new(node))
}

/// The error `pattern` gives, with its message, category and code.
fn refusal(pattern: &str) -> (String, Category, ErrorCode) {
    let error = parse(pattern).unwrap_err();
    (error.to_string(), error.category(), error.code())
}

#[rstest]
#[case::empty("", Node::Empty)]
#[case::literals("ab", Node::Concat(vec![ch('a'), ch('b')]))]
#[case::astral_literal("😀", ch('😀'))]
#[case::dot(".", Node::Any)]
#[case::anchors("^a$", Node::Concat(vec![Node::Start, ch('a'), Node::End]))]
#[case::class("[a]", class(false, CharSet::single('a')))]
#[case::class_escape("\\d", class(false, CharSet::digit()))]
#[case::word_boundary("\\b", Node::WordBoundary(true))]
#[case::not_word_boundary("\\B", Node::WordBoundary(false))]
#[case::control_escapes(
    "\\t\\n\\v\\f\\r",
    Node::Concat(['\t', '\n', '\u{b}', '\u{c}', '\r'].map(ch).to_vec())
)]
#[case::nul_then_letter("\\0a", Node::Concat(vec![ch('\0'), ch('a')]))]
#[case::capturing_group("(a)", group(ch('a')))]
#[case::non_capturing_group("(?:a)", group(ch('a')))]
#[case::empty_group("()", group(Node::Empty))]
#[case::lookahead("(?=x)", Node::Look { negated: false, node: Box::new(ch('x')) })]
#[case::negative_lookahead("(?!x)", Node::Look { negated: true, node: Box::new(ch('x')) })]
#[case::alternation("a|b|", Node::Alt(vec![ch('a'), ch('b'), Node::Empty]))]
#[case::star("a*", repeat(ch('a'), 0, None, false))]
#[case::plus("a+", repeat(ch('a'), 1, None, false))]
#[case::optional("a?", repeat(ch('a'), 0, Some(1), false))]
#[case::exact("a{2}", repeat(ch('a'), 2, Some(2), false))]
#[case::at_least("a{2,}", repeat(ch('a'), 2, None, false))]
#[case::lazy_range("a{2,3}?", repeat(ch('a'), 2, Some(3), true))]
#[case::lazy_star("a*?", repeat(ch('a'), 0, None, true))]
#[case::largest_bound("a{65535}", repeat(ch('a'), 65_535, Some(65_535), false))]
#[case::quantified_group(
    "(ab)+",
    repeat(group(Node::Concat(vec![ch('a'), ch('b')])), 1, None, false)
)]
fn parse_accepts_dialect(#[case] pattern: &str, #[case] expected: Node) {
    assert_that!(parse(pattern).unwrap()).is_equal_to(expected);
}

#[rstest]
#[case::backreference("(a)\\1")]
#[case::backreference_nine("\\9")]
#[case::named_backreference("\\k<n>")]
#[case::lookbehind("(?<=a)b")]
#[case::negative_lookbehind("(?<!a)b")]
#[case::named_group("(?<n>a)")]
#[case::unicode_property("\\p{L}")]
#[case::negated_unicode_property("\\P{L}")]
#[case::control_letter("\\cA")]
#[case::legacy_octal("\\01")]
#[case::legacy_octal_in_class("[\\1]")]
#[case::inline_flag("(?i)a")]
#[case::other_group_modifier("(?-i:a)")]
#[case::reversed_range("a{3,2}")]
#[case::dangling_quantifier("*a")]
#[case::double_quantifier("a**")]
#[case::quantified_quantifier("a{2}{3}")]
#[case::quantified_anchor("^*")]
#[case::quantified_lookahead("(?=a)*")]
#[case::bound_above_limit("a{70000}")]
#[case::huge_bound("a{99999999999999999999999}")]
#[case::lone_open_brace("a{")]
#[case::brace_without_digits("a{,5}")]
#[case::lone_close_brace("a}")]
#[case::lone_close_bracket("a]")]
#[case::unterminated_group("(a")]
#[case::unmatched_paren("a)")]
#[case::unterminated_class("[a")]
#[case::invalid_pattern("[invalid(")]
#[case::reversed_class_range("[z-a]")]
#[case::class_escape_range_start("[\\d-z]")]
#[case::class_escape_range_end("[a-\\d]")]
#[case::not_word_boundary_in_class("[\\B]")]
#[case::identity_escape_of_letter("\\a")]
#[case::dash_escape_outside_class("\\-")]
#[case::short_hex("\\x4")]
#[case::short_unicode("\\u41")]
#[case::unicode_above_max("\\u{110000}")]
#[case::unicode_braces_empty("\\u{}")]
#[case::trailing_backslash("a\\")]
fn parse_refuses_unsupported_syntax(#[case] pattern: &str) {
    let (message, category, code) = refusal(pattern);
    assert_that!(message).starts_with("UNSUPPORTED_SYNTAX: ");
    assert_that!(category).is_equal_to(Category::Expression);
    assert_that!(code).is_equal_to(ErrorCode::ExpressionError);
}

#[test]
fn parse_backreference_names_the_construct() {
    let (message, _, _) = refusal("(a)\\1");
    assert_that!(message).contains("backreference");
}

#[test]
fn parse_lookbehind_names_the_construct() {
    let (message, _, _) = refusal("(?<=a)b");
    assert_that!(message).contains("lookbehind");
}

#[test]
fn length_257_code_points_is_pattern_too_long() {
    let (message, category, _) = refusal(&"a".repeat(257));
    assert_that!(message).starts_with("PATTERN_TOO_LONG: ");
    assert_that!(category).is_equal_to(Category::Expression);
}

#[test]
fn length_256_astral_code_points_is_accepted() {
    let pattern = "😀".repeat(256);
    assert_that!(parse(&pattern)).is_ok();
}

#[test]
fn parse_127_nested_groups_is_accepted() {
    let pattern = format!("{}a{}", "(".repeat(127), ")".repeat(127));
    assert_that!(parse(&pattern)).is_ok();
}

#[test]
fn parse_256_open_groups_is_unsupported_syntax() {
    let (message, _, _) = refusal(&"(".repeat(256));
    assert_that!(message).starts_with("UNSUPPORTED_SYNTAX: ");
}

#[test]
fn parse_64_nested_lookaheads_is_accepted() {
    let pattern = format!("{}{}", "(?=".repeat(64), ")".repeat(64));
    assert_that!(parse(&pattern)).is_ok();
}

#[test]
fn parse_255_open_groups_in_a_small_stack_is_refused_without_overflow() {
    let pattern = "(".repeat(255);
    let refused = std::thread::Builder::new()
        .stack_size(64 * 1024)
        .spawn(move || parse(&pattern).is_err())
        .unwrap()
        .join()
        .unwrap();
    assert_that!(refused).is_true();
}
