impl DebugControl {
    /// Drain buffered runtime events, oldest first.
    #[must_use]
    pub fn drain_runtime_events(&self) -> Vec<RuntimeEvent> {
        self.lock().runtime_events.drain(..).collect()
    }

    /// Count the buffered runtime events dropped at the buffer bound since this control was
    /// created. Draining does not reset the count, so a caller that drains the buffer can tell
    /// that it missed events (specification 11, event store).
    #[must_use]
    pub fn runtime_events_dropped(&self) -> u64 {
        self.lock().runtime_events_dropped
    }

    /// The runtime time of the published state that [`Self::snapshot`] reads: the held scan's
    /// while the debugger holds the scan, and otherwise the last scan's; none before a scan ended
    /// and while debugging is disabled.
    #[must_use]
    pub fn snapshot_time(&self) -> Option<Duration> {
        self.compiled()?.state().map(|state| state.now())
    }

    /// The program as the last scan left it, which Control Studio's requests read between scans
    /// (specification 11 §6.13.1; specification 44 §8.8, §13.3): the state the program's debugger
    /// published, the held scan's while the debugger holds the scan; none before a scan ended and
    /// while debugging is disabled.
    pub(crate) fn published_program(&self) -> Option<Arc<CompiledState>> {
        self.compiled()?.state()
    }

    /// The image of the program the resource runs, whatever the debugging switch, whose type and
    /// symbol tables a service reads without the program's memory (specification 44 §8.8); none
    /// before a program is set.
    #[must_use]
    pub fn program_image(&self) -> Option<Arc<trust_runtime_core::native::program::AdmittedImage>> {
        self.compiled()?.image()
    }

    /// The published state of the program: the held scan's while the debugger holds the scan, and
    /// otherwise the last scan's (specification 11 section 6.13.1), which a service reads through
    /// the program's symbol table (specification 44 §8.8, §13.3); none before a scan ended and
    /// while debugging is disabled.
    #[must_use]
    pub fn snapshot(&self) -> Option<DebugSnapshot> {
        let state = self.compiled()?.state()?;
        Some(DebugSnapshot {
            storage: crate::memory::VariableStorage::new(),
            now: state.now(),
            compiled: Some(state),
        })
    }
}
