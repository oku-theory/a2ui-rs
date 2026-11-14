//! Depth and size caps on JSON that agents send, and on the input an error quotes.

use std::collections::{BTreeMap, BTreeSet};
use std::ptr;

use serde_json::Value;

use crate::bundle::{Located, REF_KEY, RefPointer};

/// Deepest nesting of arrays and objects accepted in a catalog or a validated value.
pub(crate) const MAX_JSON_DEPTH: usize = 64;

/// Compiled size cap of one schema `pattern`, in bytes, for the regex and its lazy DFA.
pub(crate) const MAX_PATTERN_BYTES: usize = 128 * 1024;

/// Whether `value` nests arrays and objects more than `limit` levels deep.
///
/// A scalar has depth 0 and `{"a": 1}` depth 1. The walk is iterative, allocates nothing per
/// node, and checks a container's level before pushing its children, so a value deeper than the
/// stack allows is rejected at the first level past `limit`, not followed.
pub(crate) fn deeper_than(value: &Value, limit: usize) -> bool {
    let mut stack = vec![(value, 0)];
    while let Some((node, level)) = stack.pop() {
        match node {
            Value::Array(items) => {
                if level >= limit {
                    return true;
                }
                stack.extend(items.iter().map(|child| (child, level + 1)));
            }
            Value::Object(map) => {
                if level >= limit {
                    return true;
                }
                stack.extend(map.values().map(|child| (child, level + 1)));
            }
            _ => {}
        }
    }
    false
}

/// Largest catalog text [`crate::Catalog::parse`] reads, in bytes.
pub(crate) const MAX_CATALOG_BYTES: usize = 4 * 1024 * 1024;

/// Most JSON nodes, of any kind, one catalog document may hold.
pub(crate) const MAX_CATALOG_NODES: usize = 200_000;

/// Most components and functions, together, one catalog may define.
pub(crate) const MAX_CATALOG_ENTRIES: usize = 1024;

/// Most schema nodes one catalog entry may expand to once its `$ref`s are followed.
///
/// Compiling and checking an `allOf` fan-out under `unevaluatedProperties` is exponential in its
/// depth; the published basic catalog's largest entry is 4 493 nodes.
pub(crate) const MAX_ENTRY_EXPANSION: usize = 16_384;

/// Most schema nodes all entries of one catalog may expand to together.
pub(crate) const MAX_CATALOG_EXPANSION: usize = 262_144;

/// Deepest level of a catalog entry once its `$ref`s are followed, each hop one level.
///
/// It bounds the recursion of the schema build; the basic catalog reaches 23.
pub(crate) const MAX_SCHEMA_DEPTH: usize = 128;

/// Most `pattern` and `patternProperties` regexes all entries of one catalog may compile.
pub(crate) const MAX_CATALOG_PATTERNS: usize = 256;

/// Longest object key a catalog document may hold, in bytes.
pub(crate) const MAX_KEY_BYTES: usize = 256;

/// Longest `$ref` string a catalog document may hold, in bytes.
pub(crate) const MAX_REF_BYTES: usize = 4096;

/// Most input characters an error keeps of one quoted value.
///
/// Longer input keeps its first and last halves around an elision marker. Each character it
/// keeps takes at most nine bytes escaped (`\u{e007f}`), so a quoted value stays under 4.6 KiB.
pub(crate) const MAX_QUOTED_CHARS: usize = 512;

// Head and tail take half of the cap each.
const _: () = assert!(MAX_QUOTED_CHARS.is_multiple_of(2));

/// The first cap of load rule 1 a catalog document breaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Oversize {
    /// More JSON nodes than the limit given.
    Nodes,
    /// An object key longer than [`MAX_KEY_BYTES`].
    Key,
    /// A `$ref` string longer than [`MAX_REF_BYTES`].
    Ref,
}

/// The first of the node, key and `$ref` caps `value` breaks, with at most `max_nodes` nodes.
///
/// Each object, array and scalar is one node. The walk is iterative and counts a container's
/// children before queuing them, so it stops as soon as the count passes `max_nodes`; it
/// checks each key, and each `$ref` string, as it counts it. Every later walk of the document
/// thus reads strings of bounded length where it decodes or compares them.
pub(crate) fn oversize(value: &Value, max_nodes: usize) -> Option<Oversize> {
    let mut count = 1;
    let mut stack = vec![value];
    while let Some(node) = stack.pop() {
        match node {
            Value::Array(items) => {
                count += items.len();
                if count > max_nodes {
                    return Some(Oversize::Nodes);
                }
                stack.extend(items);
            }
            Value::Object(map) => {
                count += map.len();
                if count > max_nodes {
                    return Some(Oversize::Nodes);
                }
                for (key, child) in map {
                    if key.len() > MAX_KEY_BYTES {
                        return Some(Oversize::Key);
                    }
                    if let Value::String(reference) = child
                        && key == REF_KEY
                        && reference.len() > MAX_REF_BYTES
                    {
                        return Some(Oversize::Ref);
                    }
                }
                stack.extend(map.values());
            }
            _ => {}
        }
        if count > max_nodes {
            return Some(Oversize::Nodes);
        }
    }
    None
}

