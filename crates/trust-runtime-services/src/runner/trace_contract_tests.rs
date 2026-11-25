//! The device runner's cycle traces (specification 48 RU-59, "Every runtime"; specification 11
//! §6.8.1, "Cycle traces"): a chip records, keeps and serves a trace as a computer does
//! (specification 34 STATE-08), sizes the recording by its free heap, and ends a recording where a
//! computer ends it. The board probe (`led := button` every 10 ms, with `Plant.button` at `%IX0.0`
//! and `Plant.led` at `%QX0.0`) runs compiled behind the cycle port on the test chip `esp32c6` of
//! the resource `Controller`; its button reads FALSE at every scan's input sample.

use alloc::vec::Vec;

use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use serde_json::{json, Value};
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::trace::{
    decode, Activation, Change, ChangeKind, ChangeSource, IoArea, Trace, TraceEnd, TraceOrigin,
};
use trust_runtime_core::value::{self, Value as PlcValue};
use trust_runtime_core::watchdog::{WatchdogAction, WatchdogPolicy};

use crate::capabilities::HeapFacts;
use crate::metrics::{MemoryFigure, ProgramMemory};
use crate::tests::runner_platform::TestPlatform;
use crate::tests::runner_ports::{ms, Fixture, TestRunner};

/// The board probe's loop fixture, its button wired and reading FALSE, so that each scan samples
/// the input image again.
fn probe() -> Fixture {
    let fixture = Fixture::board_probe();
    fixture.io.state.borrow_mut().inputs = alloc::vec![0];
    fixture
}

/// `trace.record` of `cycles` cycles.
fn record(id: u64, cycles: u64) -> Value {
    json!({"id": id, "type": "trace.record", "params": {"cycles": cycles}})
}

/// `trace.read` of the trace `trace`'s state.
fn read(id: u64, trace: u64) -> Value {
    json!({"id": id, "type": "trace.read", "params": {"trace": trace}})
}

/// `request` of `kind` with no parameters.
fn request(id: u64, kind: &str) -> Value {
    json!({"id": id, "type": kind})
}

/// The ended trace `trace`, its file read part by part at `at` milliseconds and decoded.
fn served(fixture: &Fixture, runner: &mut TestRunner<'_>, at: u64, trace: u64) -> Trace {
    let state = fixture.call_at(runner, at, &read(90, trace));
    let bytes = state["result"]["bytes"]
        .as_u64()
        .unwrap_or_else(|| panic!("an ended trace: {state}"));
    let bytes = usize::try_from(bytes).expect("a trace of a few kilobytes");
    let mut file = Vec::new();
    while file.len() < bytes {
        let part = fixture.call_at(
            runner,
            at,
            &json!({"id": 91, "type": "trace.read", "params": {"trace": trace, "offset": file.len()}}),
        );
        let data = STANDARD
            .decode(part["result"]["data"].as_str().expect("a part's data"))
            .expect("a part's data is base64");
        assert!(!data.is_empty(), "a part short of the file's end: {part}");
        file.extend_from_slice(&data);
    }
    decode(&file).expect("the served file decodes").trace
}

/// `trace.read` of the flight recorder's trace of the last fault.
fn read_fault(id: u64) -> Value {
    json!({"id": id, "type": "trace.read", "params": {"trace": "fault"}})
}

/// The fault's trace, its file read part by part at `at` milliseconds and decoded.
fn served_fault(fixture: &Fixture, runner: &mut TestRunner<'_>, at: u64) -> Trace {
    let state = fixture.call_at(runner, at, &read_fault(92));
    let bytes = state["result"]["bytes"]
        .as_u64()
        .unwrap_or_else(|| panic!("a fault's trace: {state}"));
    let bytes = usize::try_from(bytes).expect("a trace of a few kilobytes");
    let mut file = Vec::new();
    while file.len() < bytes {
        let part = fixture.call_at(
            runner,
            at,
            &json!({"id": 93, "type": "trace.read", "params": {"trace": "fault", "offset": file.len()}}),
        );
        let data = STANDARD
            .decode(part["result"]["data"].as_str().expect("a part's data"))
            .expect("a part's data is base64");
        assert!(!data.is_empty(), "a part short of the file's end: {part}");
        file.extend_from_slice(&data);
    }
    decode(&file).expect("the served file decodes").trace
}

