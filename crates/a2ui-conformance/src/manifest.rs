//! Upstream's executed-case manifest, one JSON line per finished case.

use std::fs::{File, OpenOptions};
use std::io::Write as _;
use std::path::PathBuf;
use std::sync::{Mutex, PoisonError};

use serde_json::json;

/// The outcome recorded for one case.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// The case ran and passed.
    Passed,
    /// The case ran and failed.
    Failed,
    /// The case was not run; a reason is required.
    Skipped,
}

impl Status {
    fn as_str(self) -> &'static str {
        match self {
            Self::Passed => "passed",
            Self::Failed => "failed",
            Self::Skipped => "skipped",
        }
    }
}

/// Writer for `$A2UI_CONFORMANCE_MANIFEST_DIR/rust-core-<pid>.jsonl`.
#[derive(Debug)]
pub struct Manifest {
    file: Mutex<File>,
}

impl Manifest {
    /// Opens the manifest when `A2UI_CONFORMANCE_MANIFEST_DIR` is set.
    pub fn from_env() -> Result<Option<Self>, String> {
        let Some(dir) = std::env::var_os("A2UI_CONFORMANCE_MANIFEST_DIR") else {
            return Ok(None);
        };
        let path = PathBuf::from(dir).join(format!("rust-core-{}.jsonl", std::process::id()));
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|err| format!("{}: {err}", path.display()))?;
        Ok(Some(Self {
            file: Mutex::new(file),
        }))
    }

    /// Appends the record for one case.
    pub fn record(
        &self,
        suite: &str,
        case: &str,
        status: Status,
        reason: Option<&str>,
    ) -> Result<(), String> {
        let mut line = json!({"harness": "rust-core", "suite": suite, "case": case, "status": status.as_str()});
        if let Some(reason) = reason {
            line["reason"] = json!(reason);
        }
        let mut file = self.file.lock().unwrap_or_else(PoisonError::into_inner);
        writeln!(file, "{line}").map_err(|err| format!("manifest: {err}"))
    }
}
