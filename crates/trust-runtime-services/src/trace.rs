//! Recording a cycle trace (specification 48 RU-59; the file, Appendix C).
//!
//! A recording reserves its memory when it is armed ([`Recorder::arm`]). Between the phases of each
//! scan the runner hands it the cycle's start, the changes made from outside, the task activations,
//! the wall clock's readings and the images, which it copies into that memory without allocating
//! (specification 34 MEM-09); at the cycle's end the record joins the trace. What does not fit ends
//! the recording `memory_full`, with the cycles before it. Once ended, [`Recorder::file`] gives the
//! `.trusttrace` file.

mod changes;
mod flight;
pub mod requests;
mod served;
pub mod session;

pub use changes::{address_size, trace_area, trace_bit};
pub use flight::{FlightCycle, FlightRecorder};
pub use served::ServedTrace;
#[cfg(test)]
mod tests;

use alloc::string::String;
use alloc::vec::Vec;

use trust_runtime_core::native::variables::CompiledVariables;
use trust_runtime_core::trace::{
    change_len, cycle_record_len, encode_change, encode_cycle_parts, encode_records, named_changes,
    named_cycle, Activation, ChangeRef, ChangeSource, CycleFault, CycleParts, ImageTables, Located,
    Meta, StartParts, TraceEnd, TraceOrigin,
};
use trust_runtime_core::value::Duration;

/// What a recording records until (specification 11 §6.8.1, `trace.record`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Goal {
    /// This many cycles of the resource.
    Cycles(u64),
    /// The cycles within this much logical time from the first.
    Duration(Duration),
    /// The cycles until the task of this index has run this many times, every cycle recorded.
    TaskRuns {
        /// The task's index in the image's task table.
        task: u32,
        /// How many runs.
        runs: u64,
    },
}

/// The program a recording reserves its memory for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sizes {
    /// The bytes of the areas of specification 44 §7.1 but the stack, by id 1 (`%I`) to 6 (`P`).
    pub areas: [usize; 6],
    /// The image's tasks.
    pub tasks: usize,
    /// The resource's cycle interval.
    pub cycle_interval: Duration,
    /// The interval of the task a [`Goal::TaskRuns`] counts, when it has one.
    pub task_interval: Option<Duration>,
}

/// What a recording may hold besides each cycle's fixed part: its time, activations and images.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Budget {
    /// Bytes of changes in one cycle.
    pub cycle_changes: usize,
    /// Wall-clock readings in one cycle.
    pub cycle_readings: usize,
    /// Bytes of changes and readings in the whole recording, beyond the cycles' fixed parts.
    pub extra: usize,
    /// Bytes of the forces in effect at the start.
    pub forces: usize,
    /// Bytes of the largest value one write records ([`Recorder::written`]).
    pub value: usize,
}

impl Budget {
    /// A computer's: 64 KiB of changes and 256 readings in a cycle, 4 MiB more for the recording,
    /// 64 KiB of forces and a value of up to 64 KiB.
    pub const COMPUTER: Self = Self {
        cycle_changes: 64 * 1024,
        cycle_readings: 256,
        extra: 4 * 1024 * 1024,
        forces: 64 * 1024,
        value: 64 * 1024,
    };

    /// The budget of a runtime with `free` bytes of memory to give, the same rule on every
    /// runtime (specification 48 RU-59, "Memory"): each room a computer's ([`Self::COMPUTER`]),
    /// but no more than a sixteenth of `free` for one cycle's changes and for the forces, a
    /// thirty-second for one value, a quarter for the recording's changes and readings, and as
    /// many readings in a cycle as a sixty-fourth holds. A runtime that does not know its memory,
    /// as a computer does not, has a computer's.
    #[must_use]
    pub fn for_free(free: Option<usize>) -> Self {
        let Some(free) = free else {
            return Self::COMPUTER;
        };
        let computer = Self::COMPUTER;
        Self {
            cycle_changes: computer.cycle_changes.min(free / 16),
            cycle_readings: computer.cycle_readings.min(free / 64 / 8),
            extra: computer.extra.min(free / 4),
            forces: computer.forces.min(free / 16),
            value: computer.value.min(free / 32),
        }
    }
}

