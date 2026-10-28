//! Run control of the shared debug state: whether a statement boundary stops, and what a pause, a
//! continue, a step, a resource stop or a schema change does to the run state (specification 13,
//! "Pause", "Continue", "Stepping Semantics" and "Breakpoint evaluation, source resolution, and hook
//! contract"; specification 11 section 6.9).
//!
//! [`RunControl`] borrows the run fields of one debugger for one decision. The composing debugger
//! keeps those fields, and the lock, the condition variable and the wait that holds a scan. This
//! module reads no clock, starts no thread and takes no lock. A breakpoint is matched by the
//! composing debugger, because its condition needs the storage: [`RunControl::boundary`] asks for
//! that match only when no step stops at the boundary, which is the order specification 13 requires.

use alloc::collections::BTreeMap;

/// The reasons a stop carries (specification 13, "Stopped Events"). The trust-runtime debugger
/// re-exports this enum as its stop reason, so a stop carries the same reason on every runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopReason {
    /// A stop at a breakpoint.
    Breakpoint,
    /// A stop that completes a step.
    Step,
    /// A stop at a pause request.
    Pause,
    /// A stop on entry.
    Entry,
}

impl StopReason {
    /// The stable wire label of the reason: how `status` names a debugger hold and how a stop
    /// names its cause (specification 13, "Stopped Events").
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Breakpoint => "breakpoint",
            Self::Step => "step",
            Self::Pause => "pause",
            Self::Entry => "entry",
        }
    }
}

/// Whether the scan runs or the debugger holds it (specification 13, "Pause").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunMode {
    /// The scan runs without a hold.
    Running,
    /// The debugger holds the scan at the boundary of its last stop.
    Paused,
}

/// The step behaviour of one thread (specification 13, "Stepping Semantics").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepKind {
    /// Stop at the next executed statement, whatever its call depth.
    Into,
    /// Stop at the next statement at the starting depth or a shallower one.
    Over,
    /// Stop after returning to the caller, at a depth below the starting one.
    Out,
}

/// A step of one thread that waits for its stop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StepState {
    /// The step behaviour.
    pub kind: StepKind,
    /// The call depth that an over or out step stops at or above.
    pub target_depth: u32,
    /// Whether the step has passed its arming boundary. A step requested from a hold is armed at
    /// once, so the next boundary may stop it. A step requested while the scan runs is armed by the
    /// next boundary, which does not stop.
    pub started: bool,
}

/// A control action of a debugger (specification 13, "Run Control").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunAction {
    /// Pause at the next statement boundary, for one thread or for every thread.
    Pause(Option<u32>),
    /// Resume every thread.
    Continue,
    /// Execute one statement, then pause again.
    StepIn(Option<u32>),
    /// Execute the current statement without entering calls, then pause again.
    StepOver(Option<u32>),
    /// Run until the current call returns, then pause again.
    StepOut(Option<u32>),
}

/// Whether a control action changed the run state. A pause while the scan is already held succeeds
/// and changes nothing (specification 13, "Pause").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunOutcome {
    /// The action changed the run state.
    Applied,
    /// The action had no effect.
    Ignored,
}

/// What applying a control action changed, for the composing debugger to act on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RunEffect {
    /// Whether the action applied or was ignored.
    pub outcome: RunOutcome,
    /// Whether the action resumed the scan, so that a waiting hold must wake.
    pub resumed: bool,
}

/// A stop taken at a statement boundary: its reason, and for a breakpoint stop the generation of the
/// breakpoint set that names it (specification 13, "Runtime debug state, mutation, and projection
/// invariants").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoundaryStop {
    /// The reason the stop is emitted with.
    pub reason: StopReason,
    /// The breakpoint generation of a breakpoint stop, and `None` for any other stop.
    pub breakpoint_generation: Option<u64>,
}

