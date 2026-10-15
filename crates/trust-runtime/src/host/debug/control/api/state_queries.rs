impl DebugControl {
    /// Get the current execution mode.
    #[must_use]
    pub fn mode(&self) -> DebugMode {
        let (lock, _) = &*self.state;
        let state = lock.lock().expect("debug state poisoned");
        state.mode
    }

    /// Get the last observed statement location.
    #[must_use]
    pub fn last_location(&self) -> Option<SourceLocation> {
        let (lock, _) = &*self.state;
        let state = lock.lock().expect("debug state poisoned");
        state.last_location
    }

    /// Get the current breakpoint generation for a file id.
    #[must_use]
    pub fn breakpoint_generation(&self, file_id: u32) -> Option<u64> {
        let (lock, _) = &*self.state;
        let state = lock.lock().expect("debug state poisoned");
        state.breakpoint_generation.get(&file_id).copied()
    }

    /// Get the last observed location for a frame.
    #[must_use]
    pub fn frame_location(&self, frame_id: FrameId) -> Option<SourceLocation> {
        let (lock, _) = &*self.state;
        let state = lock.lock().expect("debug state poisoned");
        state.frame_locations.get(&frame_id).copied()
    }

    /// Snapshot all recorded frame locations.
    #[must_use]
    pub fn frame_locations(&self) -> HashMap<FrameId, SourceLocation> {
        let (lock, _) = &*self.state;
        let state = lock.lock().expect("debug state poisoned");
        state.frame_locations.clone()
    }

    /// Get the last observed call depth.
    #[must_use]
    pub fn last_call_depth(&self) -> u32 {
        let (lock, _) = &*self.state;
        let state = lock.lock().expect("debug state poisoned");
        state.last_call_depth
    }

    /// Set the current thread id for the active statement.
    pub fn set_current_thread(&self, thread_id: Option<u32>) {
        let (lock, _) = &*self.state;
        let mut state = lock.lock().expect("debug state poisoned");
        state.current_thread = thread_id;
    }

    /// Get the current thread id, if any.
    #[must_use]
    pub fn current_thread(&self) -> Option<u32> {
        let (lock, _) = &*self.state;
        let state = lock.lock().expect("debug state poisoned");
        state.current_thread
    }

    /// Get the currently targeted thread for stepping/pausing, if any.
    #[must_use]
    pub fn target_thread(&self) -> Option<u32> {
        let (lock, _) = &*self.state;
        let state = lock.lock().expect("debug state poisoned");
        state.target_thread
    }

    /// Drain buffered log output.
    #[must_use]
    pub fn drain_logs(&self) -> Vec<DebugLog> {
        let (lock, _) = &*self.state;
        let mut state = lock.lock().expect("debug state poisoned");
        std::mem::take(&mut state.logs)
    }

    /// Drain buffered runtime events, oldest first.
    #[must_use]
    pub fn drain_runtime_events(&self) -> Vec<RuntimeEvent> {
        let (lock, _) = &*self.state;
        let mut state = lock.lock().expect("debug state poisoned");
        state.runtime_events.drain(..).collect()
    }

    /// Count the buffered runtime events dropped at the buffer bound since this control was
    /// created. Draining does not reset the count, so a caller that drains the buffer can tell
    /// that it missed events (specification 11, event store).
    #[must_use]
    pub fn runtime_events_dropped(&self) -> u64 {
        let (lock, _) = &*self.state;
        let state = lock.lock().expect("debug state poisoned");
        state.runtime_events_dropped
    }

    /// Get the last captured debug snapshot, if any.
    #[must_use]
    pub fn snapshot(&self) -> Option<DebugSnapshot> {
        let (lock, _) = &*self.state;
        let state = lock.lock().expect("debug state poisoned");
        state.snapshot.clone()
    }

    /// Return whether execution is currently paused.
    #[must_use]
    pub fn is_paused(&self) -> bool {
        let (lock, _) = &*self.state;
        let state = lock.lock().expect("debug state poisoned");
        matches!(state.mode, DebugMode::Paused)
    }

    /// Return why a debugger hold keeps the scan paused, if one is active. A
    /// pause requested but not yet taken at a statement reports its own reason.
    #[must_use]
    pub fn hold_reason(&self) -> Option<crate::debug::DebugStopReason> {
        let (lock, _) = &*self.state;
        let state = lock.lock().expect("debug state poisoned");
        if !matches!(state.mode, DebugMode::Paused) {
            return None;
        }
        Some(
            state
                .pending_stop
                .or_else(|| state.last_stop.as_ref().map(|stop| stop.reason))
                .unwrap_or(crate::debug::DebugStopReason::Pause),
        )
    }

    /// Return the most recent stop, if any.
    #[must_use]
    pub fn last_stop(&self) -> Option<DebugStop> {
        let (lock, _) = &*self.state;
        let state = lock.lock().expect("debug state poisoned");
        state.last_stop.clone()
    }

    /// Drain buffered stop events.
    #[must_use]
    pub fn drain_stops(&self) -> Vec<DebugStop> {
        let (lock, _) = &*self.state;
        let mut state = lock.lock().expect("debug state poisoned");
        std::mem::take(&mut state.stops)
    }

    /// Mutate the stored snapshot, if one exists.
    pub fn with_snapshot<T>(&self, f: impl FnOnce(&mut DebugSnapshot) -> T) -> Option<T> {
        let (lock, _) = &*self.state;
        let mut state = lock.lock().expect("debug state poisoned");
        state.snapshot.as_mut().map(f)
    }
}
