//! The flight recorder of a compiled program (specification 48 RU-59, "The flight recorder";
//! specification 11 §6.8.1, "Cycle traces"): the fault's trace that the control link reads.

use std::sync::Arc;

use indexmap::IndexMap;
use smol_str::SmolStr;
use trust_runtime_core::native::memory::Place;
use trust_runtime_core::native::reflect::resolve_path;
use trust_runtime_core::trace::{
    decode, Change, ChangeKind, ChangeSource, Trace, TraceEnd, TraceOrigin,
};
use trust_runtime_services::trace::requests::{TraceName, TraceState};

use super::super::trace::{lock, new_traces, SharedTraces};
use super::FlightSettings;
use crate::harness::TestHarness;

/// A program that counts its cycles into `count`, writes ten times the count to `level` at
/// `%QW0`, and divides by zero in its fifth cycle; `extra` is a global that only the tests write.
const SOURCE: &str = "
VAR_GLOBAL
    level AT %QW0 : INT;
    extra : INT;
END_VAR
PROGRAM Main
VAR
    count : INT;
    quotient : INT;
END_VAR
count := count + 1;
level := count * 10;
IF count = 5 THEN
    quotient := 100 / (count - 5);
END_IF;
END_PROGRAM
";

/// A harness of [`SOURCE`] whose runtime keeps a flight recorder of `cycles`, and its slot.
fn flown(cycles: u64) -> (TestHarness, SharedTraces) {
    let mut harness = TestHarness::from_source(SOURCE).expect("the program builds");
    let traces = new_traces();
    harness
        .runtime_mut()
        .set_flight_recorder(Some(FlightSettings {
            cycles,
            traces: Arc::clone(&traces),
            runtime_id: SmolStr::new("test"),
        }));
    (harness, traces)
}

/// The fault's trace in the slot, read back as a reader reads its file.
fn fault_trace(traces: &SharedTraces) -> Trace {
    let slot = lock(traces);
    let TraceState::Ended { bytes, .. } = slot.state(TraceName::Fault) else {
        panic!("a fault's trace");
    };
    let mut file = vec![0; bytes];
    assert_eq!(slot.part(TraceName::Fault, 0, &mut file), Some(bytes));
    decode(&file).expect("a trace").trace
}

fn levels(trace: &Trace) -> Vec<i16> {
    trace
        .cycles
        .iter()
        .filter(|cycle| cycle.fault.is_none())
        .map(|cycle| i16::from_le_bytes([cycle.outputs[0], cycle.outputs[1]]))
        .collect()
}

#[test]
fn a_fault_keeps_the_last_cycles_from_the_memory_the_cycle_before_them_left() {
    // RU-59, "The flight recorder"; specification 11 §6.8.1: the runtime keeps the last n to 2n
    // cycles, and at a fault those up to the faulted one, which `trace.read {"trace": "fault"}`
    // reads; before a fault it answers none. Worked by hand for n = 2 and five cycles, the fifth
    // dividing by zero: the first recording keeps cycles 1 and 2 and hands over, the second keeps
    // 3 and 4 and hands over, and the fifth faults in the first again. The fault's trace joins 3,
    // 4 and 5: 3 cycles, `level` 30 and 40 in the two that ended, the fifth with its fault. It
    // starts from the memory as cycle 2 left it, `count` 2, comes from the flight recorder and
    // ended faulted. Catches a trace that starts at the first cycle or at the faulted one, a
    // hand-over whose start state is taken after the next cycle, and a fault's trace before a
    // fault.
    let (mut harness, traces) = flown(2);
    for _ in 0..4 {
        harness.cycle();
    }
    assert_eq!(lock(&traces).state(TraceName::Fault), TraceState::None);
    harness.cycle();

    let trace = fault_trace(&traces);
    assert_eq!(trace.meta.origin, TraceOrigin::FlightRecorder);
    assert_eq!(trace.meta.end, TraceEnd::Faulted);
    assert_eq!(trace.cycles.len(), 3);
    assert_eq!(levels(&trace), [30, 40]);
    assert!(trace.cycles[2].fault.is_some());
    let image = harness.runtime().image.image();
    let Place::Area { area, offset } = resolve_path(&image, "Main.count")
        .expect("Main.count")
        .place
    else {
        panic!("Main.count lies in an area");
    };
    let (_, start) = trace
        .start
        .areas
        .iter()
        .find(|(id, _)| *id == u32::from(area))
        .expect("the area of Main.count in the start state");
    let at = usize::try_from(offset).unwrap();
    assert_eq!(start[at..at + 2], [2, 0]);
}

#[test]
fn a_fault_outside_a_scan_keeps_the_cycles_before_it() {
    // RU-59: a resource that faults outside a scan, as a tool's method call can make it, keeps the
    // cycles before as the fault's trace. Worked by hand for n = 2: three cycles, the first two
    // handed over, then the fault: the trace holds cycles 1 to 3, `level` 10, 20 and 30, and ended
    // faulted. Catches a fault outside a scan that keeps no trace.
    let (mut harness, traces) = flown(2);
    for _ in 0..3 {
        harness.cycle();
    }
    harness.runtime_mut().fault_flight();
    let trace = fault_trace(&traces);
    assert_eq!(trace.meta.end, TraceEnd::Faulted);
    assert_eq!(levels(&trace), [10, 20, 30]);
}

#[test]
fn a_restart_arms_the_flight_recorder_again() {
    // RU-59; specification 11 §6.8.1: the flight recorder ends with the fault's trace, and the
    // runtime arms it again at a restart, so that the next fault has a trace too; the fault's trace
    // stays readable meanwhile. Worked by hand for n = 2: five cycles fault; the flight recorder is
    // gone and the trace is kept; after a cold restart the flight recorder is armed again and the
    // trace is still there. Catches a restart that leaves the resource without its flight recorder,
    // and one that drops the fault's trace.
    let (mut harness, traces) = flown(2);
    for _ in 0..5 {
        harness.cycle();
    }
    assert!(harness.runtime().flight.is_none());
    harness
        .runtime_mut()
        .restart(crate::RestartMode::Cold)
        .expect("the restart");
    assert!(harness.runtime().flight.is_some());
    assert!(matches!(
        lock(&traces).state(TraceName::Fault),
        TraceState::Ended { cycles: 3, .. }
    ));
}

#[test]
fn a_change_between_two_scans_is_the_next_cycles_in_the_faults_trace() {
    // RU-59: a change made between two cycles is the next cycle's, in the flight recorder's
    // recordings as in a recording on demand's. Worked by hand for n = 2: the mesh writes `extra`
    // 7 between cycles 3 and 4; the fault's trace holds cycles 3, 4 and 5, and the write is cycle
    // 4's only change, from the mesh, as the bytes 07 00. Catches a flight recorder that the
    // runtime's writers between scans do not reach.
    let (mut harness, traces) = flown(2);
    for _ in 0..3 {
        harness.cycle();
    }
    harness.runtime_mut().apply_mesh_updates(&IndexMap::from([(
        SmolStr::new("extra"),
        crate::value::Value::Int(7),
    )]));
    for _ in 0..2 {
        harness.cycle();
    }
    let trace = fault_trace(&traces);
    let changes: Vec<Vec<Change>> = trace
        .cycles
        .iter()
        .map(|cycle| cycle.changes.clone())
        .collect();
    assert_eq!(
        changes,
        [
            vec![],
            vec![Change {
                source: ChangeSource::Mesh,
                kind: ChangeKind::VariableWrite {
                    path: "extra".into(),
                    type_name: "INT".into(),
                    value: vec![7, 0],
                },
            }],
            vec![],
        ]
    );
}
