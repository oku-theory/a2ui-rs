//! Child slots: where a component schema holds the ids of its children (Decision 70).
//!
//! A slot is a `$ref` to the common type `ComponentId` or `Child` (one id) or `ChildList`
//! (a list of ids, or a template whose `componentId` is the child). The walk follows
//! `properties`, `items`, `prefixItems`, `allOf`, `anyOf`, `oneOf` and `$ref`s, iteratively and
//! within `MAX_ENTRY_EXPANSION` and `MAX_SCHEMA_DEPTH` `$ref` hops; no other string is ever
//! read as a child id.

use std::collections::{BTreeMap, HashMap, HashSet};

use serde_json::{Map, Value};

use super::compile::{Bundle, catalog_too_large, entry_too_deep, entry_too_large};
use super::entries::ComponentAttrs;
use crate::bundle::{REF_KEY, RefPointer};
use crate::error::{Error, MalformedCatalogSnafu};
use crate::limits::{MAX_CATALOG_EXPANSION, MAX_ENTRY_EXPANSION, MAX_SCHEMA_DEPTH};
use crate::quoted::{Quoted, reason};
use crate::schema::common_type_defs;

/// One place a component holds child ids, as a path from the component object.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct ChildSlot {
    /// The way from the component object to the slot's value.
    pub(crate) steps: Vec<SlotStep>,
    /// Whether the value is one id or a list of them.
    pub(crate) kind: SlotKind,
}

/// One step of a slot's path.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum SlotStep {
    /// The object property of this name.
    Key(String),
    /// Every item of an array.
    Items,
    /// The array item at this index (`prefixItems`).
    Index(usize),
}

/// What a slot's value holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum SlotKind {
    /// One component id.
    Single,
    /// An array of ids, or a template object whose `componentId` is the child.
    List,
}

/// Keywords whose schemas a slot may sit under, one schema each.
const SCHEMA_LISTS: [&str; 3] = ["allOf", "anyOf", "oneOf"];

/// Keywords holding one schema where a slot is refused.
const UNSUPPORTED_ONE: [&str; 9] = [
    "not",
    "if",
    "then",
    "else",
    "additionalProperties",
    "unevaluatedProperties",
    "unevaluatedItems",
    "contains",
    "propertyNames",
];

/// Keywords holding a map of schemas where a slot is refused.
const UNSUPPORTED_MAPS: [&str; 2] = ["patternProperties", "dependentSchemas"];

/// Finds the child slots of every kept component and records them on it.
///
/// # Errors
///
/// A `CatalogError` for a slot under an unsupported keyword, a slot inside a recursive
/// definition, a chain of more than `MAX_SCHEMA_DEPTH` `$ref`s (load rule 11's error), and a
/// walk past `MAX_ENTRY_EXPANSION` (one entry) or `MAX_CATALOG_EXPANSION` (all entries) steps.
pub(super) fn attach(
    bundle: &Bundle,
    components: &mut BTreeMap<String, ComponentAttrs>,
) -> Result<(), Error> {
    let root = bundle.document();
    let mut walk = Walk {
        root: &root,
        common: common_type_defs()?,
        memo: HashMap::new(),
        total: 0,
    };
    for attrs in components.values_mut() {
        let slots = walk.component(attrs.name(), attrs.schema())?;
        attrs.set_child_slots(slots);
    }
    Ok(())
}

/// The state shared by the walks of one catalog's components.
struct Walk<'r> {
    /// The bundled document local `$ref`s resolve in.
    root: &'r Value,
    common: &'static Map<String, Value>,
    /// The slots found under each `$ref` target walked so far, by address.
    memo: HashMap<*const Value, Vec<ChildSlot>>,
    /// The steps spent on all components so far.
    total: usize,
}

/// A slot found while scanning one schema, before its `$ref` site is known.
struct Found {
    slot: ChildSlot,
    /// The first unsupported keyword above it, if any.
    under: Option<&'static str>,
}

/// A `$ref` met while scanning one schema, to a schema walked in turn.
struct Site<'r> {
    steps: Vec<SlotStep>,
    under: Option<&'static str>,
    target: &'r Value,
    reference: &'r str,
}

/// One schema being walked: its scan, and the slots of the `$ref`s followed so far.
struct Frame<'r> {
    target: &'r Value,
    /// The path from the parent frame's schema to the `$ref` that led here.
    steps: Vec<SlotStep>,
    /// The first unsupported keyword above that `$ref`, if any.
    under: Option<&'static str>,
    sites: std::vec::IntoIter<Site<'r>>,
    slots: Vec<ChildSlot>,
    /// The `$ref` closing a cycle through this frame, if any.
    recursive: Option<&'r str>,
}

