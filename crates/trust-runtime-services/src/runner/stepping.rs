//! Stepped scans in the resource loop (`cycle.step`, specification 11 §6.8.1).
//!
//! Between scans the loop lends the dispatcher its program ([`LentProgram`]): what [`ProgramView`]
//! reads of it, the program itself for a step, which runs inside the request that asks for it, so
//! its reply and its audit record carry its result, and the program's state for a debug read. The
//! program is in a `RefCell` only for the length of one request; the loop runs no scan meanwhile.
//!
//! A cold restart of a step loads a program installed since the program was loaded, as `restart`
//! loads it (specification 34 STATE-08), and only the loop can load a program in place. Such a step
//! is deferred ([`StepState`]): the request is audited as accepted, its reply is held, and the loop
//! loads the program and runs the step after the request's pass, then writes the step's reply
//! ([`Runner::finish_loading_step`]), as it finishes an install.

use alloc::string::ToString;
use alloc::vec::Vec;
use core::cell::{Cell, OnceCell, RefCell};

use smol_str::SmolStr;
use trust_runtime_core::memory::Generation;
use trust_runtime_core::ports::{ControlLink, MonotonicClock, RunnerWait};
use trust_runtime_core::retain::RestartMode;
use trust_runtime_core::value::Duration;

use super::cycle::{DebugView, ResourceCycle};
use super::host::{ProgramView, RunnerPlatform};
use super::program::Program;
use super::{span, Runner};
use crate::cycle_step::{reply, restart_time, StepOutcome, StepRefusal, StepRequest};
use crate::protocol::ControlResponse;
use crate::status::IoDriverStatus;

/// The step the dispatcher deferred to the loop, and the step whose reply the shell holds.
#[derive(Default)]
pub(super) struct StepState {
    deferred: Cell<Option<StepRequest>>,
    held: Cell<Option<(u64, StepRequest)>>,
}

impl StepState {
    /// Defers `request` to the loop: its cold restart loads a newer installed program.
    pub(super) fn defer(&self, request: StepRequest) {
        self.deferred.set(Some(request));
    }

    /// The step the dispatcher deferred while it answered the request just served.
    pub(super) fn take_deferred(&self) -> Option<StepRequest> {
        self.deferred.take()
    }

    /// Holds the reply to request `id`, the deferred step `request`, for the loop.
    pub(super) fn hold(&self, id: u64, request: StepRequest) {
        self.held.set(Some((id, request)));
    }

    /// The step whose reply is held, with its request's number.
    pub(super) fn take_held(&self) -> Option<(u64, StepRequest)> {
        self.held.take()
    }
}

/// The loop's program, lent for one request.
pub(super) struct LentProgram<'p, Y, C> {
    /// The program, until a debug read borrows its state.
    program: RefCell<Option<&'p mut Program<Y>>>,
    /// The program a debug read borrows for the rest of the request, whose reply is built from its
    /// state; a step in the same request is then refused.
    shared: OnceCell<&'p Program<Y>>,
    /// Why no program is installed, read before the program is lent.
    missing: Option<SmolStr>,
    /// The generation and the time of the last step, which the loop keeps.
    last_step: &'p Cell<Option<(Generation, Duration)>>,
    clock: &'p C,
}

impl<'p, Y, C> LentProgram<'p, Y, C> {
    /// Lends `program` for one request, with the loop's record of its last step and its clock.
    pub(super) fn new(
        program: &'p mut Program<Y>,
        last_step: &'p Cell<Option<(Generation, Duration)>>,
        clock: &'p C,
    ) -> Self {
        Self {
            missing: program.missing_reason(),
            program: RefCell::new(Some(program)),
            shared: OnceCell::new(),
            last_step,
            clock,
        }
    }
}

