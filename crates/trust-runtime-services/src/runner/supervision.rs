//! The supervisor's side of the resource loop: the program's boundaries, its restarts, the scan,
//! its faults and the stop, at the points and by the shared decisions that trust-runtime's
//! scheduler follows (`scheduler/runner_loop.rs`, `scheduler/faulted.rs` and the fault handling of
//! `runtime/installed_cycle.rs`).

use alloc::string::ToString;
use alloc::vec::Vec;

use smol_str::SmolStr;
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::ports::{
    ControlLink, MonotonicClock, MonotonicInstant, RunnerWait, TimeSpan,
};
use trust_runtime_core::retain::RestartMode;
use trust_runtime_core::scheduler::ResourceState;
use trust_runtime_core::value;
use trust_runtime_core::vm::RetainApplyEvent;
use trust_runtime_core::watchdog::FaultDecision;

use super::engine::LiveValuesHooks;
use super::observer::ScanObserver;
use super::{span, ResourceCycle, RetainSource, Runner, RunnerPlatform};
use crate::events::{retain_apply_event, RuntimeEvent};
use crate::loading::reload::ReloadOutcome;
use crate::metrics::RuntimeMetrics;
use crate::resource::commands::RunningCommandEffect;
use crate::resource::supervisor::{
    automatic_restart_backoff, cycle_error_action, stop_outcome, watchdog_overrun_action,
    CycleErrorAction, OverrunAction, FAULTED_WAIT_POLL,
};

