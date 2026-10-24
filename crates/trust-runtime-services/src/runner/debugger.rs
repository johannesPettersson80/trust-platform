//! The debugger of a device's resource loop: the state that decides when a scan is held at a
//! statement start and when the hold ends (specification 13, "Run Control", "Stepping Semantics"
//! and "Breakpoint evaluation, source resolution, and hook contract"; specification 11 §6.9;
//! specification 34, STATE-07), and the state that the debug operations read and change.
//!
//! The decisions are the shared ones of [`crate::debug`]: [`RunControl`](crate::debug::RunControl)
//! decides pauses, steps, holds and stops, and [`observe_hit`](crate::debug::observe_hit) decides
//! which breakpoint stops. [`ChipDebug`] keeps the fields they work on, and what the debug
//! operations list: the breakpoints and their generations, the bounded buffer of stops, the
//! statement each frame last stood at and the variable handles of the current stop. It owns no
//! lock, thread or clock: a device's loop is one thread, so the fields sit in one `RefCell`.
//!
//! Every operation borrows the cell for its own work only and calls nothing outside this module
//! while it holds the borrow, so no operation can find the state in use: a hold serves a `resume`
//! after the statement hook has released it. An operation that still found it in use would not
//! panic, which on a device is a firmware fault (specification 34, STATE-05). It would do nothing
//! and record that it was refused (`ChipDebug::take_busy`), and the loop then faults the
//! resource.
//!
//! A breakpoint and a statement are placed by the `(line, column)` of the module's `LINE_TABLE`
//! (specification 12 §6.13), because a device holds no source text.

use alloc::collections::{BTreeMap, VecDeque};
use alloc::vec::Vec;
use core::cell::{Cell, RefCell};
use core::convert::Infallible;

use trust_runtime_core::debug::StatementSpan;
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::memory::FrameId;

use crate::debug::projection::{DebugVariableHandles, CHIP_HANDLE_BOUND};
use crate::debug::{
    observe_hit, Breakpoint, BreakpointView, CycleSuppression, RunAction, RunControl, RunMode,
    Span, StatementId, StepKind, StepState, StopReason, StopRecord,
};

/// A position in a source file: the 1-based `(line, column)` of the module's `LINE_TABLE`.
pub(super) type Position = (u32, u32);

/// The thread that every statement of a device runs on until the debugger tells tasks apart, as a
/// new trust-runtime debug control starts with current thread 1 (specification 13, "Runtime debug
/// state, mutation, and projection invariants").
const FIRST_THREAD: u32 = 1;

/// A statement start that the hook found: where in the call stack it runs and where in the source it
/// stands.
#[derive(Debug, Clone, Copy)]
pub(super) struct StatementStart<'a> {
    /// The call depth of the statement's POU (specification 13, "Stepping Semantics").
    pub(super) depth: u32,
    /// The statement's file and its half-open source span.
    pub(super) span: StatementSpan,
    /// The identity of the frame that runs the statement: the interpreter's activation, or a
    /// compiled program's frame by its call depth; `None` for a statement of no frame.
    pub(super) frame: Option<FrameId>,
    /// Ties the start to the hook call it was made in.
    pub(super) call: core::marker::PhantomData<&'a ()>,
}

/// What the statements of a compiled program's scan stop for (specification 44 §13.2): every
/// statement while a pause waits or a step runs, otherwise the starts of the breakpoints, each by
/// its file and its position.
#[cfg(feature = "native")]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct StopPoints {
    /// Every statement stops.
    pub(super) every: bool,
    /// The file and the start of each breakpoint's statement.
    pub(super) starts: Vec<(u32, Position)>,
}

/// What the hook does at a statement start.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Boundary {
    /// The scan runs on.
    Run,
    /// The debugger holds the scan here.
    Hold,
}

/// The latest stop that the debugger emitted (specification 13, "Stopped Events").
pub(super) type LastStop = StopRecord;

/// A stop of `reason` before the statement `at`.
fn stop_at(
    reason: StopReason,
    at: &StatementStart<'_>,
    thread: Option<u32>,
    breakpoint_generation: Option<u64>,
) -> LastStop {
    StopRecord {
        reason,
        statement: at.span,
        thread,
        breakpoint_generation,
    }
}

/// A frame on the call stack and the statement it last started, at the call depth it runs at.
#[derive(Debug, Clone, Copy)]
struct FrameStop {
    depth: u32,
    frame: FrameId,
    statement: StatementSpan,
}