/// Why a recording could not be armed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReserveError {
    /// The memory the recording needs could not be reserved.
    InsufficientMemory {
        /// The bytes the recording needs.
        needed: usize,
    },
}

impl core::fmt::Display for ReserveError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InsufficientMemory { needed } => write!(
                f,
                "the recording needs {needed} bytes, which this runtime cannot reserve"
            ),
        }
    }
}

/// How a cycle begins for a recording ([`Recorder::begin_cycle`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Begin {
    /// The trace's first cycle: before its changes the runner gives the start state,
    /// [`Recorder::start_area`] for each area and [`Recorder::start_force`] for each force.
    First,
    /// A later cycle.
    Next,
    /// The recording has ended; the cycle is not recorded.
    Ended,
}

/// What the recorded program was and where it ran, for the trace's identity (Appendix C, C.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Identity<'a> {
    /// The running image's SHA-256.
    pub image_sha256: [u8; 32],
    /// The image's sources' hash.
    pub sources_hash: [u8; 32],
    /// The image's target id.
    pub target: u32,
    /// The runtime's version.
    pub runtime_version: &'a str,
    /// The device's identity (`runtime_id`).
    pub runtime_id: &'a str,
    /// The platform.
    pub platform: &'a str,
    /// The resource's name.
    pub resource: &'a str,
    /// The task a [`Goal::TaskRuns`] counted, by name.
    pub task: Option<&'a str>,
    /// The UTC time the recording began, in milliseconds.
    pub started_utc_ms: i64,
}

/// A recording of a cycle trace (module docs).
#[derive(Debug)]
pub struct Recorder {
    goal: Goal,
    sizes: Sizes,
    /// The cycles' records, each a multiple of 8 bytes, and where each ends.
    records: Vec<u8>,
    record_ends: Vec<usize>,
    /// What the recording reserved, which its buffers never outgrow: an allocator may give a buffer
    /// more room than asked, and a recording ends `memory_full` at the same place on every device.
    limits: Limits,
    /// The areas 1 to 6 at the start, one after the other, and the forces then in effect.
    start_areas: Vec<u8>,
    start_forces: Vec<u8>,
    start_force_count: usize,
    /// Room for the bytes of one written value, read back ([`Recorder::written`]).
    value: Vec<u8>,
    first: Option<(Duration, u64)>,
    task_runs: u64,
    end: Option<TraceEnd>,
    /// Whether this recording continues another whose last cycle has just ended (the flight
    /// recorder, [`Recorder::continue_empty`]): its start state was taken then, and a change made
    /// before its first cycle is that cycle's.
    continued: bool,
    reserved: usize,
    cycle: Scratch,
}

/// The room a recording reserved for each of its buffers.
#[derive(Debug, Clone, Copy)]
struct Limits {
    records: usize,
    cycles: usize,
    forces: usize,
    changes: usize,
    readings: usize,
    activations: usize,
}

/// The cycle being recorded, in memory reserved for one cycle.
#[derive(Debug)]
struct Scratch {
    open: bool,
    overflow: bool,
    time: Duration,
    activations: Vec<Activation>,
    readings: Vec<i64>,
    changes: Vec<u8>,
    change_count: usize,
    inputs: Vec<u8>,
    outputs: Vec<u8>,
}

