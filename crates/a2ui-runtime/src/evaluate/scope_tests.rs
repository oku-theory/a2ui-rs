use speculoos::prelude::*;

use super::*;

#[test]
fn scope_new_is_outside_user_activation() {
    let data = DataModel::default();

    let scope = Scope::new("s", None, &data);

    assert_that!(scope.user_activation()).is_false();
}

#[test]
fn scope_with_user_activation_is_inside_it() {
    let data = DataModel::default();

    let scope = Scope::new("s", None, &data).with_user_activation(true);

    assert_that!(scope.user_activation()).is_true();
}
