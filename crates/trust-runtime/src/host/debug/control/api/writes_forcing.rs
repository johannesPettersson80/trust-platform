use crate::runtime::RuntimeMetadata;
use trust_runtime_core::vm::CycleAccess;
use trust_runtime_services::live_values::apply::{self, WriteSink};

impl DebugControl {
    /// Queue a one-shot I/O write. A refused write is reported as `write_rejected` and queues
    /// nothing; protocol and control producers use `try_enqueue_io_write` to receive the refusal.
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
        let (lock, _) = &*self.state;
        let mut state = lock.lock().expect("debug state poisoned");
        state.live_values.try_enqueue_io_write(address, value)
    }

    /// Withdraw the one-shot I/O write queued last for `address` and `value`. A caller uses it
    /// when its own checked write failed after the request was queued.
    pub fn withdraw_io_write(&self, address: &IoAddress, value: &Value) {
        let (lock, _) = &*self.state;
        let mut state = lock.lock().expect("debug state poisoned");
        state.live_values.withdraw_io_write(address, value);
    }

    /// Drain queued input writes.
    #[must_use]
    pub fn drain_io_writes(&self) -> Vec<(IoAddress, Value)> {
        let (lock, _) = &*self.state;
        let mut state = lock.lock().expect("debug state poisoned");
        state.live_values.drain_io_writes()
    }

    /// Queue a pending variable write to apply at the next cycle boundary. A refused write is
    /// reported as `write_rejected`; see `enqueue_io_write`.
    pub fn enqueue_global_write(&self, name: impl Into<SmolStr>, value: Value) {
        self.enqueue_var_write(PendingVarTarget::Global(name.into()), value);
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

    /// Queue a pending retained variable write to apply at the next cycle boundary.
    pub fn enqueue_retain_write(&self, name: impl Into<SmolStr>, value: Value) {
        self.enqueue_var_write(PendingVarTarget::Retain(name.into()), value);
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

    /// Queue a pending instance variable write to apply at the next cycle boundary.
    /// `generation` is the installation generation in which `instance_id` was resolved: the
    /// boundary rejects the write when another generation is installed (spec 34 §4.3.1).
    pub fn enqueue_instance_write(
        &self,
        generation: Generation,
        instance_id: InstanceId,
        name: impl Into<SmolStr>,
        value: Value,
    ) {
        self.enqueue_var_write(
            PendingVarTarget::Instance(generation, instance_id, name.into()),
            value,
        );
    }

    /// Queue an instance write from a control or protocol request. A new target past the queue
    /// bound is refused. `generation` is as for `enqueue_instance_write`.
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

    /// Queue a pending local variable write to apply at the next cycle boundary.
    pub fn enqueue_local_write(&self, frame_id: FrameId, name: impl Into<SmolStr>, value: Value) {
        self.enqueue_var_write(PendingVarTarget::Local(frame_id, name.into()), value);
    }

    /// Queue a local write from a control or protocol request. A new target past the queue bound
    /// is refused.
    pub fn try_enqueue_local_write(
        &self,
        frame_id: FrameId,
        name: impl Into<SmolStr>,
        value: Value,
    ) -> Result<(), RequestQueueFull> {
        self.try_enqueue_var_write(PendingVarTarget::Local(frame_id, name.into()), value)
    }

    /// Queue a pending lvalue write to apply at the next cycle boundary. A refused write is
    /// reported as `write_rejected`; see `enqueue_io_write`.
    pub fn enqueue_lvalue_write(&self, frame_id: Option<FrameId>, target: LValue, value: Value) {
        let label = SmolStr::new(format!("{target:?}"));
        let queued = self.try_write_lvalue(frame_id, target, value, |_| {
            Ok::<(), std::convert::Infallible>(())
        });
        if queued.is_err() {
            self.report_refused_request(label);
        }
    }

    /// Apply a debugger lvalue write to the paused snapshot and queue it for the next cycle
    /// boundary, under one lock. A full queue refuses the request with `RequestQueueFull` before
    /// the snapshot changes. `write_snapshot` applies the value to the snapshot, when there is
    /// one; its error is returned inside `Ok` and queues nothing.
    pub fn try_write_lvalue<E>(
        &self,
        frame_id: Option<FrameId>,
        target: LValue,
        value: Value,
        write_snapshot: impl FnOnce(&mut DebugSnapshot) -> Result<(), E>,
    ) -> Result<Result<(), E>, RequestQueueFull> {
        let (lock, _) = &*self.state;
        let mut state = lock.lock().expect("debug state poisoned");
        state.live_values.check_lvalue_write_capacity()?;
        if let Some(snapshot) = state.readable_snapshot_mut()
            && let Err(error) = write_snapshot(snapshot)
        {
            return Ok(Err(error));
        }
        state.live_values.enqueue_lvalue_write(PendingLValueWrite {
            frame_id,
            target,
            value,
        });
        Ok(Ok(()))
    }

    /// Drain pending variable writes. The scan applies the queue in place through the shared Live
    /// Values code, so only the control tests read the queued writes directly.
    #[must_use]
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn drain_var_writes(&self) -> Vec<PendingVarWrite> {
        let (lock, _) = &*self.state;
        let mut state = lock.lock().expect("debug state poisoned");
        state.live_values.drain_var_writes()
    }

    /// Drain pending lvalue writes. As for [`Self::drain_var_writes`], only the control tests read
    /// the queued writes directly.
    #[must_use]
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn drain_lvalue_writes(&self) -> Vec<PendingLValueWrite<LValue>> {
        let (lock, _) = &*self.state;
        let mut state = lock.lock().expect("debug state poisoned");
        state.live_values.drain_lvalue_writes()
    }

    /// Clear queued writes and active forces at a terminating runtime boundary.
    pub(crate) fn clear_runtime_mutations(&self) {
        let (lock, _) = &*self.state;
        let mut state = lock.lock().expect("debug state poisoned");
        state.live_values.clear();
    }

    fn enqueue_var_write(&self, target: PendingVarTarget, value: Value) {
        let label = apply::pending_var_label(&target);
        if self.try_enqueue_var_write(target, value).is_err() {
            self.report_refused_request(label);
        }
    }

    fn try_enqueue_var_write(
        &self,
        target: PendingVarTarget,
        value: Value,
    ) -> Result<(), RequestQueueFull> {
        let (lock, _) = &*self.state;
        let mut state = lock.lock().expect("debug state poisoned");
        state.live_values.try_upsert_var_write(target, value)
    }

    /// Force a global variable to the given value. A refused force is reported as
    /// `write_rejected`; see `enqueue_io_write`.
    pub fn force_global(&self, name: impl Into<SmolStr>, value: Value) {
        self.set_forced_var(ForcedVarTarget::Global(name.into()), value);
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

    /// Force a retained global variable to the given value.
    pub fn force_retain(&self, name: impl Into<SmolStr>, value: Value) {
        self.set_forced_var(ForcedVarTarget::Retain(name.into()), value);
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

    /// Force an instance variable to the given value. `generation` is the installation
    /// generation in which `instance_id` was resolved: the scan boundary rejects and removes the
    /// force once another generation is installed (spec 34 §4.3.1).
    pub fn force_instance(
        &self,
        generation: Generation,
        instance_id: InstanceId,
        name: impl Into<SmolStr>,
        value: Value,
    ) {
        self.set_forced_var(
            ForcedVarTarget::Instance(generation, instance_id, name.into()),
            value,
        );
    }

    /// Force an instance variable from a control or protocol request. A new target past the queue
    /// bound is refused. `generation` is as for `force_instance`.
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
        let (lock, _) = &*self.state;
        lock.lock().expect("debug state poisoned").generation
    }

    /// Record the installation generation of the runtime this control is attached to.
    pub(crate) fn set_generation(&self, generation: Generation) {
        let (lock, _) = &*self.state;
        lock.lock().expect("debug state poisoned").generation = generation;
    }

    /// A restart or replacement committed `generation`: clear the queued writes, active forces and
    /// published state of the previous lifecycle, and record the new generation, under one lock.
    /// The storage of the previous lifecycle is no longer the PLC's, so its state is not published.
    pub(crate) fn begin_lifecycle(&self, generation: Generation) {
        let (lock, _) = &*self.state;
        let mut state = lock.lock().expect("debug state poisoned");
        state.live_values.clear();
        state.snapshot = None;
        state.completed = None;
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

    /// Force an I/O address to the given value. A refused force is reported as `write_rejected`;
    /// see `enqueue_io_write`.
    pub fn force_io(&self, address: IoAddress, value: Value) {
        if self.try_force_io(address.clone(), value).is_err() {
            self.report_refused_request(SmolStr::new(format!("{address:?}")));
        }
    }

    /// Force an I/O address from a control or protocol request. A new address past the queue
    /// bound is refused; re-forcing an existing address is never refused.
    pub fn try_force_io(&self, address: IoAddress, value: Value) -> Result<(), RequestQueueFull> {
        let (lock, _) = &*self.state;
        let mut state = lock.lock().expect("debug state poisoned");
        state.live_values.try_upsert_forced_io(address, value)
    }

    /// The value of the active variable force on `target`, when the target is forced.
    pub fn forced_var_value(&self, target: &ForcedVarTarget) -> Option<Value> {
        let (lock, _) = &*self.state;
        let state = lock.lock().expect("debug state poisoned");
        state.live_values.forced_var_value(target)
    }

    /// Put back the variable force that a failed checked write had replaced: `previous` is the
    /// value forced before the failed request, or `None` when the target was not forced.
    pub fn restore_var_force(&self, target: &ForcedVarTarget, previous: Option<Value>) {
        let (lock, _) = &*self.state;
        let mut state = lock.lock().expect("debug state poisoned");
        state.live_values.restore_var_force(target, previous);
    }

    /// The value of the active I/O force on `address`, when the address is forced.
    pub fn forced_io_value(&self, address: &IoAddress) -> Option<Value> {
        let (lock, _) = &*self.state;
        let state = lock.lock().expect("debug state poisoned");
        state.live_values.forced_io_value(address)
    }

    /// Put back the I/O force that a failed checked write had replaced: `previous` is the value
    /// forced before the failed request, or `None` when the address was not forced.
    pub fn restore_io_force(&self, address: &IoAddress, previous: Option<Value>) {
        let (lock, _) = &*self.state;
        let mut state = lock.lock().expect("debug state poisoned");
        state.live_values.restore_io_force(address, previous);
    }

    /// Release a forced I/O address.
    pub fn release_io(&self, address: &IoAddress) {
        let (lock, _) = &*self.state;
        let mut state = lock.lock().expect("debug state poisoned");
        state.live_values.release_io(address);
    }

    pub(crate) fn forced_snapshot(&self) -> ForcedSnapshot {
        let (lock, _) = &*self.state;
        let state = lock.lock().expect("debug state poisoned");
        state.live_values.forced_snapshot()
    }

    fn set_forced_var(&self, target: ForcedVarTarget, value: Value) {
        let label = apply::forced_var_label(&target);
        if self.try_set_forced_var(target, value).is_err() {
            self.report_refused_request(label);
        }
    }

    fn try_set_forced_var(
        &self,
        target: ForcedVarTarget,
        value: Value,
    ) -> Result<(), RequestQueueFull> {
        let (lock, _) = &*self.state;
        let mut state = lock.lock().expect("debug state poisoned");
        state.live_values.try_upsert_forced_var(target, value)
    }

    fn clear_forced_var(&self, predicate: impl Fn(&ForcedVarTarget) -> bool) {
        let (lock, _) = &*self.state;
        let mut state = lock.lock().expect("debug state poisoned");
        state.live_values.clear_forced_vars(predicate);
    }

    /// Report a refused write from a caller with no error channel as `write_rejected`, stamped
    /// with the time of the last snapshot. The refused request itself queues nothing.
    fn report_refused_request(&self, target: SmolStr) {
        let time = {
            let (lock, _) = &*self.state;
            let state = lock.lock().expect("debug state poisoned");
            state
                .readable_snapshot()
                .map_or(crate::value::Duration::ZERO, |snapshot| snapshot.now)
        };
        self.push_runtime_event(RuntimeEvent::WriteRejected {
            target,
            error: RequestQueueFull.to_string(),
            time,
        });
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

impl DebugControl {
    /// Apply the queued Live Values writes at the cycle start through the shared code. The queued
    /// lvalue writes use the platform's HIR expression evaluation through `write_lvalue`.
    pub(crate) fn apply_live_values_cycle_start(
        &self,
        access: &mut CycleAccess<'_, '_>,
        metadata: &RuntimeMetadata,
        write_lvalue: impl FnMut(&mut CycleAccess<'_, '_>, &LValue, Value) -> Result<(), RuntimeError>,
    ) -> Result<(), RuntimeError> {
        self.run_live_values_apply(|queues, sink| {
            apply::cycle_start(
                queues,
                metadata.retained_names(),
                access,
                sink,
                write_lvalue,
            )
        })
    }

    /// Apply the queued I/O writes and then every active force before the input sample.
    pub(crate) fn apply_live_values_before_input_sample(
        &self,
        access: &mut CycleAccess<'_, '_>,
        metadata: &RuntimeMetadata,
    ) -> Result<(), RuntimeError> {
        self.run_live_values_apply(|queues, sink| {
            apply::io_writes_and_forces(queues, metadata.retained_names(), access, sink)
        })
    }

    /// Apply every active force again after the output sample.
    pub(crate) fn apply_live_values_after_output_sample(
        &self,
        access: &mut CycleAccess<'_, '_>,
        metadata: &RuntimeMetadata,
    ) -> Result<(), RuntimeError> {
        self.run_live_values_apply(|queues, sink| {
            apply::forces(queues, metadata.retained_names(), access, sink)
        })
    }

    /// Run one shared Live Values apply under the debug state lock, then report every refusal it
    /// collected through the normal event path.
    fn run_live_values_apply(
        &self,
        run: impl FnOnce(&mut LiveValueQueues<LValue>, &mut RejectedEvents) -> Result<(), RuntimeError>,
    ) -> Result<(), RuntimeError> {
        let mut sink = RejectedEvents::default();
        let result = {
            let (lock, _) = &*self.state;
            let mut state = lock.lock().expect("debug state poisoned");
            run(&mut state.live_values, &mut sink)
        };
        for event in sink.events {
            self.push_runtime_event(event);
        }
        result
    }
}