/// The budget of one component's walk.
struct Budget<'n> {
    name: &'n str,
    spent: usize,
}

impl<'r> Walk<'r> {
    /// The slots of the component `name`, deduplicated, in schema order.
    fn component<'s>(&mut self, name: &str, schema: &'s Value) -> Result<Vec<ChildSlot>, Error>
    where
        'r: 's,
    {
        let mut budget = Budget { name, spent: 0 };
        let mut current = self.frame(schema, Vec::new(), None, &mut budget)?;
        let mut parents: Vec<Frame<'s>> = Vec::new();
        let mut on_stack = HashMap::from([(address(schema), 0)]);
        loop {
            if let Some(site) = current.sites.next() {
                let key = address(site.target);
                if let Some(found) = self.memo.get(&key) {
                    let slots =
                        compose(&site.steps, site.under, found, &mut budget, &mut self.total)?;
                    current.slots.extend(slots);
                } else if let Some(&at) = on_stack.get(&key) {
                    for frame in parents.iter_mut().skip(at).chain([&mut current]) {
                        frame.recursive.get_or_insert(site.reference);
                    }
                } else {
                    // Each `$ref` hop is a level for load rule 11, so a longer chain breaks it.
                    if parents.len() + 1 > MAX_SCHEMA_DEPTH {
                        return Err(entry_too_deep(name));
                    }
                    on_stack.insert(key, parents.len() + 1);
                    let next = self.frame(site.target, site.steps, site.under, &mut budget)?;
                    parents.push(std::mem::replace(&mut current, next));
                }
                continue;
            }
            on_stack.remove(&address(current.target));
            if let Some(reference) = current.recursive
                && !current.slots.is_empty()
            {
                return Err(malformed(reason![
                    "Component '",
                    Quoted::new(name),
                    "' has a recursive child slot through '",
                    Quoted::new(&entry_name(reference)),
                    "'",
                ]));
            }
            let Some(parent) = parents.pop() else {
                return Ok(finish(current.slots));
            };
            let done = std::mem::replace(&mut current, parent);
            let slots = compose(
                &done.steps,
                done.under,
                &done.slots,
                &mut budget,
                &mut self.total,
            )?;
            current.slots.extend(slots);
            self.memo.insert(address(done.target), done.slots);
        }
    }

    /// Scans `target` for direct slots and the `$ref`s to follow.
    fn frame<'s>(
        &mut self,
        target: &'s Value,
        steps: Vec<SlotStep>,
        under: Option<&'static str>,
        budget: &mut Budget<'_>,
    ) -> Result<Frame<'s>, Error>
    where
        'r: 's,
    {
        let mut slots = Vec::new();
        let mut sites = Vec::new();
        let mut pending = vec![(target, Vec::new(), None)];
        while let Some((node, steps, under)) = pending.pop() {
            spend(budget, &mut self.total, 1)?;
            let Value::Object(map) = node else {
                continue;
            };
            if let Some(Value::String(reference)) = map.get(REF_KEY) {
                match self.classify(reference) {
                    Target::Slot(kind) => slots.push(Found {
                        slot: slot(steps.clone(), kind),
                        under,
                    }),
                    Target::Schema(target) => sites.push(Site {
                        steps: steps.clone(),
                        under,
                        target,
                        reference,
                    }),
                    Target::None => {}
                }
            }
            let mut children = Vec::new();
            for (key, value) in map {
                subschemas(key, value, &steps, under, &mut children);
            }
            pending.extend(children.into_iter().rev());
        }
        let slots = slots
            .into_iter()
            .map(|found| match found.under {
                Some(keyword) => Err(unsupported(budget.name, keyword)),
                None => Ok(found.slot),
            })
            .collect::<Result<_, _>>()?;
        Ok(Frame {
            target,
            steps,
            under,
            sites: sites.into_iter(),
            slots,
            recursive: None,
        })
    }

    /// What the `$ref` `reference` leads to.
    fn classify(&self, reference: &str) -> Target<'r> {
        let Some(pointer) = RefPointer::of(reference) else {
            return Target::None;
        };
        // Load refuses own `$defs` shadowing a common type, so the name is the common one.
        if let [section, name] = pointer.tokens()
            && section == "$defs"
            && self.common.contains_key(name.as_ref())
        {
            return match name.as_ref() {
                "ComponentId" | "Child" => Target::Slot(SlotKind::Single),
                "ChildList" => Target::Slot(SlotKind::List),
                _ => Target::None,
            };
        }
        // An unresolved `$ref` is load rule 11's to report.
        pointer
            .resolve(self.root)
            .map_or(Target::None, Target::Schema)
    }
}

/// Where a `$ref` leads.
enum Target<'r> {
    /// A common type holding child ids.
    Slot(SlotKind),
    /// A schema of the catalog, walked in turn.
    Schema(&'r Value),
    /// A common type holding none, or nothing load rule 11 accepts.
    None,
}

