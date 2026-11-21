use rstest::rstest;
use speculoos::prelude::*;

use super::*;

/// The five flags of `rules`: orphans, dangling references, missing root, unknown elements,
/// undeclared properties.
fn flags(rules: ValidationRules) -> [bool; 5] {
    [
        rules.allows_orphan_components(),
        rules.allows_dangling_references(),
        rules.allows_missing_root(),
        rules.allows_unknown_elements(),
        rules.allows_undeclared_properties(),
    ]
}

#[test]
fn strict_is_the_default() {
    assert_that!(ValidationRules::default()).is_equal_to(ValidationRules::strict());
    assert_that!(flags(ValidationRules::strict())).is_equal_to([false; 5]);
}

#[test]
fn permissive_sets_all_five() {
    assert_that!(flags(ValidationRules::permissive())).is_equal_to([true; 5]);
}

#[rstest]
#[case::orphans(ValidationRules::allow_orphan_components, 0)]
#[case::dangling(ValidationRules::allow_dangling_references, 1)]
#[case::missing_root(ValidationRules::allow_missing_root, 2)]
#[case::unknown(ValidationRules::allow_unknown_elements, 3)]
#[case::undeclared(ValidationRules::allow_undeclared_properties, 4)]
fn each_setter_toggles_only_its_flag(
    #[case] setter: fn(ValidationRules, bool) -> ValidationRules,
    #[case] index: usize,
) {
    let mut expected = [false; 5];
    expected[index] = true;

    let on = setter(ValidationRules::strict(), true);

    assert_that!(flags(on)).is_equal_to(expected);
    assert_that!(setter(on, false)).is_equal_to(ValidationRules::strict());
}
