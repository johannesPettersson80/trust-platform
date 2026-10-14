//! Existing hosted observations forwarded to prepared shared register execution.
use super::Runtime;

impl Runtime {
    /// Inspect immutable lowering and specialization outcomes from admission.
    /// Resetting RUN observations never erases these diagnostics.
    pub fn vm_register_preparation_snapshot(
        &self,
    ) -> crate::execution_backend::VmRegisterPreparationSnapshot {
        self.prepared().register_preparation_snapshot()
    }
    /// Enable the existing execution telemetry without changing installed plans.
    pub fn set_vm_register_profile_enabled(&mut self, enabled: bool) {
        self.state.set_register_profile_enabled(enabled);
    }
    /// Reset execution counters while preserving active tier selections.
    pub fn reset_vm_register_profile(&mut self) {
        self.state.reset_register_profile();
    }
    /// Inspect register execution observations from the common dispatcher.
    pub fn vm_register_profile_snapshot(
        &self,
    ) -> crate::execution_backend::VmRegisterProfileSnapshot {
        self.state.register_profile_snapshot()
    }
    /// Preserve compatibility instrumentation without permitting RUN compilation.
    pub fn set_vm_register_lowering_cache_enabled(&mut self, enabled: bool) {
        self.state.set_register_lowering_enabled(enabled);
    }
    /// Clear lowering observations; prepared plans remain installed.
    pub fn reset_vm_register_lowering_cache(&mut self) {
        self.state.reset_register_lowering_cache();
    }
    /// Observe prepared plans and lookups, rather than a lazy compilation cache.
    pub fn vm_register_lowering_cache_snapshot(
        &self,
    ) -> crate::execution_backend::VmRegisterLoweringCacheSnapshot {
        self.state.register_lowering_snapshot()
    }
    /// Enable selection of specialized plans already admitted before RUN.
    pub fn set_vm_tier1_specialized_executor_enabled(&mut self, enabled: bool) {
        self.state.set_tier1_enabled(enabled);
    }
    /// Clear hotness/selection observations while retaining prepared code.
    pub fn reset_vm_tier1_specialized_executor(&mut self) {
        self.state.reset_tier1();
    }
    /// Inspect the shared specialized execution path.
    pub fn vm_tier1_specialized_executor_snapshot(
        &self,
    ) -> crate::execution_backend::VmTier1SpecializedExecutorSnapshot {
        self.state.tier1_snapshot()
    }
    /// Resolve an immutable POU name for profiling/debug presentation.
    pub fn vm_pou_name(&self, id: u32) -> Option<String> {
        self.prepared().pou_name(id).map(str::to_owned)
    }
}
