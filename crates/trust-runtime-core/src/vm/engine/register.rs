//! Instance-owned optional optimization scratch and prepared telemetry.
use super::{EngineState, RuntimeError, Vec};
use crate::vm::register_ir::{
    PreparedRegisterProgram, RegisterContext, RegisterExecutionBuffers, RegisterLoweringCacheState,
    RegisterProfileState, RegisterTier1SpecializedExecutorState,
};
use alloc::sync::Arc;

pub(super) struct RegisterExecutionState {
    profile: RegisterProfileState,
    lowering: RegisterLoweringCacheState,
    tier1: RegisterTier1SpecializedExecutorState,
    buffers: Vec<(u32, Option<RegisterExecutionBuffers>)>,
    /// Lets parity tests route leaf calls through the pump and count in-place calls.
    #[cfg(feature = "test-support")]
    in_place_calls: (bool, u64),
}

impl RegisterExecutionState {
    pub(super) fn recycle_result_locals(
        &mut self,
        pou_id: u32,
        locals: Vec<crate::value::Value>,
    ) -> Result<(), RuntimeError> {
        let Ok(index) = self.buffers.binary_search_by_key(&pou_id, |(id, _)| *id) else {
            return Ok(());
        };
        let Some(buffers) = self.buffers[index].1.as_mut() else {
            return Ok(());
        };
        buffers.recycle_result_locals(locals)
    }

    pub(super) fn set_profile_enabled(&mut self, enabled: bool) {
        self.profile.set_enabled(enabled);
    }
    pub(super) fn profile_snapshot(&self) -> crate::vm::register_ir::VmRegisterProfileSnapshot {
        self.profile.snapshot()
    }
    pub(super) fn reset_profile(&mut self) {
        self.profile.reset();
    }
    pub(super) fn set_lowering_enabled(&mut self, enabled: bool) {
        self.lowering.set_enabled(enabled);
    }
    pub(super) fn lowering_snapshot(
        &self,
    ) -> crate::vm::register_ir::VmRegisterLoweringCacheSnapshot {
        self.lowering.snapshot()
    }
    pub(super) fn reset_lowering(&mut self) {
        self.lowering.reset();
    }
    pub(super) fn set_tier1_enabled(&mut self, enabled: bool) {
        self.tier1.set_enabled(enabled);
    }
    pub(super) fn tier1_snapshot(
        &self,
    ) -> crate::vm::register_ir::VmTier1SpecializedExecutorSnapshot {
        self.tier1.snapshot()
    }
    pub(super) fn reset_tier1(&mut self) {
        self.tier1.reset();
    }

    pub(super) fn record_call_op(&mut self, kind: crate::vm::call::context::RegisterCallOpKind) {
        self.profile.record_call_op(kind);
    }
    pub(super) fn record_value_op(&mut self, kind: crate::vm::call::context::RegisterValueOpKind) {
        self.profile.record_value_op(kind);
    }

    pub(super) fn configuration(&self) -> crate::vm::register_ir::RegisterExecutionConfiguration {
        let lowering = self.lowering.snapshot();
        crate::vm::register_ir::RegisterExecutionConfiguration {
            profile_enabled: self.profile.is_enabled(),
            lowering_enabled: lowering.enabled,
            lowering_capacity: lowering.cache_capacity,
            tier1_enabled: self.tier1.enabled(),
            tier1_hot_threshold: self.tier1.hot_block_threshold,
            tier1_capacity: self.tier1.cache_capacity,
        }
    }

    pub(super) fn new(
        configuration: crate::vm::register_ir::RegisterExecutionConfiguration,
    ) -> Self {
        let mut profile = RegisterProfileState::default();
        profile.set_enabled(configuration.profile_enabled);
        Self {
            profile,
            lowering: configuration.lowering_state(),
            tier1: configuration.tier1_state(),
            buffers: Vec::new(),
            #[cfg(feature = "test-support")]
            in_place_calls: (true, 0),
        }
    }

    #[cfg(feature = "test-support")]
    pub(super) fn set_in_place_calls_enabled(&mut self, enabled: bool) {
        self.in_place_calls.0 = enabled;
    }
    #[cfg(feature = "test-support")]
    pub(super) fn in_place_call_count(&self) -> u64 {
        self.in_place_calls.1
    }
}

impl EngineState<'_> {
    pub(super) fn prepare_register_state(&mut self) -> Result<(), RuntimeError> {
        let plans = &self.prepared.register_plans;
        // Scratch reservation work is bookkeeping (TIME-08); only bytes are charged.
        let (telemetry_bytes, _) = plans.telemetry_demand()?;
        self.charge_allocation_bytes(telemetry_bytes)?;
        let buffers = plans.reserve_execution_buffers(&self.prepared.vm, |bytes, _| {
            self.charge_allocation_bytes(bytes)
        })?;
        let state = &mut self.data.resources.register;
        state.profile.prepare(plans);
        state.lowering.prepare(plans);
        state.tier1.prepare(plans)?;
        state.buffers = buffers;
        Ok(())
    }
}

