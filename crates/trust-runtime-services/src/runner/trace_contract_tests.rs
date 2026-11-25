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
    // the control link, as a computer records it; a refused call changed nothing and is not
    // recorded. Worked by hand: a recording of 2 cycles records the probe's scans at 10 and 20 ms;
    // `Start` of `ISequence` of `Plant`, made at 15 ms, answers TRUE from the stand-in kernel, and
    // the same call on `Missing`, at 16 ms, is refused. The cycle at 10 ms holds no change; the
    // cycle at 20 ms holds the one call. The probe's image declares no method, so the call takes no
    // inputs (`method_call::typed`); the recorder's own tests write a call's inputs. Catches a call
    // not recorded, recorded in the cycle before, recorded when refused, and a chip that records
    // another instance or method.
    let fixture = probe();
    let mut runner = fixture.runner();
    fixture.pass_at(&mut runner, 0);
    let started = fixture.call_at(&mut runner, 0, &record(2, 2));
    assert_eq!(started["result"]["trace"], json!(1), "{started}");
    fixture.pass_at(&mut runner, 10);
    fixture
        .script
        .borrow_mut()
        .method_answers
        .push_back(Ok(PlcValue::Bool(true)));
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
            },
        }]
    );
}

#[test]
fn a_chip_refuses_a_recording_while_paused_and_keeps_no_fault_trace_yet() {
    // Specification 11 §6.8.1: `trace.record` records only while the resource runs, and refuses
    // with `resource_not_running` otherwise, as a computer does; the refused request leaves no
    // trace, so `trace.read` of trace 1 answers `unknown_trace`. A chip keeps no fault's trace until
    // it has a flight recorder (checklist G197, R2d-3), as a computer without one: `trace.read` of
    // "fault" answers `none`. Catches a chip that records while paused, or answers a trace it
    // never had.
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
    // give a recording 90,000 bytes, whatever its heap reports, has rooms of 5,625 bytes of changes
    // (1/16), 175 readings of 8 bytes (1/64), 22,500 bytes for the recording (1/4), 5,625 of forces
    // (1/16) and a value of 2,812 (1/32), against a computer's 65,536, 256, 4,194,304, 65,536 and
    // 65,536. Everything else it reserves is the same, so it reserves 4,171,804 + 59,911 + 62,724 +
    // 648 + 59,911 = 4,354,998 bytes less than a runtime that does not know its memory. Its heap
    // reports 300,000 bytes with 100,000 in use. Catches a budget of the heap's free bytes, 200,000,
    // instead of what the chip can give a recording, and one of no memory at all.
    let reserved = |room: Option<usize>| {
        let fixture = probe();
        let mut runner = fixture.runner_with(TestPlatform {
            heap: room.map(|_| HeapFacts {
                capacity_bytes: 300_000,
                used_bytes: 100_000,
                peak_bytes: 120_000,
            }),
            recording_room: room,
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