/// The run fields of the debugger, which [`RunControl`] borrows for one decision.
struct RunFields {
    mode: RunMode,
    pending_stop: Option<StopReason>,
    current_thread: Option<u32>,
    target_thread: Option<u32>,
    steps: BTreeMap<u32, StepState>,
    last_call_depth: u32,
    last_call_depths: BTreeMap<u32, u32>,
    stop_requested: bool,
}

impl RunFields {
    /// A running scan on the first thread at call depth 0, with no target.
    fn new() -> Self {
        Self {
            mode: RunMode::Running,
            pending_stop: None,
            current_thread: Some(FIRST_THREAD),
            target_thread: None,
            steps: BTreeMap::new(),
            last_call_depth: 0,
            // The thread's depth is recorded from the start, so recording it at a statement
            // replaces a value and allocates nothing inside a scan.
            last_call_depths: BTreeMap::from([(FIRST_THREAD, 0)]),
            stop_requested: false,
        }
    }

    fn control(&mut self) -> RunControl<'_> {
        RunControl {
            mode: &mut self.mode,
            pending_stop: &mut self.pending_stop,
            current_thread: &mut self.current_thread,
            target_thread: &mut self.target_thread,
            steps: &mut self.steps,
            last_call_depth: &mut self.last_call_depth,
            last_call_depths: &mut self.last_call_depths,
            stop_requested: &mut self.stop_requested,
        }
    }
}

struct DebugState {
    run: RunFields,
    /// The breakpoints in registration order; the first eligible one owns a stop.
    breakpoints: Vec<Breakpoint<Position>>,
    /// The generation of each file's breakpoint set; a replacement advances it (specification 13,
    /// "Runtime debug state, mutation, and projection invariants").
    generations: BTreeMap<u32, u64>,
    /// The ordinary stops taken in the current scan.
    suppression: CycleSuppression<Position>,
    last_stop: Option<LastStop>,
    /// The stops that `debug.stops` has not returned yet, oldest first, at most `stop_capacity`.
    stops: VecDeque<LastStop>,
    stop_capacity: usize,
    /// The statement each frame on the call stack last stood at, outermost frame first.
    frame_locations: Vec<FrameStop>,
    /// The variable handles of the current stop.
    handles: DebugVariableHandles,
    /// Whether a scan has ended since the program started: the published state of a read between
    /// scans exists from then on (specification 11 §6.13.1).
    published: bool,
}

/// The debugger of one device's resource loop.
pub(super) struct ChipDebug {
    state: RefCell<DebugState>,
    /// An operation found the state in use and did nothing.
    busy: Cell<bool>,
}

impl ChipDebug {
    /// A debugger that holds nothing: running, with no breakpoint. Its buffer of stops takes
    /// `stop_capacity` stops at once, so a heap that is too small for it fails here, at start,
    /// never in the middle of a scan (specification 34, ARCH-05); a capacity of 0 reserves nothing
    /// and keeps only the latest stop.
    pub(super) fn new(stop_capacity: usize) -> Self {
        Self {
            state: RefCell::new(DebugState {
                run: RunFields::new(),
                breakpoints: Vec::new(),
                generations: BTreeMap::new(),
                suppression: CycleSuppression::default(),
                last_stop: None,
                stops: VecDeque::with_capacity(stop_capacity),
                stop_capacity,
                frame_locations: Vec::new(),
                handles: DebugVariableHandles::with_bound(CHIP_HANDLE_BOUND),
                published: false,
            }),
            busy: Cell::new(false),
        }
    }

    /// Runs `operation` on the state, or records that the state was in use and runs nothing.
    /// Keeps up to `capacity` stops for `debug.stops` from now on, reserved at once, as
    /// [`Self::new`] reserves them (specification 11 §6.13.3), and drops the stops kept so far.
    /// Returns `false`, the buffer as it was, when the heap has no room for them or the state is in
    /// use.
    pub(super) fn resize_stops(&self, capacity: usize) -> bool {
        self.with(|state| {
            if capacity == state.stop_capacity {
                return true;
            }
            let mut stops = VecDeque::new();
            if stops.try_reserve_exact(capacity).is_err() {
                return false;
            }
            state.stops = stops;
            state.stop_capacity = capacity;
            true
        })
        .unwrap_or(false)
    }

    /// The stops `debug.stops` keeps at most.
    #[cfg(test)]
    pub(super) fn stop_capacity(&self) -> usize {
        self.with(|state| state.stop_capacity).unwrap_or(0)
    }

    fn with<R>(&self, operation: impl FnOnce(&mut DebugState) -> R) -> Option<R> {
        if let Ok(mut state) = self.state.try_borrow_mut() {
            Some(operation(&mut state))
        } else {
            self.busy.set(true);
            None
        }
    }

