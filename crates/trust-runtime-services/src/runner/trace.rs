//! A chip's cycle traces (specification 48 RU-59, "Every runtime"; specification 11 §6.8.1,
//! "Cycle traces"): the recording on demand that the scans fill and the slot that `trace.read`
//! reads, as a computer keeps them. The loop's shell holds them, and each hook of a scan and each
//! request borrows them for its own call only (see `shell.rs`).
//!
//! The scan feeds the recording by the computer's rules: the cycle's start, its start state before
//! the boundary's writes and the forces after them, each task's activation, the input image once the
//! scan has sampled it, the output image it wrote, the wall clock's readings, and the cycle's end or
//! fault. A scan that the debugger held is no cycle a replay could run, so it ends the recording
//! `paused` with the cycles before.
//!
//! A chip runs its scan path from RAM (`firmware/trust-esp32c6/ld/rwtext_hook.x`), and a recording's
//! work stays out of it: the scan asks [`SharedTraces::records`] inline, and only while a recording
//! runs does it call the work here, which is never inlined and runs from flash. A scan without a
//! recording runs as before.

use alloc::string::String;
use core::cell::{Cell, RefCell};

use trust_runtime_core::cycle::CyclePhase;
use trust_runtime_core::io_address::IoAddress;
use trust_runtime_core::native::variables::CompiledVariables;
use trust_runtime_core::ports::{ControlLink, MonotonicClock, RunnerWait};
use trust_runtime_core::trace::{readings, ChangeSource, CycleFault, TraceEnd};
use trust_runtime_core::value::{Duration, Value};

use crate::live_values::apply::ForceEffect;
use crate::live_values::queues::{ForcedVar, PendingVarTarget};
use crate::trace::requests::{RecordRequest, TraceName, TraceRefusal, TraceStarted, TraceState};
use crate::trace::session::{cycle_fault, Device, Program, TraceSession, TraceSlot};
use crate::trace::{Begin, Budget, Recorder};

use super::{ResourceCycle, Runner, RunnerPlatform};

/// The chip's traces as the loop's shell keeps them, borrowed by one call at a time: a call that
/// finds them in use raises the shell's fault and does nothing (see `shell.rs`).
#[derive(Clone, Copy)]
pub(super) struct SharedTraces<'s> {
    pub(super) traces: &'s RefCell<ChipTraces>,
    pub(super) fault: &'s Cell<bool>,
    pub(super) holds: &'s Cell<u64>,
}

impl SharedTraces<'_> {
    /// Runs `operation` on the traces, or raises the fault and runs nothing.
    pub(super) fn with<R>(&self, operation: impl FnOnce(&mut ChipTraces) -> R) -> Option<R> {
        if let Ok(mut traces) = self.traces.try_borrow_mut() {
            Some(operation(&mut traces))
        } else {
            self.fault.set(true);
            None
        }
    }

    /// The scans the debugger has held since the loop began.
    pub(super) fn holds(&self) -> u64 {
        self.holds.get()
    }

    /// Whether a recording runs: the one question the scan path asks inline (module docs). Traces
    /// in use answer no; the call that holds them raises the fault.
    #[inline]
    pub(super) fn records(&self) -> bool {
        self.traces
            .try_borrow()
            .is_ok_and(|traces| traces.session.is_some())
    }

    /// The recording's part of the scan's phase `phase` ([`ChipTraces::phase`]).
    #[inline(never)]
    pub(super) fn phase(&self, phase: CyclePhase, now: Duration, cycle: u64, outputs: &[u8]) {
        let holds = self.holds();
        self.with(|traces| traces.phase(phase, now, cycle, outputs, holds));
    }

    /// The recording's start memory ([`ChipTraces::start_memory`]).
    #[inline(never)]
    pub(super) fn start_memory(&self, variables: &dyn CompiledVariables) {
        self.with(|traces| traces.start_memory(variables));
    }

    /// The forces at the recording's start ([`ChipTraces::start_forces`]).
    #[inline(never)]
    pub(super) fn start_forces(
        &self,
        vars: &[ForcedVar],
        io: &[(IoAddress, Value)],
        variables: &dyn CompiledVariables,
    ) {
        self.with(|traces| traces.start_forces(vars, io, variables));
    }

    /// The input image the scan sampled ([`ChipTraces::inputs`]).
    #[inline(never)]
    pub(super) fn inputs(&self, image: &[u8]) {
        self.with(|traces| traces.inputs(image));
    }

    /// A write the control link made ([`ChipTraces::written`]).
    #[inline(never)]
    pub(super) fn written(
        &self,
        source: ChangeSource,
        target: &PendingVarTarget,
        variables: &dyn CompiledVariables,
    ) {
        self.with(|traces| traces.written(source, target, variables));
    }

    /// A one-shot write at an address ([`ChipTraces::address_written`]).
    #[inline(never)]
    pub(super) fn address_written(
        &self,
        source: ChangeSource,
        address: &IoAddress,
        value: &Value,
        variables: &dyn CompiledVariables,
    ) {
        self.with(|traces| traces.address_written(source, address, value, variables));
    }

    /// A force set or released ([`ChipTraces::forced`]).
    #[inline(never)]
    pub(super) fn forced(&self, effect: ForceEffect<'_>, variables: &dyn CompiledVariables) {
        self.with(|traces| traces.forced(effect, variables));
    }

    /// Force changes the queues lost ([`ChipTraces::force_changes_lost`]).
    #[inline(never)]
    pub(super) fn force_changes_lost(&self) {
        self.with(ChipTraces::force_changes_lost);
    }
}