impl RegisterContext for EngineState<'_> {
    fn register_profile(&self) -> &RegisterProfileState {
        &self.data.resources.register.profile
    }
    fn register_profile_mut(&mut self) -> &mut RegisterProfileState {
        &mut self.data.resources.register.profile
    }
    fn tier1_state(&self) -> &RegisterTier1SpecializedExecutorState {
        &self.data.resources.register.tier1
    }
    fn tier1_state_mut(&mut self) -> &mut RegisterTier1SpecializedExecutorState {
        &mut self.data.resources.register.tier1
    }
    fn prepared_register_plan(&self, id: u32) -> Option<Arc<PreparedRegisterProgram>> {
        self.prepared.register_plans.get(id)
    }
    fn register_leaf_plan(&self, id: u32) -> bool {
        #[cfg(feature = "test-support")]
        if !self.data.resources.register.in_place_calls.0 {
            return false;
        }
        self.prepared.register_plans.is_leaf(id)
    }
    #[cfg(feature = "test-support")]
    fn record_in_place_call(&mut self) {
        let count = &mut self.data.resources.register.in_place_calls.1;
        *count = count.saturating_add(1);
    }
    fn record_register_plan_lookup(&mut self, found: bool) {
        self.data.resources.register.lowering.record_lookup(found);
    }
    fn register_buffers_available(&self, pou_id: u32) -> bool {
        self.data
            .resources
            .register
            .buffers
            .binary_search_by_key(&pou_id, |(id, _)| *id)
            .ok()
            .and_then(|index| self.data.resources.register.buffers.get(index))
            .is_some_and(|(_, buffers)| {
                buffers
                    .as_ref()
                    .is_some_and(|buffers| buffers.can_enter_with_frame_payload())
            })
    }
    fn debug_active(&self) -> bool {
        self.services.debug_active()
    }
    fn take_register_buffers(
        &mut self,
        pou_id: u32,
        count: usize,
    ) -> Result<RegisterExecutionBuffers, RuntimeError> {
        #[cfg(all(feature = "std", feature = "test-support"))]
        let _timing = crate::vm::diagnostic_timing::begin(
            crate::vm::diagnostic_timing::Phase::RegisterBufferLifecycle,
        );
        let index = self
            .data
            .resources
            .register
            .buffers
            .binary_search_by_key(&pou_id, |(id, _)| *id)
            .map_err(|_| RuntimeError::InvalidExecutionState)?;
        let slot = &mut self.data.resources.register.buffers[index].1;
        let Some(mut buffers) = slot.take() else {
            return Err(RuntimeError::InvalidExecutionState);
        };
        if let Err(error) = buffers.select(count) {
            *slot = Some(buffers);
            return Err(error);
        }
        Ok(buffers)
    }
    fn recycle_register_buffers(
        &mut self,
        pou_id: u32,
        mut buffers: RegisterExecutionBuffers,
    ) -> Result<(), RuntimeError> {
        #[cfg(all(feature = "std", feature = "test-support"))]
        let _timing = crate::vm::diagnostic_timing::begin(
            crate::vm::diagnostic_timing::Phase::RegisterBufferLifecycle,
        );
        let index = self
            .data
            .resources
            .register
            .buffers
            .binary_search_by_key(&pou_id, |(id, _)| *id)
            .map_err(|_| RuntimeError::InvalidExecutionState)?;
        let slot = &mut self.data.resources.register.buffers[index].1;
        if slot.is_some() {
            return Err(RuntimeError::InvalidExecutionState);
        }
        buffers.clear();
        *slot = Some(buffers);
        Ok(())
    }
}

#[cfg(test)]
mod call_path_observation_reset_tests;
#[cfg(test)]
mod call_path_scratch_tests;

#[cfg(test)]
mod environment_tests {
    use super::*;
    use crate::value::DateTimeValue;
    use crate::vm::{
        register_ir::RegisterExecutionConfiguration, ExecutionServices, PreparationLimits,
        PreparedModule,
    };

    struct Configured(RegisterExecutionConfiguration);
    impl ExecutionServices for Configured {
        fn register_configuration(&self) -> RegisterExecutionConfiguration {
            self.0
        }
        fn deadline_exceeded(&self) -> bool {
            false
        }
        fn has_wall_clock(&self) -> bool {
            false
        }
        fn current_dt(&self) -> Result<DateTimeValue, RuntimeError> {
            Err(RuntimeError::UndefinedFunction("CURRENT_DT".into()))
        }
    }
    const FIXTURE: &[u8] = include_bytes!(
        "../../../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc"
    );

