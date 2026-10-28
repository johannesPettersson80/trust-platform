/// What a scan observer receives for one cycle: the process time, the program as the scan left it
/// and the I/O snapshot of the cycle, the tasks that ran in it, and whether the runtime's target is
/// simulated (Specification 35, A2.4).
pub(crate) struct ScanView<'a> {
    pub time: std::time::Duration,
    /// The program as the scan left it, which the experiment executor reads a variable of through
    /// the program image's symbol and type tables (specification 44 §13.3).
    pub program: &'a dyn ProgramRead,
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

/// What a control snapshot carries beyond the I/O snapshot: the scan's process time in
/// nanoseconds, the program as the scan left it, its task runs and its target facts.
struct ControlScan<'a> {
    time: i64,
    program: &'a dyn ProgramRead,
    tasks: &'a [(smol_str::SmolStr, u64)],
    facts: CycleFacts,
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
    /// Stream I/O snapshots to a sender.
    pub fn set_io_sender(&self, sender: Sender<IoSnapshot>) {
        self.lock().io_tx = Some(sender);
    }

    /// Stop streaming I/O snapshots.
    pub fn clear_io_sender(&self) {
        self.lock().io_tx = None;
    }

    /// Stream runtime events to a sender.
    pub fn set_runtime_sender(&self, sender: Sender<RuntimeEvent>) {
        self.lock().runtime_tx = Some(sender);
    }

    /// Stop streaming runtime events.
    pub fn clear_runtime_sender(&self) {
        self.lock().runtime_tx = None;
    }

    /// Emit an I/O snapshot to listeners, if configured, and keep it as the latest one.
    ///
    /// Control Studio's recordings and signal inspection take each scan once, from
    /// [`Self::push_control_snapshot_at`]: a scan published both ways streams its snapshot twice but
    /// is recorded once.
    pub fn push_io_snapshot(&self, snapshot: IoSnapshot) {
        self.publish_io_snapshot(snapshot, None);
    }

    /// A cycle's snapshot at process time `time`, in nanoseconds, with the program as the scan left
    /// it, the tasks that ran in the scan and the intervals each missed before it, whether the
    /// runtime's I/O is simulation-only and whether its target is simulated (Specification 35,
    /// A2.3, A2.4, A3.5, A4.1): it is streamed and kept as [`Self::push_io_snapshot`] keeps it, the
    /// recordings and the signal inspection take the scan, and the scan observer (the experiment
    /// executor) runs once the state lock is released. The runtime publishes it once per scan, after
    /// the scan.
    pub fn push_control_snapshot_at(
        &self,
        snapshot: IoSnapshot,
        time: i64,
        program: &dyn ProgramRead,
        tasks: &[(smol_str::SmolStr, u64)],
        simulation_only_io: bool,
        simulated_target: bool,
    ) {
        let scan = ControlScan {
            time,
            program,
            tasks,
            facts: CycleFacts {
                simulation_only_io,
                simulated_target,
            },
        };
        self.publish_io_snapshot(snapshot, Some(scan));
    }

    fn publish_io_snapshot(&self, snapshot: IoSnapshot, scan: Option<ControlScan<'_>>) {
        let observer = {
            let mut state = self.lock();
            if let Some(scan) = &scan {
                let state = &mut *state;
                let forced = &state.live_values.forced_vars;
                state
                    .signals
                    .capture(&snapshot, scan.time, scan.program, forced, state.generation);
                state.recordings.capture_coherent(
                    &snapshot,
                    Some(scan.time),
                    scan.program,
                    forced,
                    state.generation,
                    scan.tasks,
                );
                state.latest_simulation_only_io = Some(scan.facts.simulation_only_io);
                // The tasks that ran in the current program, for a controller's record of its mode (E7.7).
                state.parameters.record_task_runs(scan.tasks);
            }
            state.latest_io_snapshot = Some(snapshot.clone());
            if let Some(sender) = &state.io_tx
                && sender.send(snapshot.clone()).is_err()
            {
                tracing::debug!("debug I/O snapshot sender closed");
            }
            state.scan_observer.clone()
        };
        // The observer may call back into this handle, which takes the state lock again: it runs
        // only after the lock is free.
        if let (Some(observer), Some(scan)) = (observer, scan) {
            let view = ScanView {
                time: std::time::Duration::from_nanos(u64::try_from(scan.time).unwrap_or(0)),
                program: scan.program,
                tasks: scan.tasks,
                io: &snapshot,
                simulated_target: scan.facts.simulated_target,
            };
            (*observer.0)(self, &view);
        }
    }

    /// Registers the observer that runs once per published cycle, replacing any earlier one.
    pub(crate) fn set_scan_observer(&self, observer: ScanObserver) {
        self.lock().scan_observer = Some(observer);
    }

    /// Whether the runtime's I/O was simulation-only at the last published cycle; `None` before the first cycle.
    #[must_use]
    pub fn simulation_only_io(&self) -> Option<bool> {
        self.lock().latest_simulation_only_io
    }

    /// Return the latest process-image snapshot emitted by the runtime cycle.
    #[must_use]
    pub fn latest_io_snapshot(&self) -> Option<IoSnapshot> {
        self.lock().latest_io_snapshot.clone()
    }

    /// Emit a runtime event to listeners, if configured. Without a live sender the event is
    /// buffered for `drain_runtime_events`; that buffer is bounded, and `runtime_events_dropped`
    /// counts the events it has dropped.
    pub fn push_runtime_event(&self, event: RuntimeEvent) {
        let mut state = self.lock();
        if let Some(sender) = &state.runtime_tx {
            if sender.send(event.clone()).is_err() {
                state.runtime_tx = None;
                buffer_runtime_event(&mut state, event);
            }
        } else {
            buffer_runtime_event(&mut state, event);
        }
    }
}

/// Buffer a runtime event for direct draining. At the shared bound the oldest buffered event is
/// dropped and counted, so the caller can tell that it missed events.
fn buffer_runtime_event(state: &mut DebugState, event: RuntimeEvent) {
    trust_runtime_services::events::buffer_event(
        &mut state.runtime_events,
        &mut state.runtime_events_dropped,
        event,
    );
}