impl Recorder {
    /// Reserves the memory of a recording toward `goal` of a program of `sizes`, within `budget`.
    ///
    /// # Errors
    ///
    /// [`ReserveError::InsufficientMemory`], with the bytes needed, when they cannot be reserved.
    pub fn arm(goal: Goal, sizes: Sizes, budget: Budget) -> Result<Self, ReserveError> {
        let cycles = estimated_cycles(goal, &sizes);
        let records = cycles
            .saturating_mul(fixed_record_len(&sizes))
            .saturating_add(budget.extra);
        let areas = sizes
            .areas
            .iter()
            .fold(0_usize, |sum, len| sum.saturating_add(*len));
        let activations = sizes.tasks.max(1);
        let needed = needed_bytes(&sizes, &budget, cycles, records, areas, activations);
        let reserve = || -> Option<Self> {
            Some(Self {
                goal,
                sizes,
                records: room(records)?,
                record_ends: room(cycles.saturating_add(1))?,
                limits: Limits {
                    records,
                    cycles: cycles.saturating_add(1),
                    forces: budget.forces,
                    changes: budget.cycle_changes,
                    readings: budget.cycle_readings,
                    activations,
                },
                start_areas: zeroed(areas)?,
                start_forces: room(budget.forces)?,
                start_force_count: 0,
                value: zeroed(budget.value)?,
                first: None,
                task_runs: 0,
                end: None,
                continued: false,
                reserved: needed,
                cycle: Scratch {
                    open: false,
                    overflow: false,
                    time: Duration::ZERO,
                    activations: room(activations)?,
                    readings: room(budget.cycle_readings)?,
                    changes: room(budget.cycle_changes)?,
                    change_count: 0,
                    inputs: room(sizes.areas[0])?,
                    outputs: room(sizes.areas[1])?,
                },
            })
        };
        reserve().ok_or(ReserveError::InsufficientMemory { needed })
    }

    /// The bytes the recording reserved, as `trace.record` answers them.
    #[must_use]
    pub const fn reserved_bytes(&self) -> usize {
        self.reserved
    }

    /// The cycles the trace holds.
    #[must_use]
    pub fn cycles(&self) -> u64 {
        self.record_ends.len() as u64
    }

    /// Why the recording ended, once it has.
    #[must_use]
    pub const fn end(&self) -> Option<TraceEnd> {
        self.end
    }

    /// The cycle of the runtime's count `cycle_number` begins at the logical time `now`.
    pub fn begin_cycle(&mut self, now: Duration, cycle_number: u64) -> Begin {
        if self.end.is_some() {
            return Begin::Ended;
        }
        if let (Goal::Duration(duration), Some((first, _))) = (self.goal, self.first)
            && now.as_nanos().saturating_sub(first.as_nanos()) >= duration.as_nanos()
        {
            self.finish(TraceEnd::Complete);
            return Begin::Ended;
        }
        // The changes made since the last cycle ended are this cycle's (`change`), and so is their
        // overflow; the rest starts afresh.
        let cycle = &mut self.cycle;
        cycle.open = true;
        cycle.time = now;
        cycle.activations.clear();
        cycle.readings.clear();
        cycle.inputs.clear();
        cycle.outputs.clear();
        if self.first.is_none() {
            self.first = Some((now, cycle_number));
            // A continued recording took its start state when the cycle before it ended.
            return if self.continued {
                Begin::Next
            } else {
                Begin::First
            };
        }
        Begin::Next
    }

    /// Empties the recording in place, its memory kept, to continue a recording whose last cycle
    /// has just ended (the flight recorder): the runner gives its start state now
    /// ([`Self::start_area`], [`Self::start_force`]), the memory and the forces as that cycle left
    /// them, and a change made before this recording's first cycle is that cycle's. The two
    /// recordings joined then hold every change once, and this one alone is a trace of its own. It
    /// allocates nothing.
    pub(crate) fn continue_empty(&mut self) {
        self.records.clear();
        self.record_ends.clear();
        self.start_forces.clear();
        self.start_force_count = 0;
        self.first = None;
        self.task_runs = 0;
        self.end = None;
        self.continued = true;
        let cycle = &mut self.cycle;
        cycle.open = false;
        cycle.overflow = false;
        cycle.changes.clear();
        cycle.change_count = 0;
    }

