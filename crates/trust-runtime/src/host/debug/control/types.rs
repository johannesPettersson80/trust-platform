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
    breakpoint_stops_this_cycle: HashSet<BreakpointCycleKey>,
    frame_locations: HashMap<FrameId, SourceLocation>,
    logs: Vec<DebugLog>,
    snapshot: Option<DebugSnapshot>,
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
    io_writes: Vec<(IoAddress, Value)>,
    pending_var_writes: Vec<PendingVarWrite>,
    pending_lvalue_writes: Vec<PendingLValueWrite>,
    forced_vars: Vec<ForcedVar>,
    forced_io: Vec<(IoAddress, Value)>,
}

#[derive(Debug, Clone)]
struct WatchEntry {
    expr: Expr,
    registry: Option<Arc<trust_hir::types::TypeRegistry>>,
    last: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ForcedVarTarget {
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

/// Entries each bounded control queue holds before it refuses a new request: pending one-shot
/// I/O writes, pending variable writes, active I/O forces and active variable forces
/// (specification 34 section 4.3.1, specification 11 "request queue full").
pub(crate) const REQUEST_QUEUE_CAPACITY: usize = 1_024;

/// Runtime events a debug control with no event sender attached holds for callers that drain
/// them directly. At the bound the oldest event is dropped and counted (specification 11, event
/// store).
pub(crate) const RUNTIME_EVENT_BUFFER_CAPACITY: usize = 1_024;

/// A control request refused because its bounded queue is full. Its text is the stable control
/// error `request queue full`, and the refused request changes nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequestQueueFull;

impl std::fmt::Display for RequestQueueFull {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("request queue full")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PendingVarTarget {
    Global(SmolStr),
    Retain(SmolStr),
    Instance(InstanceId, SmolStr),
    Local(FrameId, SmolStr),
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