/// The bytes an online change of the probe takes beside it with [`give_figure`]'s memory figure
/// (specification 44 §12.3): its 3,000 bytes of areas and 400 of bookkeeping, without its stack
/// bound of 1,024 bytes, which the running program and the new one share.
const CHANGE_BYTES: usize = 3_400;

/// Gives the probe the memory figure of 3,000 bytes of areas, a stack bound of 1,024 bytes and 400
/// bytes of bookkeeping, and no retained values.
fn give_figure(fixture: &Fixture) {
    fixture.script.borrow_mut().program_memory = Some(ProgramMemory {
        figure: MemoryFigure {
            program: 0,
            persistent_block: 3_000,
            activation_stack: 1_024,
            operand_stack: 0,
            engine: 400,
            retain_capture: 0,
        },
        retains: false,
    });
}

/// A chip whose platform can give a recording `recording_room` bytes, of a heap of 300,000 with
/// 100,000 in use.
fn chip_platform(recording_room: usize) -> TestPlatform {
    TestPlatform {
        heap: Some(HeapFacts {
            capacity_bytes: 300_000,
            used_bytes: 100_000,
            peak_bytes: 120_000,
        }),
        recording_room: Some(recording_room),
        ..TestPlatform::default()
    }
}

/// An event that ends a recording on the probe's loop; it gives the time of its last pass.
type Event<'e> = &'e dyn Fn(&Fixture, &mut TestRunner<'_>) -> u64;

/// Starts a recording of 5 cycles after the probe's first scan, at 0 ms, and runs its first cycle,
/// the scan at 10 ms.
fn recording_one_cycle(fixture: &Fixture, runner: &mut TestRunner<'_>) {
    fixture.pass_at(runner, 0);
    let started = fixture.call_at(runner, 0, &record(2, 5));
    assert_eq!(started["result"]["trace"], json!(1), "{started}");
    fixture.pass_at(runner, 10);
}

#[test]
fn a_chip_records_keeps_and_serves_a_cycle_trace_as_a_computer_does() {
    // Specification 48 RU-59, "Every runtime": a chip records, keeps and serves traces as a
    // computer does. Worked by hand on the probe: after the first scan, at 0 ms, a recording of 2
    // cycles starts as trace 1, a second recording is refused busy, and a one-shot write of TRUE to
    // `%IX0.0` waits for the next scan. The scan at 10 ms takes the write once the inputs are
    // sampled: its record holds that address write from the control link, the input image [1] and
    // the output image [1]. The scan at 20 ms holds no change, [0] and [0]. The trace ends complete
    // with 2 cycles, and its file, read part by part, decodes to those cycles, task 0 activated at
    // each cycle's time, the two located variables and the chip's identity. Catches a chip that
    // refuses or ignores the request, a change or an image not recorded, a cycle at another time,
    // and a trace not served.
    let fixture = probe();
    let mut runner = fixture.runner();
    fixture.pass_at(&mut runner, 0);
    let started = fixture.call_at(&mut runner, 0, &record(2, 2));
    assert_eq!(started["ok"], json!(true), "{started}");
    assert_eq!(started["result"]["trace"], json!(1));
    assert!(started["result"]["reserved_bytes"].as_u64().unwrap() > 0);
    let busy = fixture.call_at(&mut runner, 0, &record(3, 5));
    assert_eq!(busy["error_code"], json!("trace_busy"), "{busy}");
    let write = fixture.call_at(
        &mut runner,
        0,
        &json!({"id": 4, "type": "io.write", "params": {"address": "%IX0.0", "value": "TRUE"}}),
    );
    assert_eq!(write["result"], json!({"status": "queued"}), "{write}");
    fixture.pass_at(&mut runner, 10);
    fixture.pass_at(&mut runner, 20);

    let state = fixture.call_at(&mut runner, 20, &read(5, 1));
    assert_eq!(state["result"]["state"], json!("ended"), "{state}");
    assert_eq!(state["result"]["reason"], json!("complete"));
    assert_eq!(state["result"]["cycles"], json!(2));
    let trace = served(&fixture, &mut runner, 20, 1);
    assert_eq!(trace.meta.origin, TraceOrigin::OnDemand);
    assert_eq!(trace.meta.end, TraceEnd::Complete);
    assert_eq!(
        (
            trace.meta.resource.as_str(),
            trace.meta.platform.as_str(),
            trace.meta.runtime_id.as_str()
        ),
        ("Controller", "esp32c6", "esp32c6")
    );
    let located: Vec<&str> = trace.located.iter().map(|at| at.path.as_str()).collect();
    assert_eq!(located, ["Plant.button", "Plant.led"]);
    assert_eq!(trace.cycles.len(), 2);
    let (first, second) = (&trace.cycles[0], &trace.cycles[1]);
    assert_eq!(first.time_ns, 10_000_000);
    assert_eq!(
        first.activations,
        [Activation {
            task: 0,
            now_ns: 10_000_000
        }]
    );
    assert_eq!(
        first.changes,
        [Change {
            source: ChangeSource::ControlLink,
            kind: ChangeKind::AddressWrite {
                area: IoArea::Input,
                bit: Some(0),
                offset: 0,
                bytes: alloc::vec![1],
            },
        }]
    );
    assert_eq!(
        (first.inputs.as_slice(), first.outputs.as_slice()),
        (&[1_u8][..], &[1_u8][..])
    );
    assert_eq!(second.time_ns, 20_000_000);
    assert_eq!(second.changes, Vec::new());
    assert_eq!(
        (second.inputs.as_slice(), second.outputs.as_slice()),
        (&[0_u8][..], &[0_u8][..])
    );
    let stop = fixture.call_at(
        &mut runner,
        20,
        &json!({"id": 7, "type": "trace.stop", "params": {"trace": 1}}),
    );
    assert_eq!(stop["result"]["state"], json!("ended"), "{stop}");
}

#[test]
fn a_recording_starts_from_the_programs_memory_and_the_forces_in_effect() {
    // Specification 48 RU-59: a trace's start state is the program's memory as its first cycle
    // begins, before that cycle's writes, and the forces in effect then, so that a replay starts
    // where the program was. Worked by hand on the probe: `io.force` of `%QX0.0` to TRUE at 0 ms,
    // after the first scan, then a recording of 1 cycle, which records the scan at 10 ms. Its start
    // state holds the areas as the scan at 0 ms left them: `%I` [0] and `%Q` [0], the force not yet
    // applied; the globals with `probe_depth` 1, [1, 0, 0, 0, 0, 0, 0, 0]; the instance `Plant`
    // with `button` and `led` FALSE and `depth_result` 1, [0, 0, 1, 0, 0, 0, 0, 0]; and the force
    // from the control link. Catches a start state taken after the cycle's writes (`%Q` [1]) or not
    // at all, and a force left out of it.
    let fixture = probe();
    let mut runner = fixture.runner();
    fixture.pass_at(&mut runner, 0);
    let forced = fixture.call_at(
        &mut runner,
        0,
        &json!({"id": 2, "type": "io.force", "params": {"address": "%QX0.0", "value": "TRUE"}}),
    );
    assert_eq!(forced["result"], json!({"status": "forced"}), "{forced}");
    let started = fixture.call_at(&mut runner, 0, &record(3, 1));
    assert_eq!(started["result"]["trace"], json!(1), "{started}");
    fixture.pass_at(&mut runner, 10);

    let trace = served(&fixture, &mut runner, 10, 1);
    let area = |id: u32| {
        trace
            .start
            .areas
            .iter()
            .find(|(area, _)| *area == id)
            .map(|(_, bytes)| bytes.as_slice())
    };
    assert_eq!(area(1), Some(&[0_u8][..]), "%I");
    assert_eq!(area(2), Some(&[0_u8][..]), "%Q");
    assert_eq!(area(4), Some(&[1_u8, 0, 0, 0, 0, 0, 0, 0][..]), "globals");
    assert_eq!(area(6), Some(&[0_u8, 0, 1, 0, 0, 0, 0, 0][..]), "instances");
    assert_eq!(
        trace.start.forces,
        [Change {
            source: ChangeSource::ControlLink,
            kind: ChangeKind::AddressForce {
                area: IoArea::Output,
                bit: Some(0),
                offset: 0,
                bytes: alloc::vec![1],
            },
        }]
    );
}

#[test]
fn a_chip_ends_a_recording_where_a_computer_ends_it() {
    // Specification 48 RU-59: a recording ends when it is stopped and when its resource pauses,
    // restarts, stops or faults, with the cycles before. Worked by hand: a recording of 5 cycles
    // records the probe's scan at 10 ms; then
    // - `trace.stop` at 10 ms ends it `stopped` at once, with that 1 cycle;
    // - `pause`, a warm `restart` or `shutdown` at 10 ms ends it `paused`, `restarted` or
    //   `resource_stopped` at the boundary at 20 ms, before any scan there: 1 cycle;
    // - a scan at 20 ms that fails before the program runs ends it `faulted` with the cycle
    //   before: 1 cycle;
    // - a scan at 20 ms that runs 6 ms of a 5 ms watchdog ends it `faulted` once that cycle is
    //   recorded, as the computer's overrun does, whether the watchdog halts or restarts: 2 cycles.
    // Catches an ending that is missed (the recording would still be recording), the wrong reason,
    // and a scan after the end recorded. Each event gives the time of its last pass, at which the
    // state is read: a faulted loop takes a request that arrives at its next poll's time only at
    // that poll.
    let stopped = |fixture: &Fixture, runner: &mut TestRunner<'_>| {
        let reply = fixture.call_at(
            runner,
            10,
            &json!({"id": 3, "type": "trace.stop", "params": {"trace": 1}}),
        );
        assert_eq!(reply["result"]["reason"], json!("stopped"), "{reply}");
        10
    };
    let at_boundary = |kind: &'static str| {
        move |fixture: &Fixture, runner: &mut TestRunner<'_>| {
            let line = if kind == "restart" {
                json!({"id": 3, "type": "restart", "params": {"mode": "warm"}})
            } else {
                request(3, kind)
            };
            let reply = fixture.call_at(runner, 10, &line);
            assert_eq!(reply["ok"], json!(true), "{reply}");
            fixture.pass_at(runner, 20);
            20
        }
    };
    let failing = |fixture: &Fixture, runner: &mut TestRunner<'_>| {
        fixture
            .script
            .borrow_mut()
            .failing_scans
            .push_back(RuntimeError::DivisionByZero);
        fixture.pass_at(runner, 20);
        20
    };
    let overrun = |fixture: &Fixture, runner: &mut TestRunner<'_>| {
        fixture.script.borrow_mut().scan_time = ms(6);
        fixture.pass_at(runner, 20);
        20
    };
    let pause = at_boundary("pause");
    let restart = at_boundary("restart");
    let shutdown = at_boundary("shutdown");
    let halt = WatchdogAction::SafeHalt;
    let cases: [(&str, Event<'_>, &str, u64, WatchdogAction); 7] = [
        ("trace.stop", &stopped, "stopped", 1, halt),
        ("pause", &pause, "paused", 1, halt),
        ("restart", &restart, "restarted", 1, halt),
        ("shutdown", &shutdown, "resource_stopped", 1, halt),
        ("a failed scan", &failing, "faulted", 1, halt),
        ("an overrun that halts", &overrun, "faulted", 2, halt),
        (
            "an overrun that restarts",
            &overrun,
            "faulted",
            2,
            WatchdogAction::Restart,
        ),
    ];
    for (event, run, reason, cycles, action) in cases {
        let fixture = probe();
        // Every case runs with a watchdog of 5 ms, which the loop takes at power on; only the
        // overrun's scan takes longer.
        fixture.script.borrow_mut().watchdog = WatchdogPolicy {
            enabled: true,
            timeout: value::Duration::from_millis(5),
            action,
        };
        let mut runner = fixture.runner();
        recording_one_cycle(&fixture, &mut runner);
        let at = run(&fixture, &mut runner);
        let state = fixture.call_at(&mut runner, at, &read(4, 1));
        assert_eq!(
            state["result"],
            json!({
                "state": "ended",
                "reason": reason,
                "cycles": cycles,
                "bytes": state["result"]["bytes"],
                "sha256": state["result"]["sha256"],
            }),
            "after {event}: {state}"
        );
    }
}

