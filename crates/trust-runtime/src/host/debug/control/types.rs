/// Debugger execution mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebugMode {
    /// Execute statements without pausing.
    Running,
    /// Pause at the next statement boundary.
    Paused,
}

/// Control actions requested by the debug adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlAction {
    /// Pause execution at the next statement boundary.
    Pause(Option<u32>),
    /// Continue running.
    Continue,
    /// Execute a single statement, then pause.
    StepIn(Option<u32>),
    /// Step over the current statement.
    StepOver(Option<u32>),
    /// Step out to the caller.
    StepOut(Option<u32>),
}

/// Outcome of applying a control action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlOutcome {
    /// The action changed the debug state.
    Applied,
    /// The action was ignored because it had no effect.
    Ignored,
}

/// Step behavior while running.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepKind {
    /// Pause after the next statement, regardless of call depth.
    Into,
    /// Pause after completing the current statement at the same call depth.
    Over,
    /// Pause after returning to the caller (lower call depth).
    Out,
}
#[derive(Debug, Clone, Copy)]
struct StepState {
    kind: StepKind,
    target_depth: u32,
    started: bool,
}

#[derive(Debug)]
struct DebugState {
    mode: DebugMode,
    last_location: Option<SourceLocation>,
    last_call_depth: u32,
    last_call_depths: HashMap<u32, u32>,
    current_thread: Option<u32>,
    target_thread: Option<u32>,
    breakpoints: Vec<DebugBreakpoint>,
    breakpoint_generation: HashMap<u32, u64>,
    /// Start of the debugger hold the scan thread is waiting in, if one is in progress. It is set
    /// and cleared under the same lock as the watchdog pause total, so hosted hang supervision
    /// never sees a hold half-recorded.
    watchdog_hold_started: Option<std::time::Instant>,
    /// Set by a resource stop (`DebugControl::release_for_stop`). The stop ended the hold, and the
    /// debugger holds the scan no more until the next resource thread starts, so no pause, step
    /// or breakpoint can delay the stop (specification 11 section 6.9, specification 13 "Pause").
    stop_requested: bool,
    breakpoint_stops_this_cycle: HashSet<BreakpointCycleKey>,
    frame_locations: HashMap<FrameId, SourceLocation>,
    logs: Vec<DebugLog>,
    /// The halted state: the storage and call stack at the stop that holds the scan. It is the
    /// state reads answer from while the debugger holds the scan, and it is cleared when the hold
    /// ends (specification 11 section 6.13.1).
    snapshot: Option<DebugSnapshot>,
    /// The state of the last completed scan, published at its end. Reads answer from it whenever
    /// no hold is in effect, so a scan in progress is never visible (specification 11 section
    /// 6.13.1). It is shared, so a read clones it outside the state lock.
    completed: Option<Arc<DebugSnapshot>>,
    /// Storage copies made for the debugger: one per completed scan that published, one per
    /// stop or conditional evaluation that built a projection.
    storage_copies: u64,
    watches: Vec<WatchEntry>,
    watch_changed: bool,
    log_tx: Option<Sender<DebugLog>>,
    io_tx: Option<Sender<IoSnapshot>>,
    stop_tx: Option<Sender<DebugStop>>,
    runtime_tx: Option<Sender<RuntimeEvent>>,
    runtime_events: VecDeque<RuntimeEvent>,
    runtime_events_dropped: u64,
    pending_stop: Option<DebugStopReason>,
    stops: Vec<DebugStop>,
    last_stop: Option<DebugStop>,
    steps: HashMap<u32, StepState>,
    /// The queued writes and active forces, with their capacity and rules (spec 11 "Live Values";
    /// spec 34 §4.3.1). Held here so every queue operation takes the one state lock.
    live_values: LiveValueQueues<LValue>,
    /// The installation generation of the attached runtime, published when it attaches this
    /// control and at every restart or replacement commit (specification 34 §11.3).
    generation: Generation,
}

impl DebugState {
    /// The state reads answer from: the halted state while a hold is in effect, and otherwise the
    /// last completed scan's.
    fn readable_snapshot(&self) -> Option<&DebugSnapshot> {
        self.snapshot.as_ref().or(self.completed.as_deref())
    }

    /// The mutable form of [`Self::readable_snapshot`]. A shared completed scan is copied before it
    /// is changed, so a reader that still holds it keeps the state it read.
    fn readable_snapshot_mut(&mut self) -> Option<&mut DebugSnapshot> {
        if self.snapshot.is_some() {
            return self.snapshot.as_mut();
        }
        self.completed.as_mut().map(Arc::make_mut)
    }
}

#[derive(Debug, Clone)]
struct WatchEntry {
    expr: Expr,
    registry: Option<Arc<trust_hir::types::TypeRegistry>>,
    last: Option<Value>,
}

/// Runtime events a debug control with no event sender attached holds for callers that drain
/// them directly. At the bound the oldest event is dropped and counted (specification 11, event
/// store). The bound and the buffering are shared runtime code (spec 34 ARCH-11).
#[cfg(test)]
pub(crate) use trust_runtime_services::events::DEBUG_EVENT_BUFFER_CAPACITY as RUNTIME_EVENT_BUFFER_CAPACITY;

/// Shared debug control and hook implementation.
#[derive(Debug, Clone)]
pub struct DebugControl {
    state: Arc<(Mutex<DebugState>, Condvar)>,
    watchdog_pause_nanos: Arc<AtomicU64>,
    /// Whether debugging is enabled (`runtime.control.debug_enabled`). The statement hook checks it
    /// first, and does nothing else while it is false (specification 11 section 6.9). It is shared
    /// with the control state, so a `config.set` takes effect here.
    enabled: Arc<AtomicBool>,
}
