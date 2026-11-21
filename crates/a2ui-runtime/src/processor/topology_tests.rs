use rstest::rstest;
use serde_json::{Value, json};
use speculoos::prelude::*;

use super::*;
use crate::catalog::test_support::on_a_small_stack;
use crate::processor::children::edges;
use crate::processor::test_support::{chain, column, slot_catalogs, staged, surface, text};
use crate::{Category, ErrorCode};

/// Checks `components`, sent at once to a fresh surface, under `rules`.
fn run(components: &[Value], rules: ValidationRules) -> Result<(), Error> {
    let surface = surface();
    let staged = staged(components);
    check(&surface.merged(&staged), &slot_catalogs(), rules)
}

/// The error checking `components` under strict rules gives.
fn refused(components: &[Value]) -> Error {
    run(components, ValidationRules::strict()).unwrap_err()
}

/// A `Card` `id` holding `child`.
fn card(id: &str, child: &str) -> Value {
    json!({"id": id, "component": "Card", "child": child})
}

/// `check` on `components`, and how many edges it visited against how many there are.
fn visits(components: &[Value]) -> (Result<(), Error>, usize, usize) {
    let surface = surface();
    let staged = staged(components);
    let view = surface.merged(&staged);
    let set = slot_catalogs();
    let count = edges(&view, &set).unwrap().len();
    EDGE_VISITS.with(|visits| visits.set(0));
    let result = check(&view, &set, ValidationRules::strict());
    (result, EDGE_VISITS.with(std::cell::Cell::get), count)
}

const RECURSION: (Category, ErrorCode) = (Category::Recursion, ErrorCode::RecursionLimitExceeded);
const INTEGRITY: (Category, ErrorCode) = (Category::Integrity, ErrorCode::IntegrityError);
const VALIDATION: (Category, ErrorCode) = (Category::Validation, ErrorCode::ValidationFailed);

/// `root` -> `m` -> `s`, and `root` -> `d0` -> ... -> `d49` -> `s`: `s` is 2 and 51 deep.
fn deep_diamond() -> Vec<Value> {
    let mut components = vec![column("root", &["m", "d0"]), card("m", "s"), text("s")];
    components.extend((0..50).map(|index| {
        let next = if index == 49 {
            "s".to_owned()
        } else {
            format!("d{}", index + 1)
        };
        card(&format!("d{index}"), &next)
    }));
    components
}

#[rstest]
#[case::self_reference(
    vec![column("root", &["root"])],
    RECURSION,
    "Self-reference detected: component 'root' lists itself as a child"
)]
#[case::dangling(
    vec![
        column("root", &["stack"]),
        json!({"id": "stack", "component": "Stack", "slots": ["t1", "missing"]}),
        text("t1"),
    ],
    INTEGRITY,
    "Dangling reference: component 'stack' references non-existent component 'missing' in \
     field 'slots[1]'"
)]
#[case::template_dangling(
    vec![json!({"id": "root", "component": "List",
        "children": {"componentId": "missing_id", "path": "/items"}})],
    INTEGRITY,
    "Dangling reference: component 'root' references non-existent component 'missing_id' in \
     field 'children.componentId'"
)]
#[case::cycle_through_root(
    vec![column("root", &["a"]), card("a", "root")],
    RECURSION,
    "Circular reference detected: circular component reference 'root' -> 'a' -> 'root'"
)]
#[case::cycle_away_from_root(
    vec![column("root", &["x"]), text("x"), card("c", "d"), card("d", "b"), card("b", "c")],
    RECURSION,
    "Circular reference detected: circular component reference 'c' -> 'd' -> 'b' -> 'c'"
)]
#[case::missing_root(vec![text("a")], VALIDATION, "Missing root component 'root'")]
#[case::orphan(
    vec![column("root", &["a"]), text("a"), text("b")],
    INTEGRITY,
    "Component 'b' is not reachable from 'root'"
)]
#[case::too_deep(
    chain(51),
    RECURSION,
    "Global recursion limit exceeded: component 'c50' is 51 levels below 'root'"
)]
#[case::deep_diamond(
    deep_diamond(),
    RECURSION,
    "Global recursion limit exceeded: component 's' is 51 levels below 'root'"
)]
fn check_rejects_invalid_topology(
    #[case] components: Vec<Value>,
    #[case] class: (Category, ErrorCode),
    #[case] text: &str,
) {
    let error = refused(&components);

    assert_that!((error.category(), error.code())).is_equal_to(class);
    assert_that!(error.to_string()).is_equal_to(text.to_owned());
    assert_that!(error.path()).is_none();
    assert_that!(error.surface_id()).is_equal_to(Some("s1"));
}