#[test]
fn a_chip_records_a_tools_method_call_as_its_next_cycles_change() {
    // Specification 48 RU-59: a tool's method call between scans is the next cycle's change from
    // the control link, with each reading of the wall clock it made, as a computer records it; a
    // refused call changed nothing and is not recorded. Worked by hand: a recording of 2 cycles
    // records the probe's scans at 10 and 20 ms; `Start` of `ISequence` of `Plant`, made at 15 ms,
    // reads the clock twice, 1,795,000,000,015 and 1,795,000,000,016 ms, and answers TRUE from the
    // stand-in kernel; the same call on `Missing`, at 16 ms, is refused. The cycle at 10 ms holds no
    // change; the cycle at 20 ms holds the one call, with its two readings in order. The probe's
    // image declares no method, so the call takes no inputs (`method_call::typed`); the recorder's
    // own tests write a call's inputs. Catches a call not recorded, recorded in the cycle before,
    // recorded when refused, a chip that records another instance or method, and one that keeps
    // no readings, or another call's, with it.
    let fixture = probe();
    let mut runner = fixture.runner();
    fixture.pass_at(&mut runner, 0);
    let started = fixture.call_at(&mut runner, 0, &record(2, 2));
    assert_eq!(started["result"]["trace"], json!(1), "{started}");
    fixture.pass_at(&mut runner, 10);
    {
        let mut script = fixture.script.borrow_mut();
        script.method_answers.push_back(Ok(PlcValue::Bool(true)));
        script
            .method_readings
            .push_back(alloc::vec![1_795_000_000_015, 1_795_000_000_016]);
    }
    let call = |id: u64, instance: &str| {
        json!({"id": id, "type": "method.call", "params": {
            "instance": instance, "interface": "ISequence", "method": "Start", "inputs": {}
        }})
    };
    let answered = fixture.call_at(&mut runner, 15, &call(3, "Plant"));
    assert_eq!(answered["result"], json!({"value": true}), "{answered}");
    let refused = fixture.call_at(&mut runner, 16, &call(4, "Missing"));
    assert_eq!(refused["ok"], json!(false), "{refused}");
    fixture.pass_at(&mut runner, 20);

    let trace = served(&fixture, &mut runner, 20, 1);
    assert_eq!(trace.cycles.len(), 2);
    assert_eq!(trace.cycles[0].changes, Vec::new());
    assert_eq!(
        trace.cycles[1].changes,
        [Change {
            source: ChangeSource::ControlLink,
            kind: ChangeKind::MethodCall {
                instance: "Plant".into(),
                interface: "ISequence".into(),
                method: "Start".into(),
                inputs: Vec::new(),
                readings: alloc::vec![1_795_000_000_015, 1_795_000_000_016],
            },
        }]
    );
}