/// Bytes of string that weigh one more schema node in [`expand`]'s count.
const STRING_BYTES_PER_NODE: usize = 64;

/// The weight of `value` reached under the object key `key` (`""` for an array item or root).
///
/// A node weighs 1; every string, a scalar value or an object key, weighs one more node per
/// [`STRING_BYTES_PER_NODE`] bytes (`1 + len / 64` for a string value). The expansion caps
/// then bound the string bytes each compiled validator keeps, about 1 MiB per entry and
/// 16 MiB per catalog, and the bytes of patterns compiled again for each entry.
fn weight(key: &str, value: &Value) -> usize {
    let text = match value {
        Value::String(text) => text.len(),
        _ => 0,
    };
    1 + key.len() / STRING_BYTES_PER_NODE + text / STRING_BYTES_PER_NODE
}

/// The [`weight`] of `value` and everything under it, counting stopped once past `limit`.
fn weight_up_to(value: &Value, limit: usize) -> usize {
    let mut count = weight("", value);
    let mut stack = vec![value];
    while let Some(node) = stack.pop() {
        if count > limit {
            break;
        }
        match node {
            Value::Array(items) => {
                count += items.iter().map(|item| weight("", item)).sum::<usize>();
                stack.extend(items);
            }
            Value::Object(map) => {
                count += map
                    .iter()
                    .map(|(key, child)| weight(key, child))
                    .sum::<usize>();
                stack.extend(map.values());
            }
            _ => {}
        }
    }
    count
}

/// How one catalog entry expands once its local `$ref`s are followed.
///
/// Reached names borrow their keys from `root`, the document `$ref`s resolve in.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Expansion<'r> {
    /// The entry fits the budget and [`MAX_SCHEMA_DEPTH`].
    Within {
        /// Nodes counted: a `$ref` target each time it is followed, plus each entry reached
        /// once, whole, since the pruned document compiled for the entry copies it whole.
        size: usize,
        /// `pattern` strings and `patternProperties` keys counted.
        patterns: usize,
        /// The `(section, name)` of every entry a followed `$ref` lands in.
        ///
        /// Each name is the entry's own key in `root`, not the decoded `$ref` token.
        reached: BTreeSet<(&'static str, &'r str)>,
    },
    /// The count passed the budget.
    TooLarge,
    /// A node lies more than [`MAX_SCHEMA_DEPTH`] levels down, `$ref` hops included.
    TooDeep,
    /// This `$ref` leads back into a node it is being expanded from.
    Cycle(String),
    /// This `$ref` names no node inside a `$defs`, `components` or `functions` entry.
    Unresolved(String),
}

enum Step<'a> {
    Visit(&'a Value, usize),
    Enter(&'a str, &'a Value, usize),
    Leave,
}

/// A walk of nodes living `'a`, resolving `$ref`s in a `root` living `'r`.
struct Walk<'r, 'a> {
    root: &'r Value,
    budget: usize,
    size: usize,
    patterns: usize,
    reached: BTreeSet<(&'static str, &'r str)>,
    /// The target each `$ref` string node resolved to, keyed by the node's address, so each
    /// `$ref` site is decoded and looked up once per walk however often it is followed.
    resolved: BTreeMap<*const String, &'a Value>,
    chain: Vec<&'a Value>,
    stack: Vec<Step<'a>>,
}

/// Expands `node` with its local `$ref`s resolved in `root`, weighing at most `budget` nodes.
///
/// Every object, array and scalar is one node, counted when queued, and a string (value or
/// key) weighs one more node per 64 bytes; a followed `$ref` counts its string, then its
/// target's nodes again each time. The first `$ref` landing in an entry
/// also counts that entry's nodes once, whole: the document compiled for `node` copies every
/// entry it reaches, not only the node a pointer names. A node lies one level below its
/// parent, and a `$ref` target one level below the object holding the `$ref`. The chain holds
/// `node` and the target of every `$ref` whose subtree is being walked: a `$ref` to a node on
/// it is a cycle, so every walk ends. The walk is iterative and stops at the first node past
/// `budget` or [`MAX_SCHEMA_DEPTH`], so it never holds more than `budget` queued nodes.
pub(crate) fn expand<'r>(root: &'r Value, node: &Value, budget: usize) -> Expansion<'r> {
    let mut walk = Walk {
        root,
        budget,
        size: 0,
        patterns: 0,
        reached: BTreeSet::new(),
        resolved: BTreeMap::new(),
        chain: vec![node],
        stack: Vec::new(),
    };
    match walk.run(node) {
        Ok(()) => Expansion::Within {
            size: walk.size,
            patterns: walk.patterns,
            reached: walk.reached,
        },
        Err(stop) => stop,
    }
}

