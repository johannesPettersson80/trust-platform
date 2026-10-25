impl DebugControl {
    /// Stream I/O snapshots to a sender.
    pub fn set_io_sender(&self, sender: Sender<IoSnapshot>) {
        self.lock().io_tx = Some(sender);
    }

    /// Stop streaming I/O snapshots.
    pub fn clear_io_sender(&self) {
        self.lock().io_tx = None;
    }

    /// Stream runtime events to a sender.
    pub fn set_runtime_sender(&self, sender: Sender<RuntimeEvent>) {
        self.lock().runtime_tx = Some(sender);
    }

    /// Stop streaming runtime events.
    pub fn clear_runtime_sender(&self) {
        self.lock().runtime_tx = None;
    }

    /// Emit an I/O snapshot to listeners, if configured.
    pub fn push_io_snapshot(&self, snapshot: IoSnapshot) {
        let state = self.lock();
        if let Some(sender) = &state.io_tx
            && sender.send(snapshot).is_err()
        {
            tracing::debug!("debug I/O snapshot sender closed");
        }
    }

    /// Emit a runtime event to listeners, if configured. Without a live sender the event is
    /// buffered for `drain_runtime_events`; that buffer is bounded, and `runtime_events_dropped`
    /// counts the events it has dropped.
    pub fn push_runtime_event(&self, event: RuntimeEvent) {
        let mut state = self.lock();
        if let Some(sender) = &state.runtime_tx {
            if sender.send(event.clone()).is_err() {
                state.runtime_tx = None;
                buffer_runtime_event(&mut state, event);
            }
        } else {
            buffer_runtime_event(&mut state, event);
        }
    }
}

/// Buffer a runtime event for direct draining. At the shared bound the oldest buffered event is
/// dropped and counted, so the caller can tell that it missed events.
fn buffer_runtime_event(state: &mut DebugState, event: RuntimeEvent) {
    trust_runtime_services::events::buffer_event(
        &mut state.runtime_events,
        &mut state.runtime_events_dropped,
        event,
    );
}
