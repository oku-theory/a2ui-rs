//! The protocol's static safety check: three structural rules, run in the protocol's order.
//!
//! Each rule walks the whole tree before the next starts, so a pattern breaking several is
//! refused under the first. The walks recurse once a node; a parsed tree is at most a few
//! levels a code point of a pattern capped at 256, so their depth is bounded.

use super::Rule;
use super::ast::Node;
use super::charset::CharSet;
use crate::error::Error;

const NESTED_QUANTIFIER: &str = "A quantifier whose upper bound is above 1 applies to a \
                                 subexpression that contains an unbounded quantifier";
const OVERLAPPING_ALTERNATION: &str = "An unbounded quantifier applies to an alternation where \
                                       two branches can start with the same character";
const ADJACENT_OVERLAP: &str = "Two unbounded-quantified atoms with overlapping character sets \
                                are adjacent in one concatenation, where only zero-width \
                                assertions or items that can match the empty string lie between \
                                them";

/// Refuses a tree that breaks a safety rule, with the protocol's wording.
pub(crate) fn check(node: &Node) -> Result<(), Error> {
    if any_node(node, &nested_quantifier) {
        return Err(Rule::NestedQuantifier.refuse(NESTED_QUANTIFIER));
    }
    if any_node(node, &overlapping_alternation) {
        return Err(Rule::OverlappingAlternation.refuse(OVERLAPPING_ALTERNATION));
    }
    if any_node(node, &adjacent_overlap) {
        return Err(Rule::AdjacentOverlap.refuse(ADJACENT_OVERLAP));
    }
    Ok(())
}

/// Whether `test` holds for `node` or any node under it, lookaheads included.
fn any_node(node: &Node, test: &dyn Fn(&Node) -> bool) -> bool {
    test(node) || children(node).iter().any(|child| any_node(child, test))
}

/// The nodes directly under `node`.
fn children(node: &Node) -> &[Node] {
    match node {
        Node::Group(inner) | Node::Look { node: inner, .. } | Node::Repeat { node: inner, .. } => {
            std::slice::from_ref(inner)
        }
        Node::Concat(items) | Node::Alt(items) => items,
        Node::Empty
        | Node::Char(_)
        | Node::Any
        | Node::Class { .. }
        | Node::Start
        | Node::End
        | Node::WordBoundary(_) => &[],
    }
}

/// Rule 2: an upper bound above 1 over a subexpression holding an unbounded quantifier.
fn nested_quantifier(node: &Node) -> bool {
    match node {
        Node::Repeat {
            node: inner, max, ..
        } => max.is_none_or(|max| max > 1) && any_node(inner, &|n| unbounded(n).is_some()),
        _ => false,
    }
}

/// Rule 3: an unbounded quantifier over an alternation two of whose branches share a first
/// character, groups unwrapped.
fn overlapping_alternation(node: &Node) -> bool {
    let Some(Node::Alt(branches)) = unbounded(node).map(unwrap_groups) else {
        return false;
    };
    let firsts: Vec<CharSet> = branches.iter().map(first_chars).collect();
    firsts
        .iter()
        .enumerate()
        .any(|(i, set)| firsts[i + 1..].iter().any(|other| set.overlaps(other)))
}

/// Rule 4: two overlapping unbounded atoms of one concatenation with only nullable items
/// (zero-width assertions included) between them.
fn adjacent_overlap(node: &Node) -> bool {
    let Node::Concat(items) = node else {
        return false;
    };
    let atoms: Vec<&Node> = items.iter().map(atom_of).collect();
    atoms.iter().enumerate().any(|(i, atom)| {
        let Some(set) = unbounded(atom).map(match_chars) else {
            return false;
        };
        for later in &atoms[i + 1..] {
            if unbounded(later).is_some_and(|inner| set.overlaps(&match_chars(inner))) {
                return true;
            }
            if !nullable(later) {
                return false;
            }
        }
        false
    })
}

/// The operand of an unbounded quantifier (`*`, `+`, `{n,}`), or `None` for any other node.
fn unbounded(node: &Node) -> Option<&Node> {
    match node {
        Node::Repeat {
            node: inner,
            max: None,
            ..
        } => Some(inner),
        _ => None,
    }
}

/// `node` with its enclosing groups removed.
fn unwrap_groups(mut node: &Node) -> &Node {
    while let Node::Group(inner) = node {
        node = inner;
    }
    node
}

/// The quantified atom a group whose only content is one stands for; else `node`.
fn atom_of(node: &Node) -> &Node {
    match unwrap_groups(node) {
        repeat @ Node::Repeat { .. } => repeat,
        _ => node,
    }
}

/// Whether `node` can match the empty string; assertions are zero-width, so they can.
fn nullable(node: &Node) -> bool {
    match node {
        Node::Empty | Node::Start | Node::End | Node::WordBoundary(_) | Node::Look { .. } => true,
        Node::Char(_) | Node::Any | Node::Class { .. } => false,
        Node::Group(inner) => nullable(inner),
        Node::Concat(items) => items.iter().all(nullable),
        Node::Alt(branches) => branches.iter().any(nullable),
        Node::Repeat {
            node: inner, min, ..
        } => *min == 0 || nullable(inner),
    }
}

/// The characters a match of `node` can start with.
fn first_chars(node: &Node) -> CharSet {
    match node {
        Node::Concat(items) => {
            let mut set = CharSet::empty();
            for item in items {
                set = set.union(&first_chars(item));
                if !nullable(item) {
                    break;
                }
            }
            set
        }
        Node::Alt(branches) => union_of(branches, first_chars),
        Node::Group(inner) => first_chars(inner),
        Node::Repeat { max: Some(0), .. } => CharSet::empty(),
        Node::Repeat { node: inner, .. } => first_chars(inner),
        leaf => match_chars(leaf),
    }
}

/// Every character a match of `node` can consume; zero-width nodes consume none.
fn match_chars(node: &Node) -> CharSet {
    match node {
        Node::Char(c) => CharSet::single(*c),
        Node::Any => CharSet::dot(),
        Node::Class {
            negated: false,
            items,
        } => items.clone(),
        Node::Class {
            negated: true,
            items,
        } => items.negate(),
        Node::Group(inner) => match_chars(inner),
        Node::Repeat { max: Some(0), .. } => CharSet::empty(),
        Node::Repeat { node: inner, .. } => match_chars(inner),
        Node::Concat(items) | Node::Alt(items) => union_of(items, match_chars),
        Node::Empty | Node::Start | Node::End | Node::WordBoundary(_) | Node::Look { .. } => {
            CharSet::empty()
        }
    }
}

/// The union of `set_of` over `nodes`.
fn union_of(nodes: &[Node], set_of: fn(&Node) -> CharSet) -> CharSet {
    nodes
        .iter()
        .fold(CharSet::empty(), |set, node| set.union(&set_of(node)))
}

#[cfg(test)]
#[path = "safety_tests.rs"]
mod tests;
