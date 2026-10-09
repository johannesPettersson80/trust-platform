/// What a scan observer receives for one cycle: the process time, the storage and the I/O snapshot of the cycle, the tasks
/// that ran in it, and whether the runtime's target is simulated (Specification 35, A2.4).
pub(crate) struct ScanView<'a> {
    pub time: std::time::Duration,
    pub storage: &'a crate::memory::VariableStorage,
    pub tasks: &'a [(smol_str::SmolStr, u64)],
    pub io: &'a IoSnapshot,
    pub simulated_target: bool,
}

/// The target facts that one published cycle carries (Specification 35, A2.3 and A2.4).
#[derive(Clone, Copy)]
struct CycleFacts {
    simulation_only_io: bool,
    simulated_target: bool,
}

/// A callback that runs once per published cycle, after the debug state lock is released, so it may call back into the
/// handle it receives.
/// The function a scan observer calls for each published cycle.
type ScanCallback = dyn for<'a> Fn(&DebugControl, &ScanView<'a>) + Send + Sync;

#[derive(Clone)]
pub(crate) struct ScanObserver(std::sync::Arc<ScanCallback>);

impl ScanObserver {
    pub(crate) fn new(
        observer: impl for<'a> Fn(&DebugControl, &ScanView<'a>) + Send + Sync + 'static,
    ) -> Self {
        Self(std::sync::Arc::new(observer))
    }
}

impl std::fmt::Debug for ScanObserver {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("ScanObserver")
    }
}

impl DebugControl {
    pub fn register_watch_expression(&self, expr: Expr) {
        let (lock, _) = &*self.state;
        let mut state = lock.lock().expect("debug state poisoned");
        state.watches.push(WatchEntry { expr, last: None });
    }

    /// Clear watch expressions.
    pub fn clear_watch_expressions(&self) {
        let (lock, _) = &*self.state;
        let mut state = lock.lock().expect("debug state poisoned");
        state.watches.clear();
        state.watch_changed = false;
    }

    /// Returns whether watch values changed since the last stop, and resets the flag.
    #[must_use]
    pub fn take_watch_changed(&self) -> bool {
        let (lock, _) = &*self.state;
        let mut state = lock.lock().expect("debug state poisoned");
        let changed = state.watch_changed;
        state.watch_changed = false;
        changed
    }

    /// Stream log output to a sender instead of buffering.
    pub fn set_log_sender(&self, sender: Sender<DebugLog>) {
        let (lock, _) = &*self.state;
        let mut state = lock.lock().expect("debug state poisoned");
        state.log_tx = Some(sender);
    }

    /// Stop streaming log output; new logs will buffer.
    pub fn clear_log_sender(&self) {
        let (lock, _) = &*self.state;
        let mut state = lock.lock().expect("debug state poisoned");
        state.log_tx = None;
    }

    /// Stream I/O snapshots to a sender.
    pub fn set_io_sender(&self, sender: Sender<IoSnapshot>) {
        let (lock, _) = &*self.state;
        let mut state = lock.lock().expect("debug state poisoned");
        state.io_tx = Some(sender);
    }

    /// Stop streaming I/O snapshots.
    pub fn clear_io_sender(&self) {
        let (lock, _) = &*self.state;
        let mut state = lock.lock().expect("debug state poisoned");
        state.io_tx = None;
    }

    /// Stream runtime events to a sender.
    pub fn set_runtime_sender(&self, sender: Sender<RuntimeEvent>) {
        let (lock, _) = &*self.state;
        let mut state = lock.lock().expect("debug state poisoned");
        state.runtime_tx = Some(sender);
    }

    /// Stop streaming runtime events.
    pub fn clear_runtime_sender(&self) {
        let (lock, _) = &*self.state;
        let mut state = lock.lock().expect("debug state poisoned");
        state.runtime_tx = None;
    }

    /// Stream stop events to a sender.
    pub fn set_stop_sender(&self, sender: Sender<DebugStop>) {
        let (lock, _) = &*self.state;
        let mut state = lock.lock().expect("debug state poisoned");
        state.stop_tx = Some(sender);
    }

    /// Stop streaming stop events.
    pub fn clear_stop_sender(&self) {
        let (lock, _) = &*self.state;
        let mut state = lock.lock().expect("debug state poisoned");
        state.stop_tx = None;
    }

