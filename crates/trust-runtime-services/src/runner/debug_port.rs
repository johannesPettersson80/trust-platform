//! The loop's debugger as the debug operations see it (specification 13; specification 11 §6.9
//! and §6.13; specification 34, ARCH-11 and SERVICE-06).
//!
//! The shared request module ([`crate::debug::requests`]) parses a request and builds its reply.
//! It reads and changes the debugger through [`Debugger`], and this module implements that over the
//! loop: the state of reads is the memory the program owns, between scans or lent by the hold, so
//! nothing is copied (specification 11 §6.13.1; specification 44 §13.2).

use alloc::vec::Vec;

use trust_runtime_core::debug::StatementSpan;
use trust_runtime_core::memory::FrameId;

use super::host::RunnerHost;
use super::RunnerPlatform;
use crate::debug::logpoint::LogRecord;
use crate::debug::projection::DebugVariableHandles;
use crate::debug::{
    BreakpointOptions, BreakpointView, Debugger, LineSources, Span, StepKind, StopRecord,
};
use crate::live_values::io_requests::IoSnapshotRead;

impl<P: RunnerPlatform, C> Debugger for RunnerHost<'_, P, C> {
    fn sources(&self) -> Option<&LineSources> {
        self.program.compiled_state()?.sources
    }

    fn io_snapshot(&self) -> IoSnapshotRead {
        self.platform.io_snapshot()
    }

    fn is_held(&self) -> bool {
        self.debug.is_held()
    }

    fn accepts_step(&self) -> bool {
        self.debug.accepts_step()
    }

    fn step(&self, kind: StepKind) {
        self.debug.step(kind);
    }

    fn last_stop(&self) -> Option<StopRecord> {
        self.debug.last_stop()
    }

    fn drain_stops(&self) -> Vec<StopRecord> {
        self.debug.drain_stops()
    }

    fn drain_logs(&self) -> Vec<LogRecord> {
        self.debug.drain_logs()
    }

    fn frame_span(&self, frame: FrameId) -> Option<StatementSpan> {
        self.debug.frame_span(frame)
    }

    fn breakpoints(&self) -> Vec<BreakpointView> {
        self.debug.breakpoints()
    }

    fn replace_breakpoints(
        &self,
        file_id: u32,
        breakpoints: Vec<(Span<(u32, u32)>, BreakpointOptions)>,
    ) -> u64 {
        self.debug.replace_breakpoints(file_id, breakpoints)
    }

    fn clear_breakpoints(&self) {
        self.debug.clear_breakpoints();
    }

    fn with_handles(&self, operation: &mut dyn FnMut(&mut DebugVariableHandles)) {
        self.debug.with_handles(operation);
    }

    /// A compiled release image has no statement hooks (specification 44 §13.2).
    fn undebuggable(&self) -> Option<&'static str> {
        if self
            .program
            .compiled_state()
            .is_some_and(|view| view.release)
        {
            return Some(crate::debug::replies::NO_DEBUG_HOOKS);
        }
        None
    }

    /// The program's memory while a scan is held or once a scan has ended (specification 11
    /// §6.13.1; specification 44 §13.2).
    fn program_memory(&self) -> Option<&dyn trust_runtime_core::native::memory::ProgramMemory> {
        let view = self.program.compiled_state()?;
        (self.debug.published() || self.debug.is_held()).then_some(view.memory)
    }

    /// The debugger evaluates the compiled debug expressions (specification 11 §6.13.2).
    fn evaluates_expressions(&self) -> bool {
        self.program.compiled_state().is_some()
    }
}
