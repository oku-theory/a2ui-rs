//! The steps one evaluation may run, shared with every template it interpolates.

use std::cell::Cell;

use super::MAX_EVALUATION_STEPS;
use crate::error::{Error, EvalFailure};

/// Steps an evaluation has run, and whether one was refused.
#[derive(Debug, Default)]
pub(super) struct Budget {
    used: Cell<usize>,
    exhausted: Cell<bool>,
}

impl Budget {
    /// Counts one function call or template part.
    ///
    /// # Errors
    ///
    /// A `RECURSION_LIMIT_EXCEEDED` error once [`MAX_EVALUATION_STEPS`] steps have run; the
    /// budget is then exhausted for good.
    pub(super) fn step(&self) -> Result<(), Error> {
        let used = self.used.get();
        if used >= MAX_EVALUATION_STEPS {
            self.exhausted.set(true);
            return Err(over_budget());
        }
        self.used.set(used + 1);
        Ok(())
    }

    /// Whether a step was refused.
    pub(super) fn exhausted(&self) -> bool {
        self.exhausted.get()
    }
}

/// The error of an evaluation past [`MAX_EVALUATION_STEPS`].
pub(super) fn over_budget() -> Error {
    EvalFailure::Steps {
        limit: MAX_EVALUATION_STEPS,
    }
    .into()
}
