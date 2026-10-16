//! Public state surface excludes internal storage and dispatcher context traits.
use super::{EngineData, EngineState, ExecutionServices, PreparedModule};
use crate::{
    error::RuntimeError,
    memory::VariableStorage,
    retain::RestartMode,
    task::TaskState,
    value::{Duration, Value},
};
use alloc::{boxed::Box, vec::Vec};

/// Single-owner execution of an immutable admitted application.
///
/// Runtime storage cannot be obtained mutably through the dispatcher's adapter
/// traits. Engineering writes use the typed methods on this state instead.
///
/// ```compile_fail,E0599
/// use trust_runtime_core::vm::RuntimeState;
/// fn bypass(state: &mut RuntimeState<'_>) {
///     let _ = state.storage_mut();
/// }
/// ```
///
#[cfg_attr(
    feature = "hir",
    doc = r#"
```compile_fail,E0277
use trust_runtime_core::vm::{RuntimeState, hosted::context::ReferenceContext};
fn bypass_admission(state: &mut RuntimeState<'_>) {
    let _ = ReferenceContext::storage_mut(state);
}
```
"#
)]
pub struct RuntimeState<'a> {
    data: Box<EngineData>,
    prepared: &'a PreparedModule,
    services: &'a dyn ExecutionServices,
}

impl PreparedModule {
    /// Construct a fresh resource using only the admitted artifact and shared engine.
    pub fn instantiate(&self, resource_index: usize) -> Result<RuntimeState<'_>, RuntimeError> {
        self.instantiate_with_services(resource_index, &super::services::LOGICAL_ONLY)
    }

    /// Construct with explicit physical-deadline, UTC and retain platform services.
    pub fn instantiate_with_services<'a>(
        &'a self,
        resource_index: usize,
        services: &'a dyn ExecutionServices,
    ) -> Result<RuntimeState<'a>, RuntimeError> {
        Ok(RuntimeState {
            data: EngineState::construct_data(self, resource_index, services)?,
            prepared: self,
            services,
        })
    }
}

