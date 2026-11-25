//! The flight recorder (specification 48 RU-59, "The flight recorder"): the last `n` to `2n` cycles
//! kept in two recordings, joined into the fault's trace, each read back from the file it gives
//! (Appendix C).

use alloc::vec;

use trust_runtime_core::deployment::sha256;
use trust_runtime_core::trace::{decode, ChangeKind, ChangeSource, Trace, TraceEnd, TraceOrigin};

use super::super::requests::TraceState;
use super::super::tests::{identity, image, ms, read_in_parts, sizes, WRITE};
use super::*;
use crate::trace::{Begin, Budget, FlightRecorder};

/// A cycle at `at` milliseconds of the runtime's count `100 + at`: task 0's activation and the
/// images given. The first recording's start state fills area 6 (`P`, 16 bytes) with `0xEE`; the
/// start state a hand-over asks for fills it with `at`, the memory as this cycle left it.
fn cycle(flight: &mut FlightRecorder, at: i64, fault: Option<CycleFault>) -> FlightCycle {
    let recorder = flight.recorder();
    if recorder.begin_cycle(ms(at), 100 + u64::try_from(at).unwrap()) == Begin::First {
        recorder.start_area(6).unwrap().fill(0xEE);
    }
    recorder.activation(0, ms(at).as_nanos());
    recorder.inputs(&[1, 0]);
    recorder.outputs(&[0, 0, 0, u8::try_from(at).unwrap()]);
    let ended = flight.end_cycle(fault);
    if ended == FlightCycle::StartDue {
        flight
            .recorder()
            .start_area(6)
            .unwrap()
            .fill(u8::try_from(at).unwrap());
    }
    ended
}

fn fault() -> CycleFault {
    CycleFault {
        code: 4,
        pou: 1,
        statement: 7,
    }
}

fn read(flight: &FlightRecorder) -> Trace {
    let file = flight
        .fault_file(&identity(), &[], &image())
        .expect("a fault's trace");
    let decoded = decode(&file).expect("a trace");
    assert_eq!(decoded.damaged_cycle, None);
    decoded.trace
}

fn times(trace: &Trace) -> Vec<i64> {
    trace
        .cycles
        .iter()
        .map(|cycle| cycle.time_ns / 1_000_000)
        .collect()
}

#[test]
fn a_fault_keeps_the_full_recording_and_the_faulted_one_from_the_full_ones_start() {
    // RU-59, "The flight recorder": the runtime keeps the last n to 2n cycles and, at a fault,
    // those cycles up to the faulted one. Worked by hand for n = 2 and five cycles at 0 to 40 ms:
    // the first recording holds 0 and 10 and hands over; the second, which starts with the memory
    // as the cycle at 10 left it (10 in each byte of area 6), holds 20 and 30 and hands over; the
    // first, emptied, starts with the memory after 30 (30) and records 40, which faults. The fault's
    // trace starts from the second recording's start, 10 in area 6, at the runtime's cycle 120, and
    // holds 20, 30
    // and 40, the last with its fault; a screen's write between 10 and 20 is 20's change, and one
    // between 30 and 40 is 40's, each once. Before the fault there is no fault's trace. Catches a
    // trace that starts from the wrong recording or the first start, a change at a hand-over lost
    // or kept twice, and a fault's trace given before a fault.
    let mut flight = FlightRecorder::arm(2, sizes(), Budget::COMPUTER).expect("armed");
    flight.began(1_000);
    assert_eq!(cycle(&mut flight, 0, None), FlightCycle::Recording);
    assert_eq!(cycle(&mut flight, 10, None), FlightCycle::StartDue);
    flight.began(2_000);
    flight.recorder().change(ChangeSource::Screen, &WRITE);
    assert_eq!(cycle(&mut flight, 20, None), FlightCycle::Recording);
    assert_eq!(cycle(&mut flight, 30, None), FlightCycle::StartDue);
    flight.began(3_000);
    assert_eq!(flight.fault_file(&identity(), &[], &image()), None);
    assert_eq!(flight.fault_began_utc_ms(), None);
    flight.recorder().change(ChangeSource::Screen, &WRITE);
    assert_eq!(cycle(&mut flight, 40, Some(fault())), FlightCycle::Faulted);
    assert!(flight.faulted());
    assert_eq!(flight.fault_cycles(), Some(3));
    // The trace begins with the second recording, which began at 2,000 ms.
    assert_eq!(flight.fault_began_utc_ms(), Some(2_000));

    let trace = read(&flight);
    assert_eq!(trace.meta.origin, TraceOrigin::FlightRecorder);
    assert_eq!(trace.meta.end, TraceEnd::Faulted);
    assert_eq!(trace.meta.first_cycle, 120);
    assert_eq!(trace.meta.cycle_count, 3);
    assert_eq!(times(&trace), [20, 30, 40]);
    let area = |id| {
        trace
            .start
            .areas
            .iter()
            .find(|(area, _)| *area == id)
            .map(|(_, bytes)| bytes.clone())
    };
    assert_eq!(area(6), Some(vec![10; 16]));
    let changes: Vec<usize> = trace
        .cycles
        .iter()
        .map(|cycle| cycle.changes.len())
        .collect();
    assert_eq!(changes, [1, 0, 1]);
    assert_eq!(trace.cycles[2].fault, Some(fault()));
    assert_eq!(trace.cycles[0].outputs, [0, 0, 0, 20]);
}

