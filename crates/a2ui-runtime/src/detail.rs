//! Where in a message an [`Error`] happened: its path, ids and per-field details.

use crate::error::Error;

/// Why one field of a message is refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum DetailCode {
    /// A required field is absent.
    MissingField,
    /// A field has the wrong JSON type.
    TypeMismatch,
    /// A field has the right type but a value or name the protocol refuses.
    InvalidValue,
    /// A component placed under a parent type its `allowedParents` does not list.
    UnallowedParent,
    /// A container holding a child type its `allowedChildren` does not list.
    UnallowedChild,
}

impl DetailCode {
    /// The code as the conformance suites spell it, such as `"missing_field"`.
    ///
    /// Composition codes keep the protocol's error code spelling, such as `"UNALLOWED_PARENT"`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::MissingField => "missing_field",
            Self::TypeMismatch => "type_mismatch",
            Self::InvalidValue => "invalid_value",
            Self::UnallowedParent => "UNALLOWED_PARENT",
            Self::UnallowedChild => "UNALLOWED_CHILD",
        }
    }
}

/// One refused field of a message: where it is and why.
///
/// A fault in the message's own shape has a dot path from the message root; a schema violation
/// of one component has a JSON Pointer relative to that component, the error's
/// [`Error::path`] saying which component; a composition fault has the error's JSON Pointer.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Detail {
    path: String,
    code: DetailCode,
    pub(crate) surface_id: Option<String>,
}

impl Detail {
    pub(crate) fn new(path: String, code: DetailCode) -> Self {
        Self {
            path,
            code,
            surface_id: None,
        }
    }

    /// Where the field is, in one of two forms.
    ///
    /// A message-shape fault gives a dot path from the message root, such as
    /// `createSurface.surfaceId`, prefixed `messages.<i>.` in a batch. Other faults give a JSON
    /// Pointer, starting with `/`: relative to the component for a schema violation, such as
    /// `/text`, and relative to the action's body for a composition fault, such as
    /// `/components/0/children/1`; in a batch, [`Error::message_index`] gives the message.
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Why the field is refused.
    pub fn code(&self) -> DetailCode {
        self.code
    }

    /// The surface the refused field belongs to, when known.
    pub fn surface_id(&self) -> Option<&str> {
        self.surface_id.as_deref()
    }
}

/// What an [`Error`] knows about where it happened; boxed so that a plain error stays small.
#[derive(Debug, Default)]
pub(crate) struct Context {
    path: Option<String>,
    surface_id: Option<String>,
    function_call_id: Option<String>,
    message_index: Option<usize>,
    details: Vec<Detail>,
}

impl Error {
    /// The JSON Pointer of the value at fault, if known.
    ///
    /// Its origin depends on the producer. Message-shape faults, composition and the
    /// processor's component checks give a pointer relative to the action's body, such as
    /// `/components/0/children/1`. [`validate_component`](crate::Catalog::validate_component)
    /// and the located component checks give one relative to the component, such as `/text`.
    /// [`Detail::path`] gives a dot path from the message root for message-shape faults.
    pub fn path(&self) -> Option<&str> {
        self.context.as_deref()?.path.as_deref()
    }

    /// The `surfaceId` of the message, when it is a string.
    pub fn surface_id(&self) -> Option<&str> {
        self.context.as_deref()?.surface_id.as_deref()
    }

    /// The `functionCallId` of a `callRendererFunction` message, when it is a string.
    pub fn function_call_id(&self) -> Option<&str> {
        self.context.as_deref()?.function_call_id.as_deref()
    }

    /// The refused message's index in its batch, for an error from a batch.
    pub fn message_index(&self) -> Option<usize> {
        self.context.as_deref()?.message_index
    }

    /// The refused fields; empty when the error names none.
    pub fn details(&self) -> &[Detail] {
        self.context
            .as_deref()
            .map_or(&[], |c| c.details.as_slice())
    }

    fn context_mut(&mut self) -> &mut Context {
        self.context.get_or_insert_with(Box::default)
    }

    /// Records `index` as the message index and prefixes each dot-path detail with
    /// `messages.<index>.`, for a message read from a batch; JSON Pointer details stay as they
    /// are, relative to their component (schema violations) or to the action's body
    /// (composition faults).
    pub(crate) fn at_message(mut self, index: usize) -> Self {
        let context = self.context_mut();
        context.message_index = Some(index);
        for detail in &mut context.details {
            if !detail.path.starts_with('/') {
                detail.path = format!("messages.{index}.{}", detail.path);
            }
        }
        self
    }

    pub(crate) fn with_path(mut self, path: String) -> Self {
        self.context_mut().path = Some(path);
        self
    }

    /// Prepends `prefix` to the path, or makes it the path when the error has none.
    pub(crate) fn prefix_path(mut self, prefix: &str) -> Self {
        let context = self.context_mut();
        context.path = Some(match context.path.take() {
            Some(path) => format!("{prefix}{path}"),
            None => prefix.to_owned(),
        });
        self
    }

    /// Sets the surface id, and that of every detail which names none.
    pub(crate) fn with_surface(mut self, id: &str) -> Self {
        let context = self.context_mut();
        context.surface_id = Some(id.to_owned());
        for detail in &mut context.details {
            detail.surface_id.get_or_insert_with(|| id.to_owned());
        }
        self
    }

    pub(crate) fn with_function_call(mut self, id: &str) -> Self {
        self.context_mut().function_call_id = Some(id.to_owned());
        self
    }

    pub(crate) fn with_detail(mut self, detail: Detail) -> Self {
        self.context_mut().details.push(detail);
        self
    }
}

#[cfg(test)]
#[path = "detail_tests.rs"]
mod tests;
