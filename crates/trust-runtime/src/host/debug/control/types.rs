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
    breakpoint_stops_this_cycle: HashSet<BreakpointCycleKey>,
    frame_locations: HashMap<FrameId, SourceLocation>,
    logs: Vec<DebugLog>,
    snapshot: Option<DebugSnapshot>,
    watches: Vec<WatchEntry>,
    watch_changed: bool,
    log_tx: Option<Sender<DebugLog>>,
    io_tx: Option<Sender<IoSnapshot>>,
    latest_io_snapshot: Option<IoSnapshot>,
    recordings: recording::IoRecordingRegistry,
    parameters: parameters::ParameterRegistry,
    transfers: transfer::TransferRegistry,
    signals: signals::SignalRegistry,
    stop_tx: Option<Sender<DebugStop>>,
    runtime_tx: Option<Sender<RuntimeEvent>>,
    runtime_events: Vec<RuntimeEvent>,
    pending_stop: Option<DebugStopReason>,
    stops: Vec<DebugStop>,
    last_stop: Option<DebugStop>,
    steps: HashMap<u32, StepState>,
    io_writes: Vec<(IoAddress, Value)>,
    pending_var_writes: Vec<PendingVarWrite>,
    pending_lvalue_writes: Vec<PendingLValueWrite>,
    forced_vars: Vec<ForcedVar>,
    forced_io: Vec<(IoAddress, Value)>,
    /// The callback that runs once per published cycle; the experiment executor registers it.
    scan_observer: Option<ScanObserver>,
    /// Whether the runtime's I/O was simulation-only at the last published cycle (Specification 35, A2.3).
    latest_simulation_only_io: Option<bool>,
}

#[derive(Debug, Clone)]
struct WatchEntry {
    expr: Expr,
    last: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ForcedVarTarget {
    Global(SmolStr),
    Retain(SmolStr),
    Instance(InstanceId, SmolStr),
}

#[derive(Debug, Clone)]
pub(crate) struct ForcedVar {
    pub target: ForcedVarTarget,
    pub value: Value,
}

#[derive(Debug, Clone)]
pub(crate) struct ForcedSnapshot {
    pub vars: Vec<ForcedVar>,
    pub io: Vec<(IoAddress, Value)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PendingVarTarget {
    Global(SmolStr),
    Retain(SmolStr),
    Instance(InstanceId, SmolStr),
    Local(FrameId, SmolStr),
    /// A variable or a field below one by its storage reference: a Control Studio experiment's
    /// variable (Specification 35, experiments by variable name).
    Reference(crate::value::ValueRef),
}

#[derive(Debug, Clone)]
pub(crate) struct PendingVarWrite {
    pub target: PendingVarTarget,
    pub value: Value,
}

#[derive(Debug, Clone)]
pub(crate) struct PendingLValueWrite {
    pub frame_id: Option<FrameId>,
    pub target: LValue,
    pub value: Value,
}

/// Shared debug control and hook implementation.
#[derive(Debug, Clone)]
pub struct DebugControl {
    state: Arc<(Mutex<DebugState>, Condvar)>,
    watchdog_pause_nanos: Arc<AtomicU64>,
}