impl RuntimeState<'_> {
    /// Capture current operational selection settings without copying plans or telemetry.
    #[cfg(feature = "register-ir")]
    pub fn register_configuration(&self) -> crate::vm::register::RegisterExecutionConfiguration {
        self.data.resources.register.configuration()
    }

    /// Test support: route calls to leaf POUs through the activation pump (`false`)
    /// or in place (`true`, the default), so parity tests can compare both.
    #[cfg(feature = "test-support")]
    pub fn set_in_place_calls_enabled(&mut self, enabled: bool) {
        self.data
            .resources
            .register
            .set_in_place_calls_enabled(enabled);
    }
    /// Test support: calls run in place since instantiation.
    #[cfg(feature = "test-support")]
    pub fn in_place_call_count(&self) -> u64 {
        self.data.resources.register.in_place_call_count()
    }

    /// Enable execution telemetry using keys already reserved during installation.
    #[cfg(feature = "register-ir")]
    pub fn set_register_profile_enabled(&mut self, enabled: bool) {
        self.data.resources.register.set_profile_enabled(enabled);
    }
    /// Read shared optimized-path observations without exposing mutable state.
    #[cfg(feature = "register-ir")]
    pub fn register_profile_snapshot(&self) -> crate::vm::register::VmRegisterProfileSnapshot {
        self.data.resources.register.profile_snapshot()
    }
    /// Clear counters without changing installed code or tier selection.
    #[cfg(feature = "register-ir")]
    pub fn reset_register_profile(&mut self) {
        self.data.resources.register.reset_profile();
    }
    /// Set the compatibility cache observation flag without changing the
    /// lifetime of installed code or enabling compilation during execution.
    #[cfg(feature = "register-ir")]
    pub fn set_register_lowering_enabled(&mut self, enabled: bool) {
        self.data.resources.register.set_lowering_enabled(enabled);
    }
    /// Observe prepared-plan and lookup counts.
    #[cfg(feature = "register-ir")]
    pub fn register_lowering_snapshot(
        &self,
    ) -> crate::vm::register::VmRegisterLoweringCacheSnapshot {
        self.data.resources.register.lowering_snapshot()
    }
    /// Reset observations without making RUN compile installed plans again.
    #[cfg(feature = "register-ir")]
    pub fn reset_register_lowering_cache(&mut self) {
        self.data.resources.register.reset_lowering();
    }
    /// Select already compiled specialized plans without lowering or allocation.
    #[cfg(feature = "register-ir")]
    pub fn set_tier1_enabled(&mut self, enabled: bool) {
        self.data.resources.register.set_tier1_enabled(enabled);
    }
    /// Read specialization preparation, selection and execution observations.
    #[cfg(feature = "register-ir")]
    pub fn tier1_snapshot(&self) -> crate::vm::register::VmTier1SpecializedExecutorSnapshot {
        self.data.resources.register.tier1_snapshot()
    }
    /// Clear active selection/hits while retaining prepared specialized code.
    #[cfg(feature = "register-ir")]
    pub fn reset_tier1(&mut self) {
        self.data.resources.register.reset_tier1();
    }

    fn engine(&mut self) -> EngineState<'_> {
        EngineState::borrow(&mut self.data, self.prepared, self.services)
    }

    fn engineering_operation<T>(
        &mut self,
        action: impl FnOnce(&mut EngineState<'_>) -> Result<T, RuntimeError>,
    ) -> Result<T, RuntimeError> {
        self.engine().engineering_operation(action)
    }

    /// Inspect variables without exposing an untyped mutation path.
    pub fn storage(&self) -> &VariableStorage {
        &self.data.storage
    }
    /// Inspect the installed date/time profile without modifying executable metadata.
    pub fn profile(&self) -> &crate::value::DateTimeProfile {
        &self.data.profile
    }
    /// Read the resource's current logical time.
    pub fn current_time(&self) -> Duration {
        self.data.now
    }
    /// Set simulation time explicitly, including an earlier instant, without
    /// changing task baselines or clearing missed-period/edge history.
    pub fn set_current_time(&mut self, time: Duration) {
        self.data.now = time;
    }
    /// Reset scheduler timing after a live pacing restart. Preserve the old
    /// hosted rule: registration baseline changes and event/overrun history clears.
    pub fn reset_task_timing(&mut self, time: Duration) {
        self.data.now = time;
        self.data.tasks.fill(TaskState::new(time));
    }
    /// Inspect the authoritative input image without permitting live resize.
    pub fn inputs(&self) -> &[u8] {
        &self.data.images.inputs
    }
    /// Borrow a declared hierarchical image value without cloning PLC state.
    pub fn read_hierarchical_image_ref(
        &self,
        address: &crate::io_address::IoAddress,
    ) -> Option<&Value> {
        super::io::read_hierarchical_image_ref(&self.data, self.prepared, address)
    }
    /// Read one checked process-image value between execution entries. Selected
    /// value/window cloning uses the existing engineering-operation allowance.
    pub fn read_image(
        &mut self,
        address: &crate::io_address::IoAddress,
    ) -> Result<Value, RuntimeError> {
        self.engineering_operation(|state| state.read_cycle_image(address))
    }
    /// Validate an engineering image write without changing image bytes. Used
    /// to qualify retained safe-output policies against a replacement candidate.
    pub fn validate_image_write(
        &mut self,
        address: &crate::io_address::IoAddress,
        value: Value,
    ) -> Result<(), RuntimeError> {
        self.engine()
            .engineering_operation(|state| state.validate_cycle_image_write(address, value))
    }

    /// Write one wire value through the shared address/window/codec checks.
    /// Rejection leaves the image and task readiness unchanged and does not
    /// latch a PLC fault. This does not resample bindings or execute a cycle.
    pub fn write_image(
        &mut self,
        address: &crate::io_address::IoAddress,
        value: Value,
    ) -> Result<(), RuntimeError> {
        self.engineering_operation(|state| state.write_cycle_image(address, value, None))
    }
    /// Sample platform inputs before the next cooperative cycle.
    pub fn inputs_mut(&mut self) -> &mut [u8] {
        &mut self.data.images.inputs
    }
    /// Borrow the last successfully published output image.
    pub fn outputs(&self) -> &[u8] {
        &self.data.images.outputs
    }
    /// Inspect the marker image without exposing unrestricted image mutation.
    pub fn memory_image(&self) -> &[u8] {
        &self.data.images.memory
    }
    /// Sample a declared hierarchical input through its admitted value codec.
    pub fn set_hierarchical_input(
        &mut self,
        address: &crate::io_address::IoAddress,
        value: Value,
    ) -> Result<(), RuntimeError> {
        self.engineering_operation(|state| state.set_hierarchical_input(address, value))
    }
    /// Observe the last published value of an admitted hierarchical output binding.
    pub fn read_hierarchical_output(
        &self,
        address: &crate::io_address::IoAddress,
    ) -> Option<&Value> {
        super::io::read_hierarchical_output(&self.data, self.prepared, address)
    }
    /// Inspect the latched execution fault, if any.
    pub fn fault(&self) -> Option<&RuntimeError> {
        self.data.fault.as_ref()
    }

    /// Record a platform failure in the common execution fault latch.
    pub fn latch_fault(&mut self, error: RuntimeError) {
        self.data.fault = Some(error);
    }
    /// Clear the existing tooling fault latch without replacing executable state.
    pub fn clear_fault(&mut self) {
        self.data.fault = None;
    }
    /// Capture an explicitly requested retained checkpoint through common policy.
    pub fn retain_snapshot(&mut self) -> Result<crate::retain::RetainSnapshot, RuntimeError> {
        self.engineering_operation(|state| state.cycle_retain_snapshot())
    }
    /// Inspect nominal deadlines and missed-period counters.
    pub fn task_states(&self) -> &[TaskState] {
        &self.data.tasks
    }
    /// Execute one resource cycle using an explicitly sampled logical time.
    pub fn execute_cycle(&mut self, now: Duration) -> Result<(), RuntimeError> {
        self.engine().execute_cycle(now)
    }
    /// Borrow selected mutable host adapters for the existing shared cycle phases.
    /// Adapter failures latch an execution fault; rejected checked requests must
    /// be reported by the adapter without turning them into a cycle error.
    pub fn execute_cycle_with_hooks(
        &mut self,
        now: Duration,
        hooks: &mut dyn super::CycleHooks,
    ) -> Result<(), RuntimeError> {
        self.engine().execute_cycle_with_hooks(now, hooks)
    }
    /// Apply a complete retained snapshot atomically through admitted types and recipes.
    pub fn apply_retain_snapshot(
        &mut self,
        snapshot: &crate::retain::RetainSnapshot,
    ) -> Result<Vec<super::RetainApplyEvent>, RuntimeError> {
        self.engine()
            .engineering_operation(|state| state.apply_retained_snapshot(snapshot))
    }

    /// Stage warm/cold reconstruction and retained restoration before replacing state.
    pub fn restart_with_retain_snapshot(
        &mut self,
        mode: RestartMode,
        snapshot: &crate::retain::RetainSnapshot,
    ) -> Result<Vec<super::RetainApplyEvent>, RuntimeError> {
        self.engine().restart_with_snapshot(mode, Some(snapshot))
    }

    /// Reconstruct at the current logical time, publishing only a complete candidate.
    pub fn restart(&mut self, mode: RestartMode) -> Result<(), RuntimeError> {
        self.engine().restart(mode)
    }
    /// Read a declared access alias, including its partial selection.
    pub fn read_access(&mut self, name: &str) -> Result<Value, RuntimeError> {
        self.engineering_operation(|state| {
            let value = state.read_access(name)?;
            state.check_entry_deadline()?;
            Ok(value)
        })
    }
    /// Write a writable access alias using admitted type and lifetime checks.
    pub fn write_access(&mut self, name: &str, value: Value) -> Result<(), RuntimeError> {
        self.engineering_operation(|state| state.write_access(name, value))
    }
    /// Write a mutable resource-global variable between cycles.
    pub fn write_global(&mut self, name: &str, value: Value) -> Result<(), RuntimeError> {
        self.engineering_operation(|state| state.write_global(name, value))
    }

    /// Validate a named engineering write without changing its destination.
    pub fn validate_global_write(&mut self, name: &str, value: Value) -> Result<(), RuntimeError> {
        self.engineering_operation(|state| state.validate_global_write(name, value))
    }

    /// Write an admitted named global or program member through a checked path.
    pub fn write_named_path(
        &mut self,
        name: &str,
        path: &[crate::value::RefSegment],
        value: Value,
    ) -> Result<(), RuntimeError> {
        self.engineering_operation(|state| state.write_named_path(name, path, value))
    }
    /// Read a currently bound reference through the shared checked operation.
    pub fn read_reference(
        &mut self,
        reference: &crate::value::ValueRef,
    ) -> Result<Value, RuntimeError> {
        self.engineering_operation(|state| state.read_checked_reference(reference))
    }
    /// Commit a checked reference write between cycles. Installation/schema
    /// replacement must re-resolve external bindings against the current module.
    pub fn write_reference(
        &mut self,
        reference: crate::value::ValueRefView<'_>,
        value: Value,
    ) -> Result<(), RuntimeError> {
        self.engineering_operation(|state| state.write_checked_reference(reference, value))
    }

    /// Resolve a declared alias against this installed controller's live identities.
    pub fn resolve_access_reference(
        &mut self,
        name: &str,
    ) -> Result<crate::value::ValueRef, RuntimeError> {
        self.engineering_operation(|state| state.resolve_access_reference(name))
    }

    /// Resolve a persistent artifact reference to this installation's live roots.
    pub fn resolve_persistent_reference(
        &mut self,
        index: u32,
    ) -> Result<crate::value::ValueRef, RuntimeError> {
        self.engineering_operation(|state| {
            let reference = state.persistent_io_reference(index)?;
            state.check_read(reference.as_view())?;
            state.check_entry_deadline()?;
            Ok(reference)
        })
    }

    /// Execute an admitted program explicitly through the shared POU entry.
    pub fn execute_program(&mut self, name: &str) -> Result<(), RuntimeError> {
        if self.data.fault.is_some() {
            return Err(RuntimeError::ResourceFaulted);
        }
        self.engineering_operation(|state| state.execute_named_program(name))
    }

    /// Construct and execute one admitted function-block template, retiring its
    /// temporary instance graph on success or failure within the same budget.
    pub fn execute_function_block_by_name(&mut self, name: &str) -> Result<(), RuntimeError> {
        if self.data.fault.is_some() {
            return Err(RuntimeError::ResourceFaulted);
        }
        self.engineering_operation(|state| state.execute_temporary_function_block(name))
    }

    /// Execute a currently live, already constructed function-block destination.
    pub fn execute_function_block(
        &mut self,
        reference: &crate::value::ValueRef,
    ) -> Result<(), RuntimeError> {
        if self.data.fault.is_some() {
            return Err(RuntimeError::ResourceFaulted);
        }
        self.engineering_operation(|state| state.execute_bound_function_block(reference))
    }
}

