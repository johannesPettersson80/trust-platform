//! The flight recorder (specification 48 RU-59, "The flight recorder"; specification 11 §6.8.1,
//! "Cycle traces"): with `[runtime.trace] flight_recorder = n` a runtime always keeps the last `n`
//! to `2n` cycles, and when the resource faults it keeps them, up to the faulted cycle, as the
//! fault's trace.
//!
//! It keeps two recordings of `n` cycles, both reserved when it is armed. The runner records into
//! one; when that one holds its `n` cycles, the other is emptied in place and records next, with
//! the program's state as the full one's last cycle left it as its start. The fault's trace joins
//! the full recording and the one that faulted, from the full one's start. Nothing is allocated
//! between the cycles (specification 34 MEM-09): emptying keeps a recording's memory.

use alloc::string::String;
use alloc::vec::Vec;

use trust_runtime_core::trace::{CycleFault, ImageTables, Located, TraceEnd, TraceOrigin};

use super::{Budget, Goal, Identity, Recorder, ReserveError, Sizes, Tally};

/// How a cycle ended for the flight recorder ([`FlightRecorder::end_cycle`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlightCycle {
    /// The recording goes on in the same recording.
    Recording,
    /// The recording that held its `n` cycles handed over to the other, emptied: the runner gives
    /// that one its start state now ([`Recorder::start_area`], [`Recorder::start_force`]), the
    /// memory and the forces as this cycle left them.
    StartDue,
    /// The recording has ended at a fault: the fault's trace ([`FlightRecorder::fault_file`]).
    Faulted,
}

/// A runtime's flight recorder (module docs).
#[derive(Debug)]
pub struct FlightRecorder {
    recordings: [Recorder; 2],
    /// The recording the runner records into.
    current: usize,
    /// Whether the other recording holds, complete, the cycles right before the current one's, so
    /// that the fault's trace starts from it.
    joined: bool,
    /// Whether the resource faulted, which ended the recording: its trace is then the fault's.
    fault: bool,
}

impl FlightRecorder {
    /// Reserves the memory of a flight recorder that keeps the last `cycles` to twice as many cycles
    /// of a program of `sizes`, each of its two recordings within `budget`.
    ///
    /// # Errors
    ///
    /// [`ReserveError::InsufficientMemory`], with the bytes both recordings need, when they cannot
    /// be reserved.
    pub fn arm(cycles: u64, sizes: Sizes, budget: Budget) -> Result<Self, ReserveError> {
        let goal = Goal::Cycles(cycles.max(1));
        let both = |ReserveError::InsufficientMemory { needed }| ReserveError::InsufficientMemory {
            needed: needed.saturating_mul(2),
        };
        let first = Recorder::arm(goal, sizes, budget).map_err(both)?;
        let second = Recorder::arm(goal, sizes, budget).map_err(both)?;
        Ok(Self {
            recordings: [first, second],
            current: 0,
            joined: false,
            fault: false,
        })
    }

    /// The bytes the flight recorder reserved.
    #[must_use]
    pub fn reserved_bytes(&self) -> usize {
        self.recordings
            .iter()
            .map(Recorder::reserved_bytes)
            .fold(0, usize::saturating_add)
    }

    /// The recording the runner records the cycle into, as it records a trace on demand.
    pub fn recorder(&mut self) -> &mut Recorder {
        &mut self.recordings[self.current]
    }

    /// The cycle ends, faulted with `fault` or not ([`Recorder::end_cycle`]). A faulted cycle ends
    /// the recording, whose trace is then the fault's, ending `faulted`, or `memory_full` when the
    /// faulted cycle did not fit. A recording that has ended without a fault, complete or out of
    /// memory, hands over to the other one, emptied in place; the fault's trace starts from a
    /// complete one only, since an out of memory recording lacks the cycle it could not keep.
    pub fn end_cycle(&mut self, fault: Option<CycleFault>) -> FlightCycle {
        let recording = &mut self.recordings[self.current];
        let ended = recording.end_cycle(fault);
        if fault.is_some() {
            self.fault = true;
            return FlightCycle::Faulted;
        }
        if !ended {
            return FlightCycle::Recording;
        }
        self.joined = recording.end() == Some(TraceEnd::Complete);
        self.current = 1 - self.current;
        self.recordings[self.current].continue_empty();
        FlightCycle::StartDue
    }

    /// The resource faulted outside a scan: the recording ends with the cycles before, as the
    /// fault's trace.
    pub fn fault(&mut self) {
        self.fault = true;
        self.recordings[self.current].finish(TraceEnd::Faulted);
    }

    /// Whether the recording has ended at a fault, so that its trace is the fault's.
    #[must_use]
    pub const fn faulted(&self) -> bool {
        self.fault
    }

    /// The cycles the fault's trace holds, once the recording has ended at a fault.
    #[must_use]
    pub fn fault_cycles(&self) -> Option<u64> {
        let (start, later) = self.parts()?;
        Some(start.cycles() + later.map_or(0, Recorder::cycles))
    }

    /// The fault's `.trusttrace` file, once the recording has ended at a fault: `identity` and the
    /// recorded image's located variables and tables, the start state of the complete recording
    /// before the faulted one, or of the faulted one when there is none, and the cycles of both,
    /// up to the faulted one, with the variables named as the requests named them.
    #[must_use]
    pub fn fault_file(
        &self,
        identity: &Identity<'_>,
        located: &[Located],
        image: &ImageTables,
    ) -> Option<Vec<u8>> {
        let (start, later) = self.parts()?;
        let records: Vec<&[u8]> = start
            .cycle_records()
            .chain(later.into_iter().flat_map(Recorder::cycle_records))
            .collect();
        let begin = start.start_with(&start.start_forces);
        Some(start.encoded(identity, located, image, &records, &begin, self.tally()?))
    }

    /// The fault's file as [`Self::fault_file`] writes it, with each variable's path and each
    /// method call's instance named by `name`, as [`Recorder::file_named`] names them.
    #[must_use]
    pub fn fault_file_named(
        &self,
        identity: &Identity<'_>,
        located: &[Located],
        image: &ImageTables,
        name: &mut dyn FnMut(&str) -> Option<String>,
    ) -> Option<Vec<u8>> {
        let (start, later) = self.parts()?;
        let named: Vec<Vec<u8>> = start
            .cycle_records()
            .chain(later.into_iter().flat_map(Recorder::cycle_records))
            .map(|record| {
                trust_runtime_core::trace::named_cycle(record, name)
                    .unwrap_or_else(|_| record.to_vec())
            })
            .collect();
        let records: Vec<&[u8]> = named.iter().map(Vec::as_slice).collect();
        let forces = start.named_forces(name);
        let begin = start.start_with(&forces);
        Some(start.encoded(identity, located, image, &records, &begin, self.tally()?))
    }

    /// The recording the fault's trace starts from, and the faulted one after it when the trace
    /// joins two; none before a fault.
    fn parts(&self) -> Option<(&Recorder, Option<&Recorder>)> {
        if !self.faulted() {
            return None;
        }
        let faulted = &self.recordings[self.current];
        Some(if self.joined {
            (&self.recordings[1 - self.current], Some(faulted))
        } else {
            (faulted, None)
        })
    }

    /// The fault's trace's count of cycles, its end, the faulted recording's, and its origin.
    fn tally(&self) -> Option<Tally> {
        Some(Tally {
            cycles: self.fault_cycles()?,
            end: self.recordings[self.current].end()?,
            origin: TraceOrigin::FlightRecorder,
        })
    }
}

#[cfg(test)]
mod tests;
