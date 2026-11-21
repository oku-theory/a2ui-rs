//! The component graph checks a surface passes before it stores components (Decision 71).
//!
//! They run on the view `commit` would leave, in this order, each reporting its first offender
//! in arrival order: self-reference, dangling reference, cycle, missing root, unreachable
//! component, depth past `MAX_MESSAGE_DEPTH`. One topological sort (Kahn's algorithm) finds
//! cycles and every component's longest distance from `root`, following each edge once.

use std::collections::VecDeque;

use super::ValidationRules;
use super::children::edges;
use crate::catalog::CatalogSet;
use crate::error::{Error, ProcessFailure};
use crate::limits::MAX_MESSAGE_DEPTH;
use crate::quoted::Quoted;
use crate::surface::Merged;

/// The id every surface's tree starts from.
const ROOT: &str = "root";

/// How many ids of a cycle its error lists.
const MAX_CYCLE_IDS: usize = 8;

#[cfg(test)]
thread_local! {
    /// How many edges this thread's checks followed.
    pub(crate) static EDGE_VISITS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Checks the component graph of `view` under `rules`.
///
/// `rules` may accept dangling references (their edges are then ignored), orphans and a
/// missing root (which skips the reachability and depth checks); never self-references, cycles
/// or depth.
///
/// # Errors
///
/// The first failed check, carrying the surface id and no path.
pub(crate) fn check(
    view: &Merged<'_>,
    set: &CatalogSet,
    rules: ValidationRules,
) -> Result<(), Error> {
    graph(view, set, rules).map_err(|error| error.with_surface(view.surface().id()))
}

fn graph(view: &Merged<'_>, set: &CatalogSet, rules: ValidationRules) -> Result<(), Error> {
    let components = view.components();
    let id = |at: usize| components[at].0;
    let edges = edges(view, set)?;
    if let Some((at, _)) = edges.iter().find(|(at, child)| id(*at) == child.id) {
        return Err(ProcessFailure::SelfReference { id: id(*at).into() }.into());
    }
    if !rules.allows_dangling_references()
        && let Some((at, child)) = edges.iter().find(|(_, child)| !view.contains(child.id))
    {
        return Err(ProcessFailure::DanglingReference {
            parent: id(*at).into(),
            child: child.id.into(),
            field: Quoted::new(&child.field()),
        }
        .into());
    }
    let mut children = vec![Vec::new(); components.len()];
    let mut parents = vec![Vec::new(); components.len()];
    for (at, child) in &edges {
        if let Some(to) = view.position(child.id) {
            children[*at].push(to);
            parents[to].push(*at);
        }
    }
    let root = view.position(ROOT);
    let depths = sort(&children, &parents, root).map_err(|cycle| {
        let more = cycle.len() > MAX_CYCLE_IDS;
        let ids = cycle
            .iter()
            .take(MAX_CYCLE_IDS)
            .map(|&at| id(at).into())
            .collect();
        Error::from(ProcessFailure::CircularReference { ids, more })
    })?;
    if root.is_none() {
        if components.is_empty() || rules.allows_missing_root() {
            return Ok(());
        }
        return Err(ProcessFailure::MissingRoot.into());
    }
    if !rules.allows_orphan_components()
        && let Some(at) = depths.iter().position(Option::is_none)
    {
        return Err(ProcessFailure::Unreachable { id: id(at).into() }.into());
    }
    let too_deep = depths.iter().enumerate().find_map(|(at, depth)| {
        depth
            .filter(|&depth| depth > MAX_MESSAGE_DEPTH)
            .map(|d| (at, d))
    });
    if let Some((at, depth)) = too_deep {
        return Err(ProcessFailure::ComponentDepth {
            id: id(at).into(),
            depth,
        }
        .into());
    }
    Ok(())
}

/// Each node's longest distance from `root`, `None` when `root` does not reach it.
///
/// `children` and `parents` hold each node's edges both ways. `Err` holds a cycle, starting at
/// its earliest node.
fn sort(
    children: &[Vec<usize>],
    parents: &[Vec<usize>],
    root: Option<usize>,
) -> Result<Vec<Option<usize>>, Vec<usize>> {
    let mut indegree: Vec<usize> = parents.iter().map(Vec::len).collect();
    let mut depths = vec![None; children.len()];
    if let Some(root) = root {
        depths[root] = Some(0);
    }
    let mut ready: VecDeque<usize> = (0..children.len())
        .filter(|&at| indegree[at] == 0)
        .collect();
    while let Some(node) = ready.pop_front() {
        for &next in &children[node] {
            #[cfg(test)]
            EDGE_VISITS.with(|visits| visits.set(visits.get() + 1));
            if let Some(depth) = depths[node] {
                let below = depth + 1;
                depths[next] = Some(depths[next].map_or(below, |known: usize| known.max(below)));
            }
            indegree[next] -= 1;
            if indegree[next] == 0 {
                ready.push_back(next);
            }
        }
    }
    match indegree.iter().position(|&count| count > 0) {
        None => Ok(depths),
        Some(start) => Err(cycle(parents, &indegree, start)),
    }
}

/// A cycle among the nodes the sort left, which all have a parent among them.
///
/// It walks parents from `start` until a node repeats, so each node and edge is seen at most
/// once; the loop is then turned to run forward and start at its earliest node.
fn cycle(parents: &[Vec<usize>], indegree: &[usize], start: usize) -> Vec<usize> {
    let mut seen_at = vec![None; parents.len()];
    let mut path = Vec::new();
    let mut node = start;
    let from = loop {
        if let Some(at) = seen_at[node] {
            break at;
        }
        seen_at[node] = Some(path.len());
        path.push(node);
        let parent = parents[node].iter().copied().find(|&parent| {
            #[cfg(test)]
            EDGE_VISITS.with(|visits| visits.set(visits.get() + 1));
            indegree[parent] > 0
        });
        // Every node left has a parent left: a node without one would have been sorted.
        debug_assert!(
            parent.is_some(),
            "an unsorted component has no unsorted parent"
        );
        match parent {
            Some(parent) => node = parent,
            None => break 0,
        }
    };
    let mut cycle = path.split_off(from);
    cycle.reverse();
    if let Some(first) = cycle
        .iter()
        .enumerate()
        .min_by_key(|&(_, &node)| node)
        .map(|(at, _)| at)
    {
        cycle.rotate_left(first);
    }
    cycle
}

#[cfg(test)]
#[path = "topology_tests.rs"]
mod tests;
