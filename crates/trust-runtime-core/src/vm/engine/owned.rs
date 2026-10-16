//! Hosted ownership of admitted metadata and mutable data, with scoped execution borrows.
use super::{EngineData, EngineState, ExecutionServices, LogicalExecutionServices, PreparedModule};
use crate::{
    error::RuntimeError,
    io_address::IoAddress,
    memory::VariableStorage,
    retain::RestartMode,
    task::TaskState,
    value::{Duration, Value},
};
use alloc::{boxed::Box, sync::Arc};

/// An owned controller using the same execution context as the borrowed MCU facade.
///
/// Services are owned by the composition. Their normal Rust bounds determine
/// whether the controller may move between threads; MCU services gain no new
/// `Send` or `Sync` requirement. No execution reference points into this object.
///
/// ```
/// use std::sync::Arc;
/// use trust_runtime_core::{
///     error::RuntimeError,
///     value::{RefSegment, Value},
///     vm::{LogicalExecutionServices, OwnedRuntimeState, PreparationLimits, PreparedModule},
/// };
/// fn install(bytes: &[u8]) -> Result<OwnedRuntimeState<LogicalExecutionServices>, RuntimeError> {
///     Arc::new(PreparedModule::from_bytes(bytes, PreparationLimits::default())?)
///         .instantiate_owned(0)
/// }
/// fn write_enabled(state: &mut OwnedRuntimeState<LogicalExecutionServices>) -> Result<(), RuntimeError> {
///     state.write_named_path("Plant", &[RefSegment::Field("enabled".into())], Value::Bool(true))
/// }
/// ```
///
/// ```compile_fail,E0599
/// use trust_runtime_core::vm::{LogicalExecutionServices, OwnedRuntimeState};
/// fn bypass(state: &mut OwnedRuntimeState<LogicalExecutionServices>) {
///     let _ = state.storage_mut();
/// }
/// ```
pub struct OwnedRuntimeState<S: ExecutionServices> {
    pub(super) data: Box<EngineData>,
    pub(super) prepared: Arc<PreparedModule>,
    pub(super) services: S,
}

impl PreparedModule {
    /// Instantiate an owned logical-time controller without retaining a caller borrow.
    pub fn instantiate_owned(
        self: Arc<Self>,
        resource_index: usize,
    ) -> Result<OwnedRuntimeState<LogicalExecutionServices>, RuntimeError> {
        self.instantiate_owned_with_services(resource_index, LogicalExecutionServices)
    }

    /// Instantiate with an owned host service composition after capability checks.
    pub fn instantiate_owned_with_services<S: ExecutionServices>(
        self: Arc<Self>,
        resource_index: usize,
        services: S,
    ) -> Result<OwnedRuntimeState<S>, RuntimeError> {
        let data = EngineState::construct_data(&self, resource_index, &services)?;
        Ok(OwnedRuntimeState {
            data,
            prepared: self,
            services,
        })
    }
}

impl<S: ExecutionServices> OwnedRuntimeState<S> {
    /// Borrow composition-owned platform services without exposing PLC mutation.
    /// Services may maintain their existing deadline/debug controls; changes to
    /// executable metadata and values still use admission and checked owner APIs.
    pub fn services(&self) -> &S {
        &self.services
    }
    /// Reconfigure composition-owned service controls between owner operations.
    /// The exclusive borrow prevents changes during execution and does not
    /// grant mutable access to PLC values, admitted types or executable metadata.
    pub fn services_mut(&mut self) -> &mut S {
        &mut self.services
    }
    /// Capture current operational selection settings without copying plans or telemetry.
    #[cfg(feature = "register-ir")]
    pub fn register_configuration(&self) -> crate::vm::register::RegisterExecutionConfiguration {
        self.data.resources.register.configuration()
    }

