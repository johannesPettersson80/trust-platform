//! The debugger of a device's resource loop: the state that decides when a scan is held at a
//! statement start and when the hold ends (specification 13, "Run Control", "Stepping Semantics"
//! and "Breakpoint evaluation, source resolution, and hook contract"; specification 11 §6.9;
//! specification 34, STATE-07).
//!
//! The decisions are the shared ones of [`crate::debug`]: [`RunControl`](crate::debug::RunControl)
//! decides pauses, steps, holds and stops, and [`observe_hit`](crate::debug::observe_hit) decides
//! which breakpoint stops. [`ChipDebug`] keeps the fields they work on. It owns no lock, thread or
//! clock: a device's loop is one thread, so the fields sit in one `RefCell`.
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

use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use core::cell::{Cell, RefCell};
use core::convert::Infallible;

use trust_runtime_core::debug::StatementSpan;
use trust_runtime_core::error::RuntimeError;

use crate::debug::{
    observe_hit, Breakpoint, CycleSuppression, RunAction, RunControl, RunMode, StepKind, StepState,
    StopReason,
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
pub(super) struct StatementStart {
    /// The call depth of the statement's POU (specification 13, "Stepping Semantics").
    pub(super) depth: u32,
    /// The statement's file and its half-open source span.
    pub(super) span: StatementSpan,
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "S3d-8's debug.state and debug.stops report where and why the scan stopped"
    )
)]
pub(super) struct LastStop {
    /// Why the scan stopped.
    pub(super) reason: StopReason,
    /// The statement that the scan stopped before.
    pub(super) statement: StatementSpan,
    /// The thread that stopped.
    pub(super) thread: Option<u32>,
    /// The generation of the breakpoint set that named a breakpoint stop, and none for any other.
    pub(super) breakpoint_generation: Option<u64>,
}

impl LastStop {
    /// A stop of `reason` before the statement `at`.
    fn at(
        reason: StopReason,
        at: &StatementStart,
        thread: Option<u32>,
        breakpoint_generation: Option<u64>,
    ) -> Self {
        Self {
            reason,
            statement: at.span,
            thread,
            breakpoint_generation,
        }
    }
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
    /// The ordinary stops taken in the current scan.
    suppression: CycleSuppression<Position>,
    last_stop: Option<LastStop>,
}

/// The debugger of one device's resource loop.
pub(super) struct ChipDebug {
    state: RefCell<DebugState>,
    /// An operation found the state in use and did nothing.
    busy: Cell<bool>,
}

impl ChipDebug {
    /// A debugger that holds nothing: running, with no breakpoint.
    pub(super) fn new() -> Self {
        Self {
            state: RefCell::new(DebugState {
                run: RunFields::new(),
                breakpoints: Vec::new(),
                suppression: CycleSuppression::default(),
                last_stop: None,
            }),
            busy: Cell::new(false),
        }
    }

    /// Runs `operation` on the state, or records that the state was in use and runs nothing.
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
    #[cfg_attr(
        not(test),
        allow(
            dead_code,
            reason = "S3d-8 routes step_in, step_over and step_out here"
        )
    )]
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

    /// Adds `breakpoint` after the breakpoints already set.
    #[cfg_attr(
        not(test),
        allow(
            dead_code,
            reason = "S3d-8's breakpoints.set resolves and sets the breakpoints"
        )
    )]
    pub(super) fn add_breakpoint(&self, breakpoint: Breakpoint<Position>) {
        self.with(|state| state.breakpoints.push(breakpoint));
    }

    /// The latest stop.
    #[cfg_attr(
        not(test),
        allow(
            dead_code,
            reason = "S3d-8's debug.stops and debug.state report the stop"
        )
    )]
    pub(super) fn last_stop(&self) -> Option<LastStop> {
        self.with(|state| state.last_stop).flatten()
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

    /// The decision at a statement start (specification 13, "Runtime Debug-Control Execution" and
    /// "Breakpoint evaluation, source resolution, and hook contract"), in the order trust-runtime's
    /// hook takes it: the boundary is recorded, the pending pause of a held scan is emitted, and a
    /// step, or else a breakpoint, stops here.
    ///
    /// # Errors
    ///
    /// [`RuntimeError::InvalidExecutionState`] when the state was in use.
    pub(super) fn at_statement(&self, at: &StatementStart) -> Result<Boundary, RuntimeError> {
        self.with(|state| {
            let DebugState {
                run,
                breakpoints,
                suppression,
                last_stop,
            } = state;
            let mut control = run.control();
            // A stop request released the holds and no new one may start.
            if control.stop_requested() {
                return Boundary::Run;
            }
            control.record_boundary(at.depth);
            if let Some(reason) = control.take_pending_stop() {
                *last_stop = Some(LastStop::at(reason, at, *control.current_thread, None));
            }
            let decided = control.boundary(at.depth, || {
                Ok::<_, Infallible>(match_breakpoint(breakpoints, suppression, at))
            });
            let Ok(stop) = decided;
            if let Some(stop) = stop {
                *last_stop = Some(LastStop::at(
                    stop.reason,
                    at,
                    *control.current_thread,
                    stop.breakpoint_generation,
                ));
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
    pub(super) fn holds(&self, at: &StatementStart) -> Result<bool, RuntimeError> {
        self.with(|state| {
            let DebugState { run, last_stop, .. } = state;
            let mut control = run.control();
            if let Some(reason) = control.take_pending_stop() {
                *last_stop = Some(LastStop::at(reason, at, *control.current_thread, None));
            }
            control.holds_here()
        })
        .ok_or(RuntimeError::InvalidExecutionState)
    }
}

/// The generation of the breakpoint that stops at the statement `at`, if there is one: the first
/// breakpoint, in registration order, whose hit the shared rule lets through. That breakpoint then
/// stops no more in this scan (specification 13, "Breakpoint evaluation, source resolution, and hook
/// contract").
fn match_breakpoint(
    breakpoints: &mut [Breakpoint<Position>],
    suppression: &mut CycleSuppression<Position>,
    at: &StatementStart,
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