    /// The reserved bytes of area `id` (1 `%I` to 6 `P`) at the start, for the runner to fill on
    /// the trace's first cycle ([`Begin::First`]).
    pub fn start_area(&mut self, id: u8) -> Option<&mut [u8]> {
        let index = usize::from(id).checked_sub(1).filter(|index| *index < 6)?;
        let start: usize = self.sizes.areas[..index].iter().sum();
        self.start_areas
            .get_mut(start..start + self.sizes.areas[index])
    }

    /// A force in effect at the start, on the trace's first cycle ([`Begin::First`]).
    pub fn start_force(&mut self, source: ChangeSource, force: &ChangeRef<'_>) {
        if !fits(&self.start_forces, change_len(force), self.limits.forces) {
            self.cycle.overflow = true;
            return;
        }
        encode_change(source, force, &mut self.start_forces);
        self.start_force_count += 1;
    }

    /// Whether a change from outside is recorded now: one made in a cycle, or between two cycles of
    /// the recording, which is the next cycle's. One made before the first cycle is in the start
    /// state, but in a continued recording's first cycle ([`Self::continue_empty`]), and one after
    /// the recording ended is not recorded.
    fn takes_changes(&self) -> bool {
        self.cycle.open || ((self.first.is_some() || self.continued) && self.end.is_none())
    }

    /// A change made from outside before the cycle, in the order the runtime applies it. One made
    /// between two cycles is the next cycle's, one made before the first cycle is in the start
    /// state, and one made after the recording ended is not recorded.
    pub fn change(&mut self, source: ChangeSource, change: &ChangeRef<'_>) {
        if !self.takes_changes() {
            return;
        }
        let cycle = &mut self.cycle;
        if !fits(&cycle.changes, change_len(change), self.limits.changes) {
            cycle.overflow = true;
            return;
        }
        encode_change(source, change, &mut cycle.changes);
        cycle.change_count += 1;
    }

    /// A write from outside to the variable `path`, which `variables` has applied: recorded as the
    /// bytes the variable now holds, at its declared size, with its type's name, read back into
    /// the reserved room for a value. A write between two cycles, such as a screen's, is the next
    /// cycle's ([`Self::change`]). A value longer than that room, or one that cannot be read back,
    /// ends the recording `memory_full` at the end of the cycle the write belongs to.
    pub fn written(&mut self, source: ChangeSource, path: &str, variables: &dyn CompiledVariables) {
        if !self.takes_changes() {
            return;
        }
        // The room is taken out while its bytes are recorded and put back: moving a vector
        // allocates nothing.
        let mut value = core::mem::take(&mut self.value);
        match variables.read_named_bytes(path, &mut value) {
            Ok((len, type_name)) => self.change(
                source,
                &ChangeRef::VariableWrite {
                    path,
                    type_name: &type_name,
                    value: &value[..len],
                },
            ),
            Err(_) => self.cycle.overflow = true,
        }
        self.value = value;
    }

    /// Readings of the wall clock were lost, which the room for them could not keep: the recording
    /// ends `memory_full` at the cycle's end.
    pub fn readings_lost(&mut self) {
        if self.cycle.open {
            self.cycle.overflow = true;
        }
    }

    /// The task of index `task` starts with the logical time `now_ns`.
    pub fn activation(&mut self, task: u32, now_ns: i64) {
        let cycle = &mut self.cycle;
        if !cycle.open {
            return;
        }
        if cycle.activations.len() >= self.limits.activations {
            cycle.overflow = true;
            return;
        }
        cycle.activations.push(Activation { task, now_ns });
    }

    /// The program read the wall clock (`CURRENT_DT`): UTC milliseconds.
    pub fn wall_clock(&mut self, millis: i64) {
        let cycle = &mut self.cycle;
        if !cycle.open {
            return;
        }
        if cycle.readings.len() >= self.limits.readings {
            cycle.overflow = true;
            return;
        }
        cycle.readings.push(millis);
    }