    /// Whether an operation found the state in use since the last call.
    pub(super) fn take_busy(&self) -> bool {
        self.busy.take()
    }

    /// Whether the state is free, for the tests that prove no borrow outlives an operation.
    #[cfg(test)]
    pub(super) fn is_free(&self) -> bool {
        self.state.try_borrow_mut().is_ok()
    }

    /// A debug-mode `pause`: the debugger holds the scan at its next statement start
    /// (specification 13, "Pause"). A pause while the scan is already held changes nothing.
    pub(super) fn pause(&self) {
        self.apply(RunAction::Pause(None));
    }

    /// `resume`: releases a hold and clears a pending pause, whichever mode the resource runs in
    /// (specification 11 §6.9; specification 13, "Continue").
    pub(super) fn resume(&self) {
        self.apply(RunAction::Continue);
    }

    /// A step of `kind` from a hold: the scan runs until its own boundary and is held again there
    /// (specification 13, "Stepping Semantics").
    pub(super) fn step(&self, kind: StepKind) {
        self.apply(match kind {
            StepKind::Into => RunAction::StepIn(None),
            StepKind::Over => RunAction::StepOver(None),
            StepKind::Out => RunAction::StepOut(None),
        });
    }

    fn apply(&self, action: RunAction) {
        self.with(|state| state.run.control().apply(action));
    }

    /// Whether the debugger holds the scan, or has a hold pending (specification 13, "Pause").
    pub(super) fn is_held(&self) -> bool {
        self.with(|state| state.run.control().is_held())
            .unwrap_or(false)
    }

    /// What the statements of a compiled program's next stretch of scan stop for (specification 44
    /// §13.2): nothing once a stop request ended the holds.
    #[cfg(feature = "native")]
    pub(super) fn stop_points(&self) -> StopPoints {
        self.with(|state| {
            let held = {
                let control = state.run.control();
                if control.stop_requested() {
                    return StopPoints::default();
                }
                control.is_held()
            };
            StopPoints {
                every: held || !state.run.steps.is_empty(),
                starts: state
                    .breakpoints
                    .iter()
                    .map(|breakpoint| (breakpoint.id.file_id, breakpoint.id.span.start))
                    .collect(),
            }
        })
        .unwrap_or_default()
    }

    /// Whether a step is accepted now: only while the debugger holds the scan (specification 11
    /// §6.9).
    pub(super) fn accepts_step(&self) -> bool {
        self.with(|state| state.run.control().accepts_step())
            .unwrap_or(false)
    }

    /// Adds `breakpoint` after the breakpoints already set.
    #[cfg(test)]
    pub(super) fn add_breakpoint(&self, breakpoint: Breakpoint<Position>) {
        self.with(|state| state.breakpoints.push(breakpoint));
    }

    /// Replaces the breakpoints of file `file_id` with breakpoints on `spans`, in the request's
    /// order, and returns the file's new generation. The file's generation advances by one, with
    /// saturation, even when `spans` is empty, and every replacement breakpoint starts with no
    /// hit; the breakpoints of other files stay (specification 13, "Runtime debug state,
    /// mutation, and projection invariants").
    pub(super) fn replace_breakpoints(&self, file_id: u32, spans: Vec<Span<Position>>) -> u64 {
        self.with(|state| {
            let generation = {
                let entry = state.generations.entry(file_id).or_insert(0);
                *entry = entry.saturating_add(1);
                *entry
            };
            state
                .breakpoints
                .retain(|breakpoint| breakpoint.id.file_id != file_id);
            state.breakpoints.extend(spans.into_iter().map(|span| {
                Breakpoint::new(StatementId {
                    file_id,
                    span,
                    generation,
                })
            }));
            generation
        })
        .unwrap_or(0)
    }

    /// Clears every breakpoint, every generation and the stops that the scan in progress took
    /// (specification 13, "Runtime debug state, mutation, and projection invariants").
    pub(super) fn clear_breakpoints(&self) {
        self.with(|state| {
            state.breakpoints.clear();
            state.generations.clear();
            state.suppression.begin_cycle();
        });
    }

    /// The breakpoints, in registration order.
    pub(super) fn breakpoints(&self) -> Vec<BreakpointView> {
        self.with(|state| {
            state
                .breakpoints
                .iter()
                .map(|breakpoint| BreakpointView {
                    file_id: breakpoint.id.file_id,
                    start: breakpoint.id.span.start,
                    end: breakpoint.id.span.end,
                })
                .collect()
        })
        .unwrap_or_default()
    }

