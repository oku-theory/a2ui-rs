//! Evaluation of dynamic values: literals, `@path` and `@call` bindings, nested.
//!
//! `scope` holds where a value is evaluated, `call` the function calls and `@index`, `select`
//! which catalog a component or call resolves in. A failed binding becomes `null` and one
//! error; its siblings are still evaluated.

mod call;
mod scope;
mod select;
#[cfg(test)]
pub(crate) mod test_support;

use serde_json::{Map, Value};

use crate::catalog::CatalogSet;
use crate::context::resolve_path;
use crate::error::{Error, EvalFailure, ProcessFailure};
use crate::function::FunctionRegistry;
use crate::keys::{CALL_KEY, PATH_KEY};

pub use scope::Scope;
pub use select::{select_catalog, select_supported};

/// Deepest nesting of objects, arrays, call arguments and template parts an evaluation enters.
pub const MAX_EVALUATION_DEPTH: usize = 100;

/// What evaluating a value gave: the value, and every error raised on the way.
#[non_exhaustive]
#[derive(Debug)]
pub struct Evaluation {
    /// The evaluated value; each failed binding in it is `null`.
    pub value: Value,
    /// One error per failed binding, in evaluation order.
    pub errors: Vec<Error>,
}

/// Evaluates dynamic values against loaded catalogs and bound implementations.
///
/// A call's policy (`allowedCallers`, `requiresUserActivation`) is read from the `FunctionDef`
/// of the catalog in `catalogs` it resolves to, never from the registry.
#[derive(Debug, Clone, Copy)]
pub struct Evaluator<'a> {
    catalogs: &'a CatalogSet,
    functions: &'a FunctionRegistry,
}

impl<'a> Evaluator<'a> {
    /// An evaluator resolving calls in `catalogs` and running `functions`.
    pub fn new(catalogs: &'a CatalogSet, functions: &'a FunctionRegistry) -> Self {
        Self {
            catalogs,
            functions,
        }
    }

    /// `value` evaluated in `scope`.
    ///
    /// An object with a string `@path` reads the data model (a missing value is `null`); one
    /// with `@call` calls a function, its arguments evaluated first; any other object is
    /// evaluated member by member, one `@` stripped from keys starting `@@` (`@@<k>` beside
    /// `@<k>` is an error and `@<k>` becomes `null`); arrays item by item. Nesting past
    /// [`MAX_EVALUATION_DEPTH`] is a `RECURSION_LIMIT_EXCEEDED` error.
    pub fn evaluate(&self, value: &Value, scope: &Scope<'_>) -> Evaluation {
        let mut walk = Walk {
            frame: Frame {
                evaluator: *self,
                scope,
            },
            errors: Vec::new(),
        };
        let value = walk.value(value, 0);
        Evaluation {
            value,
            errors: walk.errors,
        }
    }
}

/// An evaluator and the scope it evaluates in.
#[derive(Clone, Copy)]
struct Frame<'w> {
    evaluator: Evaluator<'w>,
    scope: &'w Scope<'w>,
}

/// One evaluation in progress, collecting the errors of failed bindings.
struct Walk<'w> {
    frame: Frame<'w>,
    errors: Vec<Error>,
}

impl Walk<'_> {
    // The recursive methods keep their frames small (a wasm stack is 1 MiB): error building
    // and non-recursive work live in helpers that return before the walk descends.

    /// `value`, below `depth` entered levels.
    ///
    /// The depth is checked before descending, so the recursion stays within the limit.
    fn value(&mut self, value: &Value, depth: usize) -> Value {
        match value {
            Value::Object(_) | Value::Array(_) if depth >= MAX_EVALUATION_DEPTH => self.too_deep(),
            Value::Object(map) => self.object(map, depth + 1),
            Value::Array(items) => self.array(items, depth + 1),
            scalar => scalar.clone(),
        }
    }

    /// The array `items`, entered at `depth`, item by item.
    fn array(&mut self, items: &[Value], depth: usize) -> Value {
        let mut values = Vec::with_capacity(items.len());
        for item in items {
            values.push(self.value(item, depth));
        }
        Value::Array(values)
    }

    /// The object `map`, entered at `depth`: a path binding, a call or plain members.
    fn object(&mut self, map: &Map<String, Value>, depth: usize) -> Value {
        if let Some(Value::String(path)) = map.get(PATH_KEY) {
            self.binding(path)
        } else if map.contains_key(CALL_KEY) {
            let called = self.call(map, depth);
            self.settle(called)
        } else {
            self.members(map, depth)
        }
    }

    /// The members of `map`, entered at `depth`, one `@` stripped from keys starting `@@`.
    fn members(&mut self, map: &Map<String, Value>, depth: usize) -> Value {
        let mut members = Map::new();
        for (key, value) in map {
            let value = self.value(value, depth);
            members.insert(unescaped(key).to_owned(), value);
        }
        self.collisions(map, &mut members);
        Value::Object(members)
    }

    /// Nulls each member of `members` two keys of `map` unescape to, recording an error.
    ///
    /// `@@<k>` beside a plain `@<k>` would overwrite one with the other, so neither is kept;
    /// `@@@<k>` beside `@@<k>` unescape to distinct keys.
    fn collisions(&mut self, map: &Map<String, Value>, members: &mut Map<String, Value>) {
        for escaped in map.keys().filter(|key| unescaped(key) != key.as_str()) {
            let plain = unescaped(escaped);
            if map.contains_key(plain) && unescaped(plain) == plain {
                let failure = ProcessFailure::EscapedKeyCollision {
                    escaped: escaped.into(),
                    plain: plain.into(),
                };
                self.errors.push(failure.into());
                members.insert(plain.to_owned(), Value::Null);
            }
        }
    }

    /// The path binding `path`, or `null` with its error recorded.
    fn binding(&mut self, path: &str) -> Value {
        let read = self.path(path);
        self.settle(read)
    }

    /// `null`, recording that the walk went past [`MAX_EVALUATION_DEPTH`].
    fn too_deep(&mut self) -> Value {
        self.settle(Err(too_deep()))
    }

    /// The data at `path`, resolved against the scope's base; `null` when absent.
    fn path(&self, path: &str) -> Result<Value, Error> {
        let scope = self.frame.scope;
        let pointer = resolve_path(path, scope.base())?;
        Ok(scope.data().get(&pointer).cloned().unwrap_or(Value::Null))
    }

    /// The value of `result`, or `null` with its error recorded.
    fn settle(&mut self, result: Result<Value, Error>) -> Value {
        result.unwrap_or_else(|error| {
            self.errors.push(error);
            Value::Null
        })
    }
}

/// `key` with one `@` stripped when it starts `@@`.
fn unescaped(key: &str) -> &str {
    match key.strip_prefix('@') {
        Some(rest) if rest.starts_with('@') => rest,
        _ => key,
    }
}

/// The error of a value nested past [`MAX_EVALUATION_DEPTH`].
fn too_deep() -> Error {
    EvalFailure::Depth {
        limit: MAX_EVALUATION_DEPTH,
    }
    .into()
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
