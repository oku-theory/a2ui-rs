//! What a function implementation sees of the call it runs in.

use std::fmt;

use crate::coerce;
use crate::error::{Error, EvalFailure};
use crate::evaluate::MAX_INTERPOLATED_BYTES;
use crate::expression::{Expr, parse_template};

/// Evaluates template parts for [`CallContext::interpolate`]; the evaluator implements it.
pub(crate) trait Interpolate {
    /// `part` as a value, calls in catalog `catalog_id`, at evaluation depth `depth`.
    ///
    /// The first error the part raises is returned.
    fn part(&self, part: &Expr, catalog_id: &str, depth: usize)
    -> Result<serde_json::Value, Error>;
}

/// The call an implementation runs in: its surface, catalog and user activation.
pub struct CallContext<'a> {
    user_activation: bool,
    surface_id: &'a str,
    catalog_id: &'a str,
    depth: usize,
    evaluator: &'a dyn Interpolate,
}

impl fmt::Debug for CallContext<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CallContext")
            .field("user_activation", &self.user_activation)
            .field("surface_id", &self.surface_id)
            .field("catalog_id", &self.catalog_id)
            .field("depth", &self.depth)
            .finish_non_exhaustive()
    }
}

impl<'a> CallContext<'a> {
    /// A context for a call in `catalog_id` on `surface_id`, entered at evaluation `depth`.
    pub(crate) fn new(
        user_activation: bool,
        surface_id: &'a str,
        catalog_id: &'a str,
        depth: usize,
        evaluator: &'a dyn Interpolate,
    ) -> Self {
        Self {
            user_activation,
            surface_id,
            catalog_id,
            depth,
            evaluator,
        }
    }

    /// Whether the call runs inside a user activation, such as a click.
    pub fn user_activation(&self) -> bool {
        self.user_activation
    }

    /// The id of the surface the call evaluates on.
    pub fn surface_id(&self) -> &str {
        self.surface_id
    }

    /// The id of the catalog the call resolved in.
    pub fn catalog_id(&self) -> &str {
        self.catalog_id
    }

    /// `template`'s `${…}` parts evaluated in this call's scope and joined as text.
    ///
    /// A function call in the template runs in this call's catalog (`@index` names none); each
    /// value is converted with [`coerce::text`]. Each part is one step of the evaluation's
    /// budget ([`crate::MAX_EVALUATION_STEPS`]).
    ///
    /// # Errors
    ///
    /// The template's parse error, the first error a part raises, or an `EXPRESSION_ERROR`
    /// once the text would pass [`MAX_INTERPOLATED_BYTES`], checked before each part is added.
    pub fn interpolate(&mut self, template: &str) -> Result<String, Error> {
        let parts = parse_template(template)?;
        let mut text = String::new();
        for part in &parts {
            let value = self.evaluator.part(part, self.catalog_id, self.depth)?;
            let piece = coerce::text(&value);
            if text.len().saturating_add(piece.len()) > MAX_INTERPOLATED_BYTES {
                return Err(EvalFailure::Output {
                    limit: MAX_INTERPOLATED_BYTES,
                }
                .into());
            }
            text.push_str(&piece);
        }
        Ok(text)
    }
}

#[cfg(test)]
#[path = "context_tests.rs"]
mod tests;
