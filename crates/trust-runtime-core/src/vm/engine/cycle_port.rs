//! Checked, owner-local access at the existing cooperative cycle boundaries.
use super::*;
use crate::io_address::IoAddress;
use crate::value::{RefSegment, ValueRefView};

/// Existing hosted phases around the single shared resource scheduler.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CyclePhase {
    /// Apply pending engineering writes before physical input acquisition.
    CycleStart,
    /// Acquire physical inputs, staged debug I/O and input forces.
    BeforeInputSample,
    /// Apply protocol input values after mapped image sampling.
    AfterInputSample,
    /// Record dropped activations in declaration order before priority execution.
    TaskOverrun {
        /// Task index in the admitted resource's declaration order.
        index: usize,
        /// Periodic activations dropped before this single ready activation.
        missed: u64,
    },
    /// Begin a due task, identified by its admitted resource task index.
    TaskStart(usize),
    /// Finish a due task with the same index.
    TaskEnd(usize),
    /// Begin an unscheduled program, identified by its artifact name index.
    BackgroundStart(u32),
    /// Finish that unscheduled program.
    BackgroundEnd(u32),
    /// Save retain state before any output image sampling.
    BeforeOutputSample,
    /// Normal mapped outputs are sampled; apply forces, capture protocols and drive I/O.
    AfterOutputSample,
    /// Publish the successful cycle's final observations.
    CycleEnd,
}

/// Mutable host adapters borrowed for one cycle, without owning PLC state.
///
/// A returned error is an adapter/execution failure and fails the cycle closed.
/// A rejected engineering request is instead the result of a checked access
/// method: report it through the protocol/debug adapter and continue draining
/// requests without returning it as an execution failure. Callbacks must not
/// reacquire the controller or execute/restart it recursively.
pub trait CycleHooks {
    /// Whether this adapter observes program-call durations. The ordinary
    /// portable cycle has no observers or per-program host-clock work.
    fn has_program_hooks(&self) -> bool {
        false
    }

    /// Begin observing one canonical program instance, scheduled or background.
    /// Observation does not grant mutable access to PLC state.
    fn program_started(&mut self, _name: &smol_str::SmolStr) {}

    /// Finish that observation on success or execution failure. This callback
    /// cannot replace the program's original execution result.
    fn program_finished(&mut self, _name: &smol_str::SmolStr) {}

    /// Run one existing boundary against the authoritative owner-local state.
    fn on_phase(
        &mut self,
        _phase: CyclePhase,
        _access: &mut CycleAccess<'_, '_>,
    ) -> Result<(), RuntimeError> {
        Ok(())
    }

    /// Whether AfterOutputSample/CycleEnd callbacks are installed. Returning
    /// false omits those callbacks and the flat-Q rollback journal entirely.
    /// Ordinary MCU execution uses the no-op adapter and retains its original cost.
    fn has_output_hooks(&self) -> bool {
        true
    }

    /// Submit the prepared normal output image to physical drivers. The core
    /// checks its deadline after `AfterOutputSample` and before this operation.
    /// An error restores touched flat-Q windows; success commits them before
    /// final deadline/observation checks. Physical effects cannot be rolled back.
    fn publish_outputs(&mut self, _access: &CycleAccess<'_, '_>) -> Result<(), RuntimeError> {
        Ok(())
    }
}

pub(super) struct NoopCycleHooks;
impl CycleHooks for NoopCycleHooks {
    fn has_output_hooks(&self) -> bool {
        false
    }
}

/// A callback-scoped checked cycle port. It cannot execute code, replace a
/// module, mutate a schema or return mutable PLC storage.
///
/// ```compile_fail,E0599
/// use trust_runtime_core::vm::CycleAccess;
/// fn bypass(access: &mut CycleAccess<'_, '_>) {
///     let _ = access.storage_mut();
/// }
/// ```
pub struct CycleAccess<'a, 'engine> {
    pub(super) engine: &'a mut EngineState<'engine>,
    pub(super) phase: CyclePhase,
    pub(super) output_undo: Option<&'a mut Vec<OutputUndo>>,
}

