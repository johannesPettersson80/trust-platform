//! The slot the control link reads traces from (specification 11 §6.8.1, "Cycle traces"): the
//! flight recorder's trace of the last fault beside the recordings on demand.

use alloc::vec::Vec;

use trust_runtime_core::deployment::sha256;
use trust_runtime_core::trace::{CycleFault, TraceEnd};

use super::super::tests::{identity, image, ms, read_in_parts, sizes};
use super::super::{Budget, FlightRecorder, ServedTrace};
use super::{TraceName, TraceSlot, TraceState};

/// The trace of a flight recorder of `cycles` cycles whose last cycle, at `(cycles - 1) * 10`
/// milliseconds, faulted, served part by part, and its file.
fn faults_trace(cycles: u64) -> (ServedTrace, Vec<u8>) {
    let mut flight = FlightRecorder::arm(cycles, sizes(), Budget::COMPUTER).expect("armed");
    for cycle in 0..cycles {
        let at = i64::try_from(cycle * 10).unwrap();
        let recorder = flight.recorder();
        recorder.begin_cycle(ms(at), cycle);
        recorder.activation(0, ms(at).as_nanos());
        recorder.inputs(&[1, 0]);
        recorder.outputs(&[0, 0, 0, 0]);
        let fault = (cycle + 1 == cycles).then_some(CycleFault {
            code: 4,
            pou: 1,
            statement: 7,
        });
        flight.end_cycle(fault);
    }
    let file = flight
        .fault_file(&identity(), &[], &image())
        .expect("a fault's trace");
    let served = flight
        .fault_served(&identity(), &[], image(), &mut |_| None)
        .expect("a fault's trace");
    (served, file)
}

/// The state `trace.read` answers for a fault's trace of `cycles` cycles whose file is `file`.
fn faulted(cycles: u64, file: &[u8]) -> TraceState {
    TraceState::Ended {
        reason: TraceEnd::Faulted,
        cycles,
        bytes: file.len(),
        sha256: sha256(file),
    }
}

#[test]
fn the_slot_keeps_the_last_faults_trace_until_the_next_fault() {
    // Specification 11 §6.8.1: `{"trace": "fault"}` reads the flight recorder's trace of the last
    // fault and answers `none` before there is one; the fault's trace stays readable until the next
    // fault, whatever recordings start. Worked by hand: an empty slot answers `none` and no part;
    // after it keeps a fault's trace of 2 cycles it answers that trace's state, and its bytes from
    // offset 0 to the end, none from the end and nothing past it; the recording started next, trace
    // 1, records and leaves the fault's trace as it was; a second fault's trace, of 3 cycles,
    // replaces the first. Catches a fault's trace answered before a fault, one that a recording's
    // start drops, the first fault's trace kept after the second, and a recording's trace read for
    // `fault` or the fault's for a recording.
    let mut slot = TraceSlot::default();
    assert_eq!(slot.state(TraceName::Fault), TraceState::None);
    assert_eq!(slot.part(TraceName::Fault, 0, &mut [0; 8]), None);

    let (first, first_file) = faults_trace(2);
    slot.keep_fault(first);
    assert_eq!(slot.state(TraceName::Fault), faulted(2, &first_file));
    let mut whole = alloc::vec![0; first_file.len()];
    assert_eq!(
        slot.part(TraceName::Fault, 0, &mut whole),
        Some(first_file.len())
    );
    assert_eq!(whole, first_file);
    assert_eq!(
        slot.part(TraceName::Fault, first_file.len(), &mut [0; 8]),
        Some(0)
    );
    assert_eq!(
        slot.part(TraceName::Fault, first_file.len() + 1, &mut [0; 8]),
        None
    );

    assert_eq!(slot.begin(), 1);
    assert_eq!(
        slot.state(TraceName::Id(1)),
        TraceState::Recording { cycles: 0 }
    );
    assert_eq!(slot.part(TraceName::Id(1), 0, &mut [0; 8]), None);
    assert_eq!(slot.state(TraceName::Fault), faulted(2, &first_file));

    let (second, second_file) = faults_trace(3);
    let read = read_in_parts(&second, 64);
    slot.keep_fault(second);
    assert_eq!(read, second_file);
    assert_eq!(slot.state(TraceName::Fault), faulted(3, &second_file));
    assert_eq!(
        slot.state(TraceName::Id(1)),
        TraceState::Recording { cycles: 0 }
    );
}