#[cfg(test)]
mod budget_tests {
    use super::*;

    #[test]
    fn engineering_entry_does_not_inherit_exhausted_scan_fuel() {
        let prepared = PreparedModule::from_bytes(
            include_bytes!(
                "../../../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc"
            ),
            crate::vm::PreparationLimits::default(),
        )
        .unwrap();
        let mut state = prepared.instantiate(0).unwrap();
        state.data.resources.work_budget.reset(0);
        state
            .engineering_operation(|engine| engine.charge_work_units(1))
            .unwrap();
        assert_eq!(
            state.data.resources.work_budget.remaining(),
            prepared.limits.max_work - 1
        );
        state.data.resources.work_budget.reset(0);
        state
            .engineering_operation(|engine| engine.charge_work_units(2))
            .unwrap();
        assert_eq!(
            state.data.resources.work_budget.remaining(),
            prepared.limits.max_work - 2
        );
    }

    #[test]
    fn owner_reference_calls_start_an_allowance_unlike_cumulative_cycle_callbacks() {
        let prepared = PreparedModule::from_bytes(
            include_bytes!("../../../../trust-runtime/tests/fixtures/portability/f401/gpio.stbc"),
            crate::vm::PreparationLimits::default(),
        )
        .unwrap();
        let mut state = prepared.instantiate(0).unwrap();
        let reference = state.storage().ref_for_global("probe_depth").unwrap();
        state.data.resources.work_budget.reset(0);
        state
            .write_reference(reference.as_view(), Value::Int(2))
            .unwrap();
        // TIME-08 VERIFY: a scalar root write executes no instruction and its lookup,
        // writability, normalization and scalar lifetime work are bookkeeping, so it
        // consumes the whole fresh engineering allowance.
        let first = state.data.resources.work_budget.remaining();
        assert_eq!(first, prepared.limits.max_work);
        state
            .write_reference(reference.as_view(), Value::Int(3))
            .unwrap();
        assert_eq!(
            state.data.resources.work_budget.remaining(),
            first,
            "each owner-local call starts its engineering allowance"
        );
        assert_eq!(state.read_reference(&reference).unwrap(), Value::Int(3));
        assert!(state.fault().is_none());
    }