#[test]
fn cycle_report_is_the_same_twice() {
    let components = [
        column("root", &["x"]),
        text("x"),
        card("c", "d"),
        card("d", "b"),
        card("b", "c"),
    ];

    let first = refused(&components).to_string();
    let second = refused(&components).to_string();

    assert_that!(first).is_equal_to(second);
}

#[rstest]
#[case::self_reference_before_dangling(
    vec![column("root", &["root", "missing"])],
    "Self-reference detected: component 'root' lists itself as a child"
)]
#[case::dangling_before_cycle(
    vec![column("root", &["a", "missing"]), card("a", "root")],
    "Dangling reference: component 'root' references non-existent component 'missing' in \
     field 'children[1]'"
)]
#[case::cycle_before_missing_root(
    vec![card("a", "b"), card("b", "a")],
    "Circular reference detected: circular component reference 'a' -> 'b' -> 'a'"
)]
#[case::orphan_before_depth(
    { let mut components = chain(51); components.push(text("o")); components },
    "Component 'o' is not reachable from 'root'"
)]
fn check_reports_the_first_rule_in_order(#[case] components: Vec<Value>, #[case] text: &str) {
    assert_that!(refused(&components).to_string()).is_equal_to(text.to_owned());
}

#[test]
fn fifty_one_component_chain_is_accepted() {
    assert_that!(run(&chain(50), ValidationRules::strict())).is_ok();
}

#[test]
fn template_child_is_reachable() {
    let components = [
        json!({"id": "root", "component": "List",
            "children": {"componentId": "item", "path": "/items"}}),
        text("item"),
    ];
    assert_that!(run(&components, ValidationRules::strict())).is_ok();
}

#[test]
fn empty_surface_is_valid() {
    assert_that!(run(&[], ValidationRules::strict())).is_ok();
}

#[test]
fn incremental_update_without_root_keeps_the_known_root() {
    let mut surface = surface();
    surface.commit(staged(&[column("root", &["a"]), text("a")]));
    let update = staged(&[json!({"id": "a", "component": "Text", "text": "new"})]);

    let result = check(
        &surface.merged(&update),
        &slot_catalogs(),
        ValidationRules::strict(),
    );
    surface.commit(update);

    assert_that!(result).is_ok();
    assert_that!(surface.component("root").cloned())
        .is_equal_to(column("root", &["a"]).as_object().cloned());
}

#[test]
fn allow_dangling_ignores_the_edge() {
    let rules = ValidationRules::strict().allow_dangling_references(true);
    let components = [column("root", &["a", "missing"]), text("a")];
    assert_that!(run(&components, rules)).is_ok();
}

#[test]
fn allow_orphans_accepts() {
    let rules = ValidationRules::strict().allow_orphan_components(true);
    let components = [column("root", &[]), text("b")];
    assert_that!(run(&components, rules)).is_ok();
}

