//! A chip's cycle traces (specification 48 RU-59, "Every runtime"; specification 11 §6.8.1,
//! "Cycle traces"): the recording on demand and the flight recorder that the scans fill, and the
//! slot that `trace.read` reads, as a computer keeps them. The loop's shell holds them, and each
//! hook of a scan and each request borrows them for its own call only (see `shell.rs`).
//!
//! The scan feeds the recording by the computer's rules: the cycle's start, its start state before
//! the boundary's writes and the forces after them, each task's activation, the input image once the
//! scan has sampled it, the output image it wrote, the wall clock's readings, and the cycle's end or
//! fault. A scan that the debugger held is no cycle a replay could run, so it ends the recording
//! `paused` with the cycles before, and the flight recorder starts again after it.
//!
//! The flight recorder (`[runtime.trace] flight_recorder`, which a chip takes from its deployment
//! record) is armed when the program starts, restarts or changes, within the memory the chip has
//! for a recording (RU-59, "Memory"). A recording that takes over from a full one takes its start
//! state once the scan has returned, before any change between the scans; when the resource faults,
//! the fault's trace goes to the slot, where it stays until the next fault or a reset.
//!
//! A chip runs its scan path from RAM (`firmware/trust-esp32c6/ld/rwtext_hook.x`), and a recording's
//! work stays out of it: the scan asks [`SharedTraces::records`] inline, and only while a recording
//! or the flight recorder runs does it call the work here, which is never inlined and runs from
//! flash. A scan without either runs as before.

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use core::cell::{Cell, RefCell};

use smol_str::SmolStr;
use trust_runtime_core::cycle::CyclePhase;
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::io_address::IoAddress;
use trust_runtime_core::native::image::Image;
use trust_runtime_core::native::live::io_bindings;
use trust_runtime_core::native::memory::ProgramMemory;
use trust_runtime_core::native::variables::CompiledVariables;
use trust_runtime_core::ports::{ControlLink, MonotonicClock, RunnerWait};
use trust_runtime_core::trace::{
    readings, ChangeSource, CycleFault, ImageTables, Located, TraceEnd,
};
use trust_runtime_core::value::{Duration, Value};

use crate::events::RuntimeEvent;
use crate::live_values::apply::ForceEffect;
use crate::live_values::queues::{ForcedVar, PendingVarTarget};
use crate::method_call::MethodCall;
use crate::trace::requests::{RecordRequest, TraceName, TraceRefusal, TraceStarted, TraceState};
use crate::trace::session::{
    cycle_fault, Device, Program, SessionIdentity, TraceSession, TraceSlot,
};
use crate::trace::{
    Begin, Budget, FlightCycle, FlightRecorder, Recorder, Recordings, ReserveError,
};

use super::{ResourceCycle, Runner, RunnerPlatform};

/// The memory a runtime has for a recording beside the program that runs: the platform's `room`
/// less what an online change of a program of the same figure takes beside it, so that a kept
/// trace never takes the memory the change needs (specification 48 RU-59, "Memory"; specification
/// 44 §12.3). A runtime that does not know its memory, as a computer does not, has none to count.
pub(super) fn room_beside_change(
    room: Option<usize>,
    running: Option<crate::metrics::ProgramMemory>,
) -> Option<usize> {
    room.map(|room| room.saturating_sub(running.map_or(0, |memory| memory.change_bytes())))
}

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

    /// Whether a recording or the flight recorder runs: the one question the scan path asks inline
    /// (module docs). Traces in use answer no; the call that holds them raises the fault.
    #[inline]
    pub(super) fn records(&self) -> bool {
        self.traces
            .try_borrow()
            .is_ok_and(|traces| traces.session.is_some() || traces.flight.is_some())
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

    /// Before a tool's method call between two scans: while a recording runs, the wall clock's
    /// readings the call makes are kept for it, as a computer keeps them (specification 48 RU-59).
    /// It answers whether a recording runs, for [`Self::method_called`] after the call.
    #[inline(never)]
    pub(super) fn arm_call_readings(&self) -> bool {
        let records = self.records();
        if records {
            readings::arm();
        }
        records
    }

    /// After the call, answered or refused ([`ChipTraces::method_called`]).
    #[inline(never)]
    pub(super) fn method_called(&self, call: &MethodCall, answered: bool) {
        self.with(|traces| traces.method_called(call, answered));
    }
}

