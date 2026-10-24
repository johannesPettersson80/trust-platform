// The run control is shared (trust-runtime-services `debug::run`): its decisions and the types they
// name are defined there, and this crate keeps the lock, the condition variable and the wait.
use std::collections::BTreeMap;

use trust_runtime_core::debug::LineResolver;
use trust_runtime_core::fault::FaultSite;
use trust_runtime_services::debug::{BoundaryStop, RunControl, StepState};
pub use trust_runtime_services::debug::{
    RunAction as ControlAction, RunMode as DebugMode, RunOutcome as ControlOutcome, StepKind,
};

use crate::bytecode::{BytecodeModule, SectionData, SectionId};

#[derive(Debug)]
struct DebugState {
    mode: DebugMode,
    last_location: Option<SourceLocation>,
    last_call_depth: u32,
    last_call_depths: BTreeMap<u32, u32>,
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
    /// The state of the last scan that ended, published at its end: a completed scan's, or a
    /// faulted scan's, which replaces the last completed scan until the next scan ends or a restart
    /// clears it (specification 11 section 6.13.1). Reads answer from it whenever no hold is in
    /// effect, so a scan in progress is never visible. It is shared, so a read clones it outside
    /// the state lock.
    completed: Option<Arc<EndedScan>>,
    /// Storage copies made for the debugger: one per scan that ended and published, one per stop
    /// or conditional evaluation that built a projection.
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
    steps: BTreeMap<u32, StepState>,
    /// The queued writes and active forces, with their capacity and rules (spec 11 "Live Values";
    /// spec 34 §4.3.1). Held here so every queue operation takes the one state lock.
    live_values: LiveValueQueues<LValue>,
    /// The installation generation of the attached runtime, published when it attaches this
    /// control and at every restart or replacement commit (specification 34 §11.3).
    generation: Generation,
}

impl DebugState {
    /// The state reads answer from: the halted state while a hold is in effect, and otherwise the
    /// state of the last scan that ended.
    fn readable_snapshot(&self) -> Option<&DebugSnapshot> {
        self.snapshot
            .as_ref()
            .or(self.completed.as_deref().map(|scan| &scan.snapshot))
    }

    /// The mutable form of [`Self::readable_snapshot`]. A shared scan state is copied before it is
    /// changed, so a reader that still holds it keeps the state it read.
    fn readable_snapshot_mut(&mut self) -> Option<&mut DebugSnapshot> {
        if self.snapshot.is_some() {
            return self.snapshot.as_mut();
        }
        self.completed
            .as_mut()
            .map(|scan| &mut Arc::make_mut(scan).snapshot)
    }
}

/// The published state of a scan that ended (specification 11 section 6.13.1): its storage at the
/// end, and, for a faulted scan, the position of the statement that owned the failing instruction.
#[derive(Debug, Clone)]
struct EndedScan {
    snapshot: DebugSnapshot,
    fault: Option<FaultPosition>,
}

/// The position a faulted scan left: the statement that owns the failing instruction, as the
/// installed `LINE_TABLE` names it (specification 12 section 6.13). Lines and columns are 1-based,
/// as the table holds them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FaultPosition {
    /// The statement's file label, from `DEBUG_STRING_TABLE`.
    pub file: SmolStr,
    /// 1-based line of the statement's first byte.
    pub line: u32,
    /// 1-based byte column of the statement's first byte within its line.
    pub column: u32,
}

/// The run fields of this debugger, borrowed for one decision of the shared run control. The
/// decisions that also need the breakpoints borrow those fields directly (`boundary_stop`).
fn run_control(state: &mut DebugState) -> RunControl<'_> {
    RunControl {
        mode: &mut state.mode,
        pending_stop: &mut state.pending_stop,
        current_thread: &mut state.current_thread,
        target_thread: &mut state.target_thread,
        steps: &mut state.steps,
        last_call_depth: &mut state.last_call_depth,
        last_call_depths: &mut state.last_call_depths,
        stop_requested: &mut state.stop_requested,
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
    /// The debugger of the compiled program the resource runs, when it runs one (specification 44
    /// §8.8): the debug operations, pause and continue, and a stop's release go to it.
    compiled: Arc<Mutex<Option<crate::runtime::CompiledDebugger>>>,
}
