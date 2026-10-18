impl DebugControl {
    /// Create a new debug control handle in running mode, with debugging enabled.
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: Arc::new((
                Mutex::new(DebugState {
                    mode: DebugMode::Running,
                    last_location: None,
                    last_call_depth: 0,
                    last_call_depths: BTreeMap::new(),
                    current_thread: Some(1),
                    target_thread: None,
                    breakpoints: Vec::new(),
                    breakpoint_generation: HashMap::new(),
                    watchdog_hold_started: None,
                    stop_requested: false,
                    breakpoint_stops_this_cycle: HashSet::new(),
                    frame_locations: HashMap::new(),
                    logs: Vec::new(),
                    snapshot: None,
                    completed: None,
                    storage_copies: 0,
                    watches: Vec::new(),
                    watch_changed: false,
                    log_tx: None,
                    io_tx: None,
                    stop_tx: None,
                    runtime_tx: None,
                    runtime_events: VecDeque::new(),
                    runtime_events_dropped: 0,
                    pending_stop: None,
                    stops: Vec::new(),
                    last_stop: None,
                    steps: BTreeMap::new(),
                    live_values: LiveValueQueues::new(),
                    generation: Generation::FIRST,
                }),
                Condvar::new(),
            )),
            watchdog_pause_nanos: Arc::new(AtomicU64::new(0)),
            enabled: Arc::new(AtomicBool::new(true)),
        }
    }

    /// Use `enabled` as this control's debugging switch. A runtime that shares the switch with its
    /// control state follows a `config.set` of `runtime.control.debug_enabled` in the statement hook
    /// (specification 11 section 6.9). A control without a switch stays enabled.
    #[must_use]
    pub fn with_enabled_flag(mut self, enabled: Arc<AtomicBool>) -> Self {
        self.enabled = enabled;
        self
    }

    /// Whether debugging is enabled. While it is disabled the statement hook does nothing beyond
    /// this check, and no storage is copied for the debugger (specification 11 section 6.9).
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    /// The storage copies this control has made for the debugger: one per completed scan that
    /// published, and one per stop or conditional evaluation that built a projection.
    #[doc(hidden)]
    #[must_use]
    pub fn storage_copies(&self) -> u64 {
        let (lock, _) = &*self.state;
        lock.lock().expect("debug state poisoned").storage_copies
    }

    pub(crate) fn watchdog_pause_elapsed(&self) -> std::time::Duration {
        std::time::Duration::from_nanos(self.watchdog_pause_nanos.load(Ordering::Relaxed))
    }

    /// Whether the scan thread is waiting in a debugger hold right now, and the completed pause
    /// total. Both are read under the state lock, which the hold updates, so the pair is consistent.
    pub(crate) fn watchdog_hold_view(&self) -> (bool, std::time::Duration) {
        let (lock, _) = &*self.state;
        let state = lock.lock().expect("debug state poisoned");
        (
            state.watchdog_hold_started.is_some(),
            self.watchdog_pause_elapsed(),
        )
    }

    pub(crate) fn begin_cycle(&self) {
        let (lock, _) = &*self.state;
        let mut state = lock.lock().expect("debug state poisoned");
        state.breakpoint_stops_this_cycle.clear();
    }

    /// End the debugger's hold on the scan for a resource stop, and keep the debugger from holding
    /// the scan again until `rearm_after_stop`. A stop is independent of a pause
    /// (specification 13, "Pause"): the scan thread that waits inside the hold wakes without a
    /// `resume`, the scan it suspended runs to its end with no further pause, step or breakpoint,
    /// and the scheduler then takes its ordinary stop path (specification 11 section 6.9,
    /// "Operations while the debugger holds the scan").
    ///
    /// A stop request must not fail, so a poisoned lock is recovered here.
    pub(crate) fn release_for_stop(&self) {
        let (lock, cvar) = &*self.state;
        let mut state = lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        run_control(&mut state).release_for_stop();
        state.snapshot = None;
        trace_debug("stop.release hold");
        cvar.notify_all();
    }

    /// Let the debugger hold the scan again. A new resource thread starts with it, so a control
    /// that an earlier stop released is not left inert.
    pub(crate) fn rearm_after_stop(&self) {
        let (lock, _) = &*self.state;
        let mut state = lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        run_control(&mut state).rearm();
    }

    pub(super) fn record_watchdog_pause(&self, elapsed: std::time::Duration) {
        let nanos = u64::try_from(elapsed.as_nanos()).unwrap_or(u64::MAX);
        let _ =
            self.watchdog_pause_nanos
                .try_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                    Some(current.saturating_add(nanos))
                });
    }

    /// Apply a requested control action. The shared run control decides what the action changes.
    /// An applied action ends the halted state, and a resumed scan wakes the waiting hold.
    pub fn apply_action(&self, action: ControlAction) -> ControlOutcome {
        let (lock, cvar) = &*self.state;
        let mut state = lock.lock().expect("debug state poisoned");
        let previous_mode = state.mode;
        let effect = run_control(&mut state).apply(action);
        if effect.outcome == ControlOutcome::Applied {
            state.snapshot = None;
        }
        if effect.resumed {
            cvar.notify_all();
        }

        trace_debug(&format!(
            "action={action:?} outcome={:?} mode={previous_mode:?}->{:?}",
            effect.outcome, state.mode
        ));

        effect.outcome
    }

    /// Replace all breakpoints for a given file id.
    pub fn set_breakpoints_for_file(&self, file_id: u32, breakpoints: Vec<DebugBreakpoint>) {
        let (lock, _) = &*self.state;
        let mut state = lock.lock().expect("debug state poisoned");
        let requested_count = breakpoints.len();
        let generation = {
            let entry = state.breakpoint_generation.entry(file_id).or_insert(0);
            *entry = entry.saturating_add(1);
            *entry
        };
        state
            .breakpoints
            .retain(|bp| bp.location.file_id != file_id);
        state
            .breakpoints
            .extend(breakpoints.into_iter().map(|mut bp| {
                bp.generation = generation;
                bp.hits = 0;
                bp
            }));
        trace_debug(&format!(
            "breakpoints.set file_id={} generation={} requested={} total={}",
            file_id,
            generation,
            requested_count,
            state.breakpoints.len()
        ));
    }

    /// Clear all breakpoints.
    pub fn clear_breakpoints(&self) {
        let (lock, _) = &*self.state;
        let mut state = lock.lock().expect("debug state poisoned");
        let prev_total = state.breakpoints.len();
        state.breakpoints.clear();
        state.breakpoint_generation.clear();
        state.breakpoint_stops_this_cycle.clear();
        trace_debug(&format!("breakpoints.clear prev_total={prev_total}"));
    }

    /// Returns the number of active breakpoints (primarily for tests).
    #[doc(hidden)]
    pub fn breakpoint_count(&self) -> usize {
        let (lock, _) = &*self.state;
        let state = lock.lock().expect("debug state poisoned");
        state.breakpoints.len()
    }

    /// Snapshot current breakpoints.
    #[must_use]
    pub fn breakpoints(&self) -> Vec<DebugBreakpoint> {
        let (lock, _) = &*self.state;
        let state = lock.lock().expect("debug state poisoned");
        state.breakpoints.clone()
    }

    /// Pause execution at the next statement boundary.
    pub fn pause(&self) {
        let _ = self.apply_action(ControlAction::Pause(None));
    }

    /// Pause execution at the next statement boundary for a specific thread.
    pub fn pause_thread(&self, thread_id: u32) {
        let _ = self.apply_action(ControlAction::Pause(Some(thread_id)));
    }

    /// Pause execution at the next statement boundary with an entry reason.
    pub fn pause_entry(&self) {
        let (lock, _) = &*self.state;
        let mut state = lock.lock().expect("debug state poisoned");
        if run_control(&mut state).pause_entry() == ControlOutcome::Applied {
            state.snapshot = None;
        }
    }

    /// Continue running until the next pause request.
    pub fn continue_run(&self) {
        let _ = self.apply_action(ControlAction::Continue);
    }

    /// Execute a single statement and pause again.
    pub fn step(&self) {
        let _ = self.apply_action(ControlAction::StepIn(None));
    }

    /// Execute a single statement and pause again (thread-scoped).
    pub fn step_thread(&self, thread_id: u32) {
        let _ = self.apply_action(ControlAction::StepIn(Some(thread_id)));
    }

    /// Step over the current statement at the last observed call depth.
    pub fn step_over(&self) {
        let _ = self.apply_action(ControlAction::StepOver(None));
    }

    /// Step over the current statement at the last observed call depth (thread-scoped).
    pub fn step_over_thread(&self, thread_id: u32) {
        let _ = self.apply_action(ControlAction::StepOver(Some(thread_id)));
    }

    /// Step out of the current call frame.
    pub fn step_out(&self) {
        let _ = self.apply_action(ControlAction::StepOut(None));
    }

    /// Step out of the current call frame (thread-scoped).
    pub fn step_out_thread(&self, thread_id: u32) {
        let _ = self.apply_action(ControlAction::StepOut(Some(thread_id)));
    }
}