    /// Enable execution telemetry using pre-reserved observation keys.
    #[cfg(feature = "register-ir")]
    pub fn set_register_profile_enabled(&mut self, enabled: bool) {
        self.data.resources.register.set_profile_enabled(enabled);
    }
    /// Observe the shared register execution path.
    #[cfg(feature = "register-ir")]
    pub fn register_profile_snapshot(&self) -> crate::vm::register::VmRegisterProfileSnapshot {
        self.data.resources.register.profile_snapshot()
    }
    /// Clear execution counters without changing prepared plans or tier selection.
    #[cfg(feature = "register-ir")]
    pub fn reset_register_profile(&mut self) {
        self.data.resources.register.reset_profile();
    }
    /// Set the compatibility lowering-cache observation flag. Prepared code
    /// remains installed; disabling this flag cannot authorize RUN recompilation.
    #[cfg(feature = "register-ir")]
    pub fn set_register_lowering_enabled(&mut self, enabled: bool) {
        self.data.resources.register.set_lowering_enabled(enabled);
    }
    /// Observe preparation and lookup counts, distinct from lazy cache occupancy.
    #[cfg(feature = "register-ir")]
    pub fn register_lowering_snapshot(
        &self,
    ) -> crate::vm::register::VmRegisterLoweringCacheSnapshot {
        self.data.resources.register.lowering_snapshot()
    }
    /// Clear lookup/preparation observations, retaining all installed plans.
    #[cfg(feature = "register-ir")]
    pub fn reset_register_lowering_cache(&mut self) {
        self.data.resources.register.reset_lowering();
    }
    /// Select prepared specialized plans without compiling inside RUN.
    #[cfg(feature = "register-ir")]
    pub fn set_tier1_enabled(&mut self, enabled: bool) {
        self.data.resources.register.set_tier1_enabled(enabled);
    }
    /// Observe specialization preparation and execution.
    #[cfg(feature = "register-ir")]
    pub fn tier1_snapshot(&self) -> crate::vm::register::VmTier1SpecializedExecutorSnapshot {
        self.data.resources.register.tier1_snapshot()
    }
    /// Clear active selections and hot counters; subsequent selection warms up
    /// against immutable code already prepared before installation.
    #[cfg(feature = "register-ir")]
    pub fn reset_tier1(&mut self) {
        self.data.resources.register.reset_tier1();
    }