#[test]
fn a_chip_refuses_a_recording_while_paused_and_has_no_fault_trace_without_a_flight_recorder() {
    // Specification 11 §6.8.1: `trace.record` records only while the resource runs, and refuses
    // with `resource_not_running` otherwise, as a computer does; the refused request leaves no
    // trace, so `trace.read` of trace 1 answers `unknown_trace`. A chip whose record keeps no
    // flight recorder has no fault's trace, as a computer without one: `trace.read` of "fault"
    // answers `none`. Catches a chip that records while paused, or answers a trace it never had.
    let fixture = probe();
    let mut runner = fixture.runner();
    fixture.pass_at(&mut runner, 0);
    let paused = fixture.call_at(&mut runner, 0, &request(2, "pause"));
    assert_eq!(paused["ok"], json!(true), "{paused}");
    fixture.pass_at(&mut runner, 10);
    let refused = fixture.call_at(&mut runner, 10, &record(3, 2));
    assert_eq!(
        refused["error_code"],
        json!("resource_not_running"),
        "{refused}"
    );
    let unknown = fixture.call_at(&mut runner, 10, &read(4, 1));
    assert_eq!(unknown["error_code"], json!("unknown_trace"), "{unknown}");
    let fault = fixture.call_at(
        &mut runner,
        10,
        &json!({"id": 5, "type": "trace.read", "params": {"trace": "fault"}}),
    );
    assert_eq!(fault["result"], json!({"state": "none"}), "{fault}");
}