/// A chip's traces: the slot `trace.read` reads, and the recording on demand that runs.
#[derive(Default)]
pub(super) struct ChipTraces {
    slot: TraceSlot,
    session: Option<TraceSession>,
    /// The scan that runs began while the debugger had held this many scans.
    holds_at_start: u64,
}

impl ChipTraces {
    /// Starts a recording at the next cycle (`trace.record`) of the program `program` on `device`,
    /// its memory reserved now within the budget of the memory the chip has, `free`.
    ///
    /// # Errors
    ///
    /// [`TraceRefusal::Busy`] while a recording runs, and the session's refusals: an unknown task
    /// and memory the chip cannot give.
    #[inline(never)]
    pub(super) fn record(
        &mut self,
        request: &RecordRequest,
        program: &Program<'_>,
        device: &Device<'_>,
        free: Option<usize>,
        now_utc_ms: i64,
    ) -> Result<TraceStarted, TraceRefusal> {
        if let Some(session) = &self.session {
            return Err(TraceRefusal::Busy {
                running: session.id(),
            });
        }
        let (session, started) = TraceSession::start(
            request,
            program,
            device,
            Budget::for_free(free),
            free,
            now_utc_ms,
            &mut self.slot,
        )?;
        self.session = Some(session);
        Ok(started)
    }

    /// The state of the trace `name` (`trace.read`).
    pub(super) fn state(&self, name: TraceName) -> TraceState {
        self.slot.state(name)
    }

    /// The bytes of the ended trace `name`'s file from `offset` (`trace.read`).
    pub(super) fn part(&self, name: TraceName, offset: usize, out: &mut [u8]) -> Option<usize> {
        self.slot.part(name, offset, out)
    }

    /// Ends the recording `id` at once (`trace.stop`), and the trace with it once `name` names its
    /// variables; gives the trace's state.
    #[inline(never)]
    pub(super) fn stop(
        &mut self,
        id: u64,
        name: &mut dyn FnMut(&str) -> Option<String>,
    ) -> TraceState {
        if self
            .session
            .as_ref()
            .is_some_and(|session| session.id() == id)
        {
            self.end(TraceEnd::Stopped, name);
        }
        self.slot.state(TraceName::Id(id))
    }

    /// Ends the running recording, if one runs, because of `end`: the program stopped, restarted
    /// or changed, or a recording was stopped.
    #[inline(never)]
    pub(super) fn end(&mut self, end: TraceEnd, name: &mut dyn FnMut(&str) -> Option<String>) {
        if let Some(session) = &mut self.session {
            session.recorder().finish(end);
            // A scan that did not finish left the readings armed.
            readings::disarm();
        }
        self.publish(name);
    }

    /// After a scan: the recording's progress for the control link, or once it has ended its
    /// trace, each variable named by `name` as the debugger names it.
    #[inline(never)]
    pub(super) fn publish(&mut self, name: &mut dyn FnMut(&str) -> Option<String>) {
        if let Some(session) = &mut self.session
            && session.publish(&mut self.slot, name)
        {
            self.session = None;
        }
    }

    /// The recording a change from outside goes to, while one runs.
    pub(super) fn recorder(&mut self) -> Option<&mut Recorder> {
        self.session.as_mut().map(TraceSession::recorder)
    }

    /// A write the control link made to `target`, as Live Values applied it: recorded by the path
    /// the debugger names the variable by.
    pub(super) fn written(
        &mut self,
        source: ChangeSource,
        target: &PendingVarTarget,
        variables: &dyn CompiledVariables,
    ) {
        if let Some(recorder) = self.recorder() {
            recorder.written_target(source, target, variables);
        }
    }

    /// A one-shot write of `value` at `address`, recorded as the bytes it gives the process image.
    pub(super) fn address_written(
        &mut self,
        source: ChangeSource,
        address: &IoAddress,
        value: &Value,
        variables: &dyn CompiledVariables,
    ) {
        if let Some(recorder) = self.recorder() {
            recorder.address_written(source, address, value, variables);
        }
    }

    /// A force set or released by the control link, recorded once, where it changes.
    pub(super) fn forced(&mut self, effect: ForceEffect<'_>, variables: &dyn CompiledVariables) {
        if let Some(recorder) = self.recorder() {
            recorder.forced(ChangeSource::ControlLink, effect, variables);
        }
    }

