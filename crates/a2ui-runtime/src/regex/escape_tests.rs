use rstest::rstest;
use speculoos::prelude::*;

use super::*;

fn ch(c: char) -> Node {
    Node::Char(c)
}

fn class(negated: bool, items: CharSet) -> Node {
    Node::Class { negated, items }
}

/// The class set holding each character of `text`.
fn chars(text: &str) -> CharSet {
    text.chars()
        .fold(CharSet::empty(), |set, c| set.union(&CharSet::single(c)))
}

#[rstest]
#[case::class_range_negated_escape(
    "[^a-z\\d]",
    class(true, CharSet::range('0', '9').union(&CharSet::range('a', 'z')))
)]
#[case::class_empty("[]", class(false, CharSet::empty()))]
#[case::class_negated_empty("[^]", class(true, CharSet::empty()))]
#[case::class_trailing_dash("[a-]", class(false, chars("-a")))]
#[case::class_backspace("[\\b]", class(false, CharSet::single('\u{8}')))]
#[case::class_specials("[(|{.]", class(false, chars("(|{.")))]
#[case::class_lone_surrogate("[\\uD800]", class(false, CharSet::empty()))]
fn class_reads_whole_class(#[case] pattern: &str, #[case] expected: Node) {
    assert_that!(super::class(pattern)).is_equal_to(Ok(("", expected)));
}

#[rstest]
#[case::digit("\\d", class(false, CharSet::digit()))]
#[case::not_digit("\\D", class(true, CharSet::digit()))]
#[case::word("\\w", class(false, CharSet::word()))]
#[case::not_word("\\W", class(true, CharSet::word()))]
#[case::space("\\s", class(false, CharSet::space()))]
#[case::not_space("\\S", class(true, CharSet::space()))]
#[case::nul("\\0", ch('\0'))]
#[case::hex("\\x41", ch('A'))]
#[case::unicode_four("\\u0041", ch('A'))]
#[case::unicode_braced("\\u{1F600}", ch('😀'))]
#[case::unicode_surrogate_pair("\\uD83D\\uDE00", ch('😀'))]
#[case::lone_surrogate("\\uDE00", class(false, CharSet::empty()))]
fn escape_reads_whole_escape(#[case] pattern: &str, #[case] expected: Node) {
    assert_that!(escape(pattern)).is_equal_to(Ok(("", expected)));
}

#[test]
fn escape_identity_of_each_syntax_character_is_that_character() {
    for c in IDENTITY_ESCAPES.chars() {
        assert_that!(escape(&format!("\\{c}"))).is_equal_to(Ok(("", ch(c))));
    }
}

#[test]
fn escape_nul_before_letter_stops_at_letter() {
    assert_that!(escape("\\0a")).is_equal_to(Ok(("a", ch('\0'))));
}

#[test]
fn class_unclosed_is_refused() {
    let refusal = nom::Err::Failure(Fault::Refused(CLASS_UNCLOSED));
    assert_that!(super::class("[a")).is_equal_to(Err(refusal));
}