    /// The latest stop.
    pub(super) fn last_stop(&self) -> Option<LastStop> {
        self.with(|state| state.last_stop).flatten()
    }

    /// The stops since the last call, oldest first, and an empty buffer after it (specification 13,
    /// "Runtime debug state, mutation, and projection invariants").
    pub(super) fn drain_stops(&self) -> Vec<LastStop> {
        self.with(|state| state.stops.drain(..).collect())
            .unwrap_or_default()
    }

    /// The statement that `frame` last stood at, while it is on the call stack.
    pub(super) fn frame_span(&self, frame: FrameId) -> Option<StatementSpan> {
        self.with(|state| {
            state
                .frame_locations
                .iter()
                .find(|stop| stop.frame == frame)
                .map(|stop| stop.statement)
        })
        .flatten()
    }

    /// The thread that runs the statements from now on: a task starts (specification 13,
    /// "Multi-Task Thread Model"; specification 11 §6.13.3). It is out of line, so the scan's
    /// observer, which runs from RAM on a chip, only calls it.
    #[inline(never)]
    pub(super) fn set_thread(&self, thread: Option<u32>) {
        self.with(|state| state.run.current_thread = thread);
    }

    /// A scan has ended, completed or faulted: a read between scans has a published state from
    /// now on (specification 11 §6.13.1).
    pub(super) fn scan_ended(&self) {
        self.with(|state| state.published = true);
    }

    /// The program starts again, or restarts in place: there is no published state until the next
    /// scan ends (specification 11 §6.13.1).
    pub(super) fn restarted(&self) {
        self.with(|state| state.published = false);
    }

    /// Whether a scan has ended since the program started.
    pub(super) fn published(&self) -> bool {
        self.with(|state| state.published).unwrap_or(false)
    }

    /// Runs `operation` on the variable handles of the current stop.
    pub(super) fn with_handles(&self, operation: &mut dyn FnMut(&mut DebugVariableHandles)) {
        self.with(|state| operation(&mut state.handles));
    }

    /// The wire label of the reason that a hold keeps the scan paused, as `status` names it, or
    /// none while the scan runs (specification 11 §6.9: while the debugger holds the scan, `status`
    /// reports the hold). A pause that no statement has taken yet reports its own reason.
    pub(super) fn hold_label(&self) -> Option<&'static str> {
        self.with(|state| {
            let last_stop = state.last_stop.map(|stop| stop.reason);
            state
                .run
                .control()
                .hold_reason(last_stop)
                .map(StopReason::label)
        })
        .flatten()
    }

    /// A scan begins: every ordinary stop of the last one is forgotten, so a breakpoint may stop
    /// again (specification 13, "Breakpoint evaluation, source resolution, and hook contract").
    pub(super) fn begin_cycle(&self) {
        self.with(|state| state.suppression.begin_cycle());
    }

    /// A stop ends the holds, and the debugger holds no scan until the program starts again
    /// (`rearm`): the scan that is held runs to its end with no further pause, step or
    /// breakpoint, and the resource then takes its ordinary stop path (specification 11 §6.9,
    /// "Operations while the debugger holds the scan"). It clears a pause that arrived after the
    /// request, too, so a program that ended holds nothing.
    pub(super) fn release_for_stop(&self) {
        self.with(|state| state.run.control().release_for_stop());
    }

    /// The program starts again after it ended, as a new resource thread would start: the
    /// debugger may hold it, though the stop ended the holds of the last run (specification 11
    /// §6.9: "a resource thread that starts later on the same debugger can be held again"). The
    /// breakpoints stay.
    pub(super) fn rearm(&self) {
        self.with(|state| state.run.control().rearm());
    }

    /// An online change replaced the program (specification 34 §11.3), as a computer's
    /// `invalidate_schema_observations` does: what belonged to the old statements is forgotten, the
    /// call depths, the steps, the thread identities and the last stop, and the next statement
    /// supplies them again. The breakpoints stay, and so does a pause (specification 13, "Hold
    /// duration": a configuration change does not release a hold); the change itself happens at a
    /// boundary, where no scan is held.
    pub(super) fn program_replaced(&self) {
        self.with(|state| {
            state.run.control().retire_observations();
            state.last_stop = None;
        });
    }

    /// The decision at a statement start (specification 13, "Runtime Debug-Control Execution" and
    /// "Breakpoint evaluation, source resolution, and hook contract"), in the order trust-runtime's
    /// hook takes it: the boundary is recorded and bound to its frame, the pending pause of a held
    /// scan is emitted, and a step, or else a breakpoint, stops here.
    ///
    /// # Errors
    ///
    /// [`RuntimeError::InvalidExecutionState`] when the state was in use.
    pub(super) fn at_statement(&self, at: &StatementStart<'_>) -> Result<Boundary, RuntimeError> {
        self.with(|state| {
            let DebugState {
                run,
                breakpoints,
                suppression,
                last_stop,
                stops,
                stop_capacity,
                frame_locations,
                ..
            } = state;
            let mut control = run.control();
            // A stop request released the holds and no new one may start.
            if control.stop_requested() {
                return Boundary::Run;
            }
            control.record_boundary(at.depth);
            bind_frame_location(frame_locations, at);
            if let Some(reason) = control.take_pending_stop() {
                let stop = stop_at(reason, at, *control.current_thread, None);
                record_stop(last_stop, stops, *stop_capacity, stop);
            }
            let decided = control.boundary(at.depth, || {
                Ok::<_, Infallible>(match_breakpoint(breakpoints, suppression, at))
            });
            let Ok(stop) = decided;
            if let Some(stop) = stop {
                let stop = stop_at(
                    stop.reason,
                    at,
                    *control.current_thread,
                    stop.breakpoint_generation,
                );
                record_stop(last_stop, stops, *stop_capacity, stop);
            }
            if control.holds_here() {
                Boundary::Hold
            } else {
                Boundary::Run
            }
        })
        .ok_or(RuntimeError::InvalidExecutionState)
    }

    /// Whether the scan is still held at the statement `at`, after the requests of a pass: a pause
    /// that arrived after a continue emits its stop first, as trust-runtime's hold does when it
    /// wakes.
    ///
    /// # Errors
    ///
    /// [`RuntimeError::InvalidExecutionState`] when the state was in use.
    pub(super) fn holds(&self, at: &StatementStart<'_>) -> Result<bool, RuntimeError> {
        self.with(|state| {
            let DebugState {
                run,
                last_stop,
                stops,
                stop_capacity,
                ..
            } = state;
            let mut control = run.control();
            if let Some(reason) = control.take_pending_stop() {
                let stop = stop_at(reason, at, *control.current_thread, None);
                record_stop(last_stop, stops, *stop_capacity, stop);
            }
            control.holds_here()
        })
        .ok_or(RuntimeError::InvalidExecutionState)
    }
}

