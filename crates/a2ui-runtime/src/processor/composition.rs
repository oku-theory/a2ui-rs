//! The `allowedParents` and `allowedChildren` checks a surface passes (Decision 73).
//!
//! They run after the topology checks, on the view `commit` would leave, over the edges `root`
//! reaches that have an end in the message or whose parent `root` did not reach before it (a
//! stored orphan reattached); `root` itself has the parent `Surface`. Edges come
//! in arrival order of the child, then of the parent; per edge, the child's `allowedParents`
//! is checked before the parent's `allowedChildren`. The first refusal is the error.

use std::collections::VecDeque;

use serde_json::Value;

use super::children::{ChildRef, edges};
use super::resolve::component_catalog;
use super::topology::ROOT;
use crate::catalog::{CatalogSet, ComponentDef};
use crate::detail::{Detail, DetailCode};
use crate::error::{Allowed, Error, ProcessFailure};
use crate::surface::{Merged, Staged};

/// The name and type of the parent `root` has.
const SURFACE: &str = "Surface";

/// A component's type name and its definition, `None` when its catalog lacks the type.
type Kind<'a> = (&'a str, Option<&'a ComponentDef>);

// MVP: t-1689. Composition runs only when a components message is processed; a later mutation
// that changes a reachable component's type without touching its edges must re-run it.
/// Checks the composition constraints of the edges `view` gains from `staged`.
///
/// A component whose catalog lacks its type (accepted by `allow_unknown_elements`) is checked
/// neither as a parent nor as a child, and a dangling reference is not checked.
///
/// # Errors
///
/// The first `UnallowedParent` or `UnallowedChild`, at its path in the message, with one
/// detail there; both carry the surface id. An edge with neither end in the message (a
/// reattached orphan's) has no path in it: the error then has no path and no detail.
pub(crate) fn check(view: &Merged<'_>, staged: &Staged, set: &CatalogSet) -> Result<(), Error> {
    compose(view, staged, set).map_err(|error| error.with_surface(view.surface().id()))
}

/// One edge to check, and where each refusal of it points in the message.
struct Link {
    /// The parent's position, `None` for `Surface`.
    parent: Option<usize>,
    child: usize,
    /// Where an `UnallowedParent` points: the child, else the parent's slot.
    parent_fault: Option<String>,
    /// Where an `UnallowedChild` points: the parent's slot, else the child.
    child_fault: Option<String>,
}

