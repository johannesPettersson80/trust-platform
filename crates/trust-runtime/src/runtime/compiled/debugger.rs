//! The debugger of a compiled program on a computer (specification 44 §13.2, §8.8): the chips'
//! debugger state ([`ChipDebug`]), which decides every stop as on a device, shared by the scan thread
//! and the control threads under one lock. A scan held at a statement waits on the scan thread, and
//! the control threads answer the debug operations meanwhile, through the shared operations
//! (`trust_runtime_services::debug::requests`), from a copy of the held scan's memory and call stack;
//! between scans they answer from the state of the last scan that ended.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration as StdDuration, Instant};

use trust_runtime_core::debug::StatementSpan;
use trust_runtime_core::memory::FrameId;
use trust_runtime_core::native::memory::ProgramMemory;
use trust_runtime_core::native::program::AdmittedImage;
use trust_runtime_services::debug::expression::Names;
use trust_runtime_services::debug::logpoint::LogRecord;
use trust_runtime_services::debug::projection::DebugVariableHandles;
use trust_runtime_services::debug::{
    answer, BreakpointOptions, BreakpointView, Debugger, LineSources, Span, StepKind, StopRecord,
};
use trust_runtime_services::live_values::io_requests::IoSnapshotRead;
use trust_runtime_services::protocol::{ControlRequest, ControlResponse};
use trust_runtime_services::runner::{
    Boundary, ChipDebug, CompiledHeld, CompiledHold, HoldHandle, StatementStart, StopPoints,
};

use super::state::CompiledState;
use crate::error::RuntimeError;
use crate::value::Duration;

/// The stops `debug.stops` keeps between two reads, as a computer's debugger keeps them.
const STOP_CAPACITY: usize = 64;

/// The debugger of the compiled program a computer runs. Cloning it shares it.
#[derive(Clone)]
pub struct CompiledDebugger {
    shared: Arc<Shared>,
}

struct Shared {
    state: Mutex<State>,
    /// Wakes the held scan when the debugger releases it.
    wake: Condvar,
    /// The debugging switch of `runtime.control.debug_enabled`.
    enabled: Arc<AtomicBool>,
}

/// The time the debugger held scans, which no watchdog counts (specification 11 §6.9).
#[derive(Debug, Default, Clone, Copy)]
struct HeldTime {
    /// When the scan began to wait in the hold it waits in now.
    since: Option<Instant>,
    /// The holds that ended, in total.
    total: StdDuration,
}

/// What the lock guards.
struct State {
    debug: ChipDebug,
    /// The program's image, which a held scan's copy reads, once a program is set.
    image: Option<Arc<AdmittedImage>>,
    /// The statement table and file labels of a debug image; none for a release image.
    sources: Option<LineSources>,
    /// Whether the program is a release image, which the debugger refuses.
    release: bool,
    /// The memory and call stack the reads answer from: the held scan's while one is held, and
    /// otherwise the last scan's.
    memory: Option<CompiledState>,
    held: HeldTime,
}

impl std::fmt::Debug for CompiledDebugger {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CompiledDebugger")
            .finish_non_exhaustive()
    }
}

impl CompiledDebugger {
    /// A debugger with no program yet, whose switch is `enabled`.
    #[must_use]
    pub fn new(enabled: Arc<AtomicBool>) -> Self {
        Self {
            shared: Arc::new(Shared {
                state: Mutex::new(State {
                    debug: ChipDebug::new(STOP_CAPACITY),
                    image: None,
                    sources: None,
                    release: false,
                    memory: None,
                    held: HeldTime::default(),
                }),
                wake: Condvar::new(),
                enabled,
            }),
        }
    }

    /// The state, also after a thread that held it panicked: the debugger's state stays valid.
    fn lock(&self) -> MutexGuard<'_, State> {
        self.shared
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Takes the program of `image`: its statement table, and whether it is a release image. The
    /// breakpoints stay, placed by their files and positions, and no state of the program before is
    /// read any more (specification 13).
    pub(crate) fn set_program(&self, image: Arc<AdmittedImage>) {
        let view = image.image();
        let release = view.header().is_release();
        let sources = (!release).then(|| LineSources::from_image(&view));
        let mut state = self.lock();
        state.debug.program_replaced();
        state.image = Some(Arc::clone(&image));
        state.sources = sources;
        state.release = release;
        state.memory = None;
    }