#[test]
fn a_chip_sizes_a_recording_by_the_heap_it_can_give_one() {
    // Specification 48 RU-59, "Memory": a runtime that knows its memory records within it, each
    // room at most a share of the memory it can give the recording; one that does not keeps a
    // computer's rooms. Worked by hand for a recording of 2 cycles of the probe: a chip that can
    // give a recording 90,000 bytes beside an online change of the probe (its platform's room is
    // 93,400 bytes, and the change takes 3,400: see `give_figure`), whatever its heap reports, has
    // rooms of 5,625
    // bytes of changes (1/16), 175 readings of 8 bytes (1/64), 22,500 bytes for the recording
    // (1/4), 5,625 of forces (1/16) and a value of 2,812 (1/32), against a computer's 65,536, 256,
    // 4,194,304, 65,536 and 65,536. Everything else it reserves is the same, so it reserves 4,171,804 + 59,911 + 62,724 +
    // 648 + 59,911 = 4,354,998 bytes less than a runtime that does not know its memory. Its heap
    // reports 300,000 bytes with 100,000 in use. Catches a budget of the heap's free bytes, 200,000,
    // instead of what the chip can give a recording, and one of no memory at all.
    let reserved = |room: Option<usize>| {
        let fixture = probe();
        give_figure(&fixture);
        let mut runner = fixture.runner_with(TestPlatform {
            heap: room.map(|_| HeapFacts {
                capacity_bytes: 300_000,
                used_bytes: 100_000,
                peak_bytes: 120_000,
            }),
            recording_room: room.map(|room| room + CHANGE_BYTES),
            ..TestPlatform::default()
        });
        fixture.pass_at(&mut runner, 0);
        let started = fixture.call_at(&mut runner, 0, &record(2, 2));
        started["result"]["reserved_bytes"]
            .as_u64()
            .unwrap_or_else(|| panic!("a recording: {started}"))
    };
    let computer = reserved(None);
    let chip = reserved(Some(90_000));
    assert_eq!(computer - chip, 4_354_998, "{computer} and {chip}");
}

