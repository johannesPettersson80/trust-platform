//! Runtime execution backend contracts.

#![allow(missing_docs)]

// The execution backend selection is shared runtime code (spec 34 ARCH-11).
pub use trust_runtime_services::execution_backend::ExecutionBackend;

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