/// The debugger's name for the screen's write's variable, `Main.x`, which the recordings keep as the
/// write named it.
fn debugger(path: &str) -> Option<String> {
    (path == "Main.x").then(|| String::from("Main.Level"))
}

#[test]
fn a_faults_trace_is_served_byte_for_byte_as_its_file() {
    // RU-59, "Memory" and "The flight recorder": a runtime serves the fault's trace part by part
    // from the records of the recordings it joins, never the whole file, and the file it serves is
    // the one the flight recorder writes (Appendix C). Worked by hand from the first test's cycles:
    // n = 2, cycles at 0 to 40, a screen's write before 20 (in the full recording) and one before
    // 40 (in the faulted one), and the fault at 40; the joined trace holds 20, 30 and 40, and the
    // debugger names both writes `Main.Level`. Read in parts of 1, 8 and 64 bytes and in one part,
    // it gives exactly the bytes of `fault_file_named` with the same names, and its state is
    // `faulted` with 3 cycles, the file's length and its SHA-256. Before the fault there is no trace
    // to serve, and asking takes nothing from the recordings: the trace still holds 20 and 30. A
    // fault in the first recording (n = 3, cycles at 0 and 10, the second faulted) is served alike
    // from that recording alone: 2 cycles. Catches a joined trace served from one recording's
    // records, the later recording's cycles unnamed or out of place, a trace that starts from the
    // wrong recording, and a trace served before a fault or one that empties the recordings.
    let mut flight = FlightRecorder::arm(2, sizes(), Budget::COMPUTER).expect("armed");
    cycle(&mut flight, 0, None);
    cycle(&mut flight, 10, None);
    flight.recorder().change(ChangeSource::Screen, &WRITE);
    cycle(&mut flight, 20, None);
    cycle(&mut flight, 30, None);
    assert!(flight
        .fault_served(&identity(), &[], image(), &mut debugger)
        .is_none());
    flight.recorder().change(ChangeSource::Screen, &WRITE);
    assert_eq!(cycle(&mut flight, 40, Some(fault())), FlightCycle::Faulted);
    let file = flight
        .fault_file_named(&identity(), &[], &image(), &mut debugger)
        .expect("a fault's trace");
    let trace = decode(&file).expect("a trace").trace;
    assert_eq!(times(&trace), [20, 30, 40]);
    let written: Vec<&str> = trace
        .cycles
        .iter()
        .flat_map(|cycle| &cycle.changes)
        .map(|change| match &change.kind {
            ChangeKind::VariableWrite { path, .. } => path.as_str(),
            other => panic!("a screen's write, not {other:?}"),
        })
        .collect();
    assert_eq!(written, ["Main.Level", "Main.Level"]);

    let served = flight
        .fault_served(&identity(), &[], image(), &mut debugger)
        .expect("a fault's trace");
    assert_eq!(
        served.state(),
        TraceState::Ended {
            reason: TraceEnd::Faulted,
            cycles: 3,
            bytes: file.len(),
            sha256: sha256(&file),
        }
    );
    for part in [1, 8, 64, file.len()] {
        assert_eq!(read_in_parts(&served, part), file, "parts of {part} bytes");
    }

    let mut first = FlightRecorder::arm(3, sizes(), Budget::COMPUTER).expect("armed");
    cycle(&mut first, 0, None);
    assert_eq!(cycle(&mut first, 10, Some(fault())), FlightCycle::Faulted);
    let file = first
        .fault_file_named(&identity(), &[], &image(), &mut debugger)
        .expect("a fault's trace");
    let served = first
        .fault_served(&identity(), &[], image(), &mut debugger)
        .expect("a fault's trace");
    assert_eq!(
        served.state(),
        TraceState::Ended {
            reason: TraceEnd::Faulted,
            cycles: 2,
            bytes: file.len(),
            sha256: sha256(&file),
        }
    );
    assert_eq!(read_in_parts(&served, 64), file);
}