/// A chip's traces: the slot `trace.read` reads, the recording on demand that runs, and the flight
/// recorder of the program that runs.
#[derive(Default)]
pub(super) struct ChipTraces {
    slot: TraceSlot,
    /// The recording on demand that runs, boxed, as the flight recorder is, so that the traces of
    /// a chip that records nothing keep only these words in the runner it holds from its boot
    /// (specification 34 MEM-12).
    session: Option<Box<TraceSession>>,
    flight: Option<Box<ChipFlight>>,
    /// The flight recorder's cycles `n`, from the program's deployment record; 0 keeps none.
    flight_cycles: u64,
    /// The scan that runs began while the debugger had held this many scans.
    holds_at_start: u64,
}

/// The flight recorder of the program that runs (specification 48 RU-59, "The flight recorder"),
/// as a computer keeps one: its two recordings, what the fault's trace says of the program, and
/// what it still takes after a scan.
struct ChipFlight {
    recorder: FlightRecorder,
    identity: SessionIdentity,
    located: Vec<Located>,
    tables: ImageTables,
    /// The first recording's first cycle has begun: its start state is taken at the cycle's first
    /// access, as a recording on demand's is.
    start_pending: bool,
    /// A recording took over at the scan's end: its start state is taken once the scan has
    /// returned, before any change between the scans.
    handover_pending: bool,
    /// The debugger held the scan, which no replay could run: the flight recorder is armed again
    /// once the scan has returned.
    rearm_pending: bool,
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
        self.session = Some(Box::new(session));
        Ok(started)
    }

    /// Arms the flight recorder of the program `program` on `device`, its two recordings reserved
    /// now, each within the budget of half the memory the chip has, `free`, so that together they
    /// take what one recording would, and the first begun at `now_utc_ms` (specification 48 RU-59,
    /// "Memory"). The flight recorder of the program before goes first, and its memory with it; a
    /// fault's trace already kept stays until the next fault. No flight recorder is armed when the
    /// program's record keeps none.
    ///
    /// # Errors
    ///
    /// The bytes the flight recorder needs, when the chip cannot give them: the program then runs
    /// without one.
    #[inline(never)]
    pub(super) fn arm_flight(
        &mut self,
        program: &Program<'_>,
        device: &Device<'_>,
        free: Option<usize>,
        now_utc_ms: i64,
    ) -> Result<(), usize> {
        self.flight = None;
        if self.flight_cycles == 0 {
            return Ok(());
        }
        let tables_len = program.tables_len();
        let mut recorder = FlightRecorder::arm(
            self.flight_cycles,
            program.sizes(None),
            Budget::for_free(free.map(|free| free / 2)),
        )
        .map_err(|ReserveError::InsufficientMemory { needed }| needed.saturating_add(tables_len))?;
        let tables = program
            .tables()
            .ok_or_else(|| recorder.reserved_bytes().saturating_add(tables_len))?;
        recorder.began(now_utc_ms);
        self.flight = Some(Box::new(ChipFlight {
            recorder,
            identity: SessionIdentity::of(
                program.image,
                device,
                program.resource.clone(),
                None,
                now_utc_ms,
            ),
            located: program.located(),
            tables,
            start_pending: false,
            handover_pending: false,
            rearm_pending: false,
        }));
        Ok(())
    }

    /// Keeps the flight recorder's cycles `n` of the program's record; 0 keeps none.
    pub(super) fn set_flight_cycles(&mut self, cycles: u64) {
        self.flight_cycles = cycles;
    }

    /// The bytes the flight recorder's two recordings reserved, while one is armed.
    #[cfg(test)]
    pub(super) fn flight_reserved_bytes(&self) -> Option<usize> {
        self.flight
            .as_ref()
            .map(|flight| flight.recorder.reserved_bytes())
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
    /// or changed, or a recording was stopped. A fault ends the flight recorder's recording too,
    /// with the cycles before, as the fault's trace.
    #[inline(never)]
    pub(super) fn end(&mut self, end: TraceEnd, name: &mut dyn FnMut(&str) -> Option<String>) {
        if let Some(session) = &mut self.session {
            session.recorder().finish(end);
        }
        if end == TraceEnd::Faulted
            && let Some(flight) = &mut self.flight
        {
            flight.recorder.fault();
        }
        // A scan that did not finish left the readings armed.
        readings::disarm();
        self.publish(name);
    }

    /// After a scan: the recording's progress for the control link, or once it has ended its
    /// trace, and the fault's trace once the flight recorder has ended at a fault, each variable
    /// named by `name` as the debugger names it. The flight recorder ends with its fault's trace;
    /// the program's next start arms it again.
    #[inline(never)]
    pub(super) fn publish(&mut self, name: &mut dyn FnMut(&str) -> Option<String>) {
        if let Some(session) = &mut self.session
            && session.publish(&mut self.slot, name)
        {
            self.session = None;
        }
        let Some(flight) = &mut self.flight else {
            return;
        };
        if !flight.recorder.faulted() {
            return;
        }
        let started = flight
            .recorder
            .fault_began_utc_ms()
            .unwrap_or(flight.identity.started_utc_ms());
        let identity = flight.identity.identity(started);
        let tables = core::mem::take(&mut flight.tables);
        if let Some(served) = flight
            .recorder
            .fault_served(&identity, &flight.located, tables, name)
        {
            self.slot.keep_fault(served);
        }
        self.flight = None;
    }

    /// After a scan, before any change between the scans: a flight recording that took over at
    /// the scan's end takes its start state, the memory `variables` and the forces `vars` and `io`
    /// as the scan left them. It answers whether the debugger held the scan, so that the loop arms
    /// the flight recorder again.
    #[inline(never)]
    pub(super) fn flight_after_scan(
        &mut self,
        variables: &dyn CompiledVariables,
        vars: &[ForcedVar],
        io: &[(IoAddress, Value)],
    ) -> bool {
        let Some(flight) = &mut self.flight else {
            return false;
        };
        if flight.rearm_pending {
            return true;
        }
        if flight.handover_pending {
            flight.handover_pending = false;
            let recorder = flight.recorder.recorder();
            for id in 1..=6 {
                if let Some(area) = recorder.start_area(id) {
                    variables.read_area_bytes(id, 0, area);
                }
            }
            recorder.start_forces(ChangeSource::ControlLink, vars, io, variables);
        }
        false
    }

    /// The recordings a change made now feeds: the recording on demand and the flight recorder's
    /// current recording, each when there is one.
    fn recordings(&mut self) -> Recordings<'_> {
        Recordings {
            on_demand: self.session.as_deref_mut().map(TraceSession::recorder),
            flight: self
                .flight
                .as_mut()
                .map(|flight| flight.recorder.recorder()),
        }
    }

    /// A tool's method call between two scans, answered or refused (specification 48 RU-59): the
    /// wall clock's readings it made are taken, and an answered call is the next cycle's change,
    /// with them. The readings are held only until then, in memory reserved for exactly them, as a
    /// chip reserves what a recording holds: memory it cannot give, like readings past the room,
    /// ends the recording `memory_full`.
    fn method_called(&mut self, call: &MethodCall, answered: bool) {
        let mut taken = Vec::new();
        let reserved = taken.try_reserve_exact(readings::kept()).is_ok();
        let read = readings::take(&mut |reading| {
            if reserved {
                taken.push(reading);
            }
        });
        readings::disarm();
        let mut recordings = self.recordings();
        if read > taken.len() {
            recordings.each(Recorder::readings_lost);
        }
        if answered {
            recordings.each(|recorder| {
                recorder.method_called(ChangeSource::ControlLink, &call.instance, call, &taken);
            });
        }
    }

    /// A write the control link made to `target`, as Live Values applied it: recorded by the path
    /// the debugger names the variable by.
    pub(super) fn written(
        &mut self,
        source: ChangeSource,
        target: &PendingVarTarget,
        variables: &dyn CompiledVariables,
    ) {
        self.recordings()
            .each(|recorder| recorder.written_target(source, target, variables));
    }

    /// A one-shot write of `value` at `address`, recorded as the bytes it gives the process image.
    pub(super) fn address_written(
        &mut self,
        source: ChangeSource,
        address: &IoAddress,
        value: &Value,
        variables: &dyn CompiledVariables,
    ) {
        self.recordings()
            .each(|recorder| recorder.address_written(source, address, value, variables));
    }

    /// A force set or released by the control link, recorded once, where it changes.
    pub(super) fn forced(&mut self, effect: ForceEffect<'_>, variables: &dyn CompiledVariables) {
        self.recordings()
            .each(|recorder| recorder.forced(ChangeSource::ControlLink, effect, variables));
    }

    /// Force changes that the queues could not keep: the recordings end `memory_full`.
    pub(super) fn force_changes_lost(&mut self) {
        self.recordings().each(Recorder::force_changes_lost);
    }

    /// The recordings' part of a scan's phase (specification 48 RU-59): the cycle's start, each
    /// task's activation, the output image it wrote, and the cycle's end with the wall clock's
    /// readings. When the debugger has held the scan since it began (`holds` holds counted so far),
    /// the recording on demand ends `paused` and the flight recorder starts again after the scan.
    pub(super) fn phase(
        &mut self,
        phase: CyclePhase,
        now: Duration,
        cycle: u64,
        outputs: &[u8],
        holds: u64,
    ) {
        if self.session.is_none() && self.flight.is_none() {
            return;
        }
        match phase {
            CyclePhase::CycleStart => {
                self.holds_at_start = holds;
                if let Some(session) = &mut self.session
                    && session.recorder().begin_cycle(now, cycle) == Begin::First
                {
                    session.start_pending = true;
                }
                if let Some(flight) = &mut self.flight
                    && flight.recorder.recorder().begin_cycle(now, cycle) == Begin::First
                {
                    flight.start_pending = true;
                }
                readings::arm();
            }
            CyclePhase::TaskStart(index) => {
                let task = u32::try_from(index).unwrap_or(u32::MAX);
                self.recordings()
                    .each(|recorder| recorder.activation(task, now.as_nanos()));
            }
            CyclePhase::AfterOutputSample => {
                self.recordings().each(|recorder| recorder.outputs(outputs));
            }
            CyclePhase::CycleEnd => {
                take_readings(self.recordings());
                if holds > self.holds_at_start {
                    if let Some(session) = &mut self.session {
                        session.recorder().finish(TraceEnd::Paused);
                    }
                    if let Some(flight) = &mut self.flight {
                        flight.rearm_pending = true;
                    }
                } else {
                    if let Some(session) = &mut self.session {
                        session.recorder().end_cycle(None);
                    }
                    if let Some(flight) = &mut self.flight
                        && flight.recorder.end_cycle(None) == FlightCycle::StartDue
                    {
                        flight.handover_pending = true;
                    }
                }
            }
            _ => {}
        }
    }

    /// The recordings' start state, on their first cycle, before the boundary's writes: each area
    /// as the program holds it.
    pub(super) fn start_memory(&mut self, variables: &dyn CompiledVariables) {
        let take = |recorder: &mut Recorder| {
            for id in 1..=6 {
                if let Some(area) = recorder.start_area(id) {
                    variables.read_area_bytes(id, 0, area);
                }
            }
        };
        if let Some(session) = &mut self.session
            && session.start_pending
        {
            take(session.recorder());
        }
        if let Some(flight) = &mut self.flight
            && flight.start_pending
        {
            take(flight.recorder.recorder());
        }
    }

    /// The forces in effect at the recordings' first cycle, the variables' `vars` and the
    /// addresses' `io`, once the boundary applied them: the start state is then whole.
    pub(super) fn start_forces(
        &mut self,
        vars: &[ForcedVar],
        io: &[(IoAddress, Value)],
        variables: &dyn CompiledVariables,
    ) {
        if let Some(session) = &mut self.session
            && session.start_pending
        {
            session
                .recorder()
                .start_forces(ChangeSource::ControlLink, vars, io, variables);
            session.start_pending = false;
        }
        if let Some(flight) = &mut self.flight
            && flight.start_pending
        {
            flight
                .recorder
                .recorder()
                .start_forces(ChangeSource::ControlLink, vars, io, variables);
            flight.start_pending = false;
        }
    }

    /// The input image as the scan sampled it, once everything that writes it before the program
    /// has.
    pub(super) fn inputs(&mut self, image: &[u8]) {
        self.recordings().each(|recorder| recorder.inputs(image));
    }

    /// A scan that failed: the recordings' last cycle, with `fault`; the flight recorder's ends
    /// as the fault's trace.
    pub(super) fn faulted(&mut self, fault: CycleFault) {
        take_readings(self.recordings());
        if let Some(session) = &mut self.session {
            session.recorder().end_cycle(Some(fault));
        }
        if let Some(flight) = &mut self.flight {
            flight.recorder.end_cycle(Some(fault));
        }
    }
}

