//! Hosted ownership of admitted metadata and mutable data, with scoped execution borrows.
use super::{EngineData, EngineState, ExecutionServices, LogicalExecutionServices, PreparedModule};
use crate::{
    error::RuntimeError,
    fault::{FaultRecord, NO_TASK},
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
    /// The storage projected from the flat store for [`OwnedRuntimeState::storage`], made on first
    /// use and dropped by every mutation (C1-19d).
    pub(super) projection: core::cell::OnceCell<Box<VariableStorage>>,
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
            projection: core::cell::OnceCell::new(),
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
        // Every mutation reaches the state through here, so the projection of the storage is
        // dropped (C1-19d).
        self.projection.take();
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

    /// The index, in the admitted module, of the resource this controller executes.
    pub fn resource_index(&self) -> usize {
        self.data.resource_index
    }

    /// Inspect variables without an unrestricted mutable storage escape: the storage as the value
    /// store holds it, projected from the flat store and kept until the next mutation (C1-19d).
    pub fn storage(&self) -> &VariableStorage {
        EngineState::projected_storage(&self.data, &self.prepared, &self.projection)
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

    /// Inspect the fixed-size record latched with the execution fault, if any
    /// (specification 34 MEM-05, STATE-04). The record is `Copy`, so a host reads it as often
    /// as it asks and never changes it.
    pub fn fault_record(&self) -> Option<FaultRecord> {
        self.data.fault_record
    }

    /// The allocator operations the engine made itself in the last scan, the calls it made into
    /// services not counted; `None` when the platform does not count them (specification 34, "The
    /// allocation count").
    #[must_use]
    pub fn last_scan_allocations(&self) -> Option<u64> {
        self.data.resources.scan_allocations.last()
    }

    /// Latch a platform/supervision failure in the same owner as execution
    /// faults, together with its fixed-size record. No PLC values, bindings or
    /// executable metadata change here.
    pub fn latch_fault(&mut self, error: RuntimeError) {
        self.engine().latch_fault_record(&error);
        self.data.fault = Some(error);
    }

    /// Clear only the fault latch through the existing owner tooling action.
    /// Task history, installed code and PLC values remain unchanged.
    pub fn clear_fault(&mut self) {
        self.data.fault = None;
        self.data.fault_record = None;
        self.data.fault_site = None;
        self.data.task = NO_TASK;
    }

    /// Capture an explicit persistence checkpoint using the same indexed
    /// retained declarations and value policy as an in-cycle checkpoint.
    pub fn retain_snapshot(&mut self) -> Result<crate::retain::RetainSnapshot, RuntimeError> {
        self.engine()
            .engineering_operation(|state| state.cycle_retain_snapshot())
    }

    /// Capture this resource's retained image into `capture`'s reserved buffer as the shared
    /// codec's STRN v3 payload bytes at the platform's persistence checkpoint (specification 11
    /// §6.7, specification 34 §10).
    ///
    /// The platform calls this at the established pre-output-sample checkpoint, so the payload holds
    /// the finished scan. Once the capture has reserved its staging buffer, a later capture of the
    /// same application allocates nothing (specification 34 MEM-04), which is what lets a chip
    /// capture between scans.
    ///
    /// # Errors
    ///
    /// The fault or limit failure that refuses the operation, including an expired physical
    /// deadline and an allocation-byte charge over the configured limit. The capture then holds no
    /// staged payload.
    pub fn capture_retain(
        &mut self,
        capture: &mut crate::retain::capture::RetainCapture,
    ) -> Result<(), RuntimeError> {
        self.engine()
            .engineering_operation(|state| state.capture_retained(capture))
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

    /// Prepare a distinct installation at a quiescent owner boundary. Every variable
    /// it declares at the same place and by the same name takes this installation's
    /// value, retained or not, and a variable whose declaration changed keeps what
    /// fits. The tasks continue this installation's schedule, and no retain file or
    /// checkpoint is applied (specification 34 §11.3, decisions OC-1, OC-2 and
    /// "keep what fits"). The old owner is unchanged if any preparation or
    /// initializer fails.
    ///
    /// The body is shared with `RuntimeState::prepare_replacement`, which a chip's
    /// borrowed state calls, so both replace their program by the same rules.
    pub fn prepare_replacement(
        &mut self,
        prepared: Arc<PreparedModule>,
        resource_index: usize,
        services: S,
    ) -> Result<(Self, Vec<super::CarriedValueEvent>), RuntimeError> {
        let (data, events) =
            self.engine()
                .prepare_replacement_data(&prepared, resource_index, &services)?;
        Ok((
            Self {
                data,
                prepared,
                services,
                projection: core::cell::OnceCell::new(),
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

#[cfg(test)]
mod tests {
    //! Specification 34 §11.3 and UPDATE-04; specification 11 §6.7.1. The state a chip borrows and the
    //! state a computer owns replace their program by one body, so the same replacement of the same
    //! installation builds the same candidate in both, or is refused for the same reason. The programs
    //! are the corpus fixtures `retained_blocks` (a function-block instance, a reference and a scalar
    //! in retained globals, and a retained program variable), the A4 program and the board probe.
    use super::*;
    use crate::bytecode::{BytecodeModule, SectionData, SectionId};
    use crate::memory::Generation;
    use crate::vm::{CarriedValueEvent, PreparationLimits};
    use smol_str::SmolStr;

    const BLOCKS: &[u8] = include_bytes!(
        "../../../../trust-runtime/tests/fixtures/bounded-c/retained_blocks/program.stbc"
    );
    const A4: &[u8] = include_bytes!(
        "../../../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc"
    );
    const PROBE: &[u8] =
        include_bytes!("../../../../trust-runtime/tests/fixtures/portability/f401/gpio.stbc");

    fn prepared(bytes: &[u8]) -> PreparedModule {
        PreparedModule::from_bytes(bytes, PreparationLimits::default()).unwrap()
    }

    /// `bytes` with the string `from` renamed `to`: a candidate that declares other names, which is
    /// no longer congruent with `bytes`.
    fn renamed(bytes: &[u8], from: &str, to: &str) -> Vec<u8> {
        let mut module = BytecodeModule::decode(bytes).unwrap();
        let Some(SectionData::StringTable(strings)) = module.section_mut(SectionId::StringTable)
        else {
            panic!("strings")
        };
        let entry = strings
            .entries
            .iter_mut()
            .find(|entry| entry.as_str() == from)
            .unwrap_or_else(|| panic!("the program names {from}"));
        *entry = SmolStr::from(to);
        module.encode().unwrap()
    }

    /// The `retained_blocks` program with the argument of its call of `kept` changed from DINT#1 to
    /// DINT#2: the same declarations, types and identities with another executable body, which is
    /// congruent with the original.
    fn blocks_that_count_by_two() -> Vec<u8> {
        let mut module = BytecodeModule::decode(BLOCKS).unwrap();
        let Some(SectionData::ConstPool(constants)) = module.section_mut(SectionId::ConstPool)
        else {
            panic!("constants")
        };
        assert_eq!(
            constants.entries[0].payload,
            1_i32.to_le_bytes(),
            "the first constant is the argument DINT#1 of the call of kept"
        );
        constants.entries[0].payload = 2_i32.to_le_bytes().to_vec();
        module.encode().unwrap()
    }

    /// What a state shows of an installation.
    #[derive(Debug, PartialEq)]
    struct Observed {
        events: Vec<CarriedValueEvent>,
        generation: Generation,
        time: Duration,
        inputs: Vec<u8>,
        outputs: Vec<u8>,
        memory: Vec<u8>,
        /// The globals `total` and `follow`'s target, and the instance `kept`'s `count`.
        total: Option<Value>,
        count: Option<Value>,
        followed: Option<Value>,
        /// The members `runs`, `activations`, `activations2` and `seen` of the program instance
        /// `Plant`.
        members: [Option<Value>; 4],
    }

    macro_rules! observe {
        ($state:expr, $events:expr) => {{
            let state = &$state;
            let storage = state.storage();
            let instance = |name: &str| match storage.get_global(name) {
                Some(Value::Instance(id)) => Some(*id),
                _ => None,
            };
            let plant = instance("Plant");
            let member = |name: &str| {
                plant
                    .and_then(|id| storage.get_instance_var(id, name))
                    .cloned()
            };
            Observed {
                events: $events,
                generation: storage.generation(),
                time: state.current_time(),
                inputs: state.inputs().to_vec(),
                outputs: state.outputs().to_vec(),
                memory: state.memory_image().to_vec(),
                total: storage.get_global("total").cloned(),
                count: instance("kept")
                    .and_then(|id| storage.get_instance_var(id, "count"))
                    .cloned(),
                followed: match storage.get_global("follow") {
                    Some(Value::Reference(Some(reference))) => {
                        storage.read_by_ref_ref(reference).cloned()
                    }
                    _ => None,
                },
                members: [
                    member("runs"),
                    member("activations"),
                    member("activations2"),
                    member("seen"),
                ],
            }
        }};
    }

    /// A replacement through each kind of state: the candidate as it is built, and after one more scan.
    type Replaced = Result<(Observed, Observed), RuntimeError>;

    /// The candidate and its next scan, or the panic that says the replacement was refused.
    fn candidate_of(replaced: &Replaced) -> (&Observed, &Observed) {
        let Ok((built, next)) = replaced else {
            panic!("the replacement is built: {replaced:?}");
        };
        (built, next)
    }

    struct Both {
        before: Observed,
        borrowed: Replaced,
        owned: Replaced,
        after: Observed,
    }

    /// Runs `old` on a borrowed and on an owned state, both given the first input byte `input` and the
    /// scans at `scans` (milliseconds of logical time), then replaces each with `new` and runs the
    /// candidate's scan at `next` milliseconds.
    fn replace_through_both(old: &[u8], new: &[u8], input: u8, scans: &[i64], next: i64) -> Both {
        let old_module = prepared(old);
        let mut borrowed = old_module.instantiate(0).unwrap();
        let mut owned = Arc::new(prepared(old)).instantiate_owned(0).unwrap();
        if let Some(byte) = borrowed.inputs_mut().first_mut() {
            *byte = input;
        }
        if let Some(byte) = owned.inputs_mut().first_mut() {
            *byte = input;
        }
        for &at in scans {
            borrowed.execute_cycle(Duration::from_millis(at)).unwrap();
            owned.execute_cycle(Duration::from_millis(at)).unwrap();
        }
        let before = observe!(borrowed, Vec::new());
        assert_eq!(
            observe!(owned, Vec::new()),
            before,
            "the two sources are one state"
        );

        let new_module = prepared(new);
        let borrowed_result = match borrowed.prepare_replacement(
            &new_module,
            0,
            &super::super::services::LOGICAL_ONLY,
        ) {
            Ok((mut candidate, events)) => {
                let built = observe!(candidate, events);
                candidate
                    .execute_cycle(Duration::from_millis(next))
                    .unwrap();
                Ok((built, observe!(candidate, Vec::new())))
            }
            Err(error) => Err(error),
        };
        let owned_result =
            match owned.prepare_replacement(Arc::new(prepared(new)), 0, LogicalExecutionServices) {
                Ok((mut candidate, events)) => {
                    let built = observe!(candidate, events);
                    candidate
                        .execute_cycle(Duration::from_millis(next))
                        .unwrap();
                    Ok((built, observe!(candidate, Vec::new())))
                }
                Err(error) => Err(error),
            };
        let after = observe!(borrowed, Vec::new());
        assert_eq!(
            observe!(owned, Vec::new()),
            after,
            "the two sources are one state after the replacement"
        );
        Both {
            before,
            borrowed: borrowed_result,
            owned: owned_result,
            after,
        }
    }

    #[test]
    fn the_borrowed_and_owned_replacements_build_the_same_candidate() {
        // Specification 34 §11.3 and UPDATE-04; specification 11 §6.7.1: the replacement of the state a
        // chip borrows (`RuntimeState`) and of the state a computer owns (`OwnedRuntimeState`) is one
        // body. Each case replaces the same installation in both and expects the same candidate, or the
        // same refusal. Every expected value is worked out here from the programs.
        // Wrong behaviour caught: a body of the borrowed replacement that drifts from the owned one
        // (another generation, a retained object rebuilt anew, images or time dropped, another
        // refusal), and a replacement that is not the next generation of its lineage.

        // 1. A congruent candidate. `retained_blocks` runs a 10 ms task: `kept` counts by the call's
        // argument, `total` and `runs` by 1, and the program variable `seen`, which is not retained,
        // copies what `follow` reads. The scans at 10, 20 and 30 ms leave count, total, runs and seen
        // at 3, and `follow` pointing at `kept.count`. The candidate has the same declarations and
        // another body (`kept(delta := DINT#2)`). It continues the lineage (generation 2), keeps the
        // logical time of the last scan (30 ms) and every value (decision OC-1): the instance's count
        // is still 3 (an instance built anew would count 0), the reference reads that count, and
        // `seen` is 3, not its initial 0. Its scan at 40 ms runs the new body: count 3 + 2 = 5, total
        // and runs 4, and `follow` and `seen` read 5.
        let congruent = blocks_that_count_by_two();
        let one = replace_through_both(BLOCKS, &congruent, 0, &[10, 20, 30], 40);
        assert_eq!(one.borrowed, one.owned, "congruent");
        let (built, next) = candidate_of(&one.borrowed);
        assert_eq!(built.generation, Generation::FIRST.next().unwrap());
        assert_eq!(built.time, Duration::from_millis(30));
        assert_eq!(built.count, Some(Value::DInt(3)), "the live instance");
        assert_eq!(built.followed, Some(Value::DInt(3)), "the live reference");
        assert_eq!(built.total, Some(Value::DInt(3)));
        assert_eq!(built.members[0], Some(Value::DInt(3)), "runs");
        assert_eq!(
            built.members[3],
            Some(Value::DInt(3)),
            "seen, not retained, is kept"
        );
        assert!(built.events.is_empty());
        assert_eq!(next.count, Some(Value::DInt(5)), "the new body runs");
        assert_eq!(next.followed, Some(Value::DInt(5)));
        assert_eq!(next.total, Some(Value::DInt(4)));
        assert_eq!(next.members[0], Some(Value::DInt(4)));
        assert_eq!(next.members[3], Some(Value::DInt(5)));
        assert_eq!(one.after.generation, Generation::FIRST, "the source stays");
        assert_eq!(one.after, one.before);

        // 2. The task schedule continues (decision OC-2). The A4 program's 25 ms task runs at 25 ms,
        // when `activations` becomes 1; the scan at 30 ms runs no task. The candidate, the same
        // program, is built at 30 ms and keeps the task's last activation, 25 ms, so its scan at 50 ms
        // runs the task: `activations` 1 + 1 = 2. A task counted from the change (30 ms) would wait
        // until 55 ms and leave it at 1.
        let two = replace_through_both(A4, A4, 0, &[25, 30], 50);
        assert_eq!(two.borrowed, two.owned, "the task schedule");
        assert_eq!(two.before.members[1], Some(Value::DInt(1)));
        let (built, next) = candidate_of(&two.borrowed);
        assert_eq!(built.time, Duration::from_millis(30));
        assert_eq!(
            built.members[1],
            Some(Value::DInt(1)),
            "the retained value is kept"
        );
        assert_eq!(next.members[1], Some(Value::DInt(2)), "the due task runs");

        // 3. A candidate that renames a variable. The A4 program runs a 25 ms task; its scans at 25,
        // 50 and 75 ms leave the retained `activations` at 3. The candidate names that variable
        // `activations2`: the running program declares no such variable, so it starts with its
        // declared initial value, 0, and the change records it as new; `activations` is gone. The
        // generation is 2 and the time 75 ms.
        let renamed_a4 = renamed(A4, "activations", "activations2");
        let three = replace_through_both(A4, &renamed_a4, 0, &[25, 50, 75], 100);
        assert_eq!(three.borrowed, three.owned, "not congruent");
        assert_eq!(three.before.members[1], Some(Value::DInt(3)));
        let (built, next) = candidate_of(&three.borrowed);
        assert_eq!(built.generation, Generation::FIRST.next().unwrap());
        assert_eq!(built.time, Duration::from_millis(75));
        assert_eq!(built.members[1], None, "the old name is gone");
        assert_eq!(built.members[2], Some(Value::DInt(0)));
        assert_eq!(
            built.events,
            [CarriedValueEvent::New {
                name: "Plant.activations2".into()
            }]
        );
        assert_eq!(next.members[2], Some(Value::DInt(1)), "it runs on from 0");

        // 4. A candidate that renames the retained global `total` to `sum` while the program holds a
        // retained function-block instance and a retained reference to its member. The change never
        // fails because of a value ("keep what fits"): the instance `kept` is at the same place, so
        // its count stays 3, `follow` names `kept.count` in the candidate and reads 3, `runs` and
        // `seen` stay 3, and `sum` starts at its initial 0 as a new variable; `total` is gone. The
        // scan at 40 ms counts on: count 4, `sum` 1, runs 4, and `follow` reads 4. The source is
        // exactly as it was.
        let renamed_blocks = renamed(BLOCKS, "total", "sum");
        let four = replace_through_both(BLOCKS, &renamed_blocks, 0, &[10, 20, 30], 40);
        assert_eq!(four.borrowed, four.owned, "a renamed retained global");
        let (built, next) = candidate_of(&four.borrowed);
        assert_eq!(built.count, Some(Value::DInt(3)), "the instance is carried");
        assert_eq!(
            built.followed,
            Some(Value::DInt(3)),
            "the reference is carried"
        );
        assert_eq!(built.total, None, "the old name is gone");
        assert_eq!(built.members[0], Some(Value::DInt(3)));
        assert_eq!(built.members[3], Some(Value::DInt(3)));
        assert_eq!(
            built.events,
            [CarriedValueEvent::New { name: "sum".into() }]
        );
        assert_eq!(next.count, Some(Value::DInt(4)));
        assert_eq!(next.followed, Some(Value::DInt(4)));
        assert_eq!(next.members[0], Some(Value::DInt(4)));
        assert_eq!(four.after, four.before);

        // 5. The images and the logical time. The board probe copies its input to its output every
        // 10 ms. With the input at 1, the scans at 10 and 20 ms leave the input and the output image
        // at 1. The same program, congruent, carries both images into the candidate, whose outputs
        // are still 1 (an image built anew is 0), at the generation 2 and the time 20 ms.
        let five = replace_through_both(PROBE, PROBE, 1, &[10, 20], 30);
        assert_eq!(five.borrowed, five.owned, "images");
        assert_eq!(five.before.inputs, [1]);
        assert_eq!(five.before.outputs, [1]);
        let (built, _) = candidate_of(&five.borrowed);
        assert_eq!(built.inputs, [1]);
        assert_eq!(built.outputs, [1]);
        assert_eq!(built.time, Duration::from_millis(20));
        assert_eq!(built.generation, Generation::FIRST.next().unwrap());
    }
}
