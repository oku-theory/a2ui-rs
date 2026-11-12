use rstest::rstest;
use speculoos::prelude::*;

use super::*;

#[rstest]
#[case::empty("", "")]
#[case::plain("Button", "Button")]
#[case::unicode_identifier("Bouton_é", "Bouton_é")]
#[case::quotes_and_backslash("'a' \"b\" \\n", "'a' \"b\" \\n")]
#[case::combining_mark("e\u{301}", "e\u{301}")]
#[case::zero_width_joiner_is_format("a\u{200d}b", "a\\u{200d}b")]
#[case::newline("a\nb", "a\\nb")]
#[case::carriage_return_and_tab("a\r\tb", "a\\r\\tb")]
#[case::nul_and_delete("\0\u{7f}", "\\0\\u{7f}")]
#[case::next_line_c1("a\u{85}b", "a\\u{85}b")]
#[case::escape_sequence("\u{1b}[31mred", "\\u{1b}[31mred")]
#[case::arabic_letter_mark("\u{61c}", "\\u{61c}")]
#[case::marks_and_zero_width("\u{200b}\u{200f}", "\\u{200b}\\u{200f}")]
#[case::separators_and_overrides("\u{2028}\u{202e}", "\\u{2028}\\u{202e}")]
#[case::isolates_and_deprecated("\u{2066}\u{206f}", "\\u{2066}\\u{206f}")]
#[case::outside_the_ranges("\u{202f}\u{2065}\u{2070}", "\u{202f}\u{2065}\u{2070}")]
#[case::soft_hyphen_and_bom("\u{ad}\u{feff}", "\\u{ad}\\u{feff}")]
#[case::interlinear_annotations("\u{fff9}\u{fffb}", "\\u{fff9}\\u{fffb}")]
#[case::arabic_number_signs("\u{600}\u{605}\u{606}", "\\u{600}\\u{605}\u{606}")]
#[case::word_joiner_to_invisible_plus("\u{2060}\u{2064}", "\\u{2060}\\u{2064}")]
#[case::mongolian_vowel_separator("\u{180e}", "\\u{180e}")]
#[case::kaithi_and_egyptian("\u{110bd}\u{1343f}\u{13440}", "\\u{110bd}\\u{1343f}\u{13440}")]
#[case::shorthand_and_musical("\u{1bca3}\u{1d17a}\u{1d17b}", "\\u{1bca3}\\u{1d17a}\u{1d17b}")]
#[case::tag_characters("\u{e0001}\u{e0041}\u{e007f}", "\\u{e0001}\\u{e0041}\\u{e007f}")]
fn quoted_short_text_escapes_only_invisible_characters(#[case] text: &str, #[case] expected: &str) {
    assert_that!(Quoted::new(text).to_string()).is_equal_to(expected.to_owned());
}

#[test]
fn quoted_text_at_the_cap_is_kept_whole() {
    let text = "x".repeat(MAX_QUOTED_CHARS);
    assert_that!(Quoted::new(&text).to_string()).is_equal_to(text);
}

#[test]
fn quoted_text_one_past_the_cap_elides_one_character() {
    let half = MAX_QUOTED_CHARS / 2;
    let text = format!("{}m{}", "h".repeat(half), "t".repeat(half));
    let expected = format!(
        "{}…[1 characters elided]…{}",
        "h".repeat(half),
        "t".repeat(half)
    );
    assert_that!(Quoted::new(&text).to_string()).is_equal_to(expected);
}

#[test]
fn quoted_multibyte_text_past_the_cap_counts_characters() {
    let half = MAX_QUOTED_CHARS / 2;
    let text = "é".repeat(MAX_QUOTED_CHARS + 10);
    let expected = format!(
        "{}…[10 characters elided]…{}",
        "é".repeat(half),
        "é".repeat(half)
    );
    assert_that!(Quoted::new(&text).to_string()).is_equal_to(expected);
}

#[test]
fn quoted_cap_never_cuts_an_escape() {
    let half = MAX_QUOTED_CHARS / 2;
    let text = "\u{1b}".repeat(MAX_QUOTED_CHARS + 3);
    let expected = format!(
        "{}…[3 characters elided]…{}",
        "\\u{1b}".repeat(half),
        "\\u{1b}".repeat(half)
    );
    assert_that!(Quoted::new(&text).to_string()).is_equal_to(expected);
}

#[test]
fn quoted_debug_is_the_escaped_text_as_a_string_literal() {
    let debug = format!("{:?}", Quoted::new("a\nb"));
    assert_that!(debug).is_equal_to("\"a\\\\nb\"".to_owned());
}

#[test]
fn reason_quoted_piece_is_joined_as_escaped_once() {
    let reason = reason![
        "$ref '",
        Quoted::new("#/\u{1b}"),
        "' at ",
        Quoted::new("/x")
    ];
    assert_that!(reason.to_string()).is_equal_to("$ref '#/\\u{1b}' at /x".to_owned());
}

#[test]
fn reason_long_input_piece_is_capped_and_the_wording_kept() {
    let half = MAX_QUOTED_CHARS / 2;
    let input = "M".repeat(4 * MAX_QUOTED_CHARS);
    let reason = reason!["$ref '", Quoted::new(&input), "' does not resolve"];
    let expected = format!(
        "$ref '{}{}{}' does not resolve",
        "M".repeat(half),
        elision_marker(4 * MAX_QUOTED_CHARS - 2 * half),
        "M".repeat(half)
    );
    assert_that!(reason.to_string()).is_equal_to(expected);
}

/// Fixed wording longer than [`MAX_QUOTED_CHARS`] (640 characters).
const LONG_WORDING: &str = concat!(
    "0123456789012345678901234567890123456789012345678901234567890123",
    "0123456789012345678901234567890123456789012345678901234567890123",
    "0123456789012345678901234567890123456789012345678901234567890123",
    "0123456789012345678901234567890123456789012345678901234567890123",
    "0123456789012345678901234567890123456789012345678901234567890123",
    "0123456789012345678901234567890123456789012345678901234567890123",
    "0123456789012345678901234567890123456789012345678901234567890123",
    "0123456789012345678901234567890123456789012345678901234567890123",
    "0123456789012345678901234567890123456789012345678901234567890123",
    "0123456789012345678901234567890123456789012345678901234567890123",
);

#[test]
fn reason_long_fixed_text_is_never_capped() {
    let reason = reason![LONG_WORDING, Quoted::new("x"), LONG_WORDING];
    assert_that!(reason.to_string()).is_equal_to(format!("{LONG_WORDING}x{LONG_WORDING}"));
}

#[test]
fn reason_fixed_text_with_a_control_is_escaped() {
    let reason = Reason::from("a\u{1b}b\n");
    assert_that!(reason.to_string()).is_equal_to("a\\u{1b}b\\n".to_owned());
}

/// Resolves to `()` when `T` is not a [`Piece`]; ambiguous, so a compile error, when it is.
trait NotAPiece<Marker> {
    /// Names the implementation the compiler picked.
    fn check() {}
}

impl<T: ?Sized> NotAPiece<()> for T {}

impl<T: ?Sized + Piece> NotAPiece<u8> for T {}

#[test]
fn piece_unbounded_text_types_are_not_pieces() {
    <String as NotAPiece<_>>::check();
    <Reason as NotAPiece<_>>::check();
    <&String as NotAPiece<_>>::check();
    <Box<str> as NotAPiece<_>>::check();
}
