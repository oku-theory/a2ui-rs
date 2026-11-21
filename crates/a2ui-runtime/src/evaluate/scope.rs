//! Where a value is evaluated: surface, data, template item and user activation.

use crate::data::DataModel;
use crate::pointer::Pointer;

/// The surface state a value is evaluated against.
///
/// A root scope reads paths from the data model root; [`Scope::in_template`] gives the scope of
/// one item of a collection template, where relative paths start at the item and `@index`
/// answers its position.
#[derive(Debug, Clone)]
pub struct Scope<'a> {
    surface_id: &'a str,
    default_catalog: Option<&'a str>,
    data: &'a DataModel,
    base: Pointer,
    index: Option<usize>,
    user_activation: bool,
}

impl<'a> Scope<'a> {
    /// The root scope of surface `surface_id`, outside any template and user activation.
    pub fn new(surface_id: &'a str, default_catalog: Option<&'a str>, data: &'a DataModel) -> Self {
        Self {
            surface_id,
            default_catalog,
            data,
            base: Pointer::root(),
            index: None,
            user_activation: false,
        }
    }

    /// This scope inside template item `index`, whose data sits at `base`.
    pub fn in_template(self, base: Pointer, index: usize) -> Self {
        Self {
            base,
            index: Some(index),
            ..self
        }
    }

    /// This scope inside a user activation, such as a click, when `active` holds.
    pub fn with_user_activation(self, active: bool) -> Self {
        Self {
            user_activation: active,
            ..self
        }
    }

    /// Whether the evaluation runs inside a user activation.
    pub fn user_activation(&self) -> bool {
        self.user_activation
    }

    /// The id of the surface evaluated on.
    pub(crate) fn surface_id(&self) -> &'a str {
        self.surface_id
    }

    /// The surface's default catalog id, if it has one.
    pub(crate) fn default_catalog(&self) -> Option<&'a str> {
        self.default_catalog
    }

    /// The surface's data model.
    pub(crate) fn data(&self) -> &'a DataModel {
        self.data
    }

    /// Where relative paths start: the root, or the template item.
    pub(crate) fn base(&self) -> &Pointer {
        &self.base
    }

    /// The position of the innermost template item, if inside one.
    pub(crate) fn index(&self) -> Option<usize> {
        self.index
    }
}

#[cfg(test)]
#[path = "scope_tests.rs"]
mod tests;
