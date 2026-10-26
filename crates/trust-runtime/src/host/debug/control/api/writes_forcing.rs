use trust_runtime_core::memory::IoArea;
use trust_runtime_core::native::variables::{CompiledAccessPoint, CompiledVariables};
use trust_runtime_services::live_values::apply::{self, WriteSink};

impl DebugControl {
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

    /// Clear queued writes and active forces at a terminating runtime boundary.
    pub(crate) fn clear_runtime_mutations(&self) {
        self.lock().live_values.clear();
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
    /// of the previous lifecycle, and record the new generation, under one lock.
    pub(crate) fn begin_lifecycle(&self, generation: Generation) {
        let mut state = self.lock();
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

    /// Apply the queued Live Values of the program at `point` of its scan, through its symbol table
    /// (specification 44 §13.3): the variable writes and forces at the cycle start and when a hold
    /// ends, the image writes and the input and marker forces after the drivers read the inputs,
    /// and the output forces before the outputs reach the drivers. An instance target of another
    /// generation than `generation` is refused.
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
