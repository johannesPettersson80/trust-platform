//! Immutable selection settings, separate from prepared code and mutable telemetry.
use super::{RegisterLoweringCacheState, RegisterTier1SpecializedExecutorState};
use alloc::string::String;

/// Canonical hosted register configuration, captured before state reservation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegisterExecutionConfiguration {
    /// Explicit telemetry setting; no environment variable is invented for it.
    pub profile_enabled: bool,
    /// Compatibility lowering-cache observation setting; preparation stays eager.
    pub lowering_enabled: bool,
    /// Historical lowering-cache capacity observation, never a RUN compile limit.
    pub lowering_capacity: usize,
    /// Whether the precompiled specialized tier is eligible for execution.
    pub tier1_enabled: bool,
    /// Number of block hits before first activation of a prepared specialized plan.
    pub tier1_hot_threshold: u64,
    /// Maximum simultaneously selected specialized blocks in the fixed LRU.
    pub tier1_capacity: usize,
}
impl Default for RegisterExecutionConfiguration {
    fn default() -> Self {
        Self::from_states(
            RegisterLoweringCacheState::default(),
            RegisterTier1SpecializedExecutorState::default(),
        )
    }
}
impl RegisterExecutionConfiguration {
    /// Parse the established TRUST_VM_* controls through their canonical parsers.
    /// Invalid values keep historical defaults; capacities clamp to at least one.
    pub fn from_environment(mut read: impl FnMut(&str) -> Option<String>) -> Self {
        Self::from_states(
            RegisterLoweringCacheState::from_environment(&mut read),
            RegisterTier1SpecializedExecutorState::from_environment(&mut read),
        )
    }
    fn from_states(
        lowering: RegisterLoweringCacheState,
        tier1: RegisterTier1SpecializedExecutorState,
    ) -> Self {
        let lowering = lowering.snapshot();
        Self {
            profile_enabled: false,
            lowering_enabled: lowering.enabled,
            lowering_capacity: lowering.cache_capacity,
            tier1_enabled: tier1.enabled(),
            tier1_hot_threshold: tier1.hot_block_threshold,
            tier1_capacity: tier1.cache_capacity,
        }
    }
    pub(crate) fn lowering_state(self) -> RegisterLoweringCacheState {
        RegisterLoweringCacheState::with_configuration(
            self.lowering_enabled,
            self.lowering_capacity,
        )
    }
    pub(crate) fn tier1_state(self) -> RegisterTier1SpecializedExecutorState {
        let mut state = RegisterTier1SpecializedExecutorState::default();
        state.set_enabled(self.tier1_enabled);
        state.hot_block_threshold = self.tier1_hot_threshold;
        state.cache_capacity = self.tier1_capacity.max(1);
        state
    }
}
