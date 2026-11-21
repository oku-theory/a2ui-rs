//! Function calls: catalog selection, policy checks, arguments, `@index` and templates.

use serde_json::{Map, Value};
use snafu::OptionExt;

use super::{Frame, MAX_EVALUATION_DEPTH, Walk};
use crate::catalog::{AllowedCallers, Catalog};
use crate::coerce;
use crate::error::{CallShapeSnafu, Error, EvalFailure, ProcessFailure};
use crate::expression::Expr;
use crate::function::{Args, CallContext, Interpolate};
use crate::keys::CALL_KEY;
use crate::quoted::{Quoted, reason};

use super::select::select_catalog;

/// The system function giving the current template item's position.
const INDEX: &str = "@index";

/// The unevaluated arguments of a call.
#[derive(Clone, Copy)]
enum Arguments<'v> {
    /// The `args` object of a JSON call, if it has one.
    Json(Option<&'v Map<String, Value>>),
    /// The arguments of a template call, whose nested calls run in `catalog`.
    Template {
        args: &'v [(String, Expr)],
        catalog: &'v str,
    },
}

/// A JSON call's parts, its shape checked.
struct Parsed<'v> {
    name: &'v str,
    own: Option<&'v str>,
    args: Option<&'v Map<String, Value>>,
}

/// The parts of the JSON call `map`; `@index` keeps no `catalogId`, as it never resolves one.
///
/// # Errors
///
/// A `VALIDATION_FAILED` error when `@call` is not a string, `args` not an object or
/// `catalogId` not a string.
fn parse(map: &Map<String, Value>) -> Result<Parsed<'_>, Error> {
    let name = map
        .get(CALL_KEY)
        .and_then(Value::as_str)
        .context(CallShapeSnafu {
            reason: "@call must be a string",
        })?;
    let args = match map.get("args") {
        None => None,
        Some(Value::Object(args)) => Some(args),
        Some(_) => {
            let reason = reason!["args of '", Quoted::new(name), "' must be an object"];
            return Err(CallShapeSnafu { reason }.build().into());
        }
    };
    let own = match map.get("catalogId") {
        _ if name == INDEX => None,
        None => None,
        Some(Value::String(id)) => Some(id.as_str()),
        Some(_) => {
            let reason = reason!["catalogId of '", Quoted::new(name), "' must be a string"];
            return Err(CallShapeSnafu { reason }.build().into());
        }
    };
    Ok(Parsed { name, own, args })
}

impl<'w> Walk<'w> {
    // Like `value` and `object`, these recurse; `parse` and `resolve` keep the error building
    // out of their frames.

    /// The JSON call `map`, entered at `depth`.
    ///
    /// A call naming no `catalogId` runs in the surface default, never in an enclosing call's.
    pub(super) fn call(&mut self, map: &Map<String, Value>, depth: usize) -> Result<Value, Error> {
        let Parsed { name, own, args } = parse(map)?;
        let args = Arguments::Json(args);
        if name == INDEX {
            self.index(args, depth)
        } else {
            self.run(name, own, args, depth)
        }
    }

    /// Resolves `name` in `own` else the surface default, checks its policy, then runs it.
    fn run(
        &mut self,
        name: &str,
        own: Option<&str>,
        args: Arguments<'_>,
        depth: usize,
    ) -> Result<Value, Error> {
        let catalog = self.resolve(name, own)?;
        let args = self.arguments(args, depth);
        let Frame { evaluator, scope } = self.frame;
        // MVP: t-1710 (a schema-only function becomes a callAgentFunction)
        let implementation = evaluator
            .functions
            .implementation(catalog.id(), name)
            .ok_or_else(|| EvalFailure::NotImplemented {
                catalog: catalog.id().into(),
                name: name.into(),
            })?;
        let frame = self.frame;
        let mut context = CallContext::new(
            scope.user_activation(),
            scope.surface_id(),
            catalog.id(),
            depth,
            &frame,
        );
        implementation(&args, &mut context)
    }

