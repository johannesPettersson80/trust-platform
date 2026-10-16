//! Runtime execution backend contracts.

#![allow(missing_docs)]

use crate::error::RuntimeError;

/// Runtime execution backend mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExecutionBackend {
    /// Bytecode VM execution path.
    #[default]
    BytecodeVm,
}

impl ExecutionBackend {
    /// Parse backend selection text accepted by config/CLI surfaces.
    pub fn parse(text: &str) -> Result<Self, RuntimeError> {
        match text.trim().to_ascii_lowercase().as_str() {
            "vm" => Ok(Self::BytecodeVm),
            "interpreter" => Err(RuntimeError::InvalidConfig(
                "runtime.execution_backend='interpreter' is no longer supported for production runtimes; use 'vm'"
                    .into(),
            )),
            _ => Err(RuntimeError::InvalidConfig(
                format!("invalid runtime.execution_backend '{text}'").into(),
            )),
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::BytecodeVm => "vm",
        }
    }
}

/// Provenance for selected runtime execution backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExecutionBackendSource {
    /// Built-in default selection.
    #[default]
    Default,
    /// Project configuration (`runtime.execution_backend`).
    Config,
    /// CLI override (`--execution-backend`).
    Flag,
}

impl ExecutionBackendSource {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Config => "config",
            Self::Flag => "flag",
        }
    }
}

pub use trust_runtime_core::vm::register::{
    VmRegisterCallOpCounters, VmRegisterFallbackReason, VmRegisterHotBlock,
    VmRegisterLoweringCacheSnapshot, VmRegisterPreparationSnapshot, VmRegisterProfileSnapshot,
    VmRegisterRefOpCounters, VmRegisterValueOpCounters,
    VmTier1SpecializedExecutorCompileFailureReason, VmTier1SpecializedExecutorSnapshot,
};

#[cfg(test)]
mod tests {
    use super::ExecutionBackend;

    #[test]
    fn parse_accepts_case_insensitive_values() {
        assert_eq!(
            ExecutionBackend::parse("vm").expect("parse vm"),
            ExecutionBackend::BytecodeVm
        );
        assert_eq!(
            ExecutionBackend::parse("VM").expect("parse uppercase vm"),
            ExecutionBackend::BytecodeVm
        );
    }

    #[test]
    fn parse_accepts_trimmed_values() {
        assert_eq!(
            ExecutionBackend::parse(" vm ").expect("parse trimmed vm"),
            ExecutionBackend::BytecodeVm
        );
    }

    #[test]
    fn parse_rejects_empty_and_invalid_values() {
        let empty = ExecutionBackend::parse("").expect_err("empty should fail");
        assert!(empty
            .to_string()
            .contains("invalid runtime.execution_backend ''"));

        let invalid = ExecutionBackend::parse("bytecode").expect_err("invalid should fail");
        assert!(invalid
            .to_string()
            .contains("invalid runtime.execution_backend 'bytecode'"));
    }

    #[test]
    fn parse_rejects_interpreter_values() {
        let err = ExecutionBackend::parse("interpreter")
            .expect_err("interpreter backend should be rejected");
        assert!(err
            .to_string()
            .contains("runtime.execution_backend='interpreter' is no longer supported"));
    }
}