/// Records `stop` as the latest stop and in the buffer of stops. A stop beyond the buffer's
/// capacity replaces the oldest; with no capacity only the latest stop is kept. The buffer holds
/// all its capacity from the start, so this allocates nothing.
fn record_stop(
    last_stop: &mut Option<LastStop>,
    stops: &mut VecDeque<LastStop>,
    capacity: usize,
    stop: LastStop,
) {
    *last_stop = Some(stop);
    if capacity == 0 {
        return;
    }
    if stops.len() >= capacity {
        stops.pop_front();
    }
    stops.push_back(stop);
}

/// Binds the statement to the frame that runs it, by the call depth it runs at: a statement at
/// depth `d` ends every frame at depth `d` or deeper, which have returned, and leaves the frames
/// below it at the statement that called into the next one. The runtime updates the active frame's
/// location at each statement boundary (specification 13, "Runtime Debug-Control Execution"). The
/// list keeps its capacity, so a statement allocates nothing once the stack has been that deep.
fn bind_frame_location(locations: &mut Vec<FrameStop>, at: &StatementStart<'_>) {
    let Some(frame) = at.frame else {
        return;
    };
    while locations.last().is_some_and(|stop| stop.depth >= at.depth) {
        locations.pop();
    }
    locations.push(FrameStop {
        depth: at.depth,
        frame,
        statement: at.span,
    });
}

/// The generation of the breakpoint that stops at the statement `at`, if there is one: the first
/// breakpoint, in registration order, whose hit the shared rule lets through. That breakpoint then
/// stops no more in this scan (specification 13, "Breakpoint evaluation, source resolution, and hook
/// contract").
fn match_breakpoint(
    breakpoints: &mut [Breakpoint<Position>],
    suppression: &mut CycleSuppression<Position>,
    at: &StatementStart<'_>,
) -> Option<u64> {
    for breakpoint in breakpoints.iter_mut() {
        if observe_hit(
            breakpoint,
            |id| suppression.suppresses(id),
            at.span.file_index,
            at.span.start,
        ) {
            suppression.record_stop(breakpoint.id);
            return Some(breakpoint.id.generation);
        }
    }
    None
}