    /// Emit an I/O snapshot to listeners, if configured.
    pub fn push_io_snapshot(&self, snapshot: IoSnapshot) {
        self.publish_io_snapshot(snapshot,None,None,&[],None);
    }
    pub fn push_io_snapshot_at(&self,snapshot:IoSnapshot,time:i64) {
        self.publish_io_snapshot(snapshot,Some(time),None,&[],None);
    }
    /// A cycle's snapshot, with the tasks that ran in it, the intervals each missed, whether the runtime's I/O is
    /// simulation-only and whether its target is simulated (Specification 35, A2.3 and A2.4). The scan observer runs once
    /// the state lock is released.
    pub fn push_control_snapshot_at(&self,snapshot:IoSnapshot,time:i64,storage:&crate::memory::VariableStorage,tasks:&[(smol_str::SmolStr,u64)],simulation_only_io:bool,simulated_target:bool) {
        let facts=CycleFacts{simulation_only_io,simulated_target};
        self.publish_io_snapshot(snapshot,Some(time),Some(storage),tasks,Some(facts));
    }
    fn publish_io_snapshot(&self,snapshot:IoSnapshot,time:Option<i64>,storage:Option<&crate::memory::VariableStorage>,tasks:&[(smol_str::SmolStr,u64)],facts:Option<CycleFacts>) {
        let observer={
            let mut state=self.state.0.lock().expect("debug state poisoned");
            let forced=state.forced_vars.clone();
            if let (Some(time),Some(storage))=(time,storage) {state.signals.capture(&snapshot,time,storage,&forced);}
            state.recordings.capture_coherent(&snapshot,time,storage,&forced,tasks);
            state.latest_io_snapshot=Some(snapshot.clone());
            if let Some(facts)=facts {state.latest_simulation_only_io=Some(facts.simulation_only_io);}
            if let Some(sender)=&state.io_tx {
                if sender.send(snapshot.clone()).is_err(){tracing::debug!("debug I/O snapshot sender closed");}
            }
            state.scan_observer.clone()
        };
        // The observer may call back into this handle, which takes the state lock again: it runs only after the lock is free.
        if let (Some(observer),Some(time),Some(storage),Some(facts))=(observer,time,storage,facts) {
            let view=ScanView{time:std::time::Duration::from_nanos(u64::try_from(time).unwrap_or(0)),storage,tasks,io:&snapshot,simulated_target:facts.simulated_target};
            (*observer.0)(self,&view);
        }
    }
    /// Registers the observer that runs once per published cycle, replacing any earlier one.
    pub(crate) fn set_scan_observer(&self,observer:ScanObserver) {
        let (lock,_)=&*self.state;
        let mut state=lock.lock().expect("debug state poisoned");
        state.scan_observer=Some(observer);
    }
    /// Whether the runtime's I/O was simulation-only at the last published cycle; `None` before the first cycle.
    #[must_use]
    pub fn simulation_only_io(&self)->Option<bool> {
        let (lock,_)=&*self.state;
        lock.lock().expect("debug state poisoned").latest_simulation_only_io
    }

    /// Return the latest process-image snapshot emitted by the runtime cycle.
    #[must_use]
    pub fn latest_io_snapshot(&self) -> Option<IoSnapshot> {
        let (lock, _) = &*self.state;
        lock.lock()
            .expect("debug state poisoned")
            .latest_io_snapshot
            .clone()
    }

    /// Emit a runtime event to listeners, if configured.
    pub fn push_runtime_event(&self, event: RuntimeEvent) {
        let (lock, _) = &*self.state;
        let mut state = lock.lock().expect("debug state poisoned");
        if let Some(sender) = &state.runtime_tx {
            if sender.send(event.clone()).is_err() {
                state.runtime_tx = None;
                state.runtime_events.push(event);
            }
        } else {
            state.runtime_events.push(event);
        }
    }

    /// Refresh the stored snapshot using the provided debug runtime context.
    pub fn refresh_snapshot(&self, ctx: &mut DebugRuntimeContext<'_>) {
        let (lock, _) = &*self.state;
        let mut state = lock.lock().expect("debug state poisoned");
        update_watch_snapshot(&mut state, ctx);
        update_snapshot(&mut state, ctx);
    }

    /// Refresh the stored snapshot from raw runtime storage.
    ///
    /// This is used by VM backends that do not execute through the interpreter context
    /// but still need paused-state snapshots for debugger stack/scopes queries.
    pub fn refresh_snapshot_from_storage(
        &self,
        storage: &crate::memory::VariableStorage,
        now: crate::value::Duration,
    ) {
        let (lock, _) = &*self.state;
        let mut state = lock.lock().expect("debug state poisoned");
        state.snapshot = Some(DebugSnapshot {
            storage: storage.clone(),
            now,
        });
    }
}
