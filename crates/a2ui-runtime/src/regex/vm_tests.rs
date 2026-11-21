use rstest::rstest;
use speculoos::prelude::*;

use crate::error::{Category, ErrorCode};
use crate::regex::{DialectRegex, MAX_VALUE_CHARS};

/// Whether `pattern`, which the dialect must accept, matches `value`.
fn matches(pattern: &str, value: &str) -> bool {
    DialectRegex::new(pattern).unwrap().is_match(value)
}

#[rstest]
// The `regex_v10_*` cases of upstream functions.yaml that match.
#[case::dollar_before_trailing_newline("^abc$", "abc\n", false)]
#[case::digit_ascii_only("^\\d+$", "\u{661}\u{662}\u{663}", false)]
#[case::word_ascii_only("^\\w+$", "caf\u{e9}", false)]
#[case::dot_excludes_newline("^a.b$", "a\nb", false)]
#[case::case_sensitive("^abc$", "ABC", false)]
#[case::word_boundary_ascii_only("\\b", "\u{e9}", false)]
#[case::search_semantics("abc", "xxabcxx", true)]
#[case::astral_dot_one_code_point("^.$", "\u{1F600}", true)]
#[case::braced_unicode_escape("^\\u{1F600}+$", "\u{1F600}\u{1F600}", true)]
#[case::optional_outer_group("(?:ab+)?c", "abbc", true)]
#[case::disjoint_alternation("(a|b)*c", "abac", true)]
#[case::non_nullable_separator("\\w+\\s+\\w+", "hello world", true)]
#[case::anchored_email_like("^[a-z]+@[a-z]+\\.com$", "ada@example.com", true)]
#[case::lookahead_password_like("^(?=.*\\d).{8,}$", "secret42", true)]
#[case::regex_match("^[0-9]+$", "12345", true)]
#[case::regex_mismatch("^[0-9]+$", "abc", false)]
// Search, anchors and assertions.
#[case::search_inner("b", "abc", true)]
#[case::search_absent("d", "abc", false)]
#[case::empty_pattern_empty_value("", "", true)]
#[case::empty_pattern_any_value("", "abc", true)]
#[case::start_only_at_start("^b", "ab", false)]
#[case::end_only_at_end("a$", "ab", false)]
#[case::dot_excludes_carriage_return("^.$", "\r", false)]
#[case::dot_excludes_line_separator("^.$", "\u{2028}", false)]
#[case::space_includes_line_separator("^\\s$", "\u{2028}", true)]
#[case::word_boundary_between_words("a\\b \\bb", "a b", true)]
#[case::not_word_boundary_inside_word("a\\Bb", "ab", true)]
#[case::not_word_boundary_at_edge("a\\B", "a", false)]
#[case::negated_class("^[^a]$", "b", true)]
#[case::negated_class_excluded("^[^a]$", "a", false)]
// Greedy and lazy quantifiers give the same verdict.
#[case::lazy_plus("^a+?$", "aaa", true)]
#[case::lazy_star_before_literal("^a*?b$", "aab", true)]
#[case::lazy_bounded("^a{1,3}?$", "aaa", true)]
#[case::greedy_bounded_too_many("^a{1,3}$", "aaaa", false)]
#[case::greedy_bounded_too_few("^a{2,3}$", "a", false)]
#[case::at_least("^a{2,}$", "aaaa", true)]
#[case::at_least_too_few("^a{2,}$", "a", false)]
// Lookahead.
#[case::lookahead_positive("^(?=a)\\w", "ab", true)]
#[case::lookahead_positive_fails("^(?=a)\\w", "bb", false)]
#[case::lookahead_negative("^(?!a)\\w", "bb", true)]
#[case::lookahead_negative_fails("^(?!a)\\w", "ab", false)]
#[case::lookahead_zero_width("^(?=ab)ab$", "ab", true)]
#[case::lookahead_nested("^(?=a(?!b))a", "ac", true)]
#[case::lookahead_nested_fails("^(?=a(?!b))a", "ab", false)]
#[case::lookahead_repeated("^(?:(?=a)a){3}$", "aaa", true)]
#[case::lookahead_sees_end("a(?=$)", "ba", true)]
// Alternation: a later branch is tried when an earlier one fails.
#[case::alternation_second_branch("^(?:a|ab)c$", "abc", true)]
#[case::alternation_first_branch("^(?:ab|a)c$", "ac", true)]
#[case::alternation_none("^(?:ab|a)c$", "bc", false)]
#[case::alternation_empty_branch("^a(?:|b)$", "a", true)]
// `{0}` and loops over what can match nothing.
#[case::zero_count_empty("^a{0}$", "", true)]
#[case::zero_count_refuses("^a{0}$", "a", false)]
#[case::zero_count_between("^a{0}b$", "b", true)]
#[case::optional_loop("^(?:a?)*$", "aa", true)]
#[case::optional_loop_stops("^(?:a?)*$", "ab", false)]
#[case::empty_branch_loop("^(?:a|)*b$", "aab", true)]
fn is_match_follows_dialect(#[case] pattern: &str, #[case] value: &str, #[case] expected: bool) {
    assert_that!(matches(pattern, value)).is_equal_to(expected);
}