    /// The input image as the scan sampled it.
    pub fn inputs(&mut self, image: &[u8]) {
        copy_image(&mut self.cycle, image, true, self.sizes.areas[0]);
    }

    /// The output image the cycle wrote.
    pub fn outputs(&mut self, image: &[u8]) {
        copy_image(&mut self.cycle, image, false, self.sizes.areas[1]);
    }

    /// The cycle ends, faulted with `fault` or not. Its record joins the trace when the trace has
    /// room for it; it returns whether the recording has ended.
    pub fn end_cycle(&mut self, fault: Option<CycleFault>) -> bool {
        if !self.cycle.open {
            return self.end.is_some();
        }
        self.cycle.open = false;
        let ended = self.record_cycle(fault);
        // The next cycle's changes start from none.
        let cycle = &mut self.cycle;
        cycle.changes.clear();
        cycle.change_count = 0;
        cycle.overflow = false;
        ended
    }

    /// The open cycle's record joins the trace when the trace has room for it, and the recording
    /// ends where the cycle completes its goal or faulted; it gives whether the recording has ended.
    fn record_cycle(&mut self, fault: Option<CycleFault>) -> bool {
        if self.cycle.overflow {
            self.finish(TraceEnd::MemoryFull);
            return true;
        }
        let cycle = &self.cycle;
        let parts = CycleParts {
            time_ns: cycle.time.as_nanos(),
            activations: &cycle.activations,
            wall_clock_ms: &cycle.readings,
            change_count: cycle.change_count,
            changes: &cycle.changes,
            inputs: &cycle.inputs,
            outputs: &cycle.outputs,
            fault,
        };
        if !fits(&self.records, cycle_record_len(&parts), self.limits.records)
            || self.record_ends.len() >= self.limits.cycles
        {
            self.finish(TraceEnd::MemoryFull);
            return true;
        }
        encode_cycle_parts(&parts, &mut self.records);
        self.record_ends.push(self.records.len());
        if let Goal::TaskRuns { task, .. } = self.goal {
            let runs = cycle
                .activations
                .iter()
                .filter(|run| run.task == task)
                .count();
            self.task_runs = self.task_runs.saturating_add(runs as u64);
        }
        let complete = match self.goal {
            Goal::Cycles(count) => self.cycles() >= count,
            Goal::Duration(_) => false,
            Goal::TaskRuns { runs, .. } => self.task_runs >= runs,
        };
        if fault.is_some() {
            self.finish(TraceEnd::Faulted);
        } else if complete {
            self.finish(TraceEnd::Complete);
        }
        self.end.is_some()
    }

    /// Ends the recording, if it has not ended: a cycle that is open is not recorded.
    pub fn finish(&mut self, end: TraceEnd) {
        self.cycle.open = false;
        if self.end.is_none() {
            self.end = Some(end);
        }
    }

    /// The trace's `.trusttrace` file once the recording has ended: `identity`, the recorded
    /// image's located variables and tables, the start state and every recorded cycle, with the
    /// variables named as the requests named them.
    #[must_use]
    pub fn file(
        &self,
        identity: &Identity<'_>,
        located: &[Located],
        image: &ImageTables,
    ) -> Option<Vec<u8>> {
        // A recording has no file until it has ended, and asking costs nothing.
        let end = self.end?;
        let records: Vec<&[u8]> = self.cycle_records().collect();
        let start = self.start_with(&self.start_forces);
        let tally = Tally {
            cycles: self.cycles(),
            end,
            origin: TraceOrigin::OnDemand,
        };
        Some(self.encoded(identity, located, image, &records, &start, tally))
    }

