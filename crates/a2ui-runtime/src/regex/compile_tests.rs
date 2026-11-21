use rstest::rstest;
use speculoos::prelude::*;

use super::*;
use crate::error::{Category, ErrorCode};
use crate::regex::parse::parse;
use crate::regex::vm::Machine;

/// The program `pattern` compiles to; it must parse.
fn compiled(pattern: &str) -> Result<Program, Error> {
    compile(&parse(pattern).unwrap())
}

/// The instructions `pattern` compiles to.
fn instructions(pattern: &str) -> Vec<Inst> {
    compiled(pattern).unwrap().insts
}

#[test]
fn counted_repeat_expands() {
    use Inst::{Char, Match, Split};
    assert_that!(instructions("a{3}")).is_equal_to(vec![Char('a'), Char('a'), Char('a'), Match]);
    assert_that!(instructions("a{1,3}")).is_equal_to(vec![
        Char('a'),
        Split(2, 5),
        Char('a'),
        Split(4, 5),
        Char('a'),
        Match,
    ]);
}

#[test]
fn counted_repeat_lazy_prefers_exit() {
    use Inst::{Char, Match, Split};
    assert_that!(instructions("a{0,1}?")).is_equal_to(vec![Split(2, 1), Char('a'), Match]);
}

#[test]
fn star_loops_back_to_its_split() {
    use Inst::{Char, Jmp, Match, Split};
    assert_that!(instructions("a*")).is_equal_to(vec![Split(1, 3), Char('a'), Jmp(0), Match]);
    assert_that!(instructions("a*?")).is_equal_to(vec![Split(3, 1), Char('a'), Jmp(0), Match]);
}

#[test]
fn at_least_loops_over_its_last_copy() {
    use Inst::{Char, Match, Split};
    assert_that!(instructions("a{2,}")).is_equal_to(vec![Char('a'), Char('a'), Split(1, 3), Match]);
}

#[test]
fn plus_loops_without_copying_its_body() {
    use Inst::{Char, Match, Split};
    assert_that!(instructions("a+")).is_equal_to(vec![Char('a'), Split(0, 2), Match]);
    assert_that!(instructions("a+?")).is_equal_to(vec![Char('a'), Split(2, 0), Match]);
}

#[test]
fn plus_over_large_counted_body_compiles() {
    assert_that!(compiled("(?:a{1,1500})+").map(|program| program.insts.len()))
        .is_ok_containing(3001);
}

#[test]
fn alternation_splits_in_branch_order() {
    use Inst::{Char, Jmp, Match, Split};
    assert_that!(instructions("a|b|c")).is_equal_to(vec![
        Split(1, 3),
        Char('a'),
        Jmp(7),
        Split(4, 6),
        Char('b'),
        Jmp(7),
        Char('c'),
        Match,
    ]);
}

#[test]
fn lookahead_body_ends_in_its_own_match() {
    use Inst::{Char, Look, Match};
    assert_that!(instructions("(?!a)b")).is_equal_to(vec![
        Look {
            negated: true,
            start: 1,
            end: 3,
        },
        Char('a'),
        Match,
        Char('b'),
        Match,
    ]);
}

#[test]
fn repeated_lookahead_is_relocated() {
    use Inst::{Char, Look, Match};
    let look = |start, end| Look {
        negated: false,
        start,
        end,
    };
    assert_that!(instructions("(?:(?=a)){2}")).is_equal_to(vec![
        look(1, 3),
        Char('a'),
        Match,
        look(4, 6),
        Char('a'),
        Match,
        Match,
    ]);
}

#[rstest]
#[case::zero_count("a{0}")]
#[case::zero_range("a{0,0}")]
#[case::empty_star("(?:)*")]
#[case::empty_counted("(?:){65535}")]
#[case::empty_nested("(?:(?:(?:){65535}){65535}){65535}")]
fn repeat_of_nothing_compiles_to_nothing(#[case] pattern: &str) {
    assert_that!(instructions(pattern)).is_equal_to(vec![Inst::Match]);
}

#[test]
fn class_compiles_to_a_set() {
    let insts = instructions("[^a-c]");
    let Some(Inst::Set(set)) = insts.first() else {
        panic!("expected a set, got {insts:?}");
    };
    assert_that!(set.contains('d')).is_true();
    assert_that!(set.contains('b')).is_false();
    assert_that!(insts.len()).is_equal_to(2);
}

#[test]
fn program_of_4096_instructions_compiles() {
    assert_that!(compiled("a{4095}").map(|program| program.insts.len())).is_ok_containing(4096);
}

#[rstest]
#[case::one_over("a{4096}")]
#[case::nested_counts("(a{1,1000}){1,1000}")]
#[case::largest_bound("a{65535}")]
#[case::largest_nested("(?:(?:a{65535}){65535}){65535}")]
#[case::lookahead("(?=a{5000})")]
fn program_over_4096_is_pattern_too_complex(#[case] pattern: &str) {
    let error = compiled(pattern).unwrap_err();
    assert_that!(error.to_string()).starts_with("PATTERN_TOO_COMPLEX: ");
    assert_that!(error.category()).is_equal_to(Category::Expression);
    assert_that!(error.code()).is_equal_to(ErrorCode::ExpressionError);
}

#[test]
fn compile_deepest_parsed_nesting_in_a_small_stack_matches() {
    // 85 + 1 + 170 = 256 code points: the deepest nesting a pattern can hold. The safety check
    // refuses it; the compiler and the machine must still not need a deep stack.
    let pattern = format!("{}a{}", "(".repeat(85), ")+".repeat(85));
    let matched = std::thread::Builder::new()
        .stack_size(64 * 1024)
        .spawn(move || {
            let machine = Machine::new(compiled(&pattern).unwrap());
            let value: Vec<char> = "baaa".chars().collect();
            machine.is_match(&value)
        })
        .unwrap()
        .join()
        .unwrap();
    assert_that!(matched).is_true();
}
