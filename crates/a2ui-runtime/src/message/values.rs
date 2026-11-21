//! Value checks around the schema: nesting depth before it, call depth and bindings after it.
//!
//! Components stay raw JSON here, so the walks run on the message itself. Both are iterative, with
//! an explicit stack, because the value is agent input of any shape.

use serde_json::{Map, Value};

use super::AgentMessage;
use super::envelope::Action;
use crate::error::{CallDepthSnafu, Error, ValueDepthSnafu};
use crate::keys::{CALL_KEY, PATH_KEY};
use crate::limits::{MAX_CALL_DEPTH, MAX_MESSAGE_DEPTH, deeper_than};
use crate::pointer::{Pointer, escape_token};

/// Refuses a component or value nested past [`MAX_MESSAGE_DEPTH`], before the schema walks it.
///
/// Only values whose type is already right are looked at; the schema reports the others.
pub(crate) fn check_depth(action: Action, body: &Map<String, Value>) -> Result<(), Error> {
    let located = |error: Error| located(error, action, body);
    let check = |value: &Value, path: String| {
        if deeper_than(value, MAX_MESSAGE_DEPTH) {
            let error: Error = ValueDepthSnafu.build().into();
            return Err(located(error.with_path(path)));
        }
        Ok(())
    };
    match action {
        Action::CreateSurface => {
            for (index, component) in listed(body, "components") {
                check(component, format!("/components/{index}"))?;
            }
            body.get("dataModel")
                .map_or(Ok(()), |value| check(value, "/dataModel".to_owned()))
        }
        Action::UpdateComponents => {
            for (index, component) in listed(body, "components") {
                check(component, format!("/components/{index}"))?;
            }
            Ok(())
        }
        Action::UpdateDataModel => body
            .get("value")
            .map_or(Ok(()), |value| check(value, "/value".to_owned())),
        Action::CallRendererFunction => body
            .get("callFunction")
            .and_then(Value::as_object)
            .and_then(|call| call.get("args"))
            .map_or(Ok(()), |value| {
                check(value, "/callFunction/args".to_owned())
            }),
        Action::DeleteSurface | Action::AgentFunctionResponse => Ok(()),
    }
}

fn listed<'a>(
    body: &'a Map<String, Value>,
    name: &str,
) -> impl Iterator<Item = (usize, &'a Value)> {
    body.get(name)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .enumerate()
}

/// Adds the id the message names: its `functionCallId` for a call, else its `surfaceId`.
fn located(error: Error, action: Action, body: &Map<String, Value>) -> Error {
    let call = action == Action::CallRendererFunction;
    let key = if call { "functionCallId" } else { "surfaceId" };
    match body.get(key).and_then(Value::as_str) {
        Some(id) if call => error.with_function_call(id),
        Some(id) => error.with_surface(id),
        None => error,
    }
}

/// Checks the components of a message: call nesting and every `@path` string.
pub(crate) fn check_bindings(message: &AgentMessage) -> Result<(), Error> {
    let (surface_id, components) = match message {
        AgentMessage::CreateSurface(body) => (&body.surface_id, &body.components),
        AgentMessage::UpdateComponents(body) => (&body.surface_id, &body.components),
        AgentMessage::UpdateDataModel(_)
        | AgentMessage::DeleteSurface(_)
        | AgentMessage::CallRendererFunction(_)
        | AgentMessage::AgentFunctionResponse(_) => return Ok(()),
    };
    for (index, component) in components.iter().enumerate() {
        walk(component).map_err(|error| {
            error
                .prefix_path(&format!("/components/{index}"))
                .with_surface(surface_id)
        })?;
    }
    Ok(())
}

/// One step from a node to its child: an object key or an array index.
#[derive(Clone, Copy)]
enum Step<'a> {
    Key(&'a str),
    Index(usize),
}

/// The steps from a component down to a node, as a tree in an arena, so a node costs one entry.
type Arena<'a> = Vec<(Option<usize>, Step<'a>)>;

fn pointer_to(arena: &Arena<'_>, mut node: Option<usize>) -> String {
    let mut steps = Vec::new();
    while let Some(at) = node {
        let (parent, step) = arena[at];
        steps.push(step);
        node = parent;
    }
    steps
        .iter()
        .rev()
        .map(|step| match step {
            Step::Key(key) => format!("/{}", escape_token(key)),
            Step::Index(index) => format!("/{index}"),
        })
        .collect()
}

/// Walks one component in document order; an error's path is relative to the component.
fn walk(component: &Value) -> Result<(), Error> {
    let mut arena: Arena<'_> = Vec::new();
    // Each entry is a container, its arena node (none for the component) and the calls above it.
    let mut stack = vec![(component, None, 0_usize)];
    while let Some((node, at, calls)) = stack.pop() {
        let fail = |error: Error| error.with_path(pointer_to(&arena, at));
        let mut children: Vec<(Step<'_>, &Value)> = Vec::new();
        let mut calls = calls;
        match node {
            Value::Object(map) => {
                if map.contains_key(CALL_KEY) {
                    calls += 1;
                    if calls > MAX_CALL_DEPTH {
                        let error = CallDepthSnafu { depth: calls }.build();
                        return Err(fail(error.into()));
                    }
                }
                if let Some(Value::String(path)) = map.get(PATH_KEY) {
                    let parsed = if path.starts_with('/') {
                        Pointer::parse(path)
                    } else {
                        Pointer::parse_relative(path)
                    };
                    parsed.map_err(fail)?;
                }
                children.extend(map.iter().map(|(key, child)| (Step::Key(key), child)));
            }
            Value::Array(items) => {
                children.extend(items.iter().enumerate().map(|(i, c)| (Step::Index(i), c)));
            }
            _ => {}
        }
        for (step, child) in children.into_iter().rev() {
            if matches!(child, Value::Object(_) | Value::Array(_)) {
                arena.push((at, step));
                stack.push((child, Some(arena.len() - 1), calls));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "values_tests.rs"]
mod tests;