    /// The trace's file as [`Self::file`] writes it, with each variable's path and each method
    /// call's instance named by `name`, the name the debugger gives it, or kept when `name` gives
    /// none (Appendix C, C.6). The recording keeps the names as the requests gave them, since a scan
    /// cannot look a name up without allocating (specification 34 MEM-09); the file, written after
    /// the recording, names them.
    #[must_use]
    pub fn file_named(
        &self,
        identity: &Identity<'_>,
        located: &[Located],
        image: &ImageTables,
        name: &mut dyn FnMut(&str) -> Option<String>,
    ) -> Option<Vec<u8>> {
        // A recording has no file until it has ended, and asking costs nothing.
        self.end?;
        // Every record and force reads again, since this recorder wrote them; one that did not
        // would keep its names.
        let named: Vec<Vec<u8>> = self
            .cycle_records()
            .map(|record| named_cycle(record, name).unwrap_or_else(|_| record.to_vec()))
            .collect();
        let records: Vec<&[u8]> = named.iter().map(Vec::as_slice).collect();
        let forces = self.named_forces(name);
        let start = self.start_with(&forces);
        let tally = Tally {
            cycles: self.cycles(),
            end: self.end?,
            origin: TraceOrigin::OnDemand,
        };
        Some(self.encoded(identity, located, image, &records, &start, tally))
    }

    /// The forces in effect at the start, with each variable's path named by `name` as
    /// [`Self::file_named`] names them, or as recorded when they do not read again.
    fn named_forces(&self, name: &mut dyn FnMut(&str) -> Option<String>) -> Vec<u8> {
        named_changes(&self.start_forces, self.start_force_count, name)
            .unwrap_or_else(|_| self.start_forces.clone())
    }

    /// Each recorded cycle's record, in order.
    fn cycle_records(&self) -> impl Iterator<Item = &[u8]> {
        let starts = core::iter::once(0).chain(self.record_ends.iter().copied());
        starts
            .zip(&self.record_ends)
            .map(|(from, end)| &self.records[from..*end])
    }

    /// The start state this recording took, its areas and the forces `forces`, which name the
    /// paths as the file does.
    fn start_with<'f>(&self, forces: &'f [u8]) -> Start<'_, 'f> {
        let mut areas = Vec::new();
        if self.first.is_some() || self.continued {
            let mut start = 0;
            for (index, len) in self.sizes.areas.iter().enumerate() {
                let id = u32::try_from(index + 1).unwrap_or(0);
                areas.push((id, &self.start_areas[start..start + len]));
                start += len;
            }
        }
        Start {
            areas,
            forces,
            force_count: self.start_force_count,
        }
    }

    /// The file of a trace that starts with `start`, this recording's, and holds `records`, with
    /// the count, end and origin of `tally` and this recording's first cycle.
    fn encoded(
        &self,
        identity: &Identity<'_>,
        located: &[Located],
        image: &ImageTables,
        records: &[&[u8]],
        start: &Start<'_, '_>,
        tally: Tally,
    ) -> Vec<u8> {
        let meta = self.meta(identity, tally);
        let parts = StartParts {
            areas: &start.areas,
            force_count: start.force_count,
            forces: start.forces,
        };
        encode_records(&meta, located, &parts, image, records)
    }

    /// The trace's identity as its file's `META` names it (Appendix C, C.3): `identity`, the
    /// recording's first cycle and cycle interval, and `tally`.
    fn meta(&self, identity: &Identity<'_>, tally: Tally) -> Meta {
        let (_, first_cycle) = self.first.unwrap_or((Duration::ZERO, 0));
        Meta {
            image_sha256: identity.image_sha256,
            sources_hash: identity.sources_hash,
            target: identity.target,
            end: tally.end,
            cycle_interval_ns: u64::try_from(self.sizes.cycle_interval.as_nanos()).unwrap_or(0),
            first_cycle,
            cycle_count: tally.cycles,
            started_utc_ms: identity.started_utc_ms,
            runtime_version: String::from(identity.runtime_version),
            runtime_id: String::from(identity.runtime_id),
            platform: String::from(identity.platform),
            resource: String::from(identity.resource),
            task: identity.task.map(String::from),
            origin: tally.origin,
        }
    }
}

