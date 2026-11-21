//! The child references components hold, read through their catalog's child slots.

use std::collections::HashSet;
use std::fmt::Write as _;

use serde_json::{Map, Value};

use super::resolve::component_catalog;
use crate::catalog::{CatalogSet, ChildSlot, SlotKind, SlotStep};
use crate::error::Error;
use crate::surface::Merged;

/// The key of a list template naming its child.
const TEMPLATE_CHILD: &str = "componentId";

/// One child id a component holds, and where it holds it.
#[derive(Debug)]
pub(crate) struct ChildRef<'a> {
    /// The child component's id.
    pub(crate) id: &'a str,
    position: Vec<Token<'a>>,
}

/// One step from a component object to a child id.
#[derive(Debug, Clone, Copy)]
enum Token<'a> {
    Key(&'a str),
    Index(usize),
}

impl ChildRef<'_> {
    /// Where the component holds the child, such as `children[0]` or `tabs[1].child`.
    pub(crate) fn field(&self) -> String {
        let mut field = String::new();
        for (at, token) in self.position.iter().enumerate() {
            match token {
                Token::Key(key) if at == 0 => field.push_str(key),
                Token::Key(key) => {
                    field.push('.');
                    field.push_str(key);
                }
                // Writing to a `String` cannot fail.
                Token::Index(index) => _ = write!(field, "[{index}]"),
            }
        }
        field
    }
}

/// Every edge of `view`: the parent's arrival position and the child it holds.
///
/// Edges come in parent arrival order, then slot order; a parent holding a child twice gives
/// one edge. A component whose type its catalog lacks holds no children.
///
/// # Errors
///
/// The error of a component whose catalog does not resolve.
pub(crate) fn edges<'a>(
    view: &Merged<'a>,
    set: &'a CatalogSet,
) -> Result<Vec<(usize, ChildRef<'a>)>, Error> {
    let surface = view.surface();
    let mut edges = Vec::new();
    for (at, &(id, component)) in view.components().iter().enumerate() {
        let catalog = component_catalog(component, id, surface.id(), surface.catalog_id(), set)?;
        let def = component
            .get("component")
            .and_then(Value::as_str)
            .and_then(|kind| catalog.component(kind));
        if let Some(def) = def {
            let children = slot_children(component, def.child_slots());
            edges.extend(children.into_iter().map(|child| (at, child)));
        }
    }
    Ok(edges)
}

/// The children `component` holds in `slots`, each once, in slot order.
///
/// A slot value of the wrong shape holds none, and so does any non-string id.
pub(crate) fn slot_children<'a>(
    component: &'a Map<String, Value>,
    slots: &'a [ChildSlot],
) -> Vec<ChildRef<'a>> {
    let mut seen = HashSet::new();
    let mut children = Vec::new();
    for slot in slots {
        for (value, position) in reach(component, &slot.steps) {
            let mut found = |id: &'a str, position: Vec<Token<'a>>| {
                if seen.insert(id) {
                    children.push(ChildRef { id, position });
                }
            };
            match (slot.kind, value) {
                (SlotKind::Single, Value::String(id)) => found(id, position),
                (SlotKind::List, Value::Array(items)) => {
                    for (index, item) in items.iter().enumerate() {
                        if let Value::String(id) = item {
                            found(id, with(&position, Token::Index(index)));
                        }
                    }
                }
                (SlotKind::List, Value::Object(template)) => {
                    if let Some(Value::String(id)) = template.get(TEMPLATE_CHILD) {
                        found(id, with(&position, Token::Key(TEMPLATE_CHILD)));
                    }
                }
                _ => {}
            }
        }
    }
    children
}

/// The values `steps` lead to from `component`, each with its position.
fn reach<'a>(
    component: &'a Map<String, Value>,
    steps: &'a [SlotStep],
) -> Vec<(&'a Value, Vec<Token<'a>>)> {
    let Some((SlotStep::Key(first), rest)) = steps.split_first() else {
        // A slot always starts at a property: the component itself is an object.
        return Vec::new();
    };
    let mut frontier: Vec<(&Value, Vec<Token<'a>>)> = component
        .get(first)
        .map(|value| (value, vec![Token::Key(first)]))
        .into_iter()
        .collect();
    for step in rest {
        let mut next = Vec::new();
        for (value, position) in frontier {
            match (step, value) {
                (SlotStep::Key(key), Value::Object(map)) => {
                    if let Some(inner) = map.get(key) {
                        next.push((inner, with(&position, Token::Key(key))));
                    }
                }
                (SlotStep::Items, Value::Array(items)) => {
                    next.extend(
                        items
                            .iter()
                            .enumerate()
                            .map(|(index, item)| (item, with(&position, Token::Index(index)))),
                    );
                }
                (SlotStep::Index(index), Value::Array(items)) => {
                    if let Some(item) = items.get(*index) {
                        next.push((item, with(&position, Token::Index(*index))));
                    }
                }
                _ => {}
            }
        }
        frontier = next;
    }
    frontier
}

fn with<'a>(position: &[Token<'a>], token: Token<'a>) -> Vec<Token<'a>> {
    let mut position = position.to_vec();
    position.push(token);
    position
}

#[cfg(test)]
#[path = "children_tests.rs"]
mod tests;
