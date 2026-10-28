use trust_runtime_core::memory::IoArea;
use trust_runtime_core::native::variables::{CompiledAccessPoint, CompiledVariables};
use trust_runtime_services::live_values::apply::{self, WriteSink};
use trust_runtime_services::live_values::queues::REQUEST_QUEUE_CAPACITY;

impl DebugControl {
    /// Queue a one-shot I/O write. A refused write is reported as `write_rejected` and queues
    /// nothing; protocol and control producers use `try_enqueue_io_write` to receive the refusal.
    /// Control Studio's experiments and abort bindings write their addresses this way
    /// (specification 35, A3.5).
    pub fn enqueue_io_write(&self, address: IoAddress, value: Value) {
        if self.try_enqueue_io_write(address.clone(), value).is_err() {
            self.report_refused_request(SmolStr::new(format!("{address:?}")));
        }
    }

    /// Queue a one-shot I/O write from a control or protocol request. A request past the queue
    /// bound is refused with `RequestQueueFull` and queues nothing.
    pub fn try_enqueue_io_write(
        &self,
        address: IoAddress,
        value: Value,
    ) -> Result<(), RequestQueueFull> {
        self.lock().live_values.try_enqueue_io_write(address, value)
    }

    /// Withdraw the one-shot I/O write queued last for `address` and `value`. A caller uses it
    /// when its own checked write failed after the request was queued.
    pub fn withdraw_io_write(&self, address: &IoAddress, value: &Value) {
        self.lock().live_values.withdraw_io_write(address, value);
    }

