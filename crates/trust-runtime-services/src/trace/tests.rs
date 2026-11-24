//! A recording's goals, its start state and cycles, and its ends (specification 48 RU-59), each read
//! back from the file it gives (Appendix C).

use alloc::vec;
use alloc::vec::Vec;

use trust_runtime_core::trace::{decode, Change, ChangeKind, IoArea, Trace};

use super::*;

/// Areas of 2 (`%I`), 4 (`%Q`), 0, 8, 0 and 16 bytes; two tasks; a 10 ms resource; a 20 ms task.
fn sizes() -> Sizes {
    Sizes {
        areas: [2, 4, 0, 8, 0, 16],
        tasks: 2,
        cycle_interval: Duration::from_millis(10),
        task_interval: Some(Duration::from_millis(20)),
    }
}

fn identity() -> Identity<'static> {
    Identity {
        image_sha256: [0x11; 32],
        sources_hash: [0x22; 32],
        target: 3,
        runtime_version: "0.44.1",
        runtime_id: "pc-1",
        platform: "linux-x64",
        resource: "Cell",
        task: None,
        started_utc_ms: 1_795_000_000_000,
    }
}

fn image() -> ImageTables {
    ImageTables {
        areas: vec![1],
        tasks: vec![2, 2],
        iobind: vec![3],
        symbols: vec![4],
        types: vec![5],
        strings: vec![0],
    }
}

fn ms(millis: i64) -> Duration {
    Duration::from_millis(millis)
}

/// A screen's write of the `INT` `Main.x`: 8 bytes of header and 19 of data, 32 bytes padded.
const WRITE: ChangeRef<'static> = ChangeRef::VariableWrite {
    path: "Main.x",
    type_name: "INT",
    value: &[0x2A, 0],
};

/// A cycle at `at` milliseconds with the activations of `tasks`, the images given, and no change.
fn cycle(recorder: &mut Recorder, at: i64, tasks: &[u32], inputs: &[u8], outputs: &[u8]) -> bool {
    recorder.begin_cycle(ms(at), 100 + u64::try_from(at).unwrap());
    for task in tasks {
        recorder.activation(*task, ms(at).as_nanos());
    }
    recorder.inputs(inputs);
    recorder.outputs(outputs);
    recorder.end_cycle(None)
}

fn read(recorder: &Recorder) -> Trace {
    let file = recorder
        .file(&identity(), &[], &image())
        .expect("an ended recording");
    let decoded = decode(&file).expect("a trace");
    assert_eq!(decoded.damaged_cycle, None);
    decoded.trace
}

#[test]
fn a_recording_holds_its_start_and_every_cycle_as_they_ran() {
    // Catches a start state, a change, an activation, a reading or an image recorded in the wrong
    // place or cycle. Three cycles from the runtime's cycle 41, at 0, 10 and 20 ms.
    let mut recorder = Recorder::arm(Goal::Cycles(3), sizes(), Budget::COMPUTER).unwrap();
    assert_eq!(recorder.begin_cycle(ms(0), 41), Begin::First);
    for (id, fill) in [(1, 0xA0), (2, 0xB0), (4, 0x40), (6, 0x60)] {
        let area = recorder.start_area(id).expect("a reserved area");
        for (offset, byte) in area.iter_mut().enumerate() {
            *byte = fill + u8::try_from(offset).unwrap();
        }
    }
    assert!(recorder.start_area(7).is_none());
    recorder.start_force(
        ChangeSource::ControlLink,
        &ChangeRef::VariableForce {
            path: "Main.limit",
            type_name: "REAL",
            value: &21.5f32.to_le_bytes(),
        },
    );
    recorder.change(ChangeSource::Screen, &WRITE);
    recorder.activation(0, 0);
    recorder.wall_clock(1_795_000_000_123);
    recorder.inputs(&[1, 0]);
    recorder.outputs(&[0, 0, 0x2A, 0]);
    assert!(!recorder.end_cycle(None));
    assert!(!cycle(&mut recorder, 10, &[0, 1], &[0, 1], &[0; 4]));
    assert!(cycle(&mut recorder, 20, &[0], &[0, 0], &[0; 4]));
    assert_eq!(recorder.begin_cycle(ms(30), 44), Begin::Ended);

    let trace = read(&recorder);
    assert_eq!(trace.meta.end, TraceEnd::Complete);
    assert_eq!(trace.meta.cycle_count, 3);
    assert_eq!(trace.meta.first_cycle, 41);
    assert_eq!(trace.meta.cycle_interval_ns, 10_000_000);
    assert_eq!(trace.meta.origin, TraceOrigin::OnDemand);
    assert_eq!(trace.image, image());
    assert_eq!(
        trace.start.areas,
        vec![
            (1, vec![0xA0, 0xA1]),
            (2, vec![0xB0, 0xB1, 0xB2, 0xB3]),
            (3, vec![]),
            (4, (0x40..0x48).collect()),
            (5, vec![]),
            (6, (0x60..0x70).collect()),
        ]
    );
    assert_eq!(
        trace.start.forces,
        vec![Change {
            source: ChangeSource::ControlLink,
            kind: ChangeKind::VariableForce {
                path: "Main.limit".into(),
                type_name: "REAL".into(),
                value: 21.5f32.to_le_bytes().to_vec(),
            },
        }]
    );
    let first = &trace.cycles[0];
    assert_eq!(first.time_ns, 0);
    assert_eq!(first.activations, vec![Activation { task: 0, now_ns: 0 }]);
    assert_eq!(first.wall_clock_ms, vec![1_795_000_000_123]);
    assert_eq!(
        first.changes,
        vec![Change {
            source: ChangeSource::Screen,
            kind: ChangeKind::VariableWrite {
                path: "Main.x".into(),
                type_name: "INT".into(),
                value: vec![0x2A, 0],
            },
        }]
    );
    assert_eq!(
        (first.inputs.as_slice(), first.outputs.as_slice()),
        (&[1, 0][..], &[0, 0, 0x2A, 0][..])
    );
    let second = &trace.cycles[1];
    assert_eq!(second.time_ns, 10_000_000);
    assert_eq!(
        second.activations,
        vec![
            Activation {
                task: 0,
                now_ns: 10_000_000
            },
            Activation {
                task: 1,
                now_ns: 10_000_000
            }
        ]
    );
    assert!(second.changes.is_empty() && second.wall_clock_ms.is_empty());
    assert_eq!(trace.cycles[2].time_ns, 20_000_000);
}