    #[test]
    fn owner_image_write_starts_existing_engineering_allowance_without_resetting_scheduler() {
        let prepared = PreparedModule::from_bytes(
            include_bytes!("../../../../trust-runtime/tests/fixtures/portability/f401/gpio.stbc"),
            crate::vm::PreparationLimits::default(),
        )
        .unwrap();
        let mut state = prepared.instantiate(0).unwrap();
        let address = crate::io_address::IoAddress::parse("%QX0.0").unwrap();
        state.data.resources.work_budget.reset(0);
        state.write_image(&address, Value::Bool(true)).unwrap();
        // TIME-08 VERIFY: process-image sampling and publication are bookkeeping and a
        // scalar copy or lifetime check charges nothing, so the fresh allowance is intact.
        let remaining = state.data.resources.work_budget.remaining();
        assert_eq!(remaining, prepared.limits.max_work);
        state.write_image(&address, Value::Bool(false)).unwrap();
        assert_eq!(state.data.resources.work_budget.remaining(), remaining);
        assert_eq!(state.task_states()[0].last_run, Duration::ZERO);
        assert!(state.fault().is_none());
    }

    #[test]
    fn explicit_pacing_reset_clears_existing_event_history_instead_of_reseeding_inputs() {
        let prepared = PreparedModule::from_bytes(
            include_bytes!("../../../../trust-runtime/tests/fixtures/portability/f401/gpio.stbc"),
            crate::vm::PreparationLimits::default(),
        )
        .unwrap();
        let mut state = prepared.instantiate(0).unwrap();
        state.data.tasks[0].last_single = true;
        state.data.tasks[0].overrun_count = 17;
        state.reset_task_timing(Duration::from_millis(100));
        assert!(!state.task_states()[0].last_single);
        assert_eq!(state.task_states()[0].overrun_count, 0);
        assert_eq!(state.task_states()[0].last_run, Duration::from_millis(100));
    }
}