#[test]
fn a_fault_in_the_first_recording_keeps_it_from_its_own_start() {
    // RU-59: before the first hand-over the fault's trace is the first recording alone. Worked by
    // hand for n = 3: cycles at 0 and 10, the second faulted, from the first start (0xEE) at cycle
    // 100. Catches a trace joined with an empty recording, or without the faulted cycle.
    let mut flight = FlightRecorder::arm(3, sizes(), Budget::COMPUTER).expect("armed");
    cycle(&mut flight, 0, None);
    assert_eq!(cycle(&mut flight, 10, Some(fault())), FlightCycle::Faulted);
    let trace = read(&flight);
    assert_eq!(trace.meta.first_cycle, 100);
    assert_eq!(times(&trace), [0, 10]);
    assert_eq!(
        trace
            .start
            .areas
            .iter()
            .find(|(id, _)| *id == 6)
            .map(|(_, bytes)| bytes.clone()),
        Some(vec![0xEE; 16])
    );
}

#[test]
fn a_recording_out_of_memory_is_not_joined_to_the_one_after_it() {
    // A recording that ended memory_full lacks the cycle it could not keep, so the fault's trace
    // starts from the next recording's own start. Worked by hand for n = 2 with room for 16 bytes
    // of changes in a cycle: the screen's write of 32 bytes in the cycle at 10 ends the first
    // recording memory_full; the second starts with the memory after 10 (10) and its cycle at 20
    // faults: the trace holds 20 alone, from cycle 120. Catches a trace that jumps over the lost
    // cycle.
    let budget = Budget {
        cycle_changes: 16,
        ..Budget::COMPUTER
    };
    let mut flight = FlightRecorder::arm(2, sizes(), budget).expect("armed");
    cycle(&mut flight, 0, None);
    flight.recorder().change(ChangeSource::Screen, &WRITE);
    assert_eq!(cycle(&mut flight, 10, None), FlightCycle::StartDue);
    cycle(&mut flight, 20, Some(fault()));
    let trace = read(&flight);
    assert_eq!(trace.meta.first_cycle, 120);
    assert_eq!(times(&trace), [20]);
}

#[test]
fn a_fault_outside_a_scan_keeps_the_cycles_before_it() {
    // RU-59: a resource that faults outside a scan, as a tool's method call can make it, ends the
    // recording with the cycles before; the fault's trace ends faulted. Worked by hand for n = 2:
    // 0 and 10 hand over, 20 is recorded, then the fault: the trace holds 0, 10 and 20.
    let mut flight = FlightRecorder::arm(2, sizes(), Budget::COMPUTER).expect("armed");
    for at in [0, 10, 20] {
        cycle(&mut flight, at, None);
    }
    flight.fault();
    let trace = read(&flight);
    assert_eq!(trace.meta.end, TraceEnd::Faulted);
    assert_eq!(times(&trace), [0, 10, 20]);
}

#[test]
fn arming_reserves_both_recordings_or_names_what_they_need() {
    // RU-59, "Memory": the flight recorder reserves its memory when the resource starts, or names
    // the bytes it needs. Its two recordings reserve twice one recording's bytes. Catches a flight
    // recorder that reserves one recording and grows into the second in a scan.
    let one = Recorder::arm(Goal::Cycles(2), sizes(), Budget::COMPUTER).expect("armed");
    let flight = FlightRecorder::arm(2, sizes(), Budget::COMPUTER).expect("armed");
    assert_eq!(flight.reserved_bytes(), 2 * one.reserved_bytes());
    let huge = Budget {
        extra: usize::MAX / 2,
        ..Budget::COMPUTER
    };
    assert!(matches!(
        FlightRecorder::arm(2, sizes(), huge),
        Err(ReserveError::InsufficientMemory { needed }) if needed == usize::MAX
    ));
}

#[test]
fn a_faulted_cycle_that_does_not_fit_still_ends_the_recording_as_the_faults() {
    // RU-59: the fault's trace is kept whatever ended the faulted recording; a faulted cycle whose
    // record does not fit ends it memory_full, with the cycles before. Worked by hand for n = 2
    // with room for 16 bytes of changes in a cycle: the cycle at 0, then a screen's write of 32
    // bytes and the cycle at 10, which faults: the fault's trace holds 0 alone and ends memory_full.
    // Catches a faulted cycle that does not fit taken for a full recording, which hands over and
    // loses the fault.
    let budget = Budget {
        cycle_changes: 16,
        ..Budget::COMPUTER
    };
    let mut flight = FlightRecorder::arm(2, sizes(), budget).expect("armed");
    cycle(&mut flight, 0, None);
    flight.recorder().change(ChangeSource::Screen, &WRITE);
    assert_eq!(cycle(&mut flight, 10, Some(fault())), FlightCycle::Faulted);
    assert!(flight.faulted());
    let trace = read(&flight);
    assert_eq!(trace.meta.end, TraceEnd::MemoryFull);
    assert_eq!(times(&trace), [0]);
}