impl CycleAccess<'_, '_> {
    /// Check the entry deadline before a hosted immutable index lookup of an
    /// external management label. The label lookup is bookkeeping under TIME-08:
    /// it draws no work from the scan's allowance and never resets execution fuel.
    pub fn charge_management_name(&mut self, _name: &str) -> Result<(), RuntimeError> {
        self.engine.check_entry_deadline()
    }

    /// Inspect the owner's current values without acquiring another controller lock.
    pub fn storage(&self) -> &VariableStorage {
        &self.engine.data.storage
    }
    /// Logical scan time, independent of host driver clocks.
    pub fn logical_time(&self) -> Duration {
        self.engine.data.now
    }
    /// Read nominal task deadlines and missed-period accounting.
    pub fn task_states(&self) -> &[TaskState] {
        &self.engine.data.tasks
    }
    /// Capture retained values at the established pre-output checkpoint, with
    /// their allocation charged to this cycle. A host persistence adapter may save
    /// or enqueue the snapshot; it cannot mutate the installed schema.
    pub fn retain_snapshot(&mut self) -> Result<crate::retain::RetainSnapshot, RuntimeError> {
        if self.phase != CyclePhase::BeforeOutputSample {
            return Err(RuntimeError::InvalidExecutionState);
        }
        self.engine.cycle_retain_snapshot()
    }
    /// Borrow the flat Q image. During AfterOutputSample it is provisional
    /// until pre-submission checks and physical driver submission succeed.
    pub fn outputs(&self) -> &[u8] {
        &self.engine.data.images.outputs
    }
    /// Inspect the marker image without copying unused capacity.
    pub fn memory_image(&self) -> &[u8] {
        &self.engine.data.images.memory
    }
    /// Physical input drivers may fill the input image only before mapped sampling.
    pub fn inputs_mut(&mut self) -> Result<&mut [u8], RuntimeError> {
        if self.phase != CyclePhase::BeforeInputSample {
            return Err(RuntimeError::InvalidExecutionState);
        }
        Ok(&mut self.engine.data.images.inputs)
    }
    /// Read a checked flat or declared hierarchical process-image address.
    pub fn read_image(&mut self, address: &IoAddress) -> Result<Value, RuntimeError> {
        self.engine.read_cycle_image(address)
    }
    /// Write wire values through the shared address/window/codec checks. Flat Q
    /// writes after sampling join the publication journal; they do not resample
    /// PLC values. Marker/hierarchical effects retain the old hosted rollback domain.
    pub fn write_image(&mut self, address: &IoAddress, value: Value) -> Result<(), RuntimeError> {
        self.check_write_phase()?;
        self.engine
            .write_cycle_image(address, value, self.output_undo.as_deref_mut())
    }
    /// Apply one queued or forced I/O request at a scan boundary. An untyped
    /// integer literal first takes its destination's type; a rejected request
    /// writes nothing and returns its error for the cycle owner to report.
    pub fn write_admitted_image(
        &mut self,
        address: &IoAddress,
        value: Value,
    ) -> Result<(), RuntimeError> {
        self.check_write_phase()?;
        let value = self.engine.admit_cycle_image_value(address, value)?;
        self.engine
            .write_cycle_image(address, value, self.output_undo.as_deref_mut())
    }
    /// Commit a checked named value/path write without restarting scan fuel.
    pub fn write_named_path(
        &mut self,
        name: &str,
        path: &[RefSegment],
        value: Value,
    ) -> Result<(), RuntimeError> {
        self.check_write_phase()?;
        self.engine.write_named_path(name, path, value)
    }
    /// Commit a declared writable engineering alias using the common policy.
    pub fn write_access(&mut self, name: &str, value: Value) -> Result<(), RuntimeError> {
        self.check_write_phase()?;
        self.engine.write_access(name, value)
    }
    /// Apply a previously resolved protocol destination, rechecking its current
    /// declaration, type, mutability, reference lifetime and path before commit.
    pub fn write_reference(
        &mut self,
        reference: ValueRefView<'_>,
        value: Value,
    ) -> Result<(), RuntimeError> {
        self.check_write_phase()?;
        self.engine.write_checked_reference(reference, value)
    }
    /// Read a resolved protocol binding with current lifetime/visibility checks.
    /// Cloning is accounted only for its selected value, never the whole store.
    pub fn read_reference(&mut self, reference: &ValueRef) -> Result<Value, RuntimeError> {
        self.engine.read_checked_reference(reference)
    }
    fn check_write_phase(&self) -> Result<(), RuntimeError> {
        if matches!(
            self.phase,
            CyclePhase::CycleStart
                | CyclePhase::BeforeInputSample
                | CyclePhase::AfterInputSample
                | CyclePhase::AfterOutputSample
        ) {
            Ok(())
        } else {
            Err(RuntimeError::InvalidExecutionState)
        }
    }
}

