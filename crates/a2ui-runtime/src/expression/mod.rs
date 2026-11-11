//! Parser for `${...}` expression templates, as `formatString` uses them.

mod number;
mod parser;

use serde_json::{Map, Value, json};

use crate::error::{Error, ExpressionLimitSnafu, ExpressionSyntaxSnafu};

/// Longest template accepted, in characters.
pub const MAX_TEMPLATE_CHARS: usize = 10_000;
/// Most parts one template may produce.
pub const MAX_TEMPLATE_PARTS: usize = 1_000;
/// Deepest nesting of interpolations and call arguments together.
pub const MAX_DEPTH: usize = 100;

/// One parsed template part or call argument.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    /// A literal string, number or boolean.
    Literal(Value),
    /// A data binding, absolute (`/a/b`) or relative (`a/b`).
    Path(String),
    /// A function call with named arguments in source order.
    Call {
        /// The function name, such as `formatDate` or `@index`.
        name: String,
        /// The `(name, value)` arguments.
        args: Vec<(String, Expr)>,
    },
}

impl Expr {
    /// The conformance JSON form: the literal, `{path}` or `{call, args, returnType}`.
    pub fn to_json(&self) -> Value {
        match self {
            Self::Literal(value) => value.clone(),
            Self::Path(path) => json!({ "path": path }),
            Self::Call { name, args } => {
                let args: Map<String, Value> = args
                    .iter()
                    .map(|(key, value)| (key.clone(), value.to_json()))
                    .collect();
                json!({ "call": name, "args": args, "returnType": "any" })
            }
        }
    }
}

/// Parses `input` into text and interpolated expressions.
///
/// Adjacent text is joined and empty text dropped; `\${` writes a literal `${`.
pub fn parse_template(input: &str) -> Result<Vec<Expr>, Error> {
    if input.chars().count() > MAX_TEMPLATE_CHARS {
        return Err(ExpressionLimitSnafu {
            limit: "10000 characters",
        }
        .build()
        .into());
    }
    let parts = parser::Parser::new(input).template()?;
    if parts.len() > MAX_TEMPLATE_PARTS {
        return Err(ExpressionLimitSnafu {
            limit: "1000 parts",
        }
        .build()
        .into());
    }
    Ok(parts)
}

fn syntax(offset: usize, message: &'static str) -> Error {
    ExpressionSyntaxSnafu { message, offset }.build().into()
}