impl<L, W, Y, P, C> Runner<L, W, Y, P, C>
where
    L: ControlLink,
    W: RunnerWait,
    Y: ResourceCycle,
    P: RunnerPlatform,
    C: MonotonicClock + Clone,
{
    /// Takes the program's boundary when it is due: a poll of the faulted wait while faulted,
    /// otherwise the boundary of a running or paused program. A program that has ended has none.
    pub(super) fn supervise(&mut self) {
        let Some(boundary) = self.next_boundary else {
            return;
        };
        let now = self.clock.now();
        if now < boundary {
            return;
        }
        if self.shell.resource.state() == ResourceState::Faulted {
            self.faulted_poll(now);
        } else {
            self.take_boundary();
        }
        // A boundary that is due again as soon as it is taken, as after a scan that overran its
        // interval, lets the link answer one request before the next boundary (see `serve_link`).
        self.request_grant = self.boundary_due();
    }

    /// Faults the resource when the shell reports that a cell it shares was in use, which the call
    /// structure rules out (see `shell.rs`). The operation that found it did nothing, and the
    /// resource must not go on as if it had. A hold that finds it fails its scan the same way.
    pub(super) fn fault_on_shell_defect(&mut self) {
        if self.shell.take_fault() {
            self.halt(RuntimeError::InvalidExecutionState);
        }
    }

    /// The boundary of a running or paused program, as trust-runtime's scan thread begins each
    /// pass: a requested stop first, then the queued pause and resume in order, then a queued
    /// online change (specification 34 §11.3), then a pending restart, which keeps a pause and acts on
    /// the program the change gave; then the scan, or while paused the wait for the next cycle
    /// interval.
    fn take_boundary(&mut self) {
        if self.shell.stop_requested() {
            self.stop();
            return;
        }
        match self.shell.resource.take_commands() {
            Some(RunningCommandEffect::Pause) => {
                self.shell.resource.set_state(ResourceState::Paused);
            }
            Some(RunningCommandEffect::Resume) => {
                self.shell.resource.set_state(ResourceState::Running);
            }
            Some(RunningCommandEffect::Apply) | None => {}
        }
        self.apply_online_change();
        if let Some(mode) = self.shell.resource.take_pending_restart() {
            if let Err(error) = self.restart_in_place(mode, RetainSource::Checkpoint) {
                self.halt(error);
                return;
            }
            self.restarts.reset();
        }
        let now = self.clock.now();
        if self.shell.resource.state() == ResourceState::Paused {
            self.next_boundary = Some(now.saturating_add(self.interval()));
            return;
        }
        self.scan(now);
    }

    /// One poll of the faulted wait, as trust-runtime's faulted wait ends each poll period: the
    /// queued commands are dropped, an online change that succeeds recovers the program (specification
    /// 11 §6.6), a pending restart recovers the program, which then takes its boundary at once, and a
    /// requested stop ends the program and keeps the fault, without a second safe-state write or a
    /// retain save.
    fn faulted_poll(&mut self, now: MonotonicInstant) {
        let _ = self.shell.resource.take_commands();
        if self.apply_online_change() {
            // The program the change gave the resource has no fault; the old program's is gone.
            self.restarts.reset();
            self.shell.resource.set_last_error(None);
            self.shell.resource.set_state(ResourceState::Running);
            self.take_boundary();
            return;
        }
        if let Some(mode) = self.shell.resource.take_pending_restart() {
            match self.restart_in_place(mode, RetainSource::Checkpoint) {
                Ok(()) => {
                    self.restarts.reset();
                    self.shell.resource.set_state(ResourceState::Running);
                    self.take_boundary();
                    return;
                }
                // The resource stays faulted, and the new error replaces the root fault.
                Err(error) => self.shell.resource.set_last_error(Some(error)),
            }
        }
        if self.shell.stop_requested() {
            self.end();
            return;
        }
        self.next_boundary = Some(now.saturating_add(TimeSpan::new(FAULTED_WAIT_POLL)));
    }

    /// Restarts the program in place, cold or warm from `source`, as trust-runtime's scan thread
    /// does: the retained values the restart dropped or migrated are recorded at the program's
    /// current time, the task timing starts again at the program's time after the restart, the
    /// scan count starts again and the fault is cleared. The state stays as it is.
    fn restart_in_place(
        &mut self,
        mode: RestartMode,
        source: RetainSource,
    ) -> Result<(), RuntimeError> {
        let events = self.cycle.restart(mode, source)?;
        // A committed restart clears the previous lifecycle's queued writes and forces, as
        // trust-runtime's `begin_lifecycle` does (specification 11 §6.9.2); a failed one keeps
        // them, as a failed host restart keeps them.
        self.live.clear();
        self.follow_generation();
        self.record_retain_events(events);
        let now = self.logical(self.clock.now());
        self.cycle.reset_task_timing(now);
        self.program_time = now;
        self.cycle_count = 0;
        self.shell.resource.set_last_error(None);
        Ok(())
    }

    /// Gives the Live Values the installation generation of the program's state (specification 34
    /// STATE-08 and UPDATE-04). A restart that commits installs the next generation, and a request that
    /// names an instance by a bare identity belongs to the generation the Live Values carry when it is
    /// accepted, so they follow the program before the loop answers another request: a force that
    /// arrives after the restart is applied at the next boundary and not refused as stale.
    fn follow_generation(&self) {
        self.live.set_generation(self.cycle.generation());
    }

    /// A restart of a program that has ended starts it as a new trust-runtime process would,
    /// cold or warm from the retain checkpoint: its logical time and uptime start again from
    /// zero, the events, metrics and task statistics start empty, and its first scan is due at
    /// once. When the program store holds a committed container newer than the one the program came
    /// from, the loop loads that container in place first ([`Self::load_or_restart`]). A program that
    /// cannot start stays ended and faulted with the reason, and the next restart tries again.
    pub(super) fn start(&mut self, mode: RestartMode) {
        match self.load_or_restart(mode) {
            Ok(events) => {
                self.live.clear();
                // A restart in place installed the next generation. A program loaded in place has
                // Live Values of its own, in the first generation, over a state that its warm start
                // may have restarted.
                self.follow_generation();
                let start = self.clock.now();
                self.program_start = start;
                self.shell.records.restart(self.clock.clone(), start);
                // A program that starts again is a new resource thread: the debugger may hold it,
                // though a stop ended the holds of the last one (specification 11 §6.9).
                self.shell.debug.rearm();
                self.cycle.reset_task_timing(value::Duration::ZERO);
                self.program_time = value::Duration::ZERO;
                self.record_retain_events(events);
                self.cycle_count = 0;
                self.restarts.reset();
                self.shell.resource.begin();
                self.next_boundary = Some(start);
            }
            Err(error) => {
                self.shell.resource.set_last_error(Some(error));
                self.shell.resource.set_state(ResourceState::Faulted);
            }
        }
    }

    /// Runs one scan that started at `started` on the monotonic clock, and follows the shared
    /// decisions for its error or its overrun.
    fn scan(&mut self, started: MonotonicInstant) {
        let watchdog = self.cycle.watchdog().normalized_for_runtime();
        let fault_policy = self.cycle.fault_policy();
        let now = self.logical(started);
        self.program_time = now;
        // An enabled watchdog bounds the scan from its start; a paused resource runs no scan, so
        // a pause never counts against it (spec 34 STATE-07).
        let deadline = watchdog
            .enabled
            .then(|| started.saturating_add(span(watchdog.timeout)));
        // The holds that run inside the scan are no active time (spec 11 §6.6), so the loop reads
        // them before and after the scan.
        let held_before = self.cycle.held_time();
        // The scan's allocator operations are read around the cycle's work; the engine's own are
        // the engine's, the rest the services' (specification 34, "The allocation count").
        let operations_before = self.cycle.allocator_operations();
        let result = {
            let mut observer = ScanObserver::new(
                &self.tasks,
                &self.shell.records,
                &self.clock,
                self.cycle_count,
            );
            let mut hooks = LiveValuesHooks::new(
                &self.live,
                &self.shell.records,
                self.cycle_count,
                &mut observer,
            );
            // What outlives this call is the cycle's `&mut` and plain `&` borrows of the loop's own
            // fields and of the shell's records. No `RefCell` guard is alive: the shell's cells are
            // borrowed only inside its own operations, so the hold that the cycle may enter here
            // finds them free (see `shell.rs`).
            self.cycle.run_scan(now, deadline, &mut hooks)
        };
        if let (Some(before), Some(after), Some(engine)) = (
            operations_before,
            self.cycle.allocator_operations(),
            self.cycle.last_scan_allocations(),
        ) {
            let services = after.saturating_sub(before).saturating_sub(engine);
            self.shell
                .records
                .record(|metrics| metrics.record_scan_allocations(engine, services));
        }
        if let Err(error) = result {
            let reported = self.apply_fault(error, FaultDecision::from_fault_policy(fault_policy));
            match cycle_error_action(watchdog, fault_policy, reported) {
                CycleErrorAction::Restart(reason) => self.automatic_restart(&reason),
                CycleErrorAction::WatchdogHalt(error) => {
                    let error =
                        self.apply_fault(error, FaultDecision::from_watchdog(watchdog.action));
                    self.halt(error);
                }
                CycleErrorAction::Halt(error) => self.halt(error),
            }
            return;
        }
        let held = self
            .cycle
            .held_time()
            .as_duration()
            .saturating_sub(held_before.as_duration());
        self.shell.records.push_event(RuntimeEvent::CycleEnd {
            cycle: self.cycle_count,
            time: now,
        });
        // The cycle metric and the overrun check see the active time: the wall time less the holds.
        let wall = self.clock.now().saturating_since(started).as_duration();
        let active = wall.saturating_sub(held);
        self.shell
            .records
            .record(|metrics| metrics.record_cycle(active));
        self.cycle_count = self.cycle_count.saturating_add(1);
        match watchdog_overrun_action(watchdog, active) {
            None => {
                self.restarts.reset();
                self.next_boundary = Some(started.saturating_add(self.interval()));
            }
            Some(OverrunAction::Restart) => {
                self.automatic_restart(&RuntimeError::WatchdogTimeout);
            }
            Some(OverrunAction::Halt) => {
                let error = self.apply_fault(
                    RuntimeError::WatchdogTimeout,
                    FaultDecision::from_watchdog(watchdog.action),
                );
                self.halt(error);
            }
        }
    }

    /// Records a fault as trust-runtime's fault owner does: the engine latches it, the safe
    /// outputs follow when `decision` asks for them, the fault counts in the metrics and the
    /// events, and the fault to report comes back, the safe-output failure included.
    fn apply_fault(&mut self, error: RuntimeError, decision: FaultDecision) -> RuntimeError {
        // A fault clears the queued writes and active forces, as trust-runtime's `apply_fault`
        // does (specification 11 §6.9.2).
        self.live.clear();
        let time = self.program_time;
        let root = error.to_string();
        // The fixed record and the hosted message the fault log renders, built while the error
        // is still here (specification 34 MEM-05): recording the event allocates nothing.
        let event = RuntimeEvent::fault(&error, time);
        let mut reported = error.clone();
        self.cycle.latch_fault(error);
        if decision.apply_safe_state
            && let Err(safe_error) = self.cycle.apply_safe_outputs()
        {
            let safe_error = safe_error.to_string();
            reported = RuntimeError::SafeStateFailed {
                root: SmolStr::new(&root),
                error: SmolStr::new(&safe_error),
            };
            self.shell
                .records
                .push_event(RuntimeEvent::SafeStateFailed {
                    root: root.clone(),
                    error: safe_error,
                    time,
                });
        }
        self.shell.records.record(RuntimeMetrics::record_fault);
        self.shell.records.push_event(event);
        reported
    }

    /// An automatic warm restart in place for `reason`, within the shared restart budget and with
    /// its backoff; a spent budget or a failed restart leaves the resource faulted.
    fn automatic_restart(&mut self, reason: &RuntimeError) {
        let attempt = match self.restarts.next_attempt(reason) {
            Ok(attempt) => attempt,
            Err(limit) => {
                self.halt(limit);
                return;
            }
        };
        if let Err(error) = self.restart_in_place(RestartMode::Warm, RetainSource::Memory) {
            self.halt(error);
            return;
        }
        self.shell.resource.set_state(ResourceState::Running);
        let backoff = automatic_restart_backoff(self.cycle.cycle_interval(), attempt);
        self.next_boundary = Some(self.clock.now().saturating_add(span(backoff)));
    }

    /// The resource faults with `error` and runs no scan until a restart; the faulted wait polls
    /// for a restart or a stop from now on.
    fn halt(&mut self, error: RuntimeError) {
        self.shell.resource.set_last_error(Some(error));
        self.shell.resource.set_state(ResourceState::Faulted);
        self.next_boundary = Some(
            self.clock
                .now()
                .saturating_add(TimeSpan::new(FAULTED_WAIT_POLL)),
        );
    }

    /// A requested stop of a running or paused program, as trust-runtime stops it: the safe
    /// outputs and then the retain save, and the resource is Stopped, or Faulted with the first
    /// failure; the program has then ended.
    fn stop(&mut self) {
        // A stop's safe state clears the queued writes and active forces, as trust-runtime's
        // `apply_io_safe_state` does (specification 11 §6.9.2).
        self.live.clear();
        let safe_state = self.cycle.apply_safe_outputs();
        let retain_save = self.cycle.save_retain();
        let (next, error) = stop_outcome(safe_state, retain_save);
        if let Some(error) = error {
            self.shell.resource.set_last_error(Some(error));
        }
        self.shell.resource.set_state(next);
        self.end();
    }

    /// The program has ended, as a `shutdown` ends it on every runtime, a computer's trust-runtime
    /// included: the runtime stays reachable in the Stopped state (specification 34, STATE-08), the
    /// program has no boundary, and the queued commands and a pending restart are dropped.
    fn end(&mut self) {
        self.shell.set_stop_requested(false);
        self.next_boundary = None;
        // An online change that waited for a boundary is dropped with the program, never applied, as
        // it is when a computer's scan thread ends (specification 11 §6.7.1).
        self.shell.finish_change(ReloadOutcome::Stopped);
        self.shell.resource.end();
        // A program that has ended holds nothing: a pause that arrived between the stop request
        // and the stop is gone, and the debugger holds no scan until the program starts again.
        self.shell.debug.release_for_stop();
    }

    /// Records the retained values a restart dropped or migrated, at the program's current time.
    pub(super) fn record_retain_events(&self, events: Vec<RetainApplyEvent>) {
        for event in events {
            self.shell
                .records
                .push_event(retain_apply_event(event, self.program_time));
        }
    }
}