pub(super) struct OutputUndo {
    pub(super) start: usize,
    pub(super) bytes: Vec<u8>,
}

impl EngineState<'_> {
    pub(super) fn cycle_hook(
        &mut self,
        hooks: &mut dyn CycleHooks,
        phase: CyclePhase,
    ) -> Result<(), RuntimeError> {
        hooks.on_phase(
            phase,
            &mut CycleAccess {
                engine: self,
                phase,
                output_undo: None,
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn management_label_lookup_is_bookkeeping_and_draws_no_work() {
        let artifact =
            include_bytes!("../../../../trust-runtime/tests/fixtures/portability/f401/gpio.stbc");
        let prepared =
            PreparedModule::from_bytes(artifact, crate::vm::PreparationLimits::default()).unwrap();
        let mut data =
            EngineState::construct_data(&prepared, 0, &super::super::services::LOGICAL_ONLY)
                .unwrap();
        data.resources.work_budget.reset(8);
        let mut engine =
            EngineState::borrow(&mut data, &prepared, &super::super::services::LOGICAL_ONLY);
        let mut access = CycleAccess {
            engine: &mut engine,
            phase: CyclePhase::CycleStart,
            output_undo: None,
        };
        // TIME-08: label lookup is bookkeeping. Every lookup succeeds and the
        // allowance stays at 8.
        for _ in 0..3 {
            access.charge_management_name("abc").unwrap();
            assert_eq!(access.engine.data.resources.work_budget.remaining(), 8);
        }
    }

    #[test]
    fn checked_cycle_writes_do_not_restart_or_draw_the_cycle_allowance() {
        let artifact =
            include_bytes!("../../../../trust-runtime/tests/fixtures/portability/f401/gpio.stbc");
        let prepared =
            PreparedModule::from_bytes(artifact, crate::vm::PreparationLimits::default()).unwrap();
        let mut data =
            EngineState::construct_data(&prepared, 0, &super::super::services::LOGICAL_ONLY)
                .unwrap();
        let reference = data.storage.ref_for_global("probe_depth").unwrap();
        data.resources.work_budget.reset(512);
        let mut engine =
            EngineState::borrow(&mut data, &prepared, &super::super::services::LOGICAL_ONLY);
        let mut access = CycleAccess {
            engine: &mut engine,
            phase: CyclePhase::CycleStart,
            output_undo: None,
        };
        access
            .write_reference(reference.as_view(), Value::Int(2))
            .unwrap();
        // TIME-08: a scalar host store is bookkeeping and draws no work.
        assert_eq!(access.engine.data.resources.work_budget.remaining(), 512);
        // An empty cycle allowance still admits the store: the write neither starts
        // an engineering allowance nor refills the cycle's own.
        access.engine.data.resources.work_budget.reset(0);
        access
            .write_reference(reference.as_view(), Value::Int(3))
            .unwrap();
        assert_eq!(access.engine.data.resources.work_budget.remaining(), 0);
        assert_eq!(
            access.engine.data.storage.get_global("probe_depth"),
            Some(&Value::Int(3))
        );
    }

    #[test]
    fn checked_image_port_writes_do_not_restart_or_draw_the_scan_allowance() {
        let artifact =
            include_bytes!("../../../../trust-runtime/tests/fixtures/portability/f401/gpio.stbc");
        let prepared =
            PreparedModule::from_bytes(artifact, crate::vm::PreparationLimits::default()).unwrap();
        let mut data =
            EngineState::construct_data(&prepared, 0, &super::super::services::LOGICAL_ONLY)
                .unwrap();
        data.resources.work_budget.reset(0);
        let mut engine =
            EngineState::borrow(&mut data, &prepared, &super::super::services::LOGICAL_ONLY);
        let mut access = CycleAccess {
            engine: &mut engine,
            phase: CyclePhase::CycleStart,
            output_undo: None,
        };
        let address = IoAddress::parse("%QX0.0").unwrap();
        // TIME-08: process-image publication is bookkeeping, so even an empty
        // allowance admits each checked write without drawing work.
        access.write_image(&address, Value::Bool(true)).unwrap();
        assert_eq!(access.outputs(), &[1]);
        assert_eq!(access.engine.data.resources.work_budget.remaining(), 0);
        access.write_image(&address, Value::Bool(false)).unwrap();
        assert_eq!(access.outputs(), &[0]);
        assert_eq!(access.engine.data.resources.work_budget.remaining(), 0);
        assert!(
            access.engine.data.fault.is_none(),
            "checked management writes are not PLC faults"
        );
    }

    #[test]
    fn scalar_image_reads_are_bookkeeping_and_draw_no_cycle_fuel() {
        let artifact =
            include_bytes!("../../../../trust-runtime/tests/fixtures/portability/f401/gpio.stbc");
        let prepared =
            PreparedModule::from_bytes(artifact, crate::vm::PreparationLimits::default()).unwrap();
        let mut data =
            EngineState::construct_data(&prepared, 0, &super::super::services::LOGICAL_ONLY)
                .unwrap();
        data.resources.work_budget.reset(2);
        let mut engine =
            EngineState::borrow(&mut data, &prepared, &super::super::services::LOGICAL_ONLY);
        let mut access = CycleAccess {
            engine: &mut engine,
            phase: CyclePhase::CycleStart,
            output_undo: None,
        };
        let address = IoAddress::parse("%IX0.0").unwrap();
        // TIME-08: image sampling is bookkeeping, so each read is admitted and the
        // allowance stays at 2.
        for _ in 0..3 {
            assert_eq!(access.read_image(&address).unwrap(), Value::Bool(false));
            assert_eq!(access.engine.data.resources.work_budget.remaining(), 2);
        }
        assert!(access.engine.data.fault.is_none());
        assert_eq!(access.engine.data.images.inputs, [0]);
    }

    #[test]
    fn scalar_image_read_checks_expiry_before_and_after_the_selected_read() {
        struct Services {
            polls: core::cell::Cell<usize>,
            expire_at: core::cell::Cell<usize>,
        }
        impl ExecutionServices for Services {
            fn deadline_exceeded(&self) -> bool {
                let polls = self.polls.get() + 1;
                self.polls.set(polls);
                polls >= self.expire_at.get()
            }
            fn has_wall_clock(&self) -> bool {
                false
            }
            fn current_dt(&self) -> Result<crate::value::DateTimeValue, RuntimeError> {
                Err(RuntimeError::UndefinedFunction("CURRENT_DT".into()))
            }
        }
        let artifact =
            include_bytes!("../../../../trust-runtime/tests/fixtures/portability/f401/gpio.stbc");
        let prepared =
            PreparedModule::from_bytes(artifact, crate::vm::PreparationLimits::default()).unwrap();
        let services = Services {
            polls: core::cell::Cell::new(0),
            expire_at: core::cell::Cell::new(usize::MAX),
        };
        let mut data = EngineState::construct_data(&prepared, 0, &services).unwrap();
        let mut engine = EngineState::borrow(&mut data, &prepared, &services);
        let mut access = CycleAccess {
            engine: &mut engine,
            phase: CyclePhase::CycleStart,
            output_undo: None,
        };
        let address = IoAddress::parse("%IX0.0").unwrap();
        for deadline_poll in [1, 2] {
            // Separate explicit entry fixtures clear the previous expiry latch;
            // the checked read itself never resets this shared allowance.
            access.engine.data.resources.work_budget.reset(64);
            services.polls.set(0);
            services.expire_at.set(deadline_poll);
            assert_eq!(
                access.read_image(&address),
                Err(RuntimeError::ExecutionTimeout)
            );
            assert_eq!(services.polls.get(), deadline_poll);
            assert_eq!(access.engine.data.images.inputs, [0]);
            assert!(
                access.engine.data.fault.is_none(),
                "management rejection does not latch a PLC fault"
            );
        }
    }
}
