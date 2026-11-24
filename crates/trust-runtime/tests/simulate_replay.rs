//! Replaying a cycle trace in `trust-runtime simulate` (specification 48 RU-60; specification 11
//! §6.8.1, "Replaying a trace"): a trace recorded by one simulator, uploaded to another of the same
//! build or of a changed one, which replays it and reports where its outputs differ.
//!
//! The tests need the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

mod simulator_support;

use base64::Engine as _;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use simulator_support::{decoded, project_with, Simulator};

/// A program that counts its activations in the task `Fast` of `interval_ms` and writes the count
/// times `factor` to the located output `level` at `%QW0`.
fn counting_st(factor: u32, interval_ms: u32) -> String {
    format!(
        "
VAR_GLOBAL
    level AT %QW0 : INT;
END_VAR
PROGRAM Main
VAR
    count : INT := 0;
END_VAR
count := count + 1;
level := count * {factor};
END_PROGRAM
CONFIGURATION Plant
RESOURCE Probe ON PLC
    TASK Fast (INTERVAL := T#{interval_ms}ms, PRIORITY := 1);
    PROGRAM P1 WITH Fast : Main;
END_RESOURCE
END_CONFIGURATION
"
    )
}

/// Uploads `trace` for a replay (`transfer.begin {"for": "trace"}` and its parts, specification 11
/// §6.8.1, "Replaying a trace") from request `id` on, and gives the transfer's id.
fn upload_trace(simulator: &mut Simulator, id: u64, trace: &[u8]) -> Value {
    let digest: String = Sha256::digest(trace)
        .iter()
        .fold(String::new(), |mut text, byte| {
            text.push_str(&format!("{byte:02x}"));
            text
        });
    let begun = simulator.request(
        id,
        "transfer.begin",
        json!({"bytes": trace.len(), "sha256": digest, "for": "trace"}),
    );
    let transfer = begun["transfer"].clone();
    for (index, part) in trace.chunks(4_096).enumerate() {
        simulator.request(
            id + 1 + u64::try_from(index).unwrap(),
            "transfer.part",
            json!({
                "transfer": transfer,
                "offset": index * 4_096,
                "data": base64::engine::general_purpose::STANDARD.encode(part),
                "crc32": format!("{:08x}", trust_runtime_core::crc32::checksum(part)),
            }),
        );
    }
    transfer
}

#[test]
fn a_trace_replays_in_a_simulator_of_its_build_and_shows_where_a_changed_build_differs() {
    // Specification 48 RU-60 and specification 11 §6.8.1, "Replaying a trace": a simulator takes
    // a trace before its first cycle, runs the recorded cycles on the recorded inputs with the
    // recorded tasks, and compares each cycle's outputs. Worked by hand: `count * 10` gives `level`
    // 10, 20 and 30 in the 3 recorded cycles. A simulator of the same build replays them with every
    // output matched, as the same program, and holds the last state, `count` 3. A build whose
    // program writes `count * 20`, with the same declarations, first differs at cycle 0, 10 in the
    // trace against 20, and in the 2 cycles after it too, as a changed program. A simulator that
    // has run cycles, here the replay's, refuses another replay as `simulator_started`. A build
    // whose task has a 20 ms interval matches, since the recorded activations run, not its own
    // schedule. Catches a replay that runs the build's own schedule instead of the recorded tasks,
    // compares nothing, misses the first difference or its values, or replays after a cycle.
    let project = project_with(&counting_st(10, 10));
    let mut recorder = Simulator::start(&project);
    let started = recorder.request(1, "trace.record", json!({"cycles": 3}));
    let trace_id = started["trace"].as_u64().expect("a recording's id");
    recorder.advance(2, 3);
    let (_, file) = recorder.trace(3, trace_id);
    let recorded = decoded(&file);
    assert_eq!(recorded.cycles.len(), 3);

    let mut same = Simulator::start(&project);
    let transfer = upload_trace(&mut same, 10, &file);
    let replayed = same.request(20, "simulator.replay", json!({"transfer": transfer}));
    assert_eq!(
        replayed,
        json!({"matched": true, "cycles": 3, "kind": "same_program",
            "first_difference": null, "later_differing_cycles": 0, "notes": []})
    );
    assert_eq!(same.read(21, "P1.count"), json!(3));

    // The recorded tasks run, not the build's own schedule: a build whose task `Fast` has a 20 ms
    // interval, the same declarations and logic, matches every recorded cycle.
    let slower = project_with(&counting_st(10, 20));
    let mut scheduled = Simulator::start(&slower);
    let transfer = upload_trace(&mut scheduled, 22, &file);
    let replayed = scheduled.request(29, "simulator.replay", json!({"transfer": transfer}));
    assert_eq!(replayed["matched"], json!(true), "{replayed}");
    assert_eq!(replayed["kind"], json!("changed_program"));

    let changed = project_with(&counting_st(20, 10));
    let mut other = Simulator::start(&changed);
    let transfer = upload_trace(&mut other, 30, &file);
    let replayed = other.request(40, "simulator.replay", json!({"transfer": transfer}));
    assert_eq!(replayed["matched"], json!(false), "{replayed}");
    assert_eq!(replayed["kind"], json!("changed_program"));
    assert_eq!(replayed["cycles"], json!(3));
    assert_eq!(replayed["later_differing_cycles"], json!(2));
    assert_eq!(
        replayed["first_difference"],
        json!({"cycle": 0, "time_ns": recorded.cycles[0].time_ns, "outputs": [
            {"path": "level", "address": "%QW0", "trace": 10, "build": 20}]})
    );

    let transfer = upload_trace(&mut other, 50, &file);
    let refused = other.call(&json!({"id": 60, "type": "simulator.replay",
        "params": {"transfer": transfer}}));
    assert_eq!(refused["ok"], json!(false), "{refused}");
    assert_eq!(refused["error_code"], json!("simulator_started"));
}

/// A program whose output `level` adds a global the test writes and then forces, a global forced
/// before the recording, a marker it writes with `io.write`, and the total of a method it calls;
/// its output `stamp` is the scan's `CURRENT_DT()`, and `called` the one the method read.
const CHANGED_ST: &str = "
INTERFACE IBoost
METHOD Add : BOOL
VAR_INPUT Amount : INT; END_VAR
END_METHOD
END_INTERFACE

FUNCTION_BLOCK Booster IMPLEMENTS IBoost
VAR_OUTPUT total : INT; called : LDT; END_VAR
METHOD PUBLIC Add : BOOL
VAR_INPUT Amount : INT; END_VAR
    total := total + Amount;
    called := DT_TO_LDT(CURRENT_DT());
    Add := TRUE;
END_METHOD
END_FUNCTION_BLOCK

VAR_GLOBAL
    limit : INT := 5;
    offset : INT;
    marker AT %MW2 : INT;
    level AT %QW0 : INT;
    stamp AT %QL8 : LDT;
    called AT %QL16 : LDT;
END_VAR

PROGRAM Main
VAR
    boost : Booster;
END_VAR
boost();
level := limit + offset + marker + boost.total;
stamp := DT_TO_LDT(CURRENT_DT());
called := boost.called;
END_PROGRAM

CONFIGURATION Plant
RESOURCE Probe ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM P1 WITH Fast : Main;
END_RESOURCE
END_CONFIGURATION
";

#[test]
fn a_replay_applies_the_traces_forces_changes_and_clock_readings() {
    // Specification 48 RU-60: the trace's forces start in effect; each cycle's changes from outside
    // are applied before the cycle as the runtime applied them, between the scans; the scan gets the
    // trace's wall-clock readings. Worked by hand, `level` = `limit` + `offset` + `marker` +
    // `boost.total`, with `offset` forced to 100 before the recording: 5 + 100 = 105 in the cycle at
    // 0 ms; 9 + 100 + 3 + 2 = 114 at 10 ms, after a `set` of `limit` to 9, an `io.write` of 3 at
    // `%MW2` and a `method.call` of `Add` with 2; 7 + 100 + 3 + 2 = 112 at 20 and 30 ms, `limit`
    // forced to 7. A simulator's `CURRENT_DT()` reads its logical time from 1970 (specification 11
    // §6.8.1), so `stamp` holds 0, 10, 20 and 30 ms as LDT nanoseconds, and `called` 0: the method
    // ran between the cycles at 0 and 10 ms. A simulator of the same build replays the 4 cycles with
    // every output matched and nothing noted. Catches a replay that skips the start's force, a
    // variable's write, a force, an address's write or a method call; that lets the build read its
    // own clock, which stays at 0 in a replay; and that gives a call between the scans the next
    // scan's reading (10 ms) instead of the last one played.
    let project = project_with(CHANGED_ST);
    let mut recorder = Simulator::start(&project);
    recorder.request(
        1,
        "var.force",
        json!({"target": "global:offset", "value": "100"}),
    );
    let started = recorder.request(2, "trace.record", json!({"cycles": 4}));
    let trace_id = started["trace"].as_u64().expect("a recording's id");
    recorder.advance(3, 1);
    recorder.set(4, "limit", "9");
    recorder.request(5, "io.write", json!({"address": "%MW2", "value": "3"}));
    recorder.request(
        6,
        "method.call",
        json!({"instance": "P1.boost", "interface": "IBoost", "method": "Add",
            "inputs": {"Amount": "2"}}),
    );
    recorder.advance(7, 1);
    recorder.request(
        8,
        "var.force",
        json!({"target": "global:limit", "value": "7"}),
    );
    recorder.advance(9, 2);
    let (state, file) = recorder.trace(10, trace_id);
    assert_eq!(state["reason"], json!("complete"));
    let recorded = decoded(&file);
    let long = |outputs: &[u8], at: usize| {
        u64::from_le_bytes(outputs[at..at + 8].try_into().expect("eight bytes"))
    };
    let outputs: Vec<(i16, u64, u64)> = recorded
        .cycles
        .iter()
        .map(|cycle| {
            (
                i16::from_le_bytes([cycle.outputs[0], cycle.outputs[1]]),
                long(&cycle.outputs, 8),
                long(&cycle.outputs, 16),
            )
        })
        .collect();
    assert_eq!(
        outputs,
        [
            (105, 0, 0),
            (114, 10_000_000, 0),
            (112, 20_000_000, 0),
            (112, 30_000_000, 0)
        ]
    );

    let mut replayer = Simulator::start(&project);
    let transfer = upload_trace(&mut replayer, 20, &file);
    let replayed = replayer.request(30, "simulator.replay", json!({"transfer": transfer}));
    assert_eq!(
        replayed,
        json!({"matched": true, "cycles": 4, "kind": "same_program",
            "first_difference": null, "later_differing_cycles": 0, "notes": []})
    );
}

/// A program whose output `level` adds a global and a marker, and whose output `copy` copies it.
const RELEASED_ST: &str = "
VAR_GLOBAL
    limit : INT := 5;
    marker AT %MW0 : INT;
    level AT %QW0 : INT;
    copy AT %QW2 : INT;
END_VAR

PROGRAM Main
level := limit + marker;
copy := level;
END_PROGRAM

CONFIGURATION Plant
RESOURCE Probe ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM P1 WITH Fast : Main;
END_RESOURCE
END_CONFIGURATION
";

#[test]
fn a_replay_applies_the_traces_address_forces_and_releases() {
    // Specification 48 RU-60, "Each cycle": a replay applies the forces of addresses and the
    // releases of a trace as the runtime applied them. Worked by hand, (`level`, `copy`) with
    // `level` = `limit` + `marker`: (5, 5) in the cycle at 0 ms; (8, 40) at 10 ms, with `limit`
    // forced to 7, the marker `%MW0` forced to 1 and the output `%QW2` forced to 40 over the
    // program's 8; (5, 5) at 20 and 30 ms, after the three forces are released, `limit` is set to 2
    // and 3 is written at `%MW0`. A simulator of the same build replays the 4 cycles with every
    // output matched. Catches a replay that skips an address's force (`marker` 0 and `copy` 7 at
    // 10 ms), a variable's release (the force of 7 holds over the write of 2) or an address's
    // release (the forces of 1 and 40 hold).
    let project = project_with(RELEASED_ST);
    let mut recorder = Simulator::start(&project);
    let started = recorder.request(1, "trace.record", json!({"cycles": 4}));
    let trace_id = started["trace"].as_u64().expect("a recording's id");
    recorder.advance(2, 1);
    recorder.request(
        3,
        "var.force",
        json!({"target": "global:limit", "value": "7"}),
    );
    recorder.request(4, "io.force", json!({"address": "%QW2", "value": "40"}));
    recorder.request(5, "io.force", json!({"address": "%MW0", "value": "1"}));
    recorder.advance(6, 1);
    recorder.request(7, "var.unforce", json!({"target": "global:limit"}));
    recorder.set(8, "limit", "2");
    recorder.request(9, "io.unforce", json!({"address": "%QW2"}));
    recorder.request(10, "io.unforce", json!({"address": "%MW0"}));
    recorder.request(11, "io.write", json!({"address": "%MW0", "value": "3"}));
    recorder.advance(12, 2);
    let (state, file) = recorder.trace(13, trace_id);
    assert_eq!(state["reason"], json!("complete"));
    let outputs: Vec<(i16, i16)> = decoded(&file)
        .cycles
        .iter()
        .map(|cycle| {
            let word = |at: usize| i16::from_le_bytes([cycle.outputs[at], cycle.outputs[at + 1]]);
            (word(0), word(2))
        })
        .collect();
    assert_eq!(outputs, [(5, 5), (8, 40), (5, 5), (5, 5)]);

    let mut replayer = Simulator::start(&project);
    let transfer = upload_trace(&mut replayer, 20, &file);
    let replayed = replayer.request(30, "simulator.replay", json!({"transfer": transfer}));
    assert_eq!(
        replayed,
        json!({"matched": true, "cycles": 4, "kind": "same_program",
            "first_difference": null, "later_differing_cycles": 0, "notes": []})
    );
}

/// A program whose output `level` is `limit`, a DINT, times `gain`, plus a count that grows by
/// `step` each cycle.
const BEFORE_CHANGE_ST: &str = "
VAR_GLOBAL
    limit : DINT := 5;
    gain : INT := 2;
    step : INT := 1;
    level AT %QW0 : INT;
END_VAR

PROGRAM Main
VAR
    count : INT;
END_VAR
count := count + step;
level := DINT_TO_INT(limit * gain) + count;
END_PROGRAM

CONFIGURATION Plant
RESOURCE Probe ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM P1 WITH Fast : Main;
END_RESOURCE
END_CONFIGURATION
";

/// The program changed: `limit` is an INT and `step` a DINT, `gain` is gone and a fixed 2 takes
/// its place, a new global `bias` is added, and the output at `%QW0` is named `height`.
const AFTER_CHANGE_ST: &str = "
VAR_GLOBAL
    limit : INT := 5;
    step : DINT := 1;
    bias : INT;
    height AT %QW0 : INT;
END_VAR

PROGRAM Main
VAR
    count : INT;
END_VAR
count := count + DINT_TO_INT(step);
height := limit * 2 + count + bias;
END_PROGRAM

CONFIGURATION Plant
RESOURCE Probe ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM P1 WITH Fast : Main;
END_RESOURCE
END_CONFIGURATION
";

#[test]
fn a_changed_build_takes_the_traces_values_as_an_online_change_does_and_shows_where_it_differs() {
    // Specification 48 RU-60: a build whose memory is laid out otherwise takes the trace's start
    // as an online change carries a running program's values (specification 44 §12.2), and the
    // report lists what it could not carry over; a change of a variable the build lacks is noted
    // once and skipped. Worked by hand: the recorded program runs at 0 and 10 ms before the
    // recording (`count` 2), then records 20, 30 and 40 ms, with `limit` set to 7, `gain` to 3 and
    // `step` to 2 before 30 ms: `count` is 3, 5, 7 and `level` = `limit` * `gain` + `count` = 13,
    // 26, 28. The changed build takes `count` 2, `limit` 5 converted from DINT to INT, `step` 1
    // converted from INT to DINT, and the image `%QW0` by its address; its variables are carried
    // by name, so `bias` and `height`, which the trace does not name, start at their initial values
    // (specification 11 §6.7.1). The notes come in the order of the build's symbol table, sorted
    // by canonical name (specification 44 §5.7): BIAS, HEIGHT, LIMIT, STEP, then the cycles'
    // notes. Each recorded write is read by the trace's types and given back as the literal the
    // request gave: `limit` 7, a DINT, which the INT takes, as `set limit 7` would be (the checked
    // write only widens a typed number), and `step` 2, an INT, which the DINT takes; `gain` is
    // skipped. The output at `%QW0`, `height` = `limit` * 2 + `count` + `bias`, is 13 at 20 ms,
    // then 19 at 30 ms where the trace's `level` has 26, and 21 at 40 ms; both values are read as
    // the trace's `level` is. Catches a changed build refused or started from its initial values,
    // a recorded value read by the build's types (`step`'s 2 bytes do not read as a DINT), an
    // output read by the build's types (the build has no `level`), a recorded number written typed
    // so that the INT refuses it, and a change of a missing variable written anyway.
    let mut recorder = Simulator::start(&project_with(BEFORE_CHANGE_ST));
    recorder.advance(1, 2);
    let started = recorder.request(2, "trace.record", json!({"cycles": 3}));
    let trace_id = started["trace"].as_u64().expect("a recording's id");
    recorder.advance(3, 1);
    recorder.set(4, "limit", "7");
    recorder.set(5, "gain", "3");
    recorder.set(6, "step", "2");
    recorder.advance(7, 2);
    let (state, file) = recorder.trace(8, trace_id);
    assert_eq!(state["reason"], json!("complete"));
    let levels: Vec<i16> = decoded(&file)
        .cycles
        .iter()
        .map(|cycle| i16::from_le_bytes([cycle.outputs[0], cycle.outputs[1]]))
        .collect();
    assert_eq!(levels, [13, 26, 28]);

    let mut replayer = Simulator::start(&project_with(AFTER_CHANGE_ST));
    let transfer = upload_trace(&mut replayer, 20, &file);
    let replayed = replayer.request(30, "simulator.replay", json!({"transfer": transfer}));
    assert_eq!(
        replayed,
        json!({"matched": false, "cycles": 3, "kind": "changed_program",
        "first_difference": {"cycle": 1, "time_ns": 30_000_000, "outputs": [
            {"path": "level", "address": "%QW0", "trace": 26, "build": 19}]},
        "later_differing_cycles": 1,
        "notes": [
            "bias: new variable: declared initial value",
            "height: new variable: declared initial value",
            "limit: declaration changed: the trace's value converted to it",
            "step: declaration changed: the trace's value converted to it",
            "the build has no variable gain: its changes are skipped"
        ]})
    );
}
