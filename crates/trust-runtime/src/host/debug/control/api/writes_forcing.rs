impl DebugControl {
    pub fn enqueue_io_write(&self, address: IoAddress, value: Value) {
        let (lock, _) = &*self.state;
        let mut state = lock.lock().expect("debug state poisoned");
        state.io_writes.push((address, value));
    }

    /// Drain queued input writes.
    #[must_use]
    pub fn drain_io_writes(&self) -> Vec<(IoAddress, Value)> {
        let (lock, _) = &*self.state;
        let mut state = lock.lock().expect("debug state poisoned");
        std::mem::take(&mut state.io_writes)
    }

    /// Queue a pending variable write to apply at the next cycle boundary.
    pub fn enqueue_global_write(&self, name: impl Into<SmolStr>, value: Value) {
        self.enqueue_var_write(PendingVarTarget::Global(name.into()), value);
    }

    /// Queue a pending retained variable write to apply at the next cycle boundary.
    pub fn enqueue_retain_write(&self, name: impl Into<SmolStr>, value: Value) {
        self.enqueue_var_write(PendingVarTarget::Retain(name.into()), value);
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

    /// Queue a pending local variable write to apply at the next cycle boundary.
    pub fn enqueue_local_write(&self, frame_id: FrameId, name: impl Into<SmolStr>, value: Value) {
        self.enqueue_var_write(PendingVarTarget::Local(frame_id, name.into()), value);
    }

    /// Queue a pending lvalue write to apply at the next cycle boundary.
    pub fn enqueue_lvalue_write(
        &self,
        frame_id: Option<FrameId>,
        target: LValue,
        value: Value,
    ) {
        let (lock, _) = &*self.state;
        let mut state = lock.lock().expect("debug state poisoned");
        state.pending_lvalue_writes.push(PendingLValueWrite {
            frame_id,
            target,
            value,
        });
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

    fn enqueue_var_write(&self, target: PendingVarTarget, value: Value) {
        let (lock, _) = &*self.state;
        let mut state = lock.lock().expect("debug state poisoned");
        if let Some(entry) = state
            .pending_var_writes
            .iter_mut()
            .find(|entry| entry.target == target)
        {
            entry.value = value;
        } else {
            state
                .pending_var_writes
                .push(PendingVarWrite { target, value });
        }
    }

    /// Force a global variable to the given value.
    pub fn force_global(&self, name: impl Into<SmolStr>, value: Value) {
        self.set_forced_var(ForcedVarTarget::Global(name.into()), value);
    }

    /// Force a retained global variable to the given value.
    pub fn force_retain(&self, name: impl Into<SmolStr>, value: Value) {
        self.set_forced_var(ForcedVarTarget::Retain(name.into()), value);
    }

    /// Force an instance variable to the given value.
    pub fn force_instance(&self, instance_id: InstanceId, name: impl Into<SmolStr>, value: Value) {
        self.set_forced_var(ForcedVarTarget::Instance(instance_id, name.into()), value);
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

    /// Force an I/O address to the given value.
    pub fn force_io(&self, address: IoAddress, value: Value) {
        let (lock, _) = &*self.state;
        let mut state = lock.lock().expect("debug state poisoned");
        if let Some(entry) = state
            .forced_io
            .iter_mut()
            .find(|(current, _)| *current == address)
        {
            entry.1 = value;
        } else {
            state.forced_io.push((address, value));
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

    fn set_forced_var(&self, target: ForcedVarTarget, value: Value) {
        let (lock, _) = &*self.state;
        let mut state = lock.lock().expect("debug state poisoned");
        if let Some(entry) = state
            .forced_vars
            .iter_mut()
            .find(|entry| entry.target == target)
        {
            entry.value = value;
        } else {
            state.forced_vars.push(ForcedVar { target, value });
        }
    }

    fn clear_forced_var(&self, predicate: impl Fn(&ForcedVarTarget) -> bool) {
        let (lock, _) = &*self.state;
        let mut state = lock.lock().expect("debug state poisoned");
        state.forced_vars.retain(|entry| !predicate(&entry.target));
    }

}
