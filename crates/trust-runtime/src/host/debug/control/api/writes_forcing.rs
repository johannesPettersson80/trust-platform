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
        if state.io_writes.len() >= REQUEST_QUEUE_CAPACITY {
            return Err(RequestQueueFull);
        }
        state.io_writes.push((address, value));
        Ok(())
    }

    /// Withdraw the one-shot I/O write queued last for `address` and `value`. A caller uses it
    /// when its own checked write failed after the request was queued.
    pub fn withdraw_io_write(&self, address: &IoAddress, value: &Value) {
        let (lock, _) = &*self.state;
        let mut state = lock.lock().expect("debug state poisoned");
        if let Some(index) = state
            .io_writes
            .iter()
            .rposition(|(queued, queued_value)| queued == address && queued_value == value)
        {
            state.io_writes.remove(index);
        }
    }

    /// Drain queued input writes.
    #[must_use]
    pub fn drain_io_writes(&self) -> Vec<(IoAddress, Value)> {
        let (lock, _) = &*self.state;
        let mut state = lock.lock().expect("debug state poisoned");
        std::mem::take(&mut state.io_writes)
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
    pub fn enqueue_instance_write(
        &self,
        instance_id: InstanceId,
        name: impl Into<SmolStr>,
        value: Value,
    ) {
        self.enqueue_var_write(PendingVarTarget::Instance(instance_id, name.into()), value);
    }

    /// Queue an instance write from a control or protocol request. A new target past the queue
    /// bound is refused.
    pub fn try_enqueue_instance_write(
        &self,
        instance_id: InstanceId,
        name: impl Into<SmolStr>,
        value: Value,
    ) -> Result<(), RequestQueueFull> {
        self.try_enqueue_var_write(PendingVarTarget::Instance(instance_id, name.into()), value)
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
        if state.pending_lvalue_writes.len() >= REQUEST_QUEUE_CAPACITY {
            return Err(RequestQueueFull);
        }
        if let Some(snapshot) = state.snapshot.as_mut()
            && let Err(error) = write_snapshot(snapshot)
        {
            return Ok(Err(error));
        }
        state.pending_lvalue_writes.push(PendingLValueWrite {
            frame_id,
            target,
            value,
        });
        Ok(Ok(()))
    }

    /// Drain pending variable writes.
    #[must_use]
    pub(crate) fn drain_var_writes(&self) -> Vec<PendingVarWrite> {
        let (lock, _) = &*self.state;
        let mut state = lock.lock().expect("debug state poisoned");
        std::mem::take(&mut state.pending_var_writes)
    }

    /// Drain pending lvalue writes.
    #[must_use]
    pub(crate) fn drain_lvalue_writes(&self) -> Vec<PendingLValueWrite> {
        let (lock, _) = &*self.state;
        let mut state = lock.lock().expect("debug state poisoned");
        std::mem::take(&mut state.pending_lvalue_writes)
    }

    /// Clear queued writes and active forces at a terminating runtime boundary.
    pub(crate) fn clear_runtime_mutations(&self) {
        let (lock, _) = &*self.state;
        let mut state = lock.lock().expect("debug state poisoned");
        state.clear_runtime_mutations();
    }

    fn enqueue_var_write(&self, target: PendingVarTarget, value: Value) {
        let label = pending_var_label(&target);
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
        state.try_upsert_var_write(target, value)
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

    /// Force an instance variable to the given value.
    pub fn force_instance(&self, instance_id: InstanceId, name: impl Into<SmolStr>, value: Value) {
        self.set_forced_var(ForcedVarTarget::Instance(instance_id, name.into()), value);
    }

    /// Force an instance variable from a control or protocol request. A new target past the queue
    /// bound is refused.
    pub fn try_force_instance(
        &self,
        instance_id: InstanceId,
        name: impl Into<SmolStr>,
        value: Value,
    ) -> Result<(), RequestQueueFull> {
        self.try_set_forced_var(ForcedVarTarget::Instance(instance_id, name.into()), value)
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

    /// Release a forced instance variable.
    pub fn release_instance(&self, instance_id: InstanceId, name: &str) {
        self.clear_forced_var(|target| match target {
            ForcedVarTarget::Instance(current_id, current_name) => {
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
        state.try_upsert_forced_io(address, value)
    }

    /// The value of the active variable force on `target`, when the target is forced.
    pub fn forced_var_value(&self, target: &ForcedVarTarget) -> Option<Value> {
        let (lock, _) = &*self.state;
        let state = lock.lock().expect("debug state poisoned");
        state
            .forced_vars
            .iter()
            .find(|entry| entry.target == *target)
            .map(|entry| entry.value.clone())
    }

    /// Put back the variable force that a failed checked write had replaced: `previous` is the
    /// value forced before the failed request, or `None` when the target was not forced.
    pub fn restore_var_force(&self, target: &ForcedVarTarget, previous: Option<Value>) {
        let (lock, _) = &*self.state;
        let mut state = lock.lock().expect("debug state poisoned");
        match previous {
            Some(value) => state.upsert_forced_var(target.clone(), value),
            None => state.forced_vars.retain(|entry| entry.target != *target),
        }
    }

    /// The value of the active I/O force on `address`, when the address is forced.
    pub fn forced_io_value(&self, address: &IoAddress) -> Option<Value> {
        let (lock, _) = &*self.state;
        let state = lock.lock().expect("debug state poisoned");
        state
            .forced_io
            .iter()
            .find(|(current, _)| current == address)
            .map(|(_, value)| value.clone())
    }

    /// Put back the I/O force that a failed checked write had replaced: `previous` is the value
    /// forced before the failed request, or `None` when the address was not forced.
    pub fn restore_io_force(&self, address: &IoAddress, previous: Option<Value>) {
        let (lock, _) = &*self.state;
        let mut state = lock.lock().expect("debug state poisoned");
        match previous {
            Some(value) => state.upsert_forced_io(address.clone(), value),
            None => state.forced_io.retain(|(current, _)| current != address),
        }
    }

    /// Release a forced I/O address.
    pub fn release_io(&self, address: &IoAddress) {
        let (lock, _) = &*self.state;
        let mut state = lock.lock().expect("debug state poisoned");
        state.forced_io.retain(|(current, _)| current != address);
    }

    pub(crate) fn forced_snapshot(&self) -> ForcedSnapshot {
        let (lock, _) = &*self.state;
        let state = lock.lock().expect("debug state poisoned");
        ForcedSnapshot {
            vars: state.forced_vars.clone(),
            io: state.forced_io.clone(),
        }
    }

    /// Remove an I/O force whose application the scan boundary rejected, so the rejected value is
    /// not applied again. The entry goes only while it still holds the rejected value: a newer
    /// force on the same address survives.
    pub(crate) fn discard_rejected_io_force(&self, address: &IoAddress, value: &Value) {
        let (lock, _) = &*self.state;
        let mut state = lock.lock().expect("debug state poisoned");
        state
            .forced_io
            .retain(|(current, current_value)| !(current == address && current_value == value));
    }

    /// Remove a variable force whose application the scan boundary rejected. As for I/O forces,
    /// the entry goes only while it still holds the rejected value.
    pub(crate) fn discard_rejected_var_force(&self, target: &ForcedVarTarget, value: &Value) {
        let (lock, _) = &*self.state;
        let mut state = lock.lock().expect("debug state poisoned");
        state
            .forced_vars
            .retain(|entry| !(entry.target == *target && entry.value == *value));
    }

    fn set_forced_var(&self, target: ForcedVarTarget, value: Value) {
        let label = forced_var_label(&target);
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
        state.try_upsert_forced_var(target, value)
    }

    fn clear_forced_var(&self, predicate: impl Fn(&ForcedVarTarget) -> bool) {
        let (lock, _) = &*self.state;
        let mut state = lock.lock().expect("debug state poisoned");
        state.forced_vars.retain(|entry| !predicate(&entry.target));
    }

    /// Report a refused write from a caller with no error channel as `write_rejected`, stamped
    /// with the time of the last snapshot. The refused request itself queues nothing.
    fn report_refused_request(&self, target: SmolStr) {
        let time = {
            let (lock, _) = &*self.state;
            let state = lock.lock().expect("debug state poisoned");
            state
                .snapshot
                .as_ref()
                .map_or(crate::value::Duration::ZERO, |snapshot| snapshot.now)
        };
        self.push_runtime_event(RuntimeEvent::WriteRejected {
            target,
            error: RequestQueueFull.to_string(),
            time,
        });
    }
}

/// The destination label of a pending write, as the runtime reports it in events.
fn pending_var_label(target: &PendingVarTarget) -> SmolStr {
    match target {
        PendingVarTarget::Global(name) | PendingVarTarget::Retain(name) => name.clone(),
        PendingVarTarget::Instance(id, name) => SmolStr::new(format!("{}.{name}", id.0)),
        PendingVarTarget::Local(id, name) => SmolStr::new(format!("{}.{name}", id.0)),
    }
}

/// The destination label of a variable force, as the runtime reports it in events.
fn forced_var_label(target: &ForcedVarTarget) -> SmolStr {
    match target {
        ForcedVarTarget::Global(name) | ForcedVarTarget::Retain(name) => name.clone(),
        ForcedVarTarget::Instance(id, name) => SmolStr::new(format!("{}.{name}", id.0)),
    }
}

impl DebugState {
    fn clear_runtime_mutations(&mut self) {
        self.io_writes.clear();
        self.pending_var_writes.clear();
        self.pending_lvalue_writes.clear();
        self.forced_vars.clear();
        self.forced_io.clear();
    }

    /// Replace the pending write to `target` in place, or append it. Host-internal writers that
    /// are not bounded requests reach this through the unbounded wrappers.
    fn upsert_var_write(&mut self, target: PendingVarTarget, value: Value) {
        if let Some(entry) = self
            .pending_var_writes
            .iter_mut()
            .find(|entry| entry.target == target)
        {
            entry.value = value;
        } else {
            self.pending_var_writes
                .push(PendingVarWrite { target, value });
        }
    }

    /// `upsert_var_write` for a control or protocol request. Replacing a queued target is never
    /// refused; a new target past the queue bound is.
    fn try_upsert_var_write(
        &mut self,
        target: PendingVarTarget,
        value: Value,
    ) -> Result<(), RequestQueueFull> {
        let is_new = self
            .pending_var_writes
            .iter()
            .all(|entry| entry.target != target);
        if is_new && self.pending_var_writes.len() >= REQUEST_QUEUE_CAPACITY {
            return Err(RequestQueueFull);
        }
        self.upsert_var_write(target, value);
        Ok(())
    }

    fn upsert_forced_var(&mut self, target: ForcedVarTarget, value: Value) {
        if let Some(entry) = self
            .forced_vars
            .iter_mut()
            .find(|entry| entry.target == target)
        {
            entry.value = value;
        } else {
            self.forced_vars.push(ForcedVar { target, value });
        }
    }

    /// `upsert_forced_var` for a control or protocol request, with the same bound as
    /// `try_upsert_var_write`.
    fn try_upsert_forced_var(
        &mut self,
        target: ForcedVarTarget,
        value: Value,
    ) -> Result<(), RequestQueueFull> {
        let is_new = self.forced_vars.iter().all(|entry| entry.target != target);
        if is_new && self.forced_vars.len() >= REQUEST_QUEUE_CAPACITY {
            return Err(RequestQueueFull);
        }
        self.upsert_forced_var(target, value);
        Ok(())
    }

    fn upsert_forced_io(&mut self, address: IoAddress, value: Value) {
        if let Some(entry) = self
            .forced_io
            .iter_mut()
            .find(|(current, _)| *current == address)
        {
            entry.1 = value;
        } else {
            self.forced_io.push((address, value));
        }
    }

    /// `upsert_forced_io` for a control or protocol request, with the same bound as
    /// `try_upsert_var_write`.
    fn try_upsert_forced_io(
        &mut self,
        address: IoAddress,
        value: Value,
    ) -> Result<(), RequestQueueFull> {
        let is_new = self
            .forced_io
            .iter()
            .all(|(current, _)| *current != address);
        if is_new && self.forced_io.len() >= REQUEST_QUEUE_CAPACITY {
            return Err(RequestQueueFull);
        }
        self.upsert_forced_io(address, value);
        Ok(())
    }
}