// Targets resolved in `root` join the walked nodes on the stack and chain, hence `'r: 'a`.
impl<'r: 'a, 'a> Walk<'r, 'a> {
    /// Walks from `node`; `Err` carries the reason the walk stopped.
    fn run(&mut self, node: &'a Value) -> Result<(), Expansion<'r>> {
        self.push("", node, 0)?;
        while let Some(step) = self.stack.pop() {
            match step {
                Step::Visit(value, depth) => self.visit(value, depth)?,
                Step::Enter(reference, target, depth) => self.enter(reference, target, depth)?,
                Step::Leave => {
                    self.chain.pop();
                }
            }
        }
        Ok(())
    }

    fn count(&mut self, depth: usize, weight: usize) -> Result<(), Expansion<'r>> {
        self.size += weight;
        if self.size > self.budget {
            return Err(Expansion::TooLarge);
        }
        if depth > MAX_SCHEMA_DEPTH {
            return Err(Expansion::TooDeep);
        }
        Ok(())
    }

    /// Counts `node`, reached under the object key `key`, and queues it for visiting.
    fn push(&mut self, key: &str, node: &'a Value, depth: usize) -> Result<(), Expansion<'r>> {
        self.count(depth, weight(key, node))?;
        if matches!(node, Value::Object(_) | Value::Array(_)) {
            self.stack.push(Step::Visit(node, depth));
        }
        Ok(())
    }

    fn visit(&mut self, node: &'a Value, depth: usize) -> Result<(), Expansion<'r>> {
        match node {
            Value::Object(map) => {
                for (key, child) in map {
                    self.count_patterns(key, child);
                    self.push(key, child, depth + 1)?;
                    if let Value::String(reference) = child
                        && key == REF_KEY
                    {
                        self.follow(reference, depth + 1)?;
                    }
                }
            }
            Value::Array(items) => {
                for item in items {
                    self.push("", item, depth + 1)?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Counts patterns wherever the keys appear, so a property of that name counts too.
    fn count_patterns(&mut self, key: &str, child: &Value) {
        match (key, child) {
            ("pattern", Value::String(_)) => self.patterns += 1,
            ("patternProperties", Value::Object(patterns)) => self.patterns += patterns.len(),
            _ => {}
        }
    }

    /// Counts the target of `reference` at `depth` and queues it for entering.
    fn follow(&mut self, reference: &'a String, depth: usize) -> Result<(), Expansion<'r>> {
        let target = match self.resolved.get(&ptr::from_ref(reference)) {
            Some(target) => *target,
            None => self.resolve(reference)?,
        };
        self.count(depth, weight("", target))?;
        self.stack.push(Step::Enter(reference, target, depth));
        Ok(())
    }

    /// Resolves the `$ref` site `reference` once, charging its entry the first time it is reached.
    fn resolve(&mut self, reference: &'a String) -> Result<&'r Value, Expansion<'r>> {
        let unresolved = || Expansion::Unresolved(reference.clone());
        // One descent finds the entry charged and the target inside it; the name is the
        // catalog's own key, so the walk copies no name.
        let Located {
            section,
            name,
            entry,
            target,
        } = RefPointer::of(reference)
            .and_then(|pointer| pointer.locate(self.root))
            .ok_or_else(unresolved)?;
        if self.reached.insert((section, name)) {
            self.size += weight_up_to(entry, self.budget.saturating_sub(self.size));
            if self.size > self.budget {
                return Err(Expansion::TooLarge);
            }
        }
        self.resolved.insert(ptr::from_ref(reference), target);
        Ok(target)
    }

    /// Puts `target` on the chain while its subtree is walked.
    fn enter(
        &mut self,
        reference: &'a str,
        target: &'a Value,
        depth: usize,
    ) -> Result<(), Expansion<'r>> {
        if self.chain.iter().any(|on_chain| ptr::eq(*on_chain, target)) {
            return Err(Expansion::Cycle(reference.to_owned()));
        }
        self.chain.push(target);
        self.stack.push(Step::Leave);
        if matches!(target, Value::Object(_) | Value::Array(_)) {
            self.stack.push(Step::Visit(target, depth));
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "limits_tests.rs"]
mod tests;
