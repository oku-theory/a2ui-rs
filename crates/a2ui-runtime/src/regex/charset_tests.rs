use rstest::rstest;
use speculoos::prelude::*;

use super::*;

#[rstest]
#[case::inside('5', true)]
#[case::low_end('0', true)]
#[case::high_end('9', true)]
#[case::below('/', false)]
#[case::above(':', false)]
fn contains_digit_set_matches_ascii_digits(#[case] c: char, #[case] expected: bool) {
    assert_that!(CharSet::digit().contains(c)).is_equal_to(expected);
}

#[test]
fn union_merges_overlapping_and_adjacent_ranges() {
    let set = CharSet::range('a', 'c')
        .union(&CharSet::range('d', 'f'))
        .union(&CharSet::range('b', 'e'));
    assert_that!(set).is_equal_to(CharSet::range('a', 'f'));
}

#[test]
fn union_keeps_disjoint_ranges_sorted() {
    let set = CharSet::range('x', 'z').union(&CharSet::range('a', 'c'));
    assert_that!(set.ranges().to_vec()).is_equal_to(vec![('a', 'c'), ('x', 'z')]);
}

#[rstest]
#[case::shared_char(CharSet::range('a', 'm'), CharSet::range('m', 'z'), true)]
#[case::disjoint(CharSet::range('a', 'l'), CharSet::range('m', 'z'), false)]
#[case::digit_word(CharSet::digit(), CharSet::word(), true)]
#[case::digit_space(CharSet::digit(), CharSet::space(), false)]
#[case::empty(CharSet::empty(), CharSet::range('\0', char::MAX), false)]
fn overlaps_detects_shared_chars(#[case] a: CharSet, #[case] b: CharSet, #[case] expected: bool) {
    assert_that!(a.overlaps(&b)).is_equal_to(expected);
    assert_that!(b.overlaps(&a)).is_equal_to(expected);
}

#[test]
fn negate_empty_is_every_char() {
    assert_that!(CharSet::empty().negate()).is_equal_to(CharSet::range('\0', char::MAX));
}

#[test]
fn negate_twice_is_identity() {
    let set = CharSet::word().union(&CharSet::single('\u{D7FF}'));
    assert_that!(set.negate().negate()).is_equal_to(set);
}

#[test]
fn negate_spans_the_surrogate_gap() {
    let set = CharSet::range('\0', '\u{D7FE}').union(&CharSet::range('\u{E001}', char::MAX));
    assert_that!(set.negate().ranges().to_vec()).is_equal_to(vec![('\u{D7FF}', '\u{E000}')]);
}

#[rstest]
#[case::surrogates_only(0xD800, 0xDFFF, Vec::new())]
#[case::across_surrogates(0xD7FF, 0xE000, vec![('\u{D7FF}', '\u{E000}')])]
#[case::reversed(0x62, 0x61, Vec::new())]
#[case::past_max(0x10_FFFF, 0x11_0000, vec![(char::MAX, char::MAX)])]
fn from_code_range_skips_invalid_code_points(
    #[case] lo: u32,
    #[case] hi: u32,
    #[case] expected: Vec<(char, char)>,
) {
    assert_that!(CharSet::from_code_range(lo, hi).ranges().to_vec()).is_equal_to(expected);
}

#[rstest]
#[case::tab('\t', true)]
#[case::line_feed('\n', true)]
#[case::no_break_space('\u{A0}', true)]
#[case::ogham('\u{1680}', true)]
#[case::line_separator('\u{2028}', true)]
#[case::bom('\u{FEFF}', true)]
#[case::zero_width_space('\u{200B}', false)]
#[case::letter('a', false)]
fn contains_space_set_matches_ecmascript_white_space(#[case] c: char, #[case] expected: bool) {
    assert_that!(CharSet::space().contains(c)).is_equal_to(expected);
}

#[rstest]
#[case::line_feed('\n', false)]
#[case::carriage_return('\r', false)]
#[case::line_separator('\u{2028}', false)]
#[case::paragraph_separator('\u{2029}', false)]
#[case::astral('😀', true)]
#[case::tab('\t', true)]
fn contains_dot_set_excludes_line_terminators(#[case] c: char, #[case] expected: bool) {
    assert_that!(CharSet::dot().contains(c)).is_equal_to(expected);
}

#[rstest]
#[case::underscore('_', true)]
#[case::accented('é', false)]
#[case::upper('Z', true)]
fn contains_word_set_is_ascii(#[case] c: char, #[case] expected: bool) {
    assert_that!(CharSet::word().contains(c)).is_equal_to(expected);
}
