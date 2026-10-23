//! Stepped scans on a chip's loop (specification 11 §6.8.1, "Stepped scans"): the loop lends its
//! stopped program to the request, which restarts it, writes the request's inputs, runs one scan
//! at the request's time without the drivers and reads the watches. The program is the board
//! probe: `led := button` in task `Scan` every 10 ms, `button` at `%IX0.0` and `led` at `%QX0.0`,
//! and `depth_result` from the call depth the global `probe_depth` selects (3 gives
//! `ProbeMiddle() + 1` = 3). Every expected value is written out here from the specification and
//! the program.

use serde_json::{json, Value};
use trust_runtime_core::scheduler::ResourceState;

use crate::tests::runner_ports::{Fixture, IoStep, TestPlatform, TestRunner};

fn step(id: u64, params: &Value) -> Value {
    json!({"id": id, "type": "cycle.step", "params": params})
}

/// Runs the probe for its first scan and stops it, as `shutdown` stops it, at 20 ms.
fn stop(fixture: &Fixture, runner: &mut TestRunner<'_>) {
    fixture.pass_at(runner, 0);
    fixture.pass_at(runner, 10);
    let _ = fixture.call_at(runner, 15, &json!({"id": 1, "type": "shutdown"}));
    fixture.pass_at(runner, 20);
    assert_eq!(runner.resource_state(), ResourceState::Stopped);
    let _ = fixture.io.take_steps();
    let _ = fixture.take_calls();
}

#[test]
fn a_running_program_takes_no_step() {
    // A running resource refuses the step as `program.install` is refused. Catches a step that
    // scans a running program between its cycles.
    let fixture = Fixture::board_probe();
    let mut runner = fixture.runner();
    // The first scan is due as the program starts; the request is answered after it.
    fixture.pass_at(&mut runner, 0);
    let reply = fixture.call(
        &mut runner,
        &step(2, &json!({"time_ns": 0, "restart": "cold"})),
    );
    assert_eq!(reply["ok"], json!(false));
    assert_eq!(reply["error_code"], json!("resource_not_running"));
    assert_eq!(runner.resource_state(), ResourceState::Running);
}

#[test]
fn a_stopped_program_steps_with_the_requests_values_and_no_driver() {
    // The first step restarts the probe cold at time zero; the scan at 10 ms runs the due task once,
    // with `button` from the input image the request wrote and `probe_depth` 3. The drivers read and
    // drive nothing, the resource stays Stopped, and the next steps go on from 10 ms. Catches a step
    // that samples or drives the drivers, a scan at the restart's time (no task is due at 0), a
    // first step without a restart, a time that runs backwards, and a resource left running.
    let fixture = Fixture::board_probe();
    let mut runner = fixture.runner();
    stop(&fixture, &mut runner);

    let first = fixture.call(&mut runner, &step(2, &json!({"time_ns": 10_000_000})));
    assert_eq!(first["error"], json!("restart required"));

    let stepped = fixture.call(
        &mut runner,
        &step(
            3,
            &json!({
                "time_ns": 10_000_000,
                "restart": "cold",
                "inputs": {"probe_depth": "INT:3"},
                "direct_inputs": {"%IX0.0": "BOOL:true"},
                "watch": ["led", "depth_result", "probe_depth"],
                "watch_direct": ["%QX0.0"],
            }),
        ),
    );
    assert_eq!(
        stepped["result"],
        json!({
            "time_ns": 10_000_000,
            "values": {
                "depth_result": {"type": "INT", "value": 3},
                "led": {"type": "BOOL", "value": true},
                "probe_depth": {"type": "INT", "value": 3},
            },
            "direct": {"%QX0.0": {"type": "BOOL", "value": true}},
            "errors": [],
        }),
        "{stepped}"
    );
    assert_eq!(fixture.io.take_steps(), [] as [IoStep; 0], "no driver");
    assert_eq!(runner.resource_state(), ResourceState::Stopped);

    let back = fixture.call(&mut runner, &step(4, &json!({"time_ns": 5_000_000})));
    assert_eq!(back["error"], json!("time before the last step"));

    let next = fixture.call(
        &mut runner,
        &step(
            5,
            &json!({
                "time_ns": 20_000_000,
                "direct_inputs": {"%IX0.0": "BOOL:false"},
                "watch": ["led"],
                "watch_direct": ["%QX0.0"],
            }),
        ),
    );
    assert_eq!(
        next["result"]["values"]["led"],
        json!({"type": "BOOL", "value": false})
    );
    assert_eq!(
        next["result"]["direct"]["%QX0.0"],
        json!({"type": "BOOL", "value": false})
    );
    assert_eq!(fixture.io.take_steps(), [] as [IoStep; 0]);
}

#[test]
fn a_step_naming_nothing_is_refused_with_the_name() {
    // Catches a name that names no variable read as a default value.
    let fixture = Fixture::board_probe();
    let mut runner = fixture.runner();
    stop(&fixture, &mut runner);
    let reply = fixture.call(
        &mut runner,
        &step(
            2,
            &json!({"time_ns": 10_000_000, "restart": "cold", "watch": ["nope"]}),
        ),
    );
    assert_eq!(
        reply["error"],
        json!("'nope' names no variable of the program")
    );
}

#[test]
fn a_step_needs_the_debug_surface() {
    // Specification 11 §6.8.1: `cycle.step` is a request of the debug surface, refused while the
    // platform keeps it off. Catches a step that a production device without debugging runs.
    let fixture = Fixture::board_probe();
    let mut runner = fixture.runner_with(TestPlatform {
        debug_enabled: false,
        ..TestPlatform::default()
    });
    stop(&fixture, &mut runner);
    let reply = fixture.call(
        &mut runner,
        &step(2, &json!({"time_ns": 10_000_000, "restart": "cold"})),
    );
    assert_eq!(reply["error"], json!("debug disabled"));
    assert_eq!(reply["error_code"], json!("feature_disabled"));
}