    #[test]
    fn capacities_do_not_expand_reserved_plan_storage_and_limits_still_apply() {
        let prepared = PreparedModule::from_bytes(FIXTURE, PreparationLimits::default()).unwrap();
        let small = Configured(RegisterExecutionConfiguration {
            tier1_enabled: true,
            tier1_capacity: 1,
            lowering_capacity: 1,
            tier1_hot_threshold: 0,
            ..Default::default()
        });
        let huge = Configured(RegisterExecutionConfiguration {
            tier1_capacity: usize::MAX,
            lowering_capacity: usize::MAX,
            ..small.0
        });
        let a = EngineState::construct_data(&prepared, 0, &small).unwrap();
        let b = EngineState::construct_data(&prepared, 0, &huge).unwrap();
        assert_eq!(
            a.resources.construction_bytes.get(),
            b.resources.construction_bytes.get(),
            "capacity settings must not allocate capacity-sized plan arrays"
        );
        assert_eq!(b.resources.register.configuration(), huge.0);
        let limits = PreparationLimits {
            max_construction_bytes: a.resources.construction_bytes.get() - 1,
            ..PreparationLimits::default()
        };
        let constrained = PreparedModule::from_bytes(FIXTURE, limits).unwrap();
        assert!(matches!(
            EngineState::construct_data(&constrained, 0, &huge),
            Err(RuntimeError::ExecutionLimit {
                kind: crate::error::ExecutionLimitKind::AllocationBytes,
                ..
            })
        ));
    }

    #[test]
    fn prepared_pou_scratch_is_independent_and_failed_selection_restores_its_slot() {
        let prepared = PreparedModule::from_bytes(FIXTURE, PreparationLimits::default()).unwrap();
        let services = Configured(Default::default());
        let mut data = EngineState::construct_data(&prepared, 0, &services).unwrap();
        let ids = data
            .resources
            .register
            .buffers
            .iter()
            .map(|(id, _)| *id)
            .collect::<Vec<_>>();
        assert!(
            ids.len() >= 2,
            "fixture must contain distinct eligible POUs"
        );
        let mut state = EngineState::borrow(&mut data, &prepared, &services);
        let first = ids[0];
        let second = ids[1];
        assert!(state.register_buffers_available(first));
        assert!(matches!(
            state.take_register_buffers(first, usize::MAX),
            Err(RuntimeError::PreparationLimit)
        ));
        assert!(
            state.register_buffers_available(first),
            "failed selection must restore the reservation"
        );
        let first_buffers = state.take_register_buffers(first, 0).unwrap();
        assert!(!state.register_buffers_available(first));
        assert!(
            state.register_buffers_available(second),
            "independent nested POU keeps its scratch"
        );
        assert!(matches!(
            state.take_register_buffers(first, 0),
            Err(RuntimeError::InvalidExecutionState)
        ));
        let second_buffers = state.take_register_buffers(second, 0).unwrap();
        state
            .recycle_register_buffers(first, first_buffers)
            .unwrap();
        state
            .recycle_register_buffers(second, second_buffers)
            .unwrap();
        assert!(state.register_buffers_available(first));
        assert!(state.register_buffers_available(second));
        assert_eq!(
            state.recycle_register_buffers(first, RegisterExecutionBuffers::default()),
            Err(RuntimeError::InvalidExecutionState)
        );
        assert_eq!(
            state.recycle_register_buffers(u32::MAX, RegisterExecutionBuffers::default()),
            Err(RuntimeError::InvalidExecutionState)
        );
        assert!(
            state.register_buffers_available(first),
            "invalid returns cannot overwrite the prepared reservation"
        );
    }

    #[test]
    fn ordinary_restart_preserves_operational_configuration_and_observations() {
        let prepared = PreparedModule::from_bytes(FIXTURE, PreparationLimits::default()).unwrap();
        let preparation = prepared.register_preparation_snapshot();
        assert!(preparation.lowered_pous > 0);
        assert_eq!(
            preparation.pou_count,
            preparation.lowered_pous + preparation.lowering_failures
        );
        let services = Configured(RegisterExecutionConfiguration {
            profile_enabled: true,
            tier1_enabled: true,
            tier1_hot_threshold: 0,
            tier1_capacity: 1,
            ..Default::default()
        });
        let mut data = EngineState::construct_data(&prepared, 0, &services).unwrap();
        let mut state = EngineState::borrow(&mut data, &prepared, &services);
        state
            .execute_cycle(crate::value::Duration::from_millis(30))
            .unwrap();
        let profile = state.data.resources.register.profile_snapshot();
        let tier = state.data.resources.register.tier1_snapshot();
        let configuration = state.data.resources.register.configuration();
        state.restart(crate::retain::RestartMode::Cold).unwrap();
        assert_eq!(state.data.resources.register.profile_snapshot(), profile);
        assert_eq!(state.data.resources.register.tier1_snapshot(), tier);
        assert_eq!(state.data.resources.register.configuration(), configuration);
        state.data.resources.register.reset_profile();
        assert_eq!(state.data.resources.register.tier1_snapshot(), tier);
        state.data.resources.register.reset_tier1();
        state.data.resources.register.reset_lowering();
        assert_eq!(prepared.register_preparation_snapshot(), preparation);
        assert_eq!(
            state
                .data
                .resources
                .register
                .tier1_snapshot()
                .compile_attempts,
            0
        );
        assert_eq!(state.data.resources.register.lowering_snapshot().misses, 0);
        assert_eq!(state.data.resources.register.configuration(), configuration);
        assert_eq!(
            state.data.resources.register.tier1_snapshot().cached_blocks,
            0
        );
    }
}
