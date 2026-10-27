impl DebugControl {
    /// Starts a recording of addresses, which names no program (specification 35, A4.1).
    pub fn start_io_recording(&self, spec: IoRecordingSpec) -> Result<(), String> {
        if spec.program_revision.is_some()
            || spec.bindings.iter().any(|binding| binding.symbol.is_some())
        {
            return Err("program-guarded recording requires compiled metadata".into());
        }
        let mut state = self.lock();
        let DebugState {
            recordings,
            latest_io_snapshot,
            ..
        } = &mut *state;
        recordings.start(spec, latest_io_snapshot.as_ref(), None)
    }

    /// Starts a recording of the program `program`, the program as the last scan left it, whose
    /// revision a symbolic recording, or one that names a revision, must match (protocol annex
    /// E4.1): a symbolic recording also needs a current coherent inspection of its bindings (E3).
    pub fn start_io_recording_for_program(
        &self,
        spec: IoRecordingSpec,
        program: Option<&dyn ProgramRead>,
    ) -> Result<(), String> {
        let mut state = self.lock();
        let revision =
            program.map(|program| state.parameters.program_revision(&program.program_image()));
        let symbolic = spec.bindings.iter().any(|binding| binding.symbol.is_some());
        let revision_matches = match (&spec.program_revision, &revision) {
            (Some(expected), Some(current)) => expected == current,
            (Some(_), None) => false,
            (None, _) => !symbolic,
        };
        if !revision_matches {
            return Err("recording program revision changed or is missing".into());
        }
        if symbolic
            && !revision.as_ref().is_some_and(|revision| {
                state.signals.ready_for(
                    &spec.loop_revision,
                    revision,
                    &spec.bindings,
                    state
                        .latest_io_snapshot
                        .as_ref()
                        .and_then(|snapshot| snapshot.scan),
                )
            })
        {
            return Err(
                "a current coherent inspection of these symbolic bindings is required".into(),
            );
        }
        let DebugState {
            recordings,
            latest_io_snapshot,
            ..
        } = &mut *state;
        recordings.start(spec, latest_io_snapshot.as_ref(), program)
    }

    /// Stops a recording and returns its samples; a repeated stop returns the same result.
    pub fn stop_io_recording(&self, id: &str, revision: &str) -> Result<IoRecordingResult, String> {
        self.lock().recordings.stop(id, revision)
    }

    /// Releases a stopped recording.
    pub fn release_io_recording(&self, id: &str, revision: &str) -> Result<(), String> {
        self.lock().recordings.release(id, revision)
    }
}