    /// Drain the queued one-shot I/O writes. The scan applies the queue in place through the
    /// shared Live Values code, so only the tests read the queued writes directly.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn drain_io_writes(&self) -> Vec<(IoAddress, Value)> {
        self.lock().live_values.drain_io_writes()
    }

    /// Queue a global write from a control or protocol request. A new target past the queue
    /// bound is refused.
    pub fn try_enqueue_global_write(
        &self,
        name: impl Into<SmolStr>,
        value: Value,
    ) -> Result<(), RequestQueueFull> {
        self.try_enqueue_var_write(PendingVarTarget::Global(name.into()), value)
    }

    /// Queue a retained write from a control or protocol request. A new target past the queue
    /// bound is refused.
    pub fn try_enqueue_retain_write(
        &self,
        name: impl Into<SmolStr>,
        value: Value,
    ) -> Result<(), RequestQueueFull> {
        self.try_enqueue_var_write(PendingVarTarget::Retain(name.into()), value)
    }

    /// Queue an instance write from a control or protocol request. A new target past the queue
    /// bound is refused. `generation` is the installation generation in which `instance_id` was
    /// resolved: the boundary rejects the write when another generation is installed (spec 34
    /// §4.3.1).
    pub fn try_enqueue_instance_write(
        &self,
        generation: Generation,
        instance_id: InstanceId,
        name: impl Into<SmolStr>,
        value: Value,
    ) -> Result<(), RequestQueueFull> {
        self.try_enqueue_var_write(
            PendingVarTarget::Instance(generation, instance_id, name.into()),
            value,
        )
    }

    /// Queue an experiment's write of a program variable by place, applied at the next cycle start
    /// (specification 35, experiments by variable name, A3.5): a later write to a place already
    /// queued in the same generation replaces the earlier one in place. `epoch` is the runtime
    /// epoch the place was resolved in: once a reload or restart has ended it, the write is dropped,
    /// because the place names the variable only in the program that ended. A new place past the
    /// queue bound is refused and reported as `write_rejected`.
    pub(crate) fn enqueue_control_write(&self, write: ControlWrite, epoch: u64) {
        let refused = {
            let mut state = self.lock();
            if state.parameters.epoch() != epoch {
                return;
            }
            if let Some(queued) = state
                .control_writes
                .iter_mut()
                .find(|queued| queued.at == write.at && queued.generation == write.generation)
            {
                queued.value = write.value;
                return;
            }
            if state.control_writes.len() >= REQUEST_QUEUE_CAPACITY {
                Some(write.label)
            } else {
                state.control_writes.push(write);
                None
            }
        };
        if let Some(label) = refused {
            self.report_refused_request(label);
        }
    }

    /// Apply Control Studio's queued writes of program variables at the cycle start, before the Live
    /// Values, so that a force still holds its variable (specification 35, A3.5; specification 44
    /// §13.3): each at its place, converted to the place's type. A write resolved in another program
    /// generation than `generation` is dropped, since its place names nothing in this program; a
    /// write the program refuses is reported as `write_rejected` at `time`, never a fault: the writer
    /// reads its variable back.
    pub(crate) fn apply_control_writes(
        &self,
        program: &mut dyn ProgramWrite,
        generation: Generation,
        time: Duration,
    ) {
        let writes = {
            let mut state = self.lock();
            if state.control_writes.is_empty() {
                return;
            }
            std::mem::take(&mut state.control_writes)
        };
        for write in writes {
            if write.generation != generation {
                continue;
            }
            if let Err(error) = program.write_at(&write.at, &write.value) {
                self.push_runtime_event(RuntimeEvent::WriteRejected {
                    target: write.label,
                    error: error.to_string(),
                    time,
                });
            }
        }
    }

    /// Drain Control Studio's queued writes of program variables. The scan applies them in place,
    /// so only the control tests read them directly.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn drain_control_writes(&self) -> Vec<ControlWrite> {
        std::mem::take(&mut self.lock().control_writes)
    }

    /// Queue a local write from a control or protocol request. A new target past the queue bound
    /// is refused; the scan refuses the queued write, since a local of a paused frame is not
    /// reachable from it.
    pub fn try_enqueue_local_write(
        &self,
        frame_id: FrameId,
        name: impl Into<SmolStr>,
        value: Value,
    ) -> Result<(), RequestQueueFull> {
        self.try_enqueue_var_write(PendingVarTarget::Local(frame_id, name.into()), value)
    }

    /// Drain pending variable writes. The scan applies the queue in place through the shared Live
    /// Values code, so only the control tests read the queued writes directly.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn drain_var_writes(&self) -> Vec<PendingVarWrite> {
        self.lock().live_values.drain_var_writes()
    }

    /// Clear queued writes and active forces at a terminating runtime boundary (a fault or the
    /// safe outputs), and interrupt Control Studio's recordings, parameter sets and transfers and
    /// its signal inspection, which belonged to the run that ended (specification 35, A6.7).
    /// A transfer committed in a scan that faults is reverted in that scan before this
    /// interruption, by [`Self::fail_transfer`] (specification 35, A6.3).
    pub(crate) fn clear_runtime_mutations(&self) {
        let mut state = self.lock();
        state.end_control_epoch();
        state.live_values.clear();
    }

    fn try_enqueue_var_write(
        &self,
        target: PendingVarTarget,
        value: Value,
    ) -> Result<(), RequestQueueFull> {
        self.lock().live_values.try_upsert_var_write(target, value)
    }

    /// Force a global variable from a control or protocol request. A new target past the queue
    /// bound is refused.
    pub fn try_force_global(
        &self,
        name: impl Into<SmolStr>,
        value: Value,
    ) -> Result<(), RequestQueueFull> {
        self.try_set_forced_var(ForcedVarTarget::Global(name.into()), value)
    }

    /// Force a retained variable from a control or protocol request. A new target past the queue
    /// bound is refused.
    pub fn try_force_retain(
        &self,
        name: impl Into<SmolStr>,
        value: Value,
    ) -> Result<(), RequestQueueFull> {
        self.try_set_forced_var(ForcedVarTarget::Retain(name.into()), value)
    }

    /// Force an instance variable from a control or protocol request. A new target past the queue
    /// bound is refused. `generation` is the installation generation in which `instance_id` was
    /// resolved: the scan boundary rejects and removes the force once another generation is
    /// installed (spec 34 §4.3.1).
    pub fn try_force_instance(
        &self,
        generation: Generation,
        instance_id: InstanceId,
        name: impl Into<SmolStr>,
        value: Value,
    ) -> Result<(), RequestQueueFull> {
        self.try_set_forced_var(
            ForcedVarTarget::Instance(generation, instance_id, name.into()),
            value,
        )
    }

    /// The installation generation of the attached runtime. A request that names an instance by
    /// a bare identity, as a control client does, belongs to the generation installed when the
    /// request is accepted (specification 11, debug mutation lifecycle).
    #[must_use]
    pub fn generation(&self) -> Generation {
        self.lock().generation
    }

    /// The installation generation of the runtime the control is attached to, which a request that
    /// names an instance by a bare identity belongs to. Attaching the control is no restart: the
    /// queued writes and the active forces stay.
    pub(crate) fn set_generation(&self, generation: Generation) {
        self.lock().generation = generation;
    }

    /// A restart or replacement committed `generation`: clear the queued writes and active forces
    /// of the previous lifecycle, end Control Studio's epoch, whose experiments and transactions
    /// were bound to the program before (specification 35, A6.7), and record the new generation,
    /// under one lock.
    ///
    /// A replacement (an online change) also calls [`Self::invalidate_control_program`], which marks
    /// the program reloaded, so that an experiment it stops reports `program_reloaded` rather than
    /// `runtime_restarted` (specification 35, A6.7).
    pub(crate) fn begin_lifecycle(&self, generation: Generation) {
        let mut state = self.lock();
        state.end_control_epoch();
        state.live_values.clear();
        state.generation = generation;
    }

    /// Release a forced global variable.
    pub fn release_global(&self, name: &str) {
        self.clear_forced_var(|target| match target {
            ForcedVarTarget::Global(current) => current.as_str() == name,
            _ => false,
        });
    }

    /// Release a forced retained variable.
    pub fn release_retain(&self, name: &str) {
        self.clear_forced_var(|target| match target {
            ForcedVarTarget::Retain(current) => current.as_str() == name,
            _ => false,
        });
    }

    /// Release a forced instance variable, in whichever generation it was forced.
    pub fn release_instance(&self, instance_id: InstanceId, name: &str) {
        self.clear_forced_var(|target| match target {
            ForcedVarTarget::Instance(_, current_id, current_name) => {
                *current_id == instance_id && current_name.as_str() == name
            }
            _ => false,
        });
    }

    /// Force an I/O address from a control or protocol request. A new address past the queue
    /// bound is refused; re-forcing an existing address is never refused.
    pub fn try_force_io(&self, address: IoAddress, value: Value) -> Result<(), RequestQueueFull> {
        self.lock().live_values.try_upsert_forced_io(address, value)
    }

    /// The value of the active variable force on `target`, when the target is forced.
    pub fn forced_var_value(&self, target: &ForcedVarTarget) -> Option<Value> {
        self.lock().live_values.forced_var_value(target)
    }

    /// Put back the variable force that a failed checked write had replaced: `previous` is the
    /// value forced before the failed request, or `None` when the target was not forced.
    pub fn restore_var_force(&self, target: &ForcedVarTarget, previous: Option<Value>) {
        self.lock().live_values.restore_var_force(target, previous);
    }

    /// The value of the active I/O force on `address`, when the address is forced.
    pub fn forced_io_value(&self, address: &IoAddress) -> Option<Value> {
        self.lock().live_values.forced_io_value(address)
    }

    /// Put back the I/O force that a failed checked write had replaced: `previous` is the value
    /// forced before the failed request, or `None` when the address was not forced.
    pub fn restore_io_force(&self, address: &IoAddress, previous: Option<Value>) {
        self.lock().live_values.restore_io_force(address, previous);
    }

    /// Release a forced I/O address.
    pub fn release_io(&self, address: &IoAddress) {
        self.lock().live_values.release_io(address);
    }

    /// Whether an active variable force holds the variable at `at` of `program`, the program a
    /// Control Studio service resolved the variable in: the forced variable itself, or a forced
    /// structure or instance whose bytes hold it, judged by place (specification 35, A6.5).
    pub(crate) fn forces_place(&self, program: &dyn ProgramRead, at: &Located) -> bool {
        let state = self.lock();
        state
            .live_values
            .forced_vars
            .iter()
            .any(|force| symbol_path::forced_holds(program, state.generation, &force.target, at))
    }

    pub(crate) fn forced_snapshot(&self) -> ForcedSnapshot {
        self.lock().live_values.forced_snapshot()
    }

    fn try_set_forced_var(
        &self,
        target: ForcedVarTarget,
        value: Value,
    ) -> Result<(), RequestQueueFull> {
        self.lock().live_values.try_upsert_forced_var(target, value)
    }

    fn clear_forced_var(&self, predicate: impl Fn(&ForcedVarTarget) -> bool) {
        self.lock().live_values.clear_forced_vars(predicate);
    }

    /// Report a refused write from a caller with no error channel as `write_rejected`, stamped
    /// with the time of the last published state. The refused request itself queues nothing.
    fn report_refused_request(&self, target: SmolStr) {
        let time = self.snapshot_time().unwrap_or(Duration::ZERO);
        self.push_runtime_event(RuntimeEvent::WriteRejected {
            target,
            error: RequestQueueFull.to_string(),
            time,
        });
    }

    /// Apply the queued Live Values of the program at `point` of its scan, through its symbol table
    /// (specification 44 §13.3): the variable writes and forces at the cycle start and when a hold
    /// ends, the image writes and the input and marker forces after the drivers read the inputs,
    /// and the output forces before the outputs reach the drivers. An instance target of another
    /// generation than `generation` is refused. Control Studio's task-boundary transactions are no
    /// Live Values: they commit at their task's start ([`Self::commit_parameters`],
    /// [`Self::commit_transfer`]) and are confirmed at its end.
    pub(crate) fn apply_compiled_live_values(
        &self,
        point: CompiledAccessPoint,
        generation: Generation,
        variables: &mut dyn CompiledVariables,
        time: Duration,
    ) -> Result<(), RuntimeError> {
        let mut sink = RejectedEvents::default();
        let result = {
            let mut state = self.lock();
            let queues = &mut state.live_values;
            if queues.is_idle() {
                return Ok(());
            }
            match point {
                CompiledAccessPoint::CycleStart | CompiledAccessPoint::HoldEnd => {
                    apply::compiled_cycle_start(queues, generation, variables, time, &mut sink)
                }
                CompiledAccessPoint::AfterInputs => apply::compiled_images(
                    queues,
                    &[IoArea::Input, IoArea::Memory],
                    variables,
                    time,
                    &mut sink,
                ),
                // The variable forces again after the scan, then the marker and output forces,
                // before the outputs reach the drivers (specification 10 §4.7, step 6).
                CompiledAccessPoint::BeforeOutputs => apply::compiled_forces(
                    queues, generation, variables, time, &mut sink,
                )
                .and_then(|()| {
                    apply::compiled_images(
                        queues,
                        &[IoArea::Memory, IoArea::Output],
                        variables,
                        time,
                        &mut sink,
                    )
                }),
                // Live Values reach the program at the cycle's boundaries only (specification 44
                // §13.3); a task's boundaries belong to the services that commit there.
                CompiledAccessPoint::TaskStart(_) | CompiledAccessPoint::TaskEnd(_) => Ok(()),
            }
        };
        // The refusals are reported after the state lock is released, through the normal event
        // path.
        for event in sink.events {
            self.push_runtime_event(event);
        }
        result
    }
}

/// The refusals one shared Live Values apply reported. They are held until the debug state lock is
/// released, so the report reuses the normal event path instead of taking the lock again.
#[derive(Default)]
struct RejectedEvents {
    events: Vec<RuntimeEvent>,
}

impl WriteSink for RejectedEvents {
    fn refused(&mut self, target: &str, error: &RuntimeError, time: Duration) {
        self.events.push(RuntimeEvent::WriteRejected {
            target: SmolStr::new(target),
            error: error.to_string(),
            time,
        });
    }
}

impl DebugState {
    /// End Control Studio's runtime epoch: the recordings, the parameter set and the transfer of
    /// the run that ended are interrupted with their reasons, and the signal inspection is cleared
    /// (specification 35, A6.7).
    fn end_control_epoch(&mut self) {
        // A place resolved in the run that ended names its variable only there.
        self.control_writes.clear();
        self.recordings.interrupt();
        self.parameters.interrupt();
        self.transfers.interrupt();
        self.signals.clear();
    }
}
