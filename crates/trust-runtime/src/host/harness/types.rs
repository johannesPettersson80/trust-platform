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

#[derive(Debug, Clone)]
pub struct CompileError {
    message: String,
}

impl CompileError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
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