impl<'p, Y: ResourceCycle, C: MonotonicClock> LentProgram<'p, Y, C> {
    /// The program, lent shared to a debug read for the rest of the request.
    fn lend_shared(&self) -> Option<&'p Program<Y>> {
        if let Some(program) = self.shared.get() {
            return Some(*program);
        }
        let program = self.program.try_borrow_mut().ok()?.take()?;
        let _ = self.shared.set(program);
        self.shared.get().copied()
    }

    /// `read` of the program, whether a debug read borrows it or not; none while a step runs it.
    fn read<R>(&self, read: impl FnOnce(&Program<Y>) -> R) -> Option<R> {
        if let Some(program) = self.shared.get() {
            return Some(read(program));
        }
        let lent = self.program.try_borrow().ok()?;
        lent.as_deref().map(read)
    }

    /// Runs the step `request` with its restart, when it has one, at `restart_at`. The watchdog
    /// bounds its scan from now, as it bounds every scan from its start. A step whose scan ran is
    /// recorded as the last step: one with an outcome, or one whose watched value could not be read.
    fn run(&self, request: &StepRequest, restart_at: Duration) -> Result<StepOutcome, StepRefusal> {
        if let Some(reason) = &self.missing {
            return Err(StepRefusal::NoProgram(reason.to_string()));
        }
        let Ok(mut lent) = self.program.try_borrow_mut() else {
            return Err(StepRefusal::Unsupported);
        };
        let Some(program) = lent.as_deref_mut() else {
            return Err(StepRefusal::Unsupported);
        };
        let watchdog = program.watchdog().normalized_for_runtime();
        let deadline = watchdog
            .enabled
            .then(|| self.clock.now().saturating_add(span(watchdog.timeout)));
        let outcome = program.step(request, restart_at, deadline);
        if matches!(outcome, Ok(_) | Err(StepRefusal::Read { .. })) {
            self.last_step
                .set(Some((program.generation(), request.time)));
        }
        outcome
    }
}

impl<Y: ResourceCycle, C: MonotonicClock> ProgramView for LentProgram<'_, Y, C> {
    fn missing_program(&self) -> Option<&str> {
        self.missing.as_deref()
    }

    fn driver_health(&self) -> Vec<IoDriverStatus> {
        self.read(ResourceCycle::io_drivers).unwrap_or_default()
    }

    /// The step's restart happens at the time of the last step since the resource stopped, zero for
    /// the first step of the program.
    fn step(&self, request: &StepRequest) -> Result<StepOutcome, StepRefusal> {
        if let Some(reason) = &self.missing {
            return Err(StepRefusal::NoProgram(reason.to_string()));
        }
        let generation = self
            .read(ResourceCycle::generation)
            .ok_or(StepRefusal::Unsupported)?;
        let last = self
            .last_step
            .get()
            .filter(|(stepped, _)| *stepped == generation)
            .map(|(_, time)| time);
        self.run(request, restart_time(request, last)?)
    }

    /// The program's state for the debug read the request makes: the program is borrowed for the
    /// rest of the request.
    fn debug_state(&self) -> Option<DebugView<'_>> {
        self.lend_shared()?.debug_view()
    }

    #[cfg(feature = "native")]
    fn compiled_state(&self) -> Option<super::cycle::CompiledView<'_>> {
        self.lend_shared()?.compiled_view()
    }
}

impl<L, W, Y, P, C> Runner<L, W, Y, P, C>
where
    L: ControlLink,
    W: RunnerWait,
    Y: ResourceCycle,
    P: RunnerPlatform,
    C: MonotonicClock + Clone,
{
    /// Finishes the deferred step `request` of request `id`: the newest installed program is loaded
    /// in place and started cold, as `restart` loads it, and the resource stays Stopped; the step's
    /// scan then runs on it, its time counted from the load, and the step's reply is written. A
    /// program that cannot be loaded leaves the loop without one, with the reason.
    pub(super) fn finish_loading_step(&mut self, id: u64, request: StepRequest) {
        let response = match self.reload(RestartMode::Cold) {
            Ok(events) => {
                self.record_retain_events(events);
                self.record_program_memory();
                let loaded = StepRequest {
                    restart: None,
                    ..request
                };
                let program = LentProgram::new(&mut self.cycle, &self.last_step, &self.clock);
                let response = match program.run(&loaded, Duration::ZERO) {
                    Ok(outcome) => ControlResponse::ok(id, reply(loaded.time, &outcome)),
                    Err(refusal) => refusal.response(id),
                };
                self.shell.deliver(response, &program);
                return;
            }
            Err(error) => StepRefusal::Restart(error).response(id),
        };
        let program = LentProgram::new(&mut self.cycle, &self.last_step, &self.clock);
        self.shell.deliver(response, &program);
    }
}