    /// The hold the program's cycle takes.
    pub(crate) fn hold(&self) -> HostHold {
        HostHold {
            shared: Arc::clone(&self.shared),
        }
    }

    /// Publishes `state`, the state of the scan that ended, for the reads between scans, while
    /// debugging is enabled (specification 11 §6.13.1).
    pub(crate) fn publish(&self, state: impl FnOnce() -> Option<CompiledState>) {
        if !self.shared.enabled.load(Ordering::Relaxed) {
            return;
        }
        let Some(published) = state() else {
            return;
        };
        let mut guard = self.lock();
        guard.debug.scan_ended();
        guard.memory = Some(published);
    }

    /// The state the reads answer from: the held scan's while one is held, and otherwise the last
    /// scan's; none before a scan ended, and while debugging is disabled.
    #[must_use]
    pub fn state(&self) -> Option<CompiledState> {
        self.lock().memory.clone()
    }

    /// The program restarted: the debugger is no longer held, and its stops start again.
    pub(crate) fn restarted(&self) {
        self.lock().debug.restarted();
    }

    /// Releases a held scan for a resource stop, and holds no scan again until [`Self::rearm`].
    pub(crate) fn release_for_stop(&self) {
        self.lock().debug.release_for_stop();
        self.shared.wake.notify_all();
    }

    /// Lets the debugger hold a scan again after a stop.
    pub(crate) fn rearm(&self) {
        self.lock().debug.rearm();
    }

    /// The thread that runs the statements from now on: a task starts (specification 13,
    /// "Multi-Task Thread Model"; specification 11 §6.13.3).
    pub(crate) fn set_thread(&self, thread: Option<u32>) {
        self.lock().debug.set_thread(thread);
    }

    /// Why the debugger holds the scan, as `status` reports it (specification 11 §6.9).
    #[must_use]
    pub(crate) fn hold_label(&self) -> Option<&'static str> {
        self.lock().debug.hold_label()
    }

    /// The time the debugger held scans in the holds that ended, which no watchdog counts
    /// (specification 11 §6.9).
    #[must_use]
    pub(crate) fn held_total(&self) -> StdDuration {
        self.lock().held.total
    }

    /// Whether the scan waits in a hold now, and the holds that ended, in total, read under one
    /// lock, so hosted hang supervision never sees a hold half-recorded.
    #[must_use]
    pub(crate) fn hold_view(&self) -> (bool, StdDuration) {
        let held = self.lock().held;
        (held.since.is_some(), held.total)
    }

    /// Pauses at the next statement (specification 13, "Pause").
    pub fn pause(&self) {
        self.lock().debug.pause();
    }

    /// Releases a held scan to run on (specification 13, "Continue").
    pub fn resume(&self) {
        self.lock().debug.resume();
        self.shared.wake.notify_all();
    }

    /// Answers the debug operation `request` with the shared operations, `io` being the published I/O
    /// snapshot (specification 44 §13.2).
    #[must_use]
    pub fn answer(&self, request: &ControlRequest, io: IoSnapshotRead) -> ControlResponse {
        let state = self.lock();
        let view = DebugView {
            state: &state,
            wake: &self.shared.wake,
            io,
        };
        answer(request, &view)
    }

    /// Hands out the next variable handle for `handle`, as a listing hands out the handle of a
    /// compound value it shows, for the tests that list a handle through `debug.variables` between
    /// scans, where no frame has scopes (specification 44 §13.2).
    #[cfg(test)]
    #[must_use]
    pub(crate) fn alloc_handle(
        &self,
        handle: trust_runtime_services::debug::projection::VariableHandle,
    ) -> u32 {
        let mut handle = Some(handle);
        let mut reference = 0;
        self.lock().debug.with_handles(&mut |handles| {
            if let Some(handle) = handle.take() {
                reference = handles.alloc(handle);
            }
        });
        reference
    }
}