/// Pushes the subschemas of `value`, under the keyword `key`, onto `out`.
fn subschemas<'r>(
    key: &str,
    value: &'r Value,
    steps: &[SlotStep],
    under: Option<&'static str>,
    out: &mut Vec<(&'r Value, Vec<SlotStep>, Option<&'static str>)>,
) {
    let with = |step: SlotStep| {
        let mut steps = steps.to_vec();
        steps.push(step);
        steps
    };
    match (key, value) {
        ("properties", Value::Object(properties)) => out.extend(
            properties
                .iter()
                .map(|(name, schema)| (schema, with(SlotStep::Key(name.clone())), under)),
        ),
        ("items", schema) => out.push((schema, with(SlotStep::Items), under)),
        ("prefixItems", Value::Array(schemas)) => out.extend(
            schemas
                .iter()
                .enumerate()
                .map(|(at, schema)| (schema, with(SlotStep::Index(at)), under)),
        ),
        (key, Value::Array(schemas)) if SCHEMA_LISTS.contains(&key) => {
            out.extend(schemas.iter().map(|schema| (schema, steps.to_vec(), under)));
        }
        (key, schema) => {
            if let Some(&keyword) = UNSUPPORTED_ONE.iter().find(|keyword| **keyword == key) {
                out.push((schema, steps.to_vec(), under.or(Some(keyword))));
            } else if let (Some(&keyword), Value::Object(schemas)) = (
                UNSUPPORTED_MAPS.iter().find(|keyword| **keyword == key),
                schema,
            ) {
                let under = under.or(Some(keyword));
                out.extend(
                    schemas
                        .values()
                        .map(|schema| (schema, steps.to_vec(), under)),
                );
            }
        }
    }
}

/// Charges `steps` to the component's `budget` and to the catalog's `total`.
fn spend(budget: &mut Budget<'_>, total: &mut usize, steps: usize) -> Result<(), Error> {
    budget.spent = budget.spent.saturating_add(steps);
    *total = total.saturating_add(steps);
    if budget.spent > MAX_ENTRY_EXPANSION {
        return Err(entry_too_large(budget.name));
    }
    if *total > MAX_CATALOG_EXPANSION {
        return Err(catalog_too_large());
    }
    Ok(())
}

/// The slots `found` under a `$ref` target, seen from the `$ref` at `at`.
fn compose(
    at: &[SlotStep],
    under: Option<&'static str>,
    found: &[ChildSlot],
    budget: &mut Budget<'_>,
    total: &mut usize,
) -> Result<Vec<ChildSlot>, Error> {
    if let (Some(keyword), false) = (under, found.is_empty()) {
        return Err(unsupported(budget.name, keyword));
    }
    let mut slots = Vec::with_capacity(found.len());
    for inner in found {
        spend(budget, total, at.len() + inner.steps.len() + 1)?;
        let mut steps = at.to_vec();
        steps.extend(inner.steps.iter().cloned());
        slots.push(slot(steps, inner.kind));
    }
    Ok(slots)
}

/// A slot at `steps`; one id under `items` is a list of ids at the array.
fn slot(mut steps: Vec<SlotStep>, kind: SlotKind) -> ChildSlot {
    if kind == SlotKind::Single && steps.last() == Some(&SlotStep::Items) {
        steps.pop();
        return ChildSlot {
            steps,
            kind: SlotKind::List,
        };
    }
    ChildSlot { steps, kind }
}

/// The component's slots, each once, without the envelope `id`, which is never a child.
fn finish(slots: Vec<ChildSlot>) -> Vec<ChildSlot> {
    let envelope = [SlotStep::Key("id".to_owned())];
    let mut seen = HashSet::new();
    slots
        .into_iter()
        .filter(|slot| slot.steps != envelope && seen.insert(slot.clone()))
        .collect()
}

/// The entry `reference` lands in, else the reference itself.
fn entry_name(reference: &str) -> String {
    RefPointer::of(reference)
        .and_then(|pointer| pointer.entry().map(|(_, name)| name.to_owned()))
        .unwrap_or_else(|| reference.to_owned())
}

fn address(value: &Value) -> *const Value {
    std::ptr::from_ref(value)
}

fn unsupported(component: &str, keyword: &'static str) -> Error {
    malformed(reason![
        "Component '",
        Quoted::new(component),
        "' has a child slot under unsupported keyword '",
        keyword,
        "'",
    ])
}

fn malformed(reason: crate::quoted::Reason) -> Error {
    MalformedCatalogSnafu { reason }.build().into()
}

#[cfg(test)]
#[path = "slots_tests.rs"]
mod tests;