#[rstest]
#[case::reaches_the_count("^(?:a{1,1500})+$", "aaa", true)]
#[case::needs_one("^(?:a{1,1500})+$", "", false)]
fn is_match_plus_over_large_counted_body(
    #[case] pattern: &str,
    #[case] value: &str,
    #[case] expected: bool,
) {
    assert_that!(matches(pattern, value)).is_equal_to(expected);
}

#[test]
fn new_program_over_4096_is_pattern_too_complex() {
    let error = DialectRegex::new("(a{1,1000}){1,1000}").unwrap_err();
    assert_that!(error.to_string()).starts_with("PATTERN_TOO_COMPLEX: ");
    assert_that!(error.category()).is_equal_to(Category::Expression);
    assert_that!(error.code()).is_equal_to(ErrorCode::ExpressionError);
}

#[test]
fn pattern_of_256_code_points_matches() {
    let pattern = "a".repeat(256);
    assert_that!(matches(&pattern, &pattern)).is_true();
}

#[test]
fn value_4096_code_points_matches() {
    assert_that!(matches("^a+$", &"a".repeat(MAX_VALUE_CHARS))).is_true();
}

#[test]
fn value_4096_astral_code_points_matches() {
    assert_that!(matches("^.+$", &"\u{1F600}".repeat(MAX_VALUE_CHARS))).is_true();
}

#[test]
fn value_4097_code_points_is_false() {
    assert_that!(matches("^a+$", &"a".repeat(MAX_VALUE_CHARS + 1))).is_false();
}

#[test]
fn value_4097_code_points_is_false_even_for_empty_pattern() {
    assert_that!(matches("", &"a".repeat(MAX_VALUE_CHARS + 1))).is_false();
}

#[rstest]
// Review Focus 2.
#[case::counted_then_absent("a{1,1000}b", "a".repeat(4096))]
#[case::counted_then_present("a{1,1000}b", format!("{}b", "a".repeat(4095)))]
#[case::password_no_digit("^(?=.*\\d).{8,}$", "a".repeat(4096))]
#[case::password_digit_last("^(?=.*\\d).{8,}$", format!("{}1", "a".repeat(4095)))]
#[case::longest_pattern(format!("{}b$", "a?".repeat(127)), "a".repeat(4096))]
// Accepted by the protocol's safety rules, exponential in a backtracking engine.
#[case::optional_overlap_star("(?:(?:a|a)?)*", format!("{}!", "a".repeat(4095)))]
#[case::optional_overlap_star_anchored("^(?:(?:a|a)?)*$", format!("{}!", "a".repeat(4095)))]
#[case::overlap_or_b_star("(?:(?:a|a)|b)*", format!("{}!", "a".repeat(4095)))]
#[case::overlap_or_b_star_anchored("^(?:(?:a|a)|b)*$", format!("{}!", "a".repeat(4095)))]
#[case::prefixed_overlap_star("(?:x(?:a|a))*", format!("{}!!", "xa".repeat(2047)))]
#[case::prefixed_overlap_star_anchored("^(?:x(?:a|a))*$", format!("{}!!", "xa".repeat(2047)))]
#[case::plus_then_nullable("a+(?:b?a*)", format!("{}!", "a".repeat(4095)))]
#[case::plus_then_nullable_anchored("^a+(?:b?a*)$", format!("{}!", "a".repeat(4095)))]
fn vm_step_count_stays_within_bound(#[case] pattern: String, #[case] value: String) {
    let regex = DialectRegex::new(&pattern).unwrap();
    let value: Vec<char> = value.chars().collect();
    assert_that!(value.len()).is_less_than_or_equal_to(MAX_VALUE_CHARS);
    let (_, steps) = regex.machine.search(&value);
    let bound = 3 * regex.machine.len() * (value.len() + 1);
    assert_that!(steps).is_less_than_or_equal_to(bound);
}

#[test]
fn vm_anchored_search_visits_every_position() {
    // The exhaustive runs above really walk the value: one step a position at least.
    let regex = DialectRegex::new("^(?:(?:a|a)?)*$").unwrap();
    let value: Vec<char> = format!("{}!", "a".repeat(4095)).chars().collect();
    let (matched, steps) = regex.machine.search(&value);
    assert_that!(matched).is_false();
    assert_that!(steps).is_greater_than_or_equal_to(value.len());
}

#[test]
fn new_deepest_accepted_nesting_in_a_small_stack_matches() {
    // 85 + 1 + 170 = 256 code points: `?` has upper bound 1, so the safety check accepts it.
    let pattern = format!("{}a{}", "(".repeat(85), ")?".repeat(85));
    let matched = std::thread::Builder::new()
        .stack_size(64 * 1024)
        .spawn(move || matches(&pattern, "a"))
        .unwrap()
        .join()
        .unwrap();
    assert_that!(matched).is_true();
}

#[test]
fn new_deepest_parsed_nesting_in_a_small_stack_is_nested_quantifier() {
    // The same 256-code-point nesting under `+`: it compiles now, but the safety check refuses it.
    let pattern = format!("{}a{}", "(".repeat(85), ")+".repeat(85));
    let refused = std::thread::Builder::new()
        .stack_size(64 * 1024)
        .spawn(move || {
            DialectRegex::new(&pattern)
                .map(|_| ())
                .unwrap_err()
                .to_string()
        })
        .unwrap()
        .join()
        .unwrap();
    assert_that!(refused).starts_with("NESTED_QUANTIFIER: ");
}