/// The hold a computer's compiled cycle takes: the scan thread waits in it while the debugger holds
/// the scan.
pub(crate) struct HostHold {
    shared: Arc<Shared>,
}

impl HostHold {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.shared
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

impl CompiledHold for HostHold {
    /// A device loop's hold is no computer's.
    fn from_loop(_hold: HoldHandle) -> Option<Self> {
        None
    }

    /// The ordinary stops of the last scan are forgotten when the scan can take stops.
    fn scan_begins(&self) -> bool {
        let enabled = self.shared.enabled.load(Ordering::Relaxed);
        if enabled {
            self.lock().debug.begin_cycle();
        }
        enabled
    }

    fn stop_points(&self) -> StopPoints {
        self.lock().debug.stop_points()
    }

    fn at_statement(
        &self,
        at: &StatementStart<'_>,
        names: Option<&dyn Names>,
    ) -> Result<Boundary, RuntimeError> {
        self.lock().debug.at_statement(at, names)
    }

    /// Copies the held scan's memory and call stack for the reads, and waits until the debugger
    /// releases the scan; the reads then answer from the state of the last scan again.
    fn hold_compiled(
        &self,
        at: &StatementStart<'_>,
        view: &CompiledHeld<'_>,
    ) -> Result<(), RuntimeError> {
        let mut state = self.lock();
        let image = state
            .image
            .clone()
            .ok_or(RuntimeError::InvalidExecutionState)?;
        let before = state.memory.take();
        state.memory = Some(CompiledState::held(image, view.memory, Duration::ZERO));
        state.held.since = Some(Instant::now());
        let held = loop {
            match state.debug.holds(at) {
                Ok(true) => {}
                other => break other,
            }
            state = self
                .shared
                .wake
                .wait(state)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
        };
        if let Some(since) = state.held.since.take() {
            state.held.total = state.held.total.saturating_add(since.elapsed());
        }
        state.memory = before;
        held.map(drop)
    }
}

/// The debugger as the shared debug operations see it, for one request, under the lock.
struct DebugView<'a> {
    state: &'a State,
    wake: &'a Condvar,
    io: IoSnapshotRead,
}

impl Debugger for DebugView<'_> {
    fn sources(&self) -> Option<&LineSources> {
        self.state.sources.as_ref()
    }

    fn io_snapshot(&self) -> IoSnapshotRead {
        self.io.clone()
    }

    fn is_held(&self) -> bool {
        self.state.debug.is_held()
    }

    fn accepts_step(&self) -> bool {
        self.state.debug.accepts_step()
    }

    fn step(&self, kind: StepKind) {
        self.state.debug.step(kind);
        self.wake.notify_all();
    }

    fn last_stop(&self) -> Option<StopRecord> {
        self.state.debug.last_stop()
    }

    fn drain_stops(&self) -> Vec<StopRecord> {
        self.state.debug.drain_stops()
    }

    fn drain_logs(&self) -> Vec<LogRecord> {
        self.state.debug.drain_logs()
    }

    fn frame_span(&self, frame: FrameId) -> Option<StatementSpan> {
        self.state.debug.frame_span(frame)
    }

    fn breakpoints(&self) -> Vec<BreakpointView> {
        self.state.debug.breakpoints()
    }

    fn replace_breakpoints(
        &self,
        file_id: u32,
        breakpoints: Vec<(Span<(u32, u32)>, BreakpointOptions)>,
    ) -> u64 {
        self.state.debug.replace_breakpoints(file_id, breakpoints)
    }

    fn clear_breakpoints(&self) {
        self.state.debug.clear_breakpoints();
    }

    fn with_handles(&self, operation: &mut dyn FnMut(&mut DebugVariableHandles)) {
        self.state.debug.with_handles(operation);
    }

    fn program_memory(&self) -> Option<&dyn ProgramMemory> {
        self.state
            .memory
            .as_ref()
            .map(|memory| memory as &dyn ProgramMemory)
    }

    fn undebuggable(&self) -> Option<&'static str> {
        self.state
            .release
            .then_some(trust_runtime_services::debug::replies::NO_DEBUG_HOOKS)
    }

    fn evaluates_expressions(&self) -> bool {
        true
    }
}