#[test]
fn allow_missing_root_buffers_then_checks() {
    let rules = ValidationRules::strict().allow_missing_root(true);
    let mut surface = surface();
    let set = slot_catalogs();
    let first = staged(&[column("box", &["a"]), text("a")]);
    let buffered = check(&surface.merged(&first), &set, rules);
    surface.commit(first);
    let second = staged(&[column("root", &["box"]), text("stray")]);

    let error = check(&surface.merged(&second), &set, rules).unwrap_err();

    assert_that!(buffered).is_ok();
    assert_that!(error.to_string())
        .is_equal_to("Component 'stray' is not reachable from 'root'".to_owned());
}

#[test]
fn allow_missing_root_still_refuses_dangling() {
    let rules = ValidationRules::strict().allow_missing_root(true);
    let error = run(&[column("box", &["missing"])], rules).unwrap_err();
    assert_that!(error.code()).is_equal_to(ErrorCode::IntegrityError);
    assert_that!(error.to_string()).is_equal_to(
        "Dangling reference: component 'box' references non-existent component 'missing' \
         in field 'children[0]'"
            .to_owned(),
    );
}

#[rstest]
#[case::self_reference(vec![column("root", &["root"])])]
#[case::cycle(vec![card("a", "b"), card("b", "a")])]
#[case::depth(chain(51))]
fn permissive_still_refuses_self_reference_cycles_and_depth(#[case] components: Vec<Value>) {
    let error = run(&components, ValidationRules::permissive()).unwrap_err();
    assert_that!((error.category(), error.code())).is_equal_to(RECURSION);
}

#[test]
fn long_chain_is_refused_on_a_small_stack() {
    let (result, visited, count) = on_a_small_stack(|| {
        let (result, visited, count) = visits(&chain(10_000));
        (result.map_err(|error| error.to_string()), visited, count)
    });

    let expected = "Global recursion limit exceeded: component 'c50' is 51 levels below 'root'";
    assert_that!(result).is_equal_to(Err(expected.to_owned()));
    assert_that!(visited).is_less_than_or_equal_to(2 * count);
}

#[test]
fn wide_fan_out_is_accepted() {
    let ids: Vec<String> = (0..10_000).map(|index| format!("t{index}")).collect();
    let names: Vec<&str> = ids.iter().map(String::as_str).collect();
    let mut components = vec![column("root", &names)];
    components.extend(names.iter().map(|id| text(id)));

    let (result, visited, count) = visits(&components);

    assert_that!(result).is_ok();
    assert_that!(visited).is_less_than_or_equal_to(2 * count);
}

#[test]
fn diamond_lattice_walk_is_linear() {
    // 50 layers of 50, each node listing the whole next layer: every pair of layers is a
    // lattice of diamonds, and the last layer is 50 levels below `root`.
    let layer = |at: usize| -> Vec<String> { (0..50).map(|n| format!("n{at}_{n}")).collect() };
    let first = layer(0);
    let mut components = vec![column(
        "root",
        &first.iter().map(String::as_str).collect::<Vec<_>>(),
    )];
    for at in 0..50 {
        let next = if at == 49 { Vec::new() } else { layer(at + 1) };
        let next: Vec<&str> = next.iter().map(String::as_str).collect();
        components.extend(layer(at).iter().map(|id| column(id, &next)));
    }

    let (result, visited, count) = visits(&components);

    assert_that!(result).is_ok();
    assert_that!(visited).is_less_than_or_equal_to(2 * count);
}

#[test]
fn large_ring_is_a_cycle() {
    let name = |index: usize| match index % 10_000 {
        0 => "root".to_owned(),
        index => format!("r{index}"),
    };
    let components: Vec<Value> = (0..10_000)
        .map(|index| card(&name(index), &name(index + 1)))
        .collect();

    let (result, visited, count) = visits(&components);

    let expected = "Circular reference detected: circular component reference 'root' -> 'r1' -> \
                    'r2' -> 'r3' -> 'r4' -> 'r5' -> 'r6' -> 'r7' -> …";
    assert_that!(result.map_err(|error| error.to_string())).is_equal_to(Err(expected.to_owned()));
    assert_that!(visited).is_less_than_or_equal_to(2 * count);
}