#[test]
fn a_chip_keeps_an_online_changes_memory_beside_a_recording() {
    // Specification 48 RU-59, "Memory", with specification 44 §12.3: the memory a chip has for a
    // recording is its room less what an online change of the program that runs takes beside it,
    // the program's figure less its stack bound, so that a kept trace never takes the memory the
    // change needs. Worked by hand on a chip whose platform's room is 50,000 bytes, running the
    // probe with the figure of 3,000 bytes of areas, a stack bound of 1,024 and 400 of
    // bookkeeping: a recording of 100,000 cycles needs far more and is refused, naming 50,000 less
    // 3,000 + 400 = 46,600 bytes as free. Catches a room that leaves an online change nothing
    // (50,000 free), which let a kept trace take the memory a change needed until the C6 reset
    // (checklist G315), and a room less the whole figure (45,576).
    let fixture = probe();
    give_figure(&fixture);
    let mut runner = fixture.runner_with(chip_platform(50_000));
    fixture.pass_at(&mut runner, 0);
    let refused = fixture.call_at(&mut runner, 0, &record(2, 100_000));
    assert_eq!(
        refused["error_code"],
        json!("insufficient_memory"),
        "{refused}"
    );
    let error = refused["error"].as_str().unwrap_or_default();
    assert!(error.ends_with(", and 46600 are free"), "{refused}");
}

#[test]
fn a_chip_serves_a_trace_in_the_parts_its_memory_can_carry() {
    // Specification 11 §6.8.1, "Cycle traces": a part carries at most 4,096 bytes, as many as the
    // runtime can answer at once, and the client follows `next_offset`. Worked by hand on a chip
    // whose parts carry 256 bytes: the parts of a recording of 2 cycles start at 0, 256, 512 and
    // so on, each 256 bytes but the last, and together they are the file its state names. Catches a
    // chip that serves 4,096-byte parts whatever its memory, whose answer outgrew the C6's working
    // memory and faulted its program (checklist G197).
    let fixture = probe();
    let mut runner = fixture.runner_with(TestPlatform {
        trace_part_bytes: 256,
        ..TestPlatform::default()
    });
    fixture.pass_at(&mut runner, 0);
    let started = fixture.call_at(&mut runner, 0, &record(2, 2));
    assert_eq!(started["result"]["trace"], json!(1), "{started}");
    fixture.pass_at(&mut runner, 10);
    fixture.pass_at(&mut runner, 20);
    let state = fixture.call_at(&mut runner, 20, &read(3, 1));
    let bytes = usize::try_from(state["result"]["bytes"].as_u64().unwrap()).unwrap();
    assert!(bytes > 512, "a file of three parts at least: {state}");
    let mut file = Vec::new();
    while file.len() < bytes {
        let part = fixture.call_at(
            &mut runner,
            20,
            &json!({"id": 4, "type": "trace.read", "params": {"trace": 1, "offset": file.len()}}),
        );
        let data = STANDARD
            .decode(part["result"]["data"].as_str().expect("a part's data"))
            .expect("a part's data is base64");
        assert_eq!(data.len(), 256.min(bytes - file.len()), "{part}");
        assert_eq!(
            part["result"]["next_offset"],
            json!(file.len() + data.len())
        );
        file.extend_from_slice(&data);
    }
    let decoded = decode(&file).expect("the parts make the file");
    assert_eq!(decoded.trace.cycles.len(), 2);
}

