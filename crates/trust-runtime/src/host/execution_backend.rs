//! Where the `runtime.execution_backend` a status reports came from. Every runtime runs compiled
//! programs (specification 44 §15); the key is still read, so that a project that names it loads,
//! and `status` reports where the value it reports came from.

#![allow(missing_docs)]

/// Provenance of the reported execution backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExecutionBackendSource {
    /// Built-in default selection.
    #[default]
    Default,
    /// Project configuration (`runtime.execution_backend`).
    Config,
}

impl ExecutionBackendSource {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Config => "config",
        }
    }
}

/// The execution backend every runtime reports: compiled programs (specification 44 §8.8).
pub const EXECUTION_BACKEND: &str = "native";
