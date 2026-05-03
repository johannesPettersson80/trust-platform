impl DebugControl {
    /// Create a new debug control handle in running mode.
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: Arc::new((
                Mutex::new(DebugState {
                    mode: DebugMode::Running,
                    last_location: None,
                    last_call_depth: 0,
                    last_call_depths: HashMap::new(),
                    current_thread: Some(1),
                    target_thread: None,
                    breakpoints: Vec::new(),
                    breakpoint_generation: HashMap::new(),
                    frame_locations: HashMap::new(),
                    logs: Vec::new(),
                    snapshot: None,
                    watches: Vec::new(),
                    watch_changed: false,
                    log_tx: None,
                    io_tx: None,
                    stop_tx: None,
                    runtime_tx: None,
                    runtime_events: Vec::new(),
                    pending_stop: None,
                    stops: Vec::new(),
                    last_stop: None,
                    steps: HashMap::new(),
                    io_writes: Vec::new(),
                    pending_var_writes: Vec::new(),
                    pending_lvalue_writes: Vec::new(),
                    forced_vars: Vec::new(),
                    forced_io: Vec::new(),
                }),
                Condvar::new(),
            )),
        }
    }

    /// Apply a requested control action.
    pub fn apply_action(&self, action: ControlAction) -> ControlOutcome {
        let (lock, cvar) = &*self.state;
        let mut state = lock.lock().expect("debug state poisoned");
        let mut notify = false;
        let mut outcome = ControlOutcome::Applied;
        let previous_mode = state.mode;
        let step_started = matches!(previous_mode, DebugMode::Paused);

        match action {
            ControlAction::Pause(thread_id) => {
                if matches!(state.mode, DebugMode::Paused) {
                    outcome = ControlOutcome::Ignored;
                } else {
                    state.mode = DebugMode::Paused;
                    state.steps.clear();
                    state.pending_stop = Some(DebugStopReason::Pause);
                    state.snapshot = None;
                    state.target_thread = thread_id;
                }
            }
            ControlAction::Continue => {
                state.mode = DebugMode::Running;
                state.steps.clear();
                state.pending_stop = None;
                state.snapshot = None;
                state.target_thread = None;
                notify = true;
            }
            ControlAction::StepIn(thread_id) => {
                state.steps.clear();
                let target_thread = thread_id.or(state.current_thread);
                let step_key = target_thread.unwrap_or(0);
                let target_depth = state.last_call_depth;
                state.steps.insert(
                    step_key,
                    StepState {
                        kind: StepKind::Into,
                        target_depth,
                        started: step_started,
                    },
                );
                state.mode = DebugMode::Running;
                state.pending_stop = None;
                state.snapshot = None;
                state.target_thread = target_thread;
                notify = true;
            }
            ControlAction::StepOver(thread_id) => {
                state.steps.clear();
                let target_thread = thread_id.or(state.current_thread);
                let target_depth = target_thread
                    .and_then(|id| state.last_call_depths.get(&id).copied())
                    .unwrap_or(state.last_call_depth);
                let step_key = target_thread.unwrap_or(0);
                state.steps.insert(
                    step_key,
                    StepState {
                        kind: StepKind::Over,
                        target_depth,
                        started: step_started,
                    },
                );
                state.mode = DebugMode::Running;
                state.pending_stop = None;
                state.snapshot = None;
                state.target_thread = target_thread;
                notify = true;
            }
            ControlAction::StepOut(thread_id) => {
                state.steps.clear();
                let target_thread = thread_id.or(state.current_thread);
                let target_depth = target_thread
                    .and_then(|id| state.last_call_depths.get(&id).copied())
                    .unwrap_or(state.last_call_depth);
                let step_key = target_thread.unwrap_or(0);
                state.steps.insert(
                    step_key,
                    StepState {
                        kind: StepKind::Out,
                        target_depth: target_depth.saturating_sub(1),
                        started: step_started,
                    },
                );
                state.mode = DebugMode::Running;
                state.pending_stop = None;
                state.snapshot = None;
                state.target_thread = target_thread;
                notify = true;
            }
        }

        if notify {
            cvar.notify_all();
        }

        trace_debug(&format!(
            "action={action:?} outcome={outcome:?} mode={previous_mode:?}->{:?}",
            state.mode
        ));

        outcome
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
        if matches!(state.mode, DebugMode::Paused) {
            return;
        }
        state.mode = DebugMode::Paused;
        state.steps.clear();
        state.pending_stop = Some(DebugStopReason::Entry);
        state.snapshot = None;
        state.target_thread = None;
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