    /// The catalog `name` resolves in, once its `FunctionDef`'s policy allows the call.
    ///
    /// # Errors
    ///
    /// No catalog selected or loaded, no such function, an `agentOnly` function, or one
    /// requiring a user activation the scope does not hold.
    fn resolve(&self, name: &str, own: Option<&str>) -> Result<&'w Catalog, Error> {
        let Frame { evaluator, scope } = self.frame;
        let surface = scope.surface_id();
        let id = select_catalog(own, scope.default_catalog(), surface, "function", name)?;
        let catalog = evaluator
            .catalogs
            .get(id)
            .ok_or_else(|| ProcessFailure::CatalogNotFound { id: id.into() })?;
        let function = catalog
            .function(name)
            .ok_or_else(|| EvalFailure::UnknownFunction {
                catalog: id.into(),
                name: name.into(),
            })?;
        // MVP: t-1710 (agentOnly functions are reached through the RPC route)
        if function.allowed_callers() == AllowedCallers::AgentOnly {
            return Err(EvalFailure::RendererForbidden { name: name.into() }.into());
        }
        if function.requires_user_activation() && !scope.user_activation() {
            return Err(EvalFailure::ActivationRequired { name: name.into() }.into());
        }
        Ok(catalog)
    }

    /// The template item's position plus the integral `offset` argument (default 0).
    fn index(&mut self, args: Arguments<'_>, depth: usize) -> Result<Value, Error> {
        let index = self
            .frame
            .scope
            .index()
            .ok_or(EvalFailure::IndexOutsideTemplate)?;
        let args = self.arguments(args, depth);
        position(index, &args)
    }

    /// Each argument evaluated, a failed one `null` with its error recorded.
    fn arguments(&mut self, args: Arguments<'_>, depth: usize) -> Args {
        let mut values = Args::new();
        match args {
            Arguments::Json(None) => {}
            Arguments::Json(Some(args)) => {
                for (key, value) in args {
                    let value = self.value(value, depth);
                    values.insert(key.clone(), value);
                }
            }
            Arguments::Template { args, catalog } => {
                for (key, expr) in args {
                    let value = self.expr(expr, catalog, depth);
                    values.insert(key.clone(), value);
                }
            }
        }
        values
    }

    /// The template part `expr`, below `depth` entered levels; its calls run in `catalog`.
    fn expr(&mut self, expr: &Expr, catalog: &str, depth: usize) -> Value {
        if depth >= MAX_EVALUATION_DEPTH {
            return self.too_deep();
        }
        let depth = depth + 1;
        let result = match expr {
            Expr::Literal(value) => Ok(value.clone()),
            Expr::Path(path) => self.path(path),
            Expr::Call { name, args } => {
                let args = Arguments::Template { args, catalog };
                if name == INDEX {
                    self.index(args, depth)
                } else {
                    self.run(name, Some(catalog), args, depth)
                }
            }
        };
        self.settle(result)
    }
}

/// `index` plus the `offset` of `args`: 0 when absent, else an integer.
///
/// # Errors
///
/// An `EXPRESSION_ERROR` when the offset is not an integer.
fn position(index: usize, args: &Args) -> Result<Value, Error> {
    let offset = match args.get("offset") {
        None => 0,
        Some(offset) => {
            integer(offset).ok_or_else(|| Error::function(INDEX, "offset must be an integer"))?
        }
    };
    // Indices are capped at `MAX_LIST_INDEX` and offsets at ±2^53: no `i64` overflow.
    i64::try_from(index)
        .ok()
        .and_then(|index| index.checked_add(offset))
        .map(Value::from)
        .ok_or_else(|| Error::function(INDEX, "position out of range"))
}

/// Largest magnitude an `f64` holds every integer up to: 2^53.
const EXACT_F64: f64 = 9_007_199_254_740_992.0;

/// `value` as an integer, when [`coerce::number`] gives an integral one within ±2^53.
fn integer(value: &Value) -> Option<i64> {
    let number = coerce::number(value)?;
    // Integral and within ±2^53, so the conversion is exact.
    (number.fract() == 0.0 && number.abs() <= EXACT_F64).then_some(number as i64)
}

impl Interpolate for Frame<'_> {
    fn parts(&self, parts: &[Expr], catalog_id: &str, depth: usize) -> Result<Vec<Value>, Error> {
        let mut walk = Walk {
            frame: *self,
            errors: Vec::new(),
        };
        let mut values = Vec::with_capacity(parts.len());
        for part in parts {
            values.push(walk.expr(part, catalog_id, depth));
            if let Some(first) = std::mem::take(&mut walk.errors).into_iter().next() {
                return Err(first);
            }
        }
        Ok(values)
    }
}

#[cfg(test)]
#[path = "call_tests.rs"]
mod tests;