/// A trace's start state, as a recording took it ([`Recorder::start_with`]).
struct Start<'a, 'f> {
    areas: Vec<(u32, &'a [u8])>,
    forces: &'f [u8],
    force_count: usize,
}

/// What a trace's identity counts: its cycles, why it ended, and where it comes from.
#[derive(Debug, Clone, Copy)]
struct Tally {
    cycles: u64,
    end: TraceEnd,
    origin: TraceOrigin,
}

/// The bytes a recording reserves: its records and their ends, the start state, the forces, a
/// value, and one cycle's activations, readings, changes and images.
fn needed_bytes(
    sizes: &Sizes,
    budget: &Budget,
    cycles: usize,
    records: usize,
    areas: usize,
    activations: usize,
) -> usize {
    [
        records,
        cycles
            .saturating_add(1)
            .saturating_mul(core::mem::size_of::<usize>()),
        areas,
        budget.forces,
        budget.value,
        activations.saturating_mul(core::mem::size_of::<Activation>()),
        budget.cycle_readings.saturating_mul(8),
        budget.cycle_changes,
        sizes.areas[0],
        sizes.areas[1],
    ]
    .iter()
    .fold(0_usize, |sum, part| sum.saturating_add(*part))
}

/// An empty vector with room for `len` items, reserved now, or `None` when it cannot be.
fn room<T>(len: usize) -> Option<Vec<T>> {
    let mut vector = Vec::new();
    vector.try_reserve_exact(len).ok()?;
    Some(vector)
}

/// `len` zero bytes, reserved now, or `None` when they cannot be.
fn zeroed(len: usize) -> Option<Vec<u8>> {
    let mut bytes = room(len)?;
    bytes.resize(len, 0);
    Some(bytes)
}

/// The cycles a recording of `goal` reserves memory for: a duration's cycles at the resource's
/// interval, a task's runs at the task's.
fn estimated_cycles(goal: Goal, sizes: &Sizes) -> usize {
    let nanos = |duration: Duration| u64::try_from(duration.as_nanos()).unwrap_or(0);
    let interval = nanos(sizes.cycle_interval).max(1);
    let cycles = match goal {
        Goal::Cycles(count) => count,
        Goal::Duration(duration) => nanos(duration).div_ceil(interval),
        Goal::TaskRuns { runs, .. } => {
            let per_run = sizes
                .task_interval
                .map_or(1, |task| nanos(task).max(1).div_ceil(interval));
            runs.saturating_mul(per_run)
        }
    };
    usize::try_from(cycles).unwrap_or(usize::MAX)
}

/// The record of a cycle without changes or readings (Appendix C, C.6), with every task and a
/// fault: 24 bytes, 16 for each activation, the two padded images and their 16 bytes of lengths,
/// and 16 for the fault.
fn fixed_record_len(sizes: &Sizes) -> usize {
    let padded = |len: usize| len.saturating_add(7) & !7;
    24_usize
        .saturating_add(sizes.tasks.max(1).saturating_mul(16))
        .saturating_add(16)
        .saturating_add(padded(sizes.areas[0]))
        .saturating_add(padded(sizes.areas[1]))
        .saturating_add(16)
}

/// Whether `bytes` more fit into `buffer` within the `limit` it reserved.
fn fits(buffer: &[u8], bytes: usize, limit: usize) -> bool {
    buffer
        .len()
        .checked_add(bytes)
        .is_some_and(|len| len <= limit)
}

fn copy_image(cycle: &mut Scratch, image: &[u8], inputs: bool, limit: usize) {
    if !cycle.open {
        return;
    }
    let target = if inputs {
        &mut cycle.inputs
    } else {
        &mut cycle.outputs
    };
    if image.len() > limit {
        cycle.overflow = true;
        return;
    }
    target.clear();
    target.extend_from_slice(image);
}
