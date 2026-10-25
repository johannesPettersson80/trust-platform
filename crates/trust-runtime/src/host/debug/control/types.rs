#[derive(Debug)]
struct DebugState {
    io_tx: Option<Sender<IoSnapshot>>,
    runtime_tx: Option<Sender<RuntimeEvent>>,
    runtime_events: VecDeque<RuntimeEvent>,
    runtime_events_dropped: u64,
    /// The queued writes and active forces, with their capacity and rules (spec 11 "Live Values";
    /// spec 34 §4.3.1). Held here so every queue operation takes the one state lock. A compiled
    /// program takes no expression writes, so none is ever queued.
    live_values: LiveValueQueues<()>,
    /// The installation generation of the attached runtime, published when it attaches this
    /// control and at every restart or replacement commit (specification 34 §11.3).
    generation: Generation,
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