    /// Force changes that the queues could not keep: the recording ends `memory_full`.
    pub(super) fn force_changes_lost(&mut self) {
        if let Some(recorder) = self.recorder() {
            recorder.force_changes_lost();
        }
    }

    /// The recording's part of a scan's phase (specification 48 RU-59): the cycle's start, each
    /// task's activation, the output image it wrote, and the cycle's end with the wall clock's
    /// readings, or `paused` when the debugger has held the scan since it began (`holds` holds
    /// counted so far).
    pub(super) fn phase(
        &mut self,
        phase: CyclePhase,
        now: Duration,
        cycle: u64,
        outputs: &[u8],
        holds: u64,
    ) {
        let Some(session) = &mut self.session else {
            return;
        };
        match phase {
            CyclePhase::CycleStart => {
                self.holds_at_start = holds;
                if session.recorder().begin_cycle(now, cycle) == Begin::First {
                    session.start_pending = true;
                }
                readings::arm();
            }
            CyclePhase::TaskStart(index) => {
                let task = u32::try_from(index).unwrap_or(u32::MAX);
                session.recorder().activation(task, now.as_nanos());
            }
            CyclePhase::AfterOutputSample => session.recorder().outputs(outputs),
            CyclePhase::CycleEnd => {
                take_readings(session.recorder());
                if holds > self.holds_at_start {
                    session.recorder().finish(TraceEnd::Paused);
                } else {
                    session.recorder().end_cycle(None);
                }
            }
            _ => {}
        }
    }

    /// The recording's start state, on its first cycle, before the boundary's writes: each area
    /// as the program holds it.
    pub(super) fn start_memory(&mut self, variables: &dyn CompiledVariables) {
        let Some(session) = &mut self.session else {
            return;
        };
        if !session.start_pending {
            return;
        }
        let recorder = session.recorder();
        for id in 1..=6 {
            if let Some(area) = recorder.start_area(id) {
                variables.read_area_bytes(id, 0, area);
            }
        }
    }

    /// The forces in effect at the recording's first cycle, the variables' `vars` and the
    /// addresses' `io`, once the boundary applied them: the start state is then whole.
    pub(super) fn start_forces(
        &mut self,
        vars: &[ForcedVar],
        io: &[(IoAddress, Value)],
        variables: &dyn CompiledVariables,
    ) {
        let Some(session) = &mut self.session else {
            return;
        };
        if !session.start_pending {
            return;
        }
        session
            .recorder()
            .start_forces(ChangeSource::ControlLink, vars, io, variables);
        session.start_pending = false;
    }

    /// The input image as the scan sampled it, once everything that writes it before the program
    /// has.
    pub(super) fn inputs(&mut self, image: &[u8]) {
        if let Some(recorder) = self.recorder() {
            recorder.inputs(image);
        }
    }

    /// A scan that failed: the recording's last cycle, with `fault`.
    pub(super) fn faulted(&mut self, fault: CycleFault) {
        if let Some(recorder) = self.recorder() {
            take_readings(recorder);
            recorder.end_cycle(Some(fault));
        }
    }
}

/// Hands the wall clock's readings of the scan to `recorder` and stops keeping them: readings the
/// room could not keep end the recording `memory_full`.
fn take_readings(recorder: &mut Recorder) {
    let mut handed = 0;
    let read = readings::take(&mut |reading| {
        recorder.wall_clock(reading);
        handed += 1;
    });
    if read > handed {
        recorder.readings_lost();
    }
    readings::disarm();
}

impl<L, W, Y, P, C> Runner<L, W, Y, P, C>
where
    L: ControlLink,
    W: RunnerWait,
    Y: ResourceCycle,
    P: RunnerPlatform,
    C: MonotonicClock + Clone,
{
    /// Ends the running recording, if one runs, because of `end`, and publishes its trace, named
    /// as the program the recording ran names its variables (specification 48 RU-59). Out of the
    /// scan path, as the module docs say.
    #[inline(never)]
    pub(super) fn end_trace(&self, end: TraceEnd) {
        let cycle = &self.cycle;
        self.shell
            .traces()
            .with(|traces| traces.end(end, &mut |path| cycle.display_path(path)));
    }

    /// A recording's part of a scan's end, while one runs: a failed scan is its last cycle, with
    /// the kernel's record of the fault, and the recording's progress, or its trace once it has
    /// ended, is published, each variable named as the debugger names it (specification 48 RU-59).
    #[inline(never)]
    pub(super) fn trace_scan_end(&self, failed: bool) {
        if failed
            && let (Some((code, statement)), Some(memory)) =
                (self.cycle.last_fault_site(), self.cycle.variables_memory())
        {
            let fault = cycle_fault(&memory.image(), code, statement);
            self.shell.traces().with(|traces| traces.faulted(fault));
        }
        let cycle = &self.cycle;
        self.shell
            .traces()
            .with(|traces| traces.publish(&mut |path| cycle.display_path(path)));
    }
}
