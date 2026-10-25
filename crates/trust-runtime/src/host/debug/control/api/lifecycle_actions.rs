impl Default for DebugControl {
    fn default() -> Self {
        Self::new()
    }
}

impl DebugControl {
    /// A control with debugging enabled and no program yet.
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(DebugState {
                io_tx: None,
                runtime_tx: None,
                runtime_events: VecDeque::new(),
                runtime_events_dropped: 0,
                live_values: LiveValueQueues::new(),
                generation: Generation::FIRST,
            })),
            enabled: Arc::new(AtomicBool::new(true)),
            compiled: Arc::new(Mutex::new(None)),
        }
    }

    /// The state, also after a thread that held it panicked: every operation on it is complete
    /// when the lock is released, so the state stays valid.
    fn lock(&self) -> MutexGuard<'_, DebugState> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// The debugger of the compiled program the resource runs, once a program is set.
    #[must_use]
    pub fn compiled(&self) -> Option<CompiledDebugger> {
        self.compiled
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    /// The debugger of the compiled program the resource runs from now on, created on first use with
    /// this control's switch (specification 44 §8.8).
    pub(crate) fn compiled_debugger(&self) -> CompiledDebugger {
        self.compiled
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get_or_insert_with(|| CompiledDebugger::new(Arc::clone(&self.enabled)))
            .clone()
    }

    /// Use `enabled` as this control's debugging switch. A runtime that shares the switch with its
    /// control state follows a `config.set` of `runtime.control.debug_enabled` (specification 11
    /// section 6.9). A control without a switch stays enabled.
    #[must_use]
    pub fn with_enabled_flag(mut self, enabled: Arc<AtomicBool>) -> Self {
        self.enabled = enabled;
        self
    }

    /// Whether debugging is enabled (specification 11 section 6.9).
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    /// The time the debugger held scans in total, which no watchdog counts (specification 11
    /// section 6.9).
    pub(crate) fn watchdog_pause_elapsed(&self) -> std::time::Duration {
        self.compiled()
            .map_or(std::time::Duration::ZERO, |debugger| debugger.held_total())
    }

    /// Whether the scan thread waits in a debugger hold right now, and the held total, read
    /// together, so hosted hang supervision never sees a hold half-recorded.
    pub(crate) fn watchdog_hold_view(&self) -> (bool, std::time::Duration) {
        self.compiled()
            .map_or((false, std::time::Duration::ZERO), |debugger| {
                debugger.hold_view()
            })
    }

    /// Why the debugger holds the scan, as `status` reports it (specification 11 section 6.9).
    #[must_use]
    pub fn hold_label(&self) -> Option<&'static str> {
        self.compiled()?.hold_label()
    }

    /// End the debugger's hold on the scan for a resource stop, and keep the debugger from holding
    /// the scan again until `rearm_after_stop`. A stop is independent of a pause
    /// (specification 13, "Pause"): the scan thread that waits inside the hold wakes without a
    /// `resume`, the scan it suspended runs to its end with no further pause, step or breakpoint,
    /// and the scheduler then takes its ordinary stop path (specification 11 section 6.9,
    /// "Operations while the debugger holds the scan").
    pub(crate) fn release_for_stop(&self) {
        if let Some(compiled) = self.compiled() {
            compiled.release_for_stop();
        }
    }

    /// Let the debugger hold the scan again. A new resource thread starts with it, so a control
    /// that an earlier stop released is not left inert.
    pub(crate) fn rearm_after_stop(&self) {
        if let Some(compiled) = self.compiled() {
            compiled.rearm();
        }
    }
}