/// The run fields of one debugger, borrowed for one decision. The composing debugger holds them
/// under its lock, and keeps every field that the run control does not decide on.
pub struct RunControl<'a> {
    /// Running, or held at a boundary.
    pub mode: &'a mut RunMode,
    /// The stop that the next boundary of the target thread emits while the scan is held.
    pub pending_stop: &'a mut Option<StopReason>,
    /// The thread that runs the current statement, if it is known.
    pub current_thread: &'a mut Option<u32>,
    /// The thread that a pause or a step targets. `None` targets every thread.
    pub target_thread: &'a mut Option<u32>,
    /// The steps that wait for their stop, keyed by thread. Key `0` belongs to a step of no thread.
    pub steps: &'a mut BTreeMap<u32, StepState>,
    /// The call depth of the last statement boundary.
    pub last_call_depth: &'a mut u32,
    /// The call depth of the last statement boundary of each thread.
    pub last_call_depths: &'a mut BTreeMap<u32, u32>,
    /// Set by a resource stop. It ends the hold and forbids a new one until [`RunControl::rearm`]
    /// (specification 11 section 6.9).
    pub stop_requested: &'a mut bool,
}

impl RunControl<'_> {
    /// Whether the debugger holds the scan, whichever thread is the target (specification 13,
    /// "Pause").
    #[must_use]
    pub fn is_held(&self) -> bool {
        *self.mode == RunMode::Paused
    }

    /// Whether a step request is accepted now. A step acts only while the debugger holds the scan
    /// (specification 11 section 6.9, `step_in`, `step_over` and `step_out`).
    #[must_use]
    pub fn accepts_step(&self) -> bool {
        self.is_held()
    }

    /// The reason that a hold reports, or `None` while the scan runs. A pause that is still pending
    /// reports its own reason; otherwise the hold reports the reason of the last stop, which the
    /// composing debugger keeps in `last_stop`; otherwise it reports a pause (specification 11 section
    /// 6.9: while the debugger holds the scan, `status` reports the hold).
    #[must_use]
    pub fn hold_reason(&self, last_stop: Option<StopReason>) -> Option<StopReason> {
        if !self.is_held() {
            return None;
        }
        Some(
            (*self.pending_stop)
                .or(last_stop)
                .unwrap_or(StopReason::Pause),
        )
    }

    /// Whether the statement hook is at the target: no thread is targeted, or this thread is. A
    /// non-target thread never consumes or blocks on another thread's pause or step (specification
    /// 13, "Breakpoint evaluation, source resolution, and hook contract").
    #[must_use]
    pub fn is_target(&self) -> bool {
        self.target_thread.is_none() || *self.target_thread == *self.current_thread
    }

    /// Whether the scan waits in a hold at this boundary: the debugger holds the scan, this thread is
    /// the target, and no resource stop has ended the holds (specification 13, "Pause").
    #[must_use]
    pub fn holds_here(&self) -> bool {
        !*self.stop_requested && self.is_held() && self.is_target()
    }

    /// Whether a resource stop ended the holds, and no new hold may start until [`Self::rearm`].
    #[must_use]
    pub fn stop_requested(&self) -> bool {
        *self.stop_requested
    }

    /// Let the debugger hold the scan again after a resource stop. A new resource thread starts with
    /// the hold armed, so a control that an earlier stop released is not left inert.
    pub fn rearm(&mut self) {
        *self.stop_requested = false;
    }

    /// End the hold for a resource stop. The scan that the hold suspended runs to its end with no
    /// further pause, step or breakpoint, and the debugger holds no scan until [`Self::rearm`]
    /// (specification 11 section 6.9, "Operations while the debugger holds the scan").
    pub fn release_for_stop(&mut self) {
        *self.stop_requested = true;
        self.end_hold();
    }

    /// End the hold that a stop would have started, when that stop could not be published. The scan
    /// then never waits for a resume that no stop asked for (specification 11 section 6.9).
    pub fn abandon_stop(&mut self) {
        self.end_hold();
    }

    /// Clear the hold, its pending stop, its steps and its target. The scan runs on.
    fn end_hold(&mut self) {
        *self.mode = RunMode::Running;
        *self.pending_stop = None;
        self.steps.clear();
        *self.target_thread = None;
    }

    /// Retire the run state that the observations of a replaced schema carry: the call depths, the
    /// steps and the thread identities. The next statement supplies them again. A held scan stays
    /// held, and its pause is pending again, so it reports a pause until the next stop (specification
    /// 13, "Hold duration": a configuration change does not release a hold).
    pub fn retire_observations(&mut self) {
        *self.last_call_depth = 0;
        self.last_call_depths.clear();
        self.steps.clear();
        *self.current_thread = None;
        *self.target_thread = None;
        *self.pending_stop = self.is_held().then_some(StopReason::Pause);
    }

    /// Record the call depth of a statement boundary, which the steps that follow it use
    /// (specification 13, "Stepping Semantics").
    pub fn record_boundary(&mut self, call_depth: u32) {
        *self.last_call_depth = call_depth;
        if let Some(thread) = *self.current_thread {
            self.last_call_depths.insert(thread, call_depth);
        }
    }

    /// Take the pending stop when the scan is held at this boundary. One pending pause or entry is
    /// emitted once, at the next boundary of the target thread (specification 13, "Pause").
    pub fn take_pending_stop(&mut self) -> Option<StopReason> {
        if self.holds_here() {
            self.pending_stop.take()
        } else {
            None
        }
    }

    /// The stop of one statement boundary, if the boundary stops (specification 13, "Stepping
    /// Semantics" and "Breakpoint evaluation, source resolution, and hook contract").
    ///
    /// Nothing stops while a resource stop holds, or at a boundary where the debugger holds the scan.
    /// A step decides first: a step that stops here takes the boundary, and `breakpoint` is not asked.
    /// Otherwise `breakpoint` answers the generation of the breakpoint that stops here, after its hit
    /// count and its cycle suppression. A breakpoint stop ends every step and targets every thread.
    ///
    /// # Errors
    ///
    /// Returns the error of `breakpoint`. A step that armed at this boundary keeps its arming.
    pub fn boundary<E>(
        &mut self,
        call_depth: u32,
        breakpoint: impl FnOnce() -> Result<Option<u64>, E>,
    ) -> Result<Option<BoundaryStop>, E> {
        if self.stop_requested() || self.holds_here() {
            return Ok(None);
        }
        let stop = if let Some(reason) = self.step_stop(call_depth) {
            BoundaryStop {
                reason,
                breakpoint_generation: None,
            }
        } else if let Some(generation) = breakpoint()? {
            self.steps.clear();
            *self.target_thread = None;
            BoundaryStop {
                reason: StopReason::Breakpoint,
                breakpoint_generation: Some(generation),
            }
        } else {
            return Ok(None);
        };
        *self.pending_stop = None;
        *self.mode = RunMode::Paused;
        Ok(Some(stop))
    }

    /// Whether the step of the current thread stops at this boundary. A step that has not started is
    /// armed here and does not stop (specification 13, "Stepping Semantics").
    fn step_stop(&mut self, call_depth: u32) -> Option<StopReason> {
        if !self.is_target() {
            return None;
        }
        let key = self.step_key()?;
        let step = self.steps.get_mut(&key)?;
        if !step.started {
            step.started = true;
            return None;
        }
        let stops = match step.kind {
            StepKind::Into => true,
            StepKind::Over | StepKind::Out => call_depth <= step.target_depth,
        };
        if stops {
            self.steps.remove(&key);
            Some(StopReason::Step)
        } else {
            None
        }
    }

    /// The key of the step that applies to the current thread: its own step, or else the step of no
    /// thread.
    fn step_key(&self) -> Option<u32> {
        (*self.current_thread)
            .filter(|thread| self.steps.contains_key(thread))
            .or_else(|| self.steps.contains_key(&0).then_some(0))
    }

    /// The call depth that a step over or out of `target` starts from: the depth of the last boundary
    /// of that thread, or else the depth of the last boundary.
    fn depth_of(&self, target: Option<u32>) -> u32 {
        target
            .and_then(|thread| self.last_call_depths.get(&thread).copied())
            .unwrap_or(*self.last_call_depth)
    }

    /// Resume the scan. `target` becomes the target thread, and `step` replaces every earlier step.
    fn resume_with(&mut self, target: Option<u32>, step: Option<(u32, StepState)>) -> RunEffect {
        self.steps.clear();
        if let Some((key, state)) = step {
            self.steps.insert(key, state);
        }
        *self.mode = RunMode::Running;
        *self.pending_stop = None;
        *self.target_thread = target;
        RunEffect {
            outcome: RunOutcome::Applied,
            resumed: true,
        }
    }

    /// Apply a control action to the run state (specification 13, "Pause", "Continue" and "Stepping
    /// Semantics"). A step request from a hold is armed at once; one from a running scan is armed by
    /// the next boundary.
    pub fn apply(&mut self, action: RunAction) -> RunEffect {
        let step_started = *self.mode == RunMode::Paused;
        match action {
            RunAction::Pause(thread) => {
                if *self.mode == RunMode::Paused {
                    return RunEffect {
                        outcome: RunOutcome::Ignored,
                        resumed: false,
                    };
                }
                *self.mode = RunMode::Paused;
                self.steps.clear();
                *self.pending_stop = Some(StopReason::Pause);
                *self.target_thread = thread;
                RunEffect {
                    outcome: RunOutcome::Applied,
                    resumed: false,
                }
            }
            RunAction::Continue => self.resume_with(None, None),
            RunAction::StepIn(thread) => {
                let target = thread.or(*self.current_thread);
                let state = StepState {
                    kind: StepKind::Into,
                    target_depth: *self.last_call_depth,
                    started: step_started,
                };
                self.resume_with(target, Some((target.unwrap_or(0), state)))
            }
            RunAction::StepOver(thread) => {
                let target = thread.or(*self.current_thread);
                let state = StepState {
                    kind: StepKind::Over,
                    target_depth: self.depth_of(target),
                    started: step_started,
                };
                self.resume_with(target, Some((target.unwrap_or(0), state)))
            }
            RunAction::StepOut(thread) => {
                let target = thread.or(*self.current_thread);
                let state = StepState {
                    kind: StepKind::Out,
                    target_depth: self.depth_of(target).saturating_sub(1),
                    started: step_started,
                };
                self.resume_with(target, Some((target.unwrap_or(0), state)))
            }
        }
    }

    /// Start an entry pause: a pause with no thread target, whose stop reports entry (specification
    /// 13, "Runtime Debug-Control Execution"). It is ignored while the scan is already held.
    pub fn pause_entry(&mut self) -> RunOutcome {
        if *self.mode == RunMode::Paused {
            return RunOutcome::Ignored;
        }
        *self.mode = RunMode::Paused;
        self.steps.clear();
        *self.pending_stop = Some(StopReason::Entry);
        *self.target_thread = None;
        RunOutcome::Applied
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The run fields of one debugger, as a composing debugger keeps them.
    struct Fields {
        mode: RunMode,
        pending: Option<StopReason>,
        current: Option<u32>,
        target: Option<u32>,
        steps: BTreeMap<u32, StepState>,
        depth: u32,
        depths: BTreeMap<u32, u32>,
        stop_requested: bool,
    }

    impl Fields {
        /// A running scan on thread 1 at call depth 0, with no target.
        fn running() -> Self {
            Self {
                mode: RunMode::Running,
                pending: None,
                current: Some(1),
                target: None,
                steps: BTreeMap::new(),
                depth: 0,
                depths: BTreeMap::new(),
                stop_requested: false,
            }
        }

        /// The scan held at a boundary of `thread` at call depth `depth`.
        fn held(thread: u32, depth: u32) -> Self {
            let mut fields = Self {
                current: Some(thread),
                ..Self::running()
            };
            fields.mode = RunMode::Paused;
            fields.depth = depth;
            fields.depths.insert(thread, depth);
            fields
        }

        fn run(&mut self) -> RunControl<'_> {
            RunControl {
                mode: &mut self.mode,
                pending_stop: &mut self.pending,
                current_thread: &mut self.current,
                target_thread: &mut self.target,
                steps: &mut self.steps,
                last_call_depth: &mut self.depth,
                last_call_depths: &mut self.depths,
                stop_requested: &mut self.stop_requested,
            }
        }
    }

    /// The statement boundary of a statement at `depth` with no breakpoint on it: the hook records
    /// the depth, then asks the run control for the stop.
    fn arrive(fields: &mut Fields, depth: u32) -> Option<BoundaryStop> {
        fields.run().record_boundary(depth);
        fields
            .run()
            .boundary(depth, || Ok::<Option<u64>, ()>(None))
            .expect("the breakpoint lookup cannot fail")
    }

    #[test]
    fn a_step_from_a_pause_stops_only_at_the_next_boundary() {
        // Specification 13 lines 273-276: each step resumes from a paused state and pauses again at
        // exactly the next boundary. Wrong behaviour caught: a hold that outlives the step request,
        // so the held statement decides again and stops where the scan paused, and a step that is
        // only armed by the held statement, so it stops one statement late.
        let mut fields = Fields::held(1, 0);
        let effect = fields.run().apply(RunAction::StepIn(None));
        assert_eq!(effect.outcome, RunOutcome::Applied);
        assert!(!fields.run().holds_here());

        let stop = arrive(&mut fields, 0).expect("the step stops at the next boundary");
        assert_eq!(stop.reason, StopReason::Step);
        assert!(fields.steps.is_empty());
    }

    #[test]
    fn a_second_pause_while_held_emits_no_second_stop() {
        // Specification 13 line 125: a pause while already paused succeeds and does not emit another
        // pause event. Wrong behaviour caught: the second pause sets its own pending pause, so the
        // target's next boundary emits a second Pause stop for the same hold.
        let mut fields = Fields::running();
        let first = fields.run().apply(RunAction::Pause(None));
        assert_eq!(first.outcome, RunOutcome::Applied);
        assert_eq!(fields.run().take_pending_stop(), Some(StopReason::Pause));

        let second = fields.run().apply(RunAction::Pause(None));
        assert_eq!(second.outcome, RunOutcome::Ignored);
        assert_eq!(fields.run().take_pending_stop(), None);
        assert!(fields.run().holds_here());
    }

    #[test]
    fn a_continue_clears_a_pending_pause() {
        // Specification 13 line 108: a continue clears any pending pause request, and lines 387-388:
        // it removes every pending stop. Wrong behaviour caught: a continue that keeps the request, so
        // the request stays pending after the scan has resumed and fires at a later hold.
        let mut fields = Fields::running();
        fields.run().apply(RunAction::Pause(None));
        let effect = fields.run().apply(RunAction::Continue);
        assert_eq!(
            effect,
            RunEffect {
                outcome: RunOutcome::Applied,
                resumed: true,
            }
        );
        assert_eq!(fields.pending, None);
        assert_eq!(arrive(&mut fields, 0), None);
        assert_eq!(fields.run().take_pending_stop(), None);
    }

    #[test]
    fn step_over_ignores_deeper_frames_and_step_out_stops_after_the_return() {
        // Specification 13 lines 273-276: step over ignores deeper frames and stops at the starting
        // depth; step out ignores the current and deeper frames and stops after returning to the caller.
        // Wrong behaviour caught: a step over that stops in a callee, a step out that stops in the
        // frame it leaves, and a step out that stops two frames up.
        let mut over = Fields::held(1, 0);
        over.run().apply(RunAction::StepOver(None));
        assert_eq!(arrive(&mut over, 1), None);
        let returned = arrive(&mut over, 0).expect("the caller's next statement stops the step");
        assert_eq!(returned.reason, StopReason::Step);

        let mut out = Fields::held(1, 2);
        out.run().apply(RunAction::StepOut(None));
        assert_eq!(arrive(&mut out, 2), None);
        assert_eq!(arrive(&mut out, 3), None);
        let caller = arrive(&mut out, 1).expect("the caller of the left frame stops the step");
        assert_eq!(caller.reason, StopReason::Step);
    }

    #[test]
    fn a_non_target_thread_neither_consumes_nor_blocks_on_the_targets_pause() {
        // Specification 13 lines 362-363: a non-target runtime thread never consumes or blocks on
        // another thread's pause or step. Wrong behaviour caught: thread 2 takes the pending pause of
        // target thread 1, or waits in the hold that thread 1 requested.
        let mut fields = Fields {
            current: Some(2),
            target: Some(1),
            mode: RunMode::Paused,
            pending: Some(StopReason::Pause),
            ..Fields::running()
        };
        assert_eq!(fields.run().take_pending_stop(), None);
        assert!(!fields.run().holds_here());
        assert_eq!(arrive(&mut fields, 0), None);
        assert_eq!(fields.pending, Some(StopReason::Pause));
    }

    #[test]
    fn a_resource_stop_holds_no_scan_until_the_debugger_rearms() {
        // Specification 11 section 6.9, "A stop request is independent of the hold": the stop ends the
        // hold, and the debugger holds the scan no more until the next resource thread. Wrong behaviour
        // caught: a breakpoint stop or a hold that starts after the resource stop, and a rearm that
        // leaves the hold forbidden.
        let mut fields = Fields::held(1, 0);
        fields.run().release_for_stop();
        assert_eq!(fields.mode, RunMode::Running);
        assert!(fields.run().stop_requested());
        assert!(!fields.run().holds_here());
        let breakpoint = fields.run().boundary(0, || Ok::<_, ()>(Some(9)));
        assert_eq!(breakpoint, Ok(None));

        fields.run().rearm();
        assert!(!fields.run().stop_requested());
        let stop = fields
            .run()
            .boundary(0, || Ok::<_, ()>(Some(9)))
            .expect("the breakpoint lookup cannot fail")
            .expect("the rearmed debugger stops at the breakpoint");
        assert_eq!(stop.reason, StopReason::Breakpoint);
        assert_eq!(stop.breakpoint_generation, Some(9));
    }

    #[test]
    fn a_held_scan_reports_its_pending_reason_then_its_last_stop_then_a_pause() {
        // Specification 11 section 6.9 (line 4646): while a debugger holds the scan, `status` reports
        // the hold, and reports nothing while the scan runs. The precedence (a pending pause, then the
        // last stop, then a pause) is the rule of the debugger's hold reason. Wrong behaviour caught: the
        // last stop shadows a pause requested after it, a running scan reports a hold, or a held scan
        // with no recorded stop reports no reason.
        let mut fields = Fields::running();
        assert_eq!(fields.run().hold_reason(Some(StopReason::Breakpoint)), None);

        fields.mode = RunMode::Paused;
        assert_eq!(fields.run().hold_reason(None), Some(StopReason::Pause));
        assert_eq!(
            fields.run().hold_reason(Some(StopReason::Breakpoint)),
            Some(StopReason::Breakpoint)
        );

        fields.pending = Some(StopReason::Entry);
        assert_eq!(
            fields.run().hold_reason(Some(StopReason::Breakpoint)),
            Some(StopReason::Entry)
        );
    }

    #[test]
    fn a_schema_change_keeps_the_hold_and_pends_its_pause_again() {
        // Specification 13 lines 111-114 ("Hold duration"): a configuration change does not release a
        // hold. The retired observations are supplied again by the next statement. Wrong behaviour
        // caught: a schema change that releases the hold, leaves a step armed on the old statements, or
        // drops the pause, so the held scan reports no reason.
        let mut fields = Fields::held(1, 3);
        fields.steps.insert(
            1,
            StepState {
                kind: StepKind::Over,
                target_depth: 3,
                started: true,
            },
        );
        fields.run().retire_observations();
        assert_eq!(fields.mode, RunMode::Paused);
        assert_eq!(fields.pending, Some(StopReason::Pause));
        assert_eq!(fields.depth, 0);
        assert!(fields.depths.is_empty());
        assert!(fields.steps.is_empty());
        assert_eq!(fields.current, None);
        assert_eq!(fields.target, None);
        assert_eq!(
            fields.run().hold_reason(None),
            Some(StopReason::Pause),
            "the held scan still reports its hold"
        );

        let mut running = Fields::running();
        running.run().retire_observations();
        assert_eq!(running.mode, RunMode::Running);
        assert_eq!(running.pending, None);
    }

    #[test]
    fn a_stop_reason_has_the_wire_label_of_its_cause() {
        // Specification 13, "Stopped Events": the reason is `breakpoint` for an active breakpoint,
        // `step` for a stepping command, `pause` for an explicit pause and `entry` for a stop on
        // entry. Wrong behaviour caught: a swapped or renamed label, which tells a client that a
        // pause was a breakpoint.
        assert_eq!(StopReason::Breakpoint.label(), "breakpoint");
        assert_eq!(StopReason::Step.label(), "step");
        assert_eq!(StopReason::Pause.label(), "pause");
        assert_eq!(StopReason::Entry.label(), "entry");
    }

    #[test]
    fn a_step_is_accepted_only_while_the_scan_is_held() {
        // Specification 11 section 6.9 (line 4640): step_in, step_over and step_out act only while the
        // debugger holds the scan. Wrong behaviour caught: a step accepted while the scan runs, which
        // would arm a step that no hold started.
        let mut fields = Fields::running();
        assert!(!fields.run().accepts_step());
        fields.mode = RunMode::Paused;
        assert!(fields.run().accepts_step());
    }
}
