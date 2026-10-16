//! Configuration and observations for the optional prepared register executor.
//!
//! Execution uses admitted `PreparedModule` and the ordinary owned/borrowed
//! controller APIs. Raw lowering, mutable registers and unchecked fixture bodies
//! are implementation details, not a second public execution entry point.

pub use super::register_ir::{
    RegisterExecutionConfiguration, VmRegisterCallOpCounters, VmRegisterFallbackReason,
    VmRegisterHotBlock, VmRegisterLoweringCacheSnapshot, VmRegisterPreparationSnapshot,
    VmRegisterProfileSnapshot, VmRegisterRefOpCounters, VmRegisterValueOpCounters,
    VmTier1SpecializedExecutorCompileFailureReason, VmTier1SpecializedExecutorSnapshot,
};

impl super::PreparedModule {
    /// Inspect preparation outcomes for this installation, independent of RUN counters.
    pub fn register_preparation_snapshot(&self) -> VmRegisterPreparationSnapshot {
        self.register_plans.preparation_snapshot()
    }
}
