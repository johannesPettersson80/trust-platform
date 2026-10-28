#[derive(Debug)]
struct DebugState {
    io_tx: Option<Sender<IoSnapshot>>,
    latest_io_snapshot: Option<IoSnapshot>,
    recordings: recording::IoRecordingRegistry,
    parameters: parameters::ParameterRegistry,
    transfers: transfer::TransferRegistry,
    signals: signals::SignalRegistry,
    runtime_tx: Option<Sender<RuntimeEvent>>,
    runtime_events: VecDeque<RuntimeEvent>,
    runtime_events_dropped: u64,
    /// The queued writes and active forces, with their capacity and rules (spec 11 "Live Values";
    /// spec 34 §4.3.1). Held here so every queue operation takes the one state lock. A compiled
    /// program takes no expression writes, so none is ever queued.
    live_values: LiveValueQueues,
    /// The experiments' writes of program variables by place, which the scan applies at its cycle
    /// start (specification 35, experiments by variable name): no Live Values, since each holds a
    /// place resolved in one program generation.
    control_writes: Vec<ControlWrite>,
    /// The installation generation of the attached runtime, published when it attaches this
    /// control and at every restart or replacement commit (specification 34 §11.3).
    generation: Generation,
    /// The callback that runs once per published cycle; the experiment executor registers it.
    scan_observer: Option<ScanObserver>,
    /// Whether the runtime's I/O was simulation-only at the last published cycle (Specification 35, A2.3).
    latest_simulation_only_io: Option<bool>,
}

/// An experiment's write of a program variable (specification 35, experiments by variable name): the
/// place a path resolved to in the program generation `generation`, which names the variable only in
/// that generation, the path the write is reported by, and the value.
#[derive(Debug, Clone)]
pub(crate) struct ControlWrite {
    pub(crate) at: crate::runtime::Located,
    pub(crate) generation: Generation,
    pub(crate) label: SmolStr,
    pub(crate) value: Value,
}

/// Runtime events a debug control with no event sender attached holds for callers that drain
/// them directly. At the bound the oldest event is dropped and counted (specification 11, event
/// store). The bound and the buffering are shared runtime code (spec 34 ARCH-11).
#[cfg(test)]
pub(crate) use trust_runtime_services::events::DEBUG_EVENT_BUFFER_CAPACITY as RUNTIME_EVENT_BUFFER_CAPACITY;

/// The debugger's control of a resource. Cloning it shares it.
#[derive(Debug, Clone)]
pub struct DebugControl {
    state: Arc<Mutex<DebugState>>,
    /// Whether debugging is enabled (`runtime.control.debug_enabled`), shared with the control
    /// state, so a `config.set` takes effect here and in the compiled debugger (specification 11
    /// section 6.9).
    enabled: Arc<AtomicBool>,
    /// The debugger of the compiled program the resource runs, once a program is set
    /// (specification 44 §8.8): the debug operations, pause and continue, and a stop's release go
    /// to it.
    compiled: Arc<Mutex<Option<CompiledDebugger>>>,
}
