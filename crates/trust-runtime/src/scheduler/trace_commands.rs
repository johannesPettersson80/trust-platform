//! The cycle trace's commands, which the scan thread serves between two scans (specification 48
//! RU-59; specification 11, "Cycle traces"): a recording is armed there, and stopped there.

use std::sync::mpsc::Sender;

use smol_str::SmolStr;
use trust_runtime_services::trace::requests::{
    RecordRequest, TraceRefusal, TraceStarted, TraceState,
};

use super::ResourceState;
use crate::runtime::SharedTraces;
use crate::CompiledRuntime;

/// `trace.record`'s command: the request, the runtime's identity and platform as `status` gives
/// them, and the slot that the control link reads.
#[derive(Debug, Clone)]
pub struct TraceStart {
    /// The request.
    pub request: RecordRequest,
    /// The runtime's identity (`runtime_id`).
    pub runtime_id: SmolStr,
    /// The platform the runtime runs on.
    pub platform: &'static str,
    /// The slot that the control link reads.
    pub traces: SharedTraces,
    /// Where the answer goes.
    pub respond_to: Sender<Result<TraceStarted, TraceRefusal>>,
}

impl TraceStart {
    /// Arms the recording at the next cycle, its memory reserved now, and answers.
    pub(super) fn serve(self, runtime: &mut CompiledRuntime) {
        let device = (&self.runtime_id, self.platform);
        let _ = self
            .respond_to
            .send(runtime.start_trace(&self.request, device, &self.traces));
    }

    /// Refuses the recording, as a resource in `state` runs no cycle to record.
    pub(super) fn refuse(self, state: ResourceState) {
        let _ = self.respond_to.send(Err(TraceRefusal::NotRunning(state)));
    }
}

/// `trace.stop`'s command: the recording, and the slot that the control link reads.
#[derive(Debug, Clone)]
pub struct TraceStop {
    /// The recording's id.
    pub trace: u64,
    /// The slot that the control link reads.
    pub traces: SharedTraces,
    /// Where the trace's state goes.
    pub respond_to: Sender<TraceState>,
}

impl TraceStop {
    /// Ends the recording at once, when it runs, and answers its trace's state.
    pub(super) fn serve(self, runtime: &mut CompiledRuntime) {
        let _ = self
            .respond_to
            .send(runtime.stop_trace(self.trace, &self.traces));
    }
}
