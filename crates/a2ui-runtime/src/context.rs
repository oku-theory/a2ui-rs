//! Resolution of scoped data paths, as used inside templates and repeated lists.

use crate::error::Error;
use crate::pointer::Pointer;

/// Resolves `path` against `scope` into an absolute pointer.
///
/// `""` and `"."` name the scope itself, a path starting with `/` ignores the
/// scope, and any other path is appended to the scope's segments.
pub fn resolve_path(path: &str, scope: &Pointer) -> Result<Pointer, Error> {
    match path {
        "" | "." => Ok(scope.clone()),
        absolute if absolute.starts_with('/') => Pointer::parse(absolute),
        relative => Ok(scope.join(&Pointer::parse_relative(relative)?)),
    }
}

#[cfg(test)]
#[path = "context_tests.rs"]
mod tests;