    fn engine(&mut self) -> EngineState<'_> {
        EngineState::borrow(&mut self.data, &self.prepared, &self.services)
    }

    /// Inspect the immutable module admitted for this controller.
    pub fn prepared(&self) -> &PreparedModule {
        &self.prepared
    }

    /// Pin the immutable admitted module while assembling hosted metadata or a
    /// candidate. This shares only executable metadata, never mutable PLC state.
    pub fn shared_prepared(&self) -> Arc<PreparedModule> {
        Arc::clone(&self.prepared)
    }

    /// Inspect variables without an unrestricted mutable storage escape.
    pub fn storage(&self) -> &VariableStorage {
        &self.data.storage
    }
    /// Inspect the installed date/time profile without mutating the type/schema.
    pub fn profile(&self) -> &crate::value::DateTimeProfile {
        &self.data.profile
    }
    /// Read the resource's sampled logical time.
    pub fn current_time(&self) -> Duration {
        self.data.now
    }
    /// Set simulation time independently of nominal deadlines and edge/overrun history.
    pub fn set_current_time(&mut self, time: Duration) {
        self.data.now = time;
    }
    /// Reset the pacing restart baseline and clear edge/overrun task history.
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
        super::io::read_hierarchical_image_ref(&self.data, &self.prepared, address)
    }
    /// Read one checked process-image value between execution entries. Selected
    /// value/window cloning uses the existing engineering-operation allowance.
    pub fn read_image(
        &mut self,
        address: &crate::io_address::IoAddress,
    ) -> Result<Value, RuntimeError> {
        self.engine()
            .engineering_operation(|state| state.read_cycle_image(address))
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
        self.engine()
            .engineering_operation(|state| state.write_cycle_image(address, value, None))
    }
    /// Sample the platform input image between cycles.
    pub fn inputs_mut(&mut self) -> &mut [u8] {
        &mut self.data.images.inputs
    }

    /// Inspect the last successfully published output image.
    pub fn outputs(&self) -> &[u8] {
        &self.data.images.outputs
    }
    /// Inspect marker image effects under the selected platform's cycle policy.
    pub fn memory_image(&self) -> &[u8] {
        &self.data.images.memory
    }

    /// Inspect the latched PLC execution fault.
    pub fn fault(&self) -> Option<&RuntimeError> {
        self.data.fault.as_ref()
    }

    /// Latch a platform/supervision failure in the same owner as execution
    /// faults. No PLC values, bindings or executable metadata change here.
    pub fn latch_fault(&mut self, error: RuntimeError) {
        self.data.fault = Some(error);
    }

    /// Clear only the fault latch through the existing owner tooling action.
    /// Task history, installed code and PLC values remain unchanged.
    pub fn clear_fault(&mut self) {
        self.data.fault = None;
    }

    /// Capture an explicit persistence checkpoint using the same indexed
    /// retained declarations and value policy as an in-cycle checkpoint.
    pub fn retain_snapshot(&mut self) -> Result<crate::retain::RetainSnapshot, RuntimeError> {
        self.engine()
            .engineering_operation(|state| state.cycle_retain_snapshot())
    }

    /// Inspect nominal task deadlines and missed activation counts.
    pub fn task_states(&self) -> &[TaskState] {
        &self.data.tasks
    }

    /// Execute the common cooperative cycle at the supplied logical time.
    pub fn execute_cycle(&mut self, now: Duration) -> Result<(), RuntimeError> {
        self.engine().execute_cycle(now)
    }
    /// Run the common scheduler with mutable host adapters borrowed for this cycle.
    /// No adapter owns a second PLC store or obtains an execution/mutation bypass.
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

    /// Prepare a distinct installation at a quiescent owner boundary. Live
    /// retained object graphs require congruent admitted identities; serializable
    /// checkpoints migrate by name when the schema changes. The old owner is
    /// unchanged if any preparation, initializer or checkpoint fails.
    pub fn prepare_replacement(
        &mut self,
        prepared: Arc<PreparedModule>,
        resource_index: usize,
        services: S,
        durable: Option<&crate::retain::RetainSnapshot>,
    ) -> Result<(Self, Vec<super::RetainApplyEvent>), RuntimeError> {
        let congruent = super::restart::warm_graph_congruent(&self.prepared, &prepared);
        let snapshot = if congruent {
            None
        } else {
            let mut source = self.engine();
            source
                .engineering_operation(|source| {
                    source.require_serializable_retained_values()?;
                    source.cycle_retain_snapshot()
                })
                .map(Some)?
        };
        let source = self.engine();
        let mut data = EngineState::build(
            &prepared,
            resource_index,
            &services,
            true,
            congruent.then_some(&source),
            Some(&source.data.images),
            source.data.now,
        )?;
        let mut candidate = EngineState::borrow(&mut data, &prepared, &services);
        let mut events = if congruent {
            candidate.restore_retained_program_values(&source)?;
            Vec::new()
        } else {
            candidate.apply_retained_snapshot(
                snapshot
                    .as_ref()
                    .ok_or(RuntimeError::InvalidExecutionState)?,
            )?
        };
        if let Some(snapshot) = durable {
            let restored = candidate.apply_retained_snapshot(snapshot)?;
            candidate.charge_allocation_bytes(
                restored
                    .len()
                    .checked_mul(core::mem::size_of::<super::RetainApplyEvent>())
                    .ok_or(RuntimeError::Overflow)?,
            )?;
            events
                .try_reserve_exact(restored.len())
                .map_err(|_| RuntimeError::Overflow)?;
            events.extend(restored);
        }
        candidate.check_entry_deadline()?;
        Ok((
            Self {
                data,
                prepared,
                services,
            },
            events,
        ))
    }

    /// Construct a complete restart candidate before replacing mutable state.
    pub fn restart(&mut self, mode: RestartMode) -> Result<(), RuntimeError> {
        self.engine().restart(mode)
    }

    /// Stage a typed hierarchical input through its admitted codec.
    pub fn set_hierarchical_input(
        &mut self,
        address: &IoAddress,
        value: Value,
    ) -> Result<(), RuntimeError> {
        self.engine()
            .engineering_operation(|state| state.set_hierarchical_input(address, value))
    }

    /// Borrow a published hierarchical output value.
    pub fn read_hierarchical_output(&self, address: &IoAddress) -> Option<&Value> {
        super::io::read_hierarchical_output(&self.data, &self.prepared, address)
    }

    /// Read an admitted engineering access alias.
    pub fn read_access(&mut self, name: &str) -> Result<Value, RuntimeError> {
        self.engine().engineering_operation(|state| {
            let value = state.read_access(name)?;
            state.check_entry_deadline()?;
            Ok(value)
        })
    }

    /// Commit a checked engineering write to an admitted writable alias.
    pub fn write_access(&mut self, name: &str, value: Value) -> Result<(), RuntimeError> {
        self.engine()
            .engineering_operation(|state| state.write_access(name, value))
    }

    /// Commit a checked global value write without reinitializing the controller.
    pub fn write_global(&mut self, name: &str, value: Value) -> Result<(), RuntimeError> {
        self.engine()
            .engineering_operation(|state| state.write_global(name, value))
    }

    /// Validate a named engineering write without changing its destination.
    pub fn validate_global_write(&mut self, name: &str, value: Value) -> Result<(), RuntimeError> {
        self.engine()
            .engineering_operation(|state| state.validate_global_write(name, value))
    }
    /// Write a global and return its canonical admitted representation. Result
    /// allocation and the physical deadline are checked before publication.
    pub fn write_global_canonical(
        &mut self,
        name: &str,
        value: Value,
    ) -> Result<Value, RuntimeError> {
        self.engine()
            .engineering_operation(|state| state.write_global_canonical(name, value))
    }

    /// Resolve and write a named program/global path using the current installation.
    pub fn write_named_path(
        &mut self,
        name: &str,
        path: &[crate::value::RefSegment],
        value: Value,
    ) -> Result<(), RuntimeError> {
        self.engine()
            .engineering_operation(|state| state.write_named_path(name, path, value))
    }
    /// Read a currently bound destination through the common declared policy.
    pub fn read_reference(
        &mut self,
        reference: &crate::value::ValueRef,
    ) -> Result<Value, RuntimeError> {
        self.engine()
            .engineering_operation(|state| state.read_checked_reference(reference))
    }
    /// Commit a checked value write without executing or reinitializing the program.
    /// Bindings must be re-resolved when installation/schema changes.
    pub fn write_reference(
        &mut self,
        reference: crate::value::ValueRefView<'_>,
        value: Value,
    ) -> Result<(), RuntimeError> {
        self.engine()
            .engineering_operation(|state| state.write_checked_reference(reference, value))
    }

    /// Resolve an installed access alias to its current live destination. This
    /// is a checked value binding, never an artifact owner id cast to a live id.
    pub fn resolve_access_reference(
        &mut self,
        name: &str,
    ) -> Result<crate::value::ValueRef, RuntimeError> {
        self.engine()
            .engineering_operation(|state| state.resolve_access_reference(name))
    }

    /// Resolve an admitted persistent binding through the live root mapping.
    /// Invalid or frame-local artifact references are rejected by the same
    /// resolver used for process-image and task bindings.
    pub fn resolve_persistent_reference(
        &mut self,
        index: u32,
    ) -> Result<crate::value::ValueRef, RuntimeError> {
        self.engine().engineering_operation(|state| {
            let reference = state.persistent_io_reference(index)?;
            state.check_read(reference.as_view())?;
            state.check_entry_deadline()?;
            Ok(reference)
        })
    }

    /// Execute one installed program explicitly, with the same cleanup, services
    /// and prepared register plans as a scheduled activation.
    pub fn execute_program(&mut self, name: &str) -> Result<(), RuntimeError> {
        if self.data.fault.is_some() {
            return Err(RuntimeError::ResourceFaulted);
        }
        self.engine()
            .engineering_operation(|state| state.execute_named_program(name))
    }

    /// Construct and execute one admitted function-block template, retiring its
    /// temporary instance graph on success or failure within the same budget.
    pub fn execute_function_block_by_name(&mut self, name: &str) -> Result<(), RuntimeError> {
        if self.data.fault.is_some() {
            return Err(RuntimeError::ResourceFaulted);
        }
        self.engine()
            .engineering_operation(|state| state.execute_temporary_function_block(name))
    }

    /// Execute an already constructed, currently live function-block destination.
    pub fn execute_function_block(
        &mut self,
        reference: &crate::value::ValueRef,
    ) -> Result<(), RuntimeError> {
        if self.data.fault.is_some() {
            return Err(RuntimeError::ResourceFaulted);
        }
        self.engine()
            .engineering_operation(|state| state.execute_bound_function_block(reference))
    }
}
