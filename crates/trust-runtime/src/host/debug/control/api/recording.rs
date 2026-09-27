impl DebugControl {
    pub fn start_io_recording(&self, spec: IoRecordingSpec) -> Result<(), String> {
        if spec.program_revision.is_some()||spec.bindings.iter().any(|binding|binding.symbol.is_some()) {return Err("program-guarded recording requires compiled metadata".into());}
        let mut state=self.state.0.lock().map_err(|_|"runtime recording state unavailable")?;
        start_recording_locked(&mut state,spec)
    }
    pub fn start_io_recording_for_program(&self,spec:IoRecordingSpec,metadata:&crate::runtime::RuntimeMetadata)->Result<(),String>{
        let mut state=self.state.0.lock().map_err(|_|"runtime recording state unavailable")?;
        let revision=state.parameters.program_revision(metadata);
        let symbolic=spec.bindings.iter().any(|binding|binding.symbol.is_some());
        if spec.program_revision.as_ref().is_some_and(|expected|expected!=&revision)||(symbolic&&spec.program_revision.is_none()) {return Err("recording program revision changed or is missing".into());}
        if symbolic&&!state.signals.ready_for(&spec.loop_revision,&revision,&spec.bindings,state.latest_io_snapshot.as_ref().and_then(|snapshot|snapshot.scan)){return Err("a current coherent inspection of these symbolic bindings is required".into());}
        start_recording_locked(&mut state,spec)
    }
    pub fn stop_io_recording(&self, id: &str, revision: &str) -> Result<IoRecordingResult, String> {
        let (lock, _) = &*self.state;
        lock.lock().map_err(|_| "Runtime recording state is unavailable.")?.recordings.stop(id, revision)
    }
    pub fn release_io_recording(&self, id: &str, revision: &str) -> Result<(), String> {
        let (lock, _) = &*self.state;
        lock.lock().map_err(|_| "Runtime recording state is unavailable.")?.recordings.release(id, revision)
    }
}
fn start_recording_locked(state:&mut DebugState,spec:IoRecordingSpec)->Result<(),String>{
    let DebugState{recordings,latest_io_snapshot,snapshot,..}=state;
    recordings.start(spec,latest_io_snapshot.as_ref(),snapshot.as_ref().map(|snapshot|&snapshot.storage))
}