fn compose(view: &Merged<'_>, staged: &Staged, set: &CatalogSet) -> Result<(), Error> {
    let Some(root) = view.position(ROOT) else {
        return Ok(());
    };
    let components = view.components();
    let mut in_message = vec![None; components.len()];
    for (index, (id, _)) in staged.components.iter().enumerate() {
        if let Some(at) = view.position(id) {
            in_message[at] = Some(index);
        }
    }
    let edges = edges(view, set)?;
    let children = adjacency(view, &edges);
    let reached = reach(&children, root);
    let before = reached_before(view, set)?;
    // Stored components keep their position in `view`; one past them arrives in this message,
    // so `root` never reached it before.
    let was_reached = |at: usize| before.get(at).copied().unwrap_or(false);
    let at = |index: Option<usize>| index.map(|index| format!("/components/{index}"));
    let mut links: Vec<Link> = at(in_message[root])
        .map(|own| Link {
            parent: None,
            child: root,
            parent_fault: Some(own.clone()),
            child_fault: Some(own),
        })
        .into_iter()
        .collect();
    for (parent, list) in children.iter().enumerate() {
        if !reached[parent] {
            continue;
        }
        let reattached = !was_reached(parent);
        links.extend(list.iter().filter_map(|&(child, slot)| {
            let own = at(in_message[child]);
            let slot = at(in_message[parent]).map(|path| path + &slot.pointer());
            (reattached || own.is_some() || slot.is_some()).then(|| Link {
                parent: Some(parent),
                child,
                parent_fault: own.clone().or_else(|| slot.clone()),
                child_fault: slot.or(own),
            })
        }));
    }
    links.sort_by_key(|link| (link.child, link.parent));
    let surface = view.surface();
    let kind = |at: usize| -> Result<Option<Kind<'_>>, Error> {
        let (id, component) = components[at];
        // The components stage refused any component whose `component` is not a string, and
        // stored ones passed it too: `None` only skips what that stage already refused.
        let Some(name) = component.get("component").and_then(Value::as_str) else {
            return Ok(None);
        };
        let catalog = component_catalog(component, id, surface.id(), surface.catalog_id(), set)?;
        Ok(Some((name, catalog.component(name))))
    };
    for link in &links {
        let Some((child_type, Some(child_def))) = kind(link.child)? else {
            continue;
        };
        let child = components[link.child].0;
        let (parent, parent_type, parent_def) = match link.parent {
            None => (SURFACE, SURFACE, None),
            Some(at) => match kind(at)? {
                Some((parent_type, Some(def))) => (components[at].0, parent_type, Some(def)),
                _ => continue,
            },
        };
        if let Some(allowed) = child_def.allowed_parents()
            && !allowed.iter().any(|name| name == parent_type)
        {
            let failure = ProcessFailure::UnallowedParent {
                child: child.into(),
                child_type: child_type.into(),
                parent: parent.into(),
                parent_type: parent_type.into(),
                allowed: Allowed::new(allowed),
            };
            return Err(refuse(
                failure,
                link.parent_fault.as_deref(),
                DetailCode::UnallowedParent,
            ));
        }
        if let Some(allowed) = parent_def.and_then(ComponentDef::allowed_children)
            && !allowed.iter().any(|name| name == child_type)
        {
            let failure = ProcessFailure::UnallowedChild {
                parent: parent.into(),
                parent_type: parent_type.into(),
                child: child.into(),
                child_type: child_type.into(),
                allowed: Allowed::new(allowed),
            };
            return Err(refuse(
                failure,
                link.child_fault.as_deref(),
                DetailCode::UnallowedChild,
            ));
        }
    }
    Ok(())
}

/// Which components `root` reaches over `children`, each visited once.
fn reach(children: &[Vec<(usize, &ChildRef<'_>)>], root: usize) -> Vec<bool> {
    let mut reached = vec![false; children.len()];
    reached[root] = true;
    let mut pending = VecDeque::from([root]);
    while let Some(node) = pending.pop_front() {
        for &(child, _) in &children[node] {
            if !reached[child] {
                reached[child] = true;
                pending.push_back(child);
            }
        }
    }
    reached
}

/// Which stored components `root` reached before this message, by their (unchanged) position.
///
/// Empty when the surface held no `root`.
fn reached_before(view: &Merged<'_>, set: &CatalogSet) -> Result<Vec<bool>, Error> {
    let nothing = Staged {
        components: Vec::new(),
    };
    let stored = view.surface().merged(&nothing);
    let Some(root) = stored.position(ROOT) else {
        return Ok(Vec::new());
    };
    let edges = edges(&stored, set)?;
    Ok(reach(&adjacency(&stored, &edges), root))
}

/// Each component's children in `view` with their slot; dangling references left out.
fn adjacency<'e, 'a>(
    view: &Merged<'_>,
    edges: &'e [(usize, ChildRef<'a>)],
) -> Vec<Vec<(usize, &'e ChildRef<'a>)>> {
    let mut children = vec![Vec::new(); view.components().len()];
    for (parent, child) in edges {
        if let Some(to) = view.position(child.id) {
            children[*parent].push((to, child));
        }
    }
    children
}

/// `failure` at `path`, with one detail there; with neither when `path` is `None`.
fn refuse(failure: ProcessFailure, path: Option<&str>, code: DetailCode) -> Error {
    let error = Error::from(failure);
    match path {
        Some(path) => error
            .with_path(path.to_owned())
            .with_detail(Detail::new(path.to_owned(), code)),
        None => error,
    }
}

#[cfg(test)]
#[path = "composition_tests.rs"]
mod tests;
