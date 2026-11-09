//! Harness shared types.

#![allow(missing_docs)]

use crate::error::RuntimeError;
use crate::value::Duration;

/// A source file and optional path metadata.
#[derive(Debug, Clone)]
pub struct SourceFile {
    pub path: Option<String>,
    pub text: String,
}

impl SourceFile {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            path: None,
            text: text.into(),
        }
    }

    pub fn with_path(path: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            path: Some(path.into()),
            text: text.into(),
        }
    }
}

/// One diagnostic of a compile, where it is (specification 11 §6.8.1c): the path of its source,
/// its range as 1-based lines and columns, a column counted in UTF-16 code units as the language
/// server and `trust-lsp check` count it, its code (`E101`), its severity and its message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceDiagnostic {
    pub file: Option<String>,
    pub line: u32,
    pub column: u32,
    pub end_line: u32,
    pub end_column: u32,
    pub code: &'static str,
    pub severity: &'static str,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct CompileError {
    message: String,
    diagnostics: Vec<SourceDiagnostic>,
}

impl CompileError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            diagnostics: Vec::new(),
        }
    }

    /// The error of the sources' `diagnostics`, whose text is `message`.
    pub fn of_diagnostics(message: impl Into<String>, diagnostics: Vec<SourceDiagnostic>) -> Self {
        Self {
            message: message.into(),
            diagnostics,
        }
    }

    /// The diagnostics the error reports, each where it is; none for an error that names no
    /// place in the sources.
    #[must_use]
    pub fn diagnostics(&self) -> &[SourceDiagnostic] {
        &self.diagnostics
    }
}

impl std::fmt::Display for CompileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for CompileError {}

/// Result of a single execution cycle.
#[derive(Debug, Clone)]
pub struct CycleResult {
    pub cycle_number: u64,
    pub elapsed_time: Duration,
    pub errors: Vec<RuntimeError>,
}
