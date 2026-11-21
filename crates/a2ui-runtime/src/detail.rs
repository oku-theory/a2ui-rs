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
}

impl DetailCode {
    /// The code as the conformance suites spell it, such as `"missing_field"`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::MissingField => "missing_field",
            Self::TypeMismatch => "type_mismatch",
            Self::InvalidValue => "invalid_value",
        }
    }
}

/// One refused field of a message: its dot path from the message root and why.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Detail {
    path: String,
    code: DetailCode,
}

impl Detail {
    pub(crate) fn new(path: String, code: DetailCode) -> Self {
        Self { path, code }
    }

    /// The dot path from the message root, such as `createSurface.surfaceId`.
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Why the field is refused.
    pub fn code(&self) -> DetailCode {
        self.code
    }
}

/// What an [`Error`] knows about where it happened; boxed so that a plain error stays small.
#[derive(Debug, Default)]
pub(crate) struct Context {
    path: Option<String>,
    surface_id: Option<String>,
    function_call_id: Option<String>,
    details: Vec<Detail>,
}

impl Error {
    /// The JSON Pointer, relative to the action's body, of the value at fault, if known.
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

    /// The refused fields; empty when the error names none.
    pub fn details(&self) -> &[Detail] {
        self.context
            .as_deref()
            .map_or(&[], |c| c.details.as_slice())
    }

    fn context_mut(&mut self) -> &mut Context {
        self.context.get_or_insert_with(Box::default)
    }

    pub(crate) fn with_path(mut self, path: String) -> Self {
        self.context_mut().path = Some(path);
        self
    }

    pub(crate) fn with_surface(mut self, id: &str) -> Self {
        self.context_mut().surface_id = Some(id.to_owned());
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