#[test]
fn a_duration_goal_records_the_cycles_within_it() {
    // Catches a duration counted inclusively or from the wrong cycle: of the cycles at 0, 10 and 20
    // ms, those within 20 ms of the first are the first two; the one at 20 ms is past it.
    let mut recorder = Recorder::arm(Goal::Duration(ms(20)), sizes(), Budget::COMPUTER).unwrap();
    for at in [0, 10] {
        assert!(!cycle(&mut recorder, at, &[0], &[0, 0], &[0; 4]));
    }
    assert_eq!(recorder.begin_cycle(ms(20), 2), Begin::Ended);
    let trace = read(&recorder);
    assert_eq!(
        (trace.meta.end, trace.meta.cycle_count),
        (TraceEnd::Complete, 2)
    );
}

#[test]
fn a_task_goal_records_every_cycle_until_the_tasks_runs() {
    // Catches a task goal that records only the task's cycles, or counts the resource's: task 1 of
    // 20 ms runs in the cycles at 0 and 20 ms, so its two runs take three cycles of 10 ms.
    let goal = Goal::TaskRuns { task: 1, runs: 2 };
    let mut recorder = Recorder::arm(goal, sizes(), Budget::COMPUTER).unwrap();
    assert!(!cycle(&mut recorder, 0, &[0, 1], &[0, 0], &[0; 4]));
    assert!(!cycle(&mut recorder, 10, &[0], &[0, 0], &[0; 4]));
    assert!(cycle(&mut recorder, 20, &[0, 1], &[0, 0], &[0; 4]));
    let trace = read(&recorder);
    assert_eq!(
        (trace.meta.end, trace.meta.cycle_count),
        (TraceEnd::Complete, 3)
    );
    assert_eq!(trace.cycles[1].activations.len(), 1);
}

#[test]
fn a_faulted_cycle_is_the_last_and_holds_its_fault() {
    // Catches a recording that drops the faulted cycle or records on after it.
    let mut recorder = Recorder::arm(Goal::Cycles(5), sizes(), Budget::COMPUTER).unwrap();
    assert!(!cycle(&mut recorder, 0, &[0], &[0, 0], &[0; 4]));
    recorder.begin_cycle(ms(10), 1);
    recorder.activation(0, ms(10).as_nanos());
    let fault = CycleFault {
        code: 3,
        pou: 12,
        statement: 345,
    };
    assert!(recorder.end_cycle(Some(fault)));
    assert_eq!(recorder.begin_cycle(ms(20), 2), Begin::Ended);
    let trace = read(&recorder);
    assert_eq!(
        (trace.meta.end, trace.meta.cycle_count),
        (TraceEnd::Faulted, 2)
    );
    assert_eq!(trace.cycles[1].fault, Some(fault));
}

#[test]
fn a_cycle_whose_changes_outgrow_their_room_ends_the_recording_memory_full() {
    // Catches a change written past its reserved room, or a cycle kept without it. Room for 32 bytes
    // of changes a cycle holds one screen write (32 bytes), not two.
    let budget = Budget {
        cycle_changes: 32,
        ..Budget::COMPUTER
    };
    let mut recorder = Recorder::arm(Goal::Cycles(5), sizes(), budget).unwrap();
    recorder.begin_cycle(ms(0), 0);
    recorder.change(ChangeSource::Screen, &WRITE);
    assert!(!recorder.end_cycle(None));
    recorder.begin_cycle(ms(10), 1);
    recorder.change(ChangeSource::Screen, &WRITE);
    recorder.change(ChangeSource::Screen, &WRITE);
    assert!(recorder.end_cycle(None));
    let trace = read(&recorder);
    assert_eq!(
        (trace.meta.end, trace.meta.cycle_count),
        (TraceEnd::MemoryFull, 1)
    );
    assert_eq!(trace.cycles[0].changes.len(), 1);
}

