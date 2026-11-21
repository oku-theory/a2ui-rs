use rstest::rstest;
use speculoos::prelude::*;

use super::*;
use crate::error::{Category, ErrorCode};
use crate::regex::parse::parse;

/// The safety check's verdict on `pattern`, which must parse.
fn checked(pattern: &str) -> Result<(), crate::Error> {
    check(&parse(pattern).unwrap())
}

#[rstest]
#[case::nested_plus("(a+)+", "NESTED_QUANTIFIER")]
#[case::nested_plus_anchored("(a+)+$", "NESTED_QUANTIFIER")]
#[case::nested_bounded_outer("(a*){2,5}", "NESTED_QUANTIFIER")]
#[case::nested_star_star("(a*)*b", "NESTED_QUANTIFIER")]
#[case::nested_deep("(?:x(?:y+))*", "NESTED_QUANTIFIER")]
#[case::nested_in_lookahead("(?:(?=a+)b){2}", "NESTED_QUANTIFIER")]
#[case::overlapping_prefix("(ab|ac)*", "OVERLAPPING_ALTERNATION")]
#[case::overlapping_prefix_upstream("(a|ab)*c", "OVERLAPPING_ALTERNATION")]
#[case::overlapping_class("([a-c]|b)+", "OVERLAPPING_ALTERNATION")]
#[case::overlapping_class_upstream("(\\d|[0-5])+", "OVERLAPPING_ALTERNATION")]
#[case::overlapping_same("(a|a)*", "OVERLAPPING_ALTERNATION")]
#[case::overlapping_through_nullable("(b?a|a)*", "OVERLAPPING_ALTERNATION")]
#[case::overlapping_at_least("(?:(?:a|ab)){3,}", "OVERLAPPING_ALTERNATION")]
#[case::adjacent_direct("\\d+\\d+", "ADJACENT_OVERLAP")]
#[case::adjacent_across_nullable("a+b?a+", "ADJACENT_OVERLAP")]
#[case::adjacent_across_star("\\d+\\s*\\d+", "ADJACENT_OVERLAP")]
#[case::adjacent_across_assertion("a+\\ba+", "ADJACENT_OVERLAP")]
#[case::adjacent_group_atom("(a+)a*", "ADJACENT_OVERLAP")]
#[case::adjacent_dot_letter(".*x+", "ADJACENT_OVERLAP")]
#[case::adjacent_in_group("x(a*a+)", "ADJACENT_OVERLAP")]
fn safety_refuses(#[case] pattern: &str, #[case] rule: &str) {
    let error = checked(pattern).unwrap_err();
    assert_that!(error.to_string()).starts_with(format!("{rule}: "));
    assert_that!(error.category()).is_equal_to(Category::Expression);
    assert_that!(error.code()).is_equal_to(ErrorCode::ExpressionError);
}

#[rstest]
#[case::optional_outer("(a+)?")]
#[case::optional_outer_upstream("(?:ab+)?c")]
#[case::disjoint_alternation("(a|b)*")]
#[case::disjoint_alternation_upstream("(a|b)*c")]
#[case::bounded_alternation("(a|ab){1,5}")]
#[case::separate_atoms("a+b+")]
#[case::non_nullable_separator("a+-a+")]
#[case::non_nullable_separator_upstream("\\w+\\s+\\w+")]
#[case::anchored_email("^[^@\\s]+@[^@\\s]+\\.[a-z]{2,}$")]
#[case::anchored_email_upstream("^[a-z]+@[a-z]+\\.com$")]
#[case::lookahead_password("^(?=.*\\d).{8,}$")]
#[case::counted_nesting("(a{1,1000}){1,1000}")]
#[case::bounded_adjacent("\\d{1,3}\\d{1,3}")]
#[case::disjoint_adjacent("[a-z]+[0-9]+")]
#[case::empty_branch("(a|)*")]
#[case::plain_literals("abc")]
#[case::anchored_literals("^abc$")]
#[case::anchored_digits("^\\d+$")]
#[case::anchored_word("^\\w+$")]
#[case::anchored_dot_between("^a.b$")]
#[case::word_boundary("\\b")]
#[case::anchored_dot("^.$")]
#[case::anchored_astral_plus("^\\u{1F600}+$")]
fn safety_accepts(#[case] pattern: &str) {
    assert_that!(checked(pattern)).is_ok();
}

#[test]
fn safety_nested_quantifier_uses_protocol_wording() {
    let error = checked("(a+)+").unwrap_err();
    assert_that!(error.to_string()).is_equal_to(
        "NESTED_QUANTIFIER: A quantifier whose upper bound is above 1 applies to a subexpression \
         that contains an unbounded quantifier"
            .to_owned(),
    );
}

#[test]
fn safety_rules_apply_in_protocol_order() {
    let error = checked("(a+)+\\d+\\d+").unwrap_err();
    assert_that!(error.to_string()).starts_with("NESTED_QUANTIFIER: ");
}

#[test]
fn safety_deepest_parsed_nesting_in_a_small_stack_is_nested_quantifier() {
    // 85 + 1 + 170 = 256 code points: the deepest quantified nesting a pattern can hold.
    let pattern = format!("{}a{}", "(".repeat(85), ")+".repeat(85));
    let message = std::thread::Builder::new()
        .stack_size(64 * 1024)
        .spawn(move || checked(&pattern).unwrap_err().to_string())
        .unwrap()
        .join()
        .unwrap();
    assert_that!(message).starts_with("NESTED_QUANTIFIER: ");
}