#[test]
fn a_chip_keeps_its_flight_recorders_trace_of_a_fault() {
    // Specification 48 RU-59, "The flight recorder", and specification 11 §6.8.1: with
    // `flight_recorder = 2` from its record, a chip always keeps the last 2 to 4 cycles and, when
    // the resource faults, those cycles as the fault's trace, which `trace.read` of "fault" serves
    // until the next fault, as a computer does. Worked by hand for the probe's scans at 0, 10, 20
    // and 30 ms and a scan at 40 that fails before the program runs: the first recording holds 0
    // and 10 and hands over; the second holds 20 and 30, complete, and hands over to the first,
    // emptied; the fault ends it with no cycle. The fault's trace joins the complete recording and
    // the empty one: 2 cycles at 20 and 30 ms, from the flight recorder, ending `faulted`, each as
    // the scan ran it: the probe's one task at the cycle's time, its button reading FALSE and its
    // led off. Before the fault there is none; a warm restart afterwards keeps it. Catches a chip
    // that keeps no flight recorder, serves the trace before a fault, joins the wrong recordings,
    // feeds the flight recorder nothing but the cycles' bounds, loses the trace at the restart, or
    // serves it as a recording on demand.
    let fixture = probe();
    let mut runner = fixture.runner_with(chip_platform(200_000));
    runner.set_flight_recorder(2);
    for at in [0, 10, 20, 30] {
        fixture.pass_at(&mut runner, at);
    }
    let before = fixture.call_at(&mut runner, 30, &read_fault(2));
    assert_eq!(before["result"], json!({"state": "none"}), "{before}");
    fixture
        .script
        .borrow_mut()
        .failing_scans
        .push_back(RuntimeError::DivisionByZero);
    fixture.pass_at(&mut runner, 40);

    let state = fixture.call_at(&mut runner, 40, &read_fault(3));
    assert_eq!(
        state["result"],
        json!({
            "state": "ended",
            "reason": "faulted",
            "cycles": 2,
            "bytes": state["result"]["bytes"],
            "sha256": state["result"]["sha256"],
        }),
        "{state}"
    );
    let trace = served_fault(&fixture, &mut runner, 40);
    assert_eq!(trace.meta.origin, TraceOrigin::FlightRecorder);
    assert_eq!(trace.meta.end, TraceEnd::Faulted);
    let times: Vec<i64> = trace
        .cycles
        .iter()
        .map(|cycle| cycle.time_ns / 1_000_000)
        .collect();
    assert_eq!(times, [20, 30]);
    for (cycle, now_ns) in trace.cycles.iter().zip([20_000_000, 30_000_000]) {
        assert_eq!(cycle.activations, [Activation { task: 0, now_ns }]);
        assert_eq!(
            (cycle.inputs.as_slice(), cycle.outputs.as_slice()),
            (&[0_u8][..], &[0_u8][..])
        );
    }

    let restarted = fixture.call_at(
        &mut runner,
        40,
        &json!({"id": 4, "type": "restart", "params": {"mode": "warm"}}),
    );
    assert_eq!(restarted["ok"], json!(true), "{restarted}");
    fixture.pass_at(&mut runner, 50);
    let kept = fixture.call_at(&mut runner, 50, &read_fault(5));
    assert_eq!(kept["result"], state["result"], "{kept}");
}