#[test]
fn a_trace_without_room_for_the_next_record_ends_memory_full() {
    // Catches a record appended past the trace's reserved room. Two cycles reserve 2 x 104 bytes:
    // 24, two activations (32), the fault (16), the 2-byte input image (16) and the 4-byte output
    // image (16). A cycle of one activation and a 32-byte change takes 24 + 16 + 32 + 16 + 16 = 104;
    // the next, of two activations and that change, 120 more, which do not fit into 208.
    let budget = Budget {
        extra: 0,
        ..Budget::COMPUTER
    };
    let mut recorder = Recorder::arm(Goal::Cycles(2), sizes(), budget).unwrap();
    recorder.begin_cycle(ms(0), 0);
    recorder.change(ChangeSource::Screen, &WRITE);
    recorder.activation(0, 0);
    recorder.inputs(&[0, 0]);
    recorder.outputs(&[0; 4]);
    assert!(!recorder.end_cycle(None));
    recorder.begin_cycle(ms(10), 1);
    recorder.change(ChangeSource::Screen, &WRITE);
    recorder.activation(0, ms(10).as_nanos());
    recorder.activation(1, ms(10).as_nanos());
    recorder.inputs(&[0, 0]);
    recorder.outputs(&[0; 4]);
    assert!(recorder.end_cycle(None));
    let trace = read(&recorder);
    assert_eq!(
        (trace.meta.end, trace.meta.cycle_count),
        (TraceEnd::MemoryFull, 1)
    );
}

#[test]
fn a_recording_ended_inside_a_cycle_keeps_the_cycles_before() {
    // Catches a half-recorded cycle kept when the program pauses, stops or changes inside it.
    let mut recorder = Recorder::arm(Goal::Cycles(5), sizes(), Budget::COMPUTER).unwrap();
    assert!(!cycle(&mut recorder, 0, &[0], &[0, 0], &[0; 4]));
    recorder.begin_cycle(ms(10), 1);
    recorder.change(ChangeSource::Screen, &WRITE);
    recorder.finish(TraceEnd::Paused);
    assert!(recorder.end_cycle(None));
    let trace = read(&recorder);
    assert_eq!(
        (trace.meta.end, trace.meta.cycle_count),
        (TraceEnd::Paused, 1)
    );
}

#[test]
fn arming_more_than_can_be_reserved_is_refused_with_the_bytes_needed() {
    // Catches a recording that starts without its memory, or a refusal that names no figure
    // (specification 48 RU-59, "Memory").
    let huge = Sizes {
        areas: [usize::MAX / 4, 0, 0, 0, 0, 0],
        ..sizes()
    };
    let refused = Recorder::arm(Goal::Cycles(1), huge, Budget::COMPUTER).unwrap_err();
    let ReserveError::InsufficientMemory { needed } = refused;
    assert!(needed >= usize::MAX / 4);
    assert!(alloc::format!("{refused}").contains(&alloc::format!("{needed} bytes")));
}

#[test]
fn an_address_change_and_a_method_call_read_back_as_recorded() {
    // Catches the address and method-call kinds recorded otherwise than the file reads them.
    let mut recorder = Recorder::arm(Goal::Cycles(1), sizes(), Budget::COMPUTER).unwrap();
    recorder.begin_cycle(ms(0), 0);
    recorder.change(
        ChangeSource::ControlLink,
        &ChangeRef::AddressWrite {
            area: IoArea::Memory,
            bit: None,
            offset: 4,
            bytes: &[7, 8],
        },
    );
    recorder.change(
        ChangeSource::ControlStudio,
        &ChangeRef::MethodCall {
            instance: "Main.Heating",
            interface: "Plant.ISequence",
            method: "Start",
            inputs: &[("Setpoint", "21.5")],
        },
    );
    assert!(recorder.end_cycle(None));
    let changes: Vec<ChangeKind> = read(&recorder).cycles[0]
        .changes
        .iter()
        .map(|change| change.kind.clone())
        .collect();
    assert_eq!(
        changes,
        vec![
            ChangeKind::AddressWrite {
                area: IoArea::Memory,
                bit: None,
                offset: 4,
                bytes: vec![7, 8],
            },
            ChangeKind::MethodCall {
                instance: "Main.Heating".into(),
                interface: "Plant.ISequence".into(),
                method: "Start".into(),
                inputs: vec![("Setpoint".into(), "21.5".into())],
            },
        ]
    );
}
