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

/// VM register-executor profiling snapshot captured from runtime state.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VmRegisterProfileSnapshot {
    pub enabled: bool,
    pub register_programs_executed: u64,
    pub register_program_fallbacks: u64,
    pub fallback_reasons: Vec<VmRegisterFallbackReason>,
    pub hot_blocks: Vec<VmRegisterHotBlock>,
    pub ref_ops: VmRegisterRefOpCounters,
    pub call_ops: VmRegisterCallOpCounters,
    pub value_ops: VmRegisterValueOpCounters,
}

/// Fallback reason counter for register-executor eligibility.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VmRegisterFallbackReason {
    pub reason: String,
    pub count: u64,
}

/// Per-block hotness counter in register-executor runs.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VmRegisterHotBlock {
    pub pou_id: u32,
    pub block_id: u32,
    pub start_pc: u32,
    pub hits: u64,
}

/// Ref-operation counters captured from register-executor runs.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VmRegisterRefOpCounters {
    pub load_ref: u64,
    pub store_ref: u64,
    pub load_ref_addr: u64,
    pub ref_field: u64,
    pub ref_index: u64,
    pub load_dynamic: u64,
    pub store_dynamic: u64,
    pub instance_field_lookups: u64,
}

/// Call/frame-operation counters captured from VM execution paths.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VmRegisterCallOpCounters {
    pub frame_pushes: u64,
    pub frame_pops: u64,
    pub function_block_call_entries: u64,
    pub parameter_bindings: u64,
    pub output_copy_backs: u64,
}

/// Value-clone/move counters captured from VM execution paths.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VmRegisterValueOpCounters {
    pub const_load_clones: u64,
    pub register_read_clones: u64,
    pub register_read_moves: u64,
    pub read_value_clones: u64,
    pub binding_expr_clones: u64,
    pub output_value_clones: u64,
}

/// Tier-1 specialized register-executor runtime snapshot.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VmTier1SpecializedExecutorSnapshot {
    pub enabled: bool,
    pub hot_block_threshold: u64,
    pub cache_capacity: usize,
    pub cached_blocks: usize,
    pub compile_attempts: u64,
    pub compile_successes: u64,
    pub compile_failures: u64,
    pub compile_failure_reasons: Vec<VmTier1SpecializedExecutorCompileFailureReason>,
    pub cache_evictions: u64,
    pub block_executions: u64,
    pub deopt_count: u64,
    pub deopt_reasons: Vec<VmTier1SpecializedExecutorDeoptReason>,
}

/// Tier-1 specialized register-executor compile failure reason counter.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VmTier1SpecializedExecutorCompileFailureReason {
    pub reason: String,
    pub count: u64,
}

/// Tier-1 specialized register-executor deoptimization reason counter.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VmTier1SpecializedExecutorDeoptReason {
    pub reason: String,
    pub count: u64,
}

/// Register-IR lowering cache snapshot captured from runtime state.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VmRegisterLoweringCacheSnapshot {
    pub enabled: bool,
    pub cache_capacity: usize,
    pub cached_entries: usize,
    pub hits: u64,
    pub misses: u64,
    pub build_errors: u64,
    pub cache_evictions: u64,
    pub invalidations: u64,
}

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