#[test]
fn a_chip_runs_without_a_flight_recorder_whose_memory_it_cannot_give() {
    // Specification 48 RU-59, "Memory", and specification 11 §6.8.1: a runtime that cannot give
    // the flight recorder's memory starts without it, and a `runtime_trace_memory` event names the
    // bytes needed and the bytes it has. Worked by hand: a flight recorder of 2^50 cycles, whose two
    // recordings need more than 2^50 bytes, on a chip that can give a recording 200,000 bytes beside
    // an online change of the probe (a room of 203,400 bytes, of which the change takes 3,400); the
    // event names free 200,000 and needed above 2^50, at the
    // program's time 0; the program runs on,
    // and after a failed scan `trace.read` of "fault" answers `none`. Catches a refusal without its
    // event, an event without the chip's figures, and a chip that stops or keeps a trace without a
    // flight recorder.
    let fixture = probe();
    give_figure(&fixture);
    let mut runner = fixture.runner_with(chip_platform(200_000 + CHANGE_BYTES));
    runner.set_flight_recorder(1 << 50);
    fixture.pass_at(&mut runner, 0);
    let events = fixture.call_at(
        &mut runner,
        0,
        &json!({"id": 2, "type": "events.tail", "params": {"limit": 10}}),
    );
    let refused = events["result"]["events"]
        .as_array()
        .and_then(|events| {
            events
                .iter()
                .find(|event| event["type"] == json!("runtime_trace_memory"))
        })
        .unwrap_or_else(|| panic!("a runtime_trace_memory event: {events}"))
        .clone();
    assert_eq!(refused["free"], json!(200_000), "{refused}");
    assert_eq!(refused["time_ns"], json!(0), "{refused}");
    assert!(
        refused["needed"]
            .as_u64()
            .is_some_and(|needed| needed > 1 << 50),
        "{refused}"
    );
    fixture.pass_at(&mut runner, 10);
    fixture
        .script
        .borrow_mut()
        .failing_scans
        .push_back(RuntimeError::DivisionByZero);
    fixture.pass_at(&mut runner, 20);
    let fault = fixture.call_at(&mut runner, 20, &read_fault(3));
    assert_eq!(fault["result"], json!({"state": "none"}), "{fault}");
}

#[test]
fn a_chips_flight_recorder_takes_what_one_recording_would() {
    // Specification 48 RU-59, "Memory": the flight recorder's two recordings each take the shares
    // of half the memory the chip has for a recording, so that together they take what one
    // recording would, and a recording on demand still fits beside them. Worked by hand for a flight
    // recorder of 2 cycles of the probe on chips that can give a recording 200,000 and 400,000
    // bytes beside an online change of the probe (rooms of 203,400 and 403,400 bytes, of which the
    // change takes 3,400): each recording's rooms are those of 100,000 and
    // of 200,000 bytes: a cycle's changes
    // 6,250 and 12,500 (1/16), its readings 195 and 256 of 8 bytes (1/64, at most 256), the
    // recording's extra room 25,000 and 50,000 (1/4), the forces 6,250 and 12,500 (1/16), a value
    // 3,125 and 6,250 (1/32), and everything else the same. So the second reserves (6,250 + 61 x 8
    // + 25,000 + 6,250 + 3,125) x 2 = 82,226 bytes more. Rooms of the whole memory would differ by
    // (12,500 + 0 + 50,000 + 12,500 + 6,250) x 2 = 162,500. Catches a flight recorder sized to the
    // whole memory, which leaves no room for a recording on demand.
    let reserved = |room: usize| {
        let fixture = probe();
        give_figure(&fixture);
        let runner = fixture.runner_with(chip_platform(room + CHANGE_BYTES));
        runner.set_flight_recorder(2);
        runner
            .shell
            .traces()
            .with(|traces| traces.flight_reserved_bytes())
            .flatten()
            .expect("an armed flight recorder")
    };
    assert_eq!(reserved(400_000) - reserved(200_000), 82_226);
}

#[test]
fn a_chip_that_records_nothing_keeps_only_a_few_words_for_its_traces() {
    // Specification 34 MEM-12: a chip measures the memory it has for programs at boot, after its
    // runner, which holds the trace state, is built. That state keeps the slot's ended trace and
    // fault trace, the recording on demand and the flight recorder boxed, so a chip that records
    // nothing keeps a few words for them: on this computer 56 bytes for the slot and 88 for the
    // traces. Inline, one served trace alone is several hundred bytes; the C6 lost 2,064 bytes of
    // its memory for programs to them (checklist G319: -888 with #783, -320 with #788, -856 with
    // #794). Catches any of the four put back inline.
    assert!(
        core::mem::size_of::<crate::trace::session::TraceSlot>() <= 64,
        "the slot takes {} bytes",
        core::mem::size_of::<crate::trace::session::TraceSlot>()
    );
    assert!(
        core::mem::size_of::<super::trace::ChipTraces>() <= 128,
        "the traces take {} bytes",
        core::mem::size_of::<super::trace::ChipTraces>()
    );
}