/// Hands the wall clock's readings of the scan to `recordings` and stops keeping them: readings the
/// room could not keep end the recordings `memory_full`.
fn take_readings(mut recordings: Recordings<'_>) {
    let mut handed = 0;
    let read = readings::take(&mut |reading| {
        recordings.each(|recorder| recorder.wall_clock(reading));
        handed += 1;
    });
    if read > handed {
        recordings.each(Recorder::readings_lost);
    }
    readings::disarm();
}

/// The program's memory between two scans, read only: what a flight recording's start state takes
/// after the scan (the core reads every program memory as its variables).
struct ReadMemory<'m>(&'m dyn ProgramMemory);

impl ProgramMemory for ReadMemory<'_> {
    fn image(&self) -> Image<'_> {
        self.0.image()
    }

    fn read_area(&self, id: u8, offset: usize, out: &mut [u8]) -> bool {
        self.0.read_area(id, offset, out)
    }

    fn write_area(&mut self, _id: u8, _offset: usize, _data: &[u8]) -> bool {
        false
    }

    fn input_image(&self) -> &[u8] {
        self.0.input_image()
    }

    fn write_image(&mut self, _address: &IoAddress, _value: Value) -> Result<(), RuntimeError> {
        Err(RuntimeError::ControlError(SmolStr::new_static(
            "a cycle trace reads the program's memory only",
        )))
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
    /// Keeps the flight recorder of `cycles` cycles, the program's deployment record's (0 keeps
    /// none), and arms it now for the program that runs, as a computer arms its flight recorder of
    /// `runtime.toml` (specification 11 §6.8.1; specification 34 §5.1.1, section 10). A restart and
    /// an online change arm it again.
    pub fn set_flight_recorder(&self, cycles: u64) {
        self.shell
            .traces()
            .with(|traces| traces.set_flight_cycles(cycles));
        self.arm_flight();
    }

    /// Arms the flight recorder for the program that runs, within the memory the chip has for a
    /// recording beside an online change of it ([`room_beside_change`]), between two scans. Memory
    /// the chip cannot give leaves the program without a flight recorder, and a
    /// `runtime_trace_memory` event names the bytes needed and the bytes free (specification 11
    /// §6.8.1).
    #[inline(never)]
    pub(super) fn arm_flight(&self) {
        // The program before keeps its flight recorder no longer, and a record that keeps none arms
        // none.
        let wanted = self.shell.traces().with(|traces| {
            traces.flight = None;
            traces.flight_cycles > 0
        });
        if wanted != Some(true) {
            return;
        }
        let Some(memory) = self.cycle.variables_memory() else {
            return;
        };
        let image = memory.image();
        let Ok((bindings, binding_names)) = io_bindings(&image) else {
            return;
        };
        let resource = self
            .shell
            .with_platform(|platform| SmolStr::new(platform.resource_name()))
            .unwrap_or_default();
        let running = self.cycle.program_memory();
        let armed = self.shell.with_platform(|platform| {
            let free = room_beside_change(platform.recording_room(), running);
            let now_utc_ms = platform
                .utc_ms()
                .and_then(|millis| i64::try_from(millis).ok())
                .unwrap_or(0);
            let device = Device {
                runtime_id: platform.runtime_id(),
                platform: platform.platform(),
            };
            let program = Program {
                image: &image,
                bindings: &bindings,
                binding_names: &binding_names,
                resource: &resource,
                cycle_interval: self.cycle.cycle_interval(),
            };
            self.shell
                .traces()
                .with(|traces| traces.arm_flight(&program, &device, free, now_utc_ms))
                .map(|armed| armed.map_err(|needed| (needed, free)))
        });
        if let Some(Some(Err((needed, free)))) = armed {
            self.shell.records.push_event(RuntimeEvent::TraceMemory {
                needed,
                free,
                time: self.program_time,
            });
        }
    }

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

    /// The recordings' part of a scan's end, while one runs: a failed scan is their last cycle,
    /// with the kernel's record of the fault; a flight recording that took over takes its start
    /// state before any change between the scans; and the recording's progress, or its trace once
    /// it has ended, and a fault's trace are published, each variable named as the debugger names
    /// it (specification 48 RU-59).
    #[inline(never)]
    pub(super) fn trace_scan_end(&self, failed: bool) {
        if failed
            && let (Some((code, statement)), Some(memory)) =
                (self.cycle.last_fault_site(), self.cycle.variables_memory())
        {
            let fault = cycle_fault(&memory.image(), code, statement);
            self.shell.traces().with(|traces| traces.faulted(fault));
        }
        let mut rearm = false;
        if let Some(memory) = self.cycle.variables_memory() {
            let variables = ReadMemory(memory);
            self.live.with_forces(&mut |vars, io| {
                rearm = self
                    .shell
                    .traces()
                    .with(|traces| traces.flight_after_scan(&variables, vars, io))
                    .unwrap_or(false);
            });
        }
        let cycle = &self.cycle;
        self.shell
            .traces()
            .with(|traces| traces.publish(&mut |path| cycle.display_path(path)));
        if rearm {
            self.arm_flight();
        }
    }
}
