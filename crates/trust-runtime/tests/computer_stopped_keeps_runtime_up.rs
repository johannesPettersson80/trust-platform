//! A computer behaves like a chip when it is stopped (specification 34 STATE-08; specification 11
//! sections 6.6, 6.8.2 and 6.9; decision Q11 on computers, owner, 18 October 2026). `shutdown` stops
//! the program and keeps the real `trust-runtime` process, with its control server and its web
//! interface, running in the Stopped state. `restart` starts the installed program again, an
//! install is accepted only while the resource is Stopped, and only a signal or the explicit exit
//! ends the process. Every test runs the real binary on a private copy of an example project.

#[path = "support/runtime_process.rs"]
mod runtime_process;

use std::fs;
use std::time::Duration;

use base64::Engine as _;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use trust_runtime_services::installation::encode_sequenced;

use runtime_process::{
    build_program, hold_for, BuiltProgram, ExampleRuntime, Runtime, CLIMB, WAIT,
};

const EXAMPLE: &str = "memory_marker_counter";
/// The statement of the example that climbs `%MW0` by one in each scan, and the one that climbs it
/// by two: the second program of the install tests.
const STEP_ONE: &str = "Counter := Counter + 1;";
const STEP_TWO: &str = "Counter := Counter + 2;";
/// The scan count that a test waits for before it stops the program, two seconds of 50 ms scans.
/// A cold restart takes the count back to zero, so a later snapshot with a lower count belongs to
/// the new run.
const SCANS_BEFORE_STOP: u64 = 40;

/// The program of the example that climbs `%MW0` by two in each scan.
fn step_two_program() -> BuiltProgram {
    build_program(EXAMPLE, &[("src/Main.st", STEP_ONE, STEP_TWO)])
}

/// Stops the program with the plain `shutdown` and waits until the runtime reports it stopped.
fn stop_the_program(runtime: &Runtime) {
    runtime.ctl_ok(&["shutdown"]);
    runtime.wait_until(WAIT, "the stopped state", || {
        runtime.status().state == "stopped"
    });
}

/// Starts the program with the `restart cold` of the operator guide and waits until it runs.
fn start_the_program(runtime: &Runtime) {
    runtime.ctl_ok(&["restart", "cold"]);
    runtime.wait_until(WAIT, "the running state", || {
        runtime.status().state == "running"
    });
}

/// The result of a control response that must have succeeded.
fn result_of(response: &Value) -> &Value {
    assert_eq!(response["ok"], json!(true), "{response}");
    &response["result"]
}

/// Lets the counter run to `SCANS_BEFORE_STOP` scans and returns the count it has reached.
fn run_until_the_counter_is_high(runtime: &Runtime) -> u64 {
    runtime
        .wait_for_snapshot(CLIMB, "the counter past the stop mark", |s| {
            s.scan() >= SCANS_BEFORE_STOP
        })
        .scan()
}

/// Proves that the running program climbs `%MW0` by `step` in each scan, by two snapshots of one
/// run: the climb between them is `step` times the scans between them. It first waits for the
/// scan counter to fall below `stale`, which only a new run of the program does, so the snapshot
/// that the stopped program left behind is never read as the new program's.
fn assert_climbs_by(runtime: &Runtime, stale: u64, step: i64) {
    let first = runtime.wait_for_snapshot(WAIT, "a snapshot of the new run", |s| s.scan() < stale);
    let second = runtime.wait_for_snapshot(WAIT, "later scans of the new run", |s| {
        s.scan() >= first.scan() + 3
    });
    let scans = second.scan() as i64 - first.scan() as i64;
    assert_eq!(
        second.int("%MW0") - first.int("%MW0"),
        step * scans,
        "the counter climbs by {step} in each scan"
    );
}

#[test]
fn shutdown_on_a_computer_keeps_the_control_server_up_in_stopped() {
    // Specification 34 STATE-08 and 11 section 6.9: `shutdown` stops the program and the runtime
    // stays up in Stopped, with its control server and its web interface; it reports
    // `keeps_control`, the program runs no more scans, and the process lives. Wrong behaviour: the
    // process ends with the program (the old behaviour), so the status request finds nobody; or
    // the program keeps scanning.
    let mut runtime = ExampleRuntime::new(EXAMPLE).web().start();
    runtime.wait_for_snapshot(CLIMB, "five scans", |s| s.scan() >= 5);

    let reply = runtime.control_request(json!({"id": 7, "type": "shutdown"}));
    assert_eq!(reply["id"], json!(7));
    assert_eq!(result_of(&reply), &json!({"status": "stopping"}));
    runtime.wait_until(WAIT, "the stopped state", || {
        runtime.status().state == "stopped"
    });

    let status = runtime.control_request(json!({"id": 8, "type": "status"}));
    let status = result_of(&status);
    assert_eq!(status["state"], json!("stopped"));
    assert_eq!(status["fault"], Value::Null, "a stop is no fault");
    assert_eq!(status["program_control"], json!("keeps_control"));

    let events = ureq::get(&format!("{}/api/events?limit=1", runtime.web_base()))
        .call()
        .expect("the web interface answers while the program is stopped")
        .body_mut()
        .read_to_string()
        .expect("read the web answer");
    let events: Value = serde_json::from_str(&events).expect("the web answer is JSON");
    assert_eq!(events["ok"], json!(true), "{events}");

    let stopped_at = runtime.scan();
    hold_for(Duration::from_millis(300));
    assert_eq!(
        runtime.scan(),
        stopped_at,
        "a stopped program runs no scan\n{}",
        runtime.diagnostics()
    );
    assert!(
        runtime.is_running(),
        "the process is alive\n{}",
        runtime.diagnostics()
    );
}

#[test]
fn restart_after_shutdown_runs_the_program_again() {
    // Specification 34 STATE-08: a cold `restart` of the stopped runtime starts the program as a
    // new process would: the scan counter begins again, the uptime and the event store begin
    // empty, and the logical time of the events begins at zero. The store keeps no event that
    // every scan makes (specification 11, the event store), so each run leaves one by a write
    // its scan refuses, which needs the debug surface enabled. Wrong behaviour: the restart is
    // refused in Stopped, the counter continues, or the records of the stopped program remain.
    let runtime = ExampleRuntime::new(EXAMPLE).debug_enabled(true).start();
    let before = run_until_the_counter_is_high(&runtime);
    refused_write_times(&runtime, 10);
    stop_the_program(&runtime);
    hold_for(Duration::from_millis(1_000));
    let stopped = runtime.control_request(json!({"id": 1, "type": "status"}));
    let uptime_stopped = result_of(&stopped)["uptime_ms"]
        .as_u64()
        .expect("a stopped runtime reports its uptime");
    let events = runtime.control_request(json!({"id": 2, "type": "events.tail"}));
    let newest_old_event = result_of(&events)["events"]
        .as_array()
        .expect("events")
        .iter()
        .filter_map(|event| event["time_ns"].as_i64())
        .max()
        .unwrap_or_else(|| panic!("the stopped program left its refused write: {events}"));

    start_the_program(&runtime);

    let first = runtime.wait_for_snapshot(WAIT, "the counter back below its old count", |s| {
        s.scan() < before
    });
    runtime.wait_for_snapshot(WAIT, "the counter climbing again", |s| {
        s.scan() > first.scan()
    });
    assert_eq!(runtime.status().fault, "none", "{}", runtime.diagnostics());
    let restarted = runtime.control_request(json!({"id": 3, "type": "status"}));
    let uptime_restarted = result_of(&restarted)["uptime_ms"]
        .as_u64()
        .expect("a running runtime reports its uptime");
    assert!(
        uptime_restarted < uptime_stopped,
        "the uptime begins again: {uptime_restarted} ms after the restart, {uptime_stopped} ms \
         while stopped"
    );
    let times = refused_write_times(&runtime, 20);
    assert!(
        !times.is_empty() && times.iter().all(|&time| time < newest_old_event),
        "the events of the stopped program are gone and the new ones begin at zero: {times:?} \
         against {newest_old_event}"
    );
}

/// Queues a write to a global the program does not declare, which the next scan refuses and the
/// event store keeps as `write_rejected` with the scan's logical time (specification 11, the event
/// store; specification 34 HOST-09), waits until the store holds it, and returns the logical
/// times of the store's events then. Requests take the numbers from `id` on.
fn refused_write_times(runtime: &Runtime, id: u64) -> Vec<i64> {
    let queued = runtime.control_request(json!({
        "id": id,
        "type": "set",
        "params": {"target": "global:NotDeclared", "value": "7"},
    }));
    result_of(&queued);
    let mut times = Vec::new();
    runtime.wait_until(WAIT, "the refused write in the event store", || {
        let events = runtime.control_request(json!({"id": id + 1, "type": "events.tail"}));
        let events = result_of(&events)["events"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        times = events
            .iter()
            .filter_map(|event| event["time_ns"].as_i64())
            .collect();
        events
            .iter()
            .any(|event| event["type"] == json!("write_rejected"))
    });
    times
}

#[test]
fn the_explicit_exit_ends_the_process() {
    // Specification 34 STATE-08 and 11 section 6.8.1: `shutdown --exit` stops the program and then
    // ends the process, with status 0 when the resource ends Stopped. Wrong behaviour: the exit
    // option is ignored and the process stays up as after a plain `shutdown`.
    let mut runtime = ExampleRuntime::new(EXAMPLE).start();
    runtime.wait_for_snapshot(WAIT, "a running scan", |_| true);

    runtime.ctl_ok(&["shutdown", "--exit"]);

    let exit = runtime.wait_for_exit(Duration::from_secs(10));
    assert!(
        exit.success(),
        "the process exits successfully: {exit}\n{}",
        runtime.diagnostics()
    );
}

#[test]
fn the_explicit_exit_ends_a_process_whose_program_is_stopped() {
    // Specification 34 STATE-08: the explicit exit ends the process also after a plain
    // `shutdown`. Wrong behaviour: the process waits in Stopped for a restart that the exit does
    // not wake.
    let mut runtime = ExampleRuntime::new(EXAMPLE).start();
    runtime.wait_for_snapshot(WAIT, "a running scan", |_| true);
    stop_the_program(&runtime);

    runtime.ctl_ok(&["shutdown", "--exit"]);

    let exit = runtime.wait_for_exit(Duration::from_secs(10));
    assert!(
        exit.success(),
        "the process exits with status 0 when the resource ends Stopped: {exit}\n{}",
        runtime.diagnostics()
    );
}

#[cfg(unix)]
#[test]
fn a_signal_still_ends_the_process() {
    // Specification 11 section 6.8: SIGTERM ends the process whether the program runs or is
    // stopped (`signal_smoke.rs` covers the running program). Wrong behaviour: the process waits
    // in Stopped for a restart and the service manager's stop signal never ends it.
    let mut runtime = ExampleRuntime::new(EXAMPLE).start();
    runtime.wait_for_snapshot(WAIT, "a running scan", |_| true);
    stop_the_program(&runtime);

    let status = std::process::Command::new("kill")
        .arg("-TERM")
        .arg(runtime.pid().to_string())
        .status()
        .expect("send SIGTERM with kill");
    assert!(status.success(), "kill -TERM failed with {status}");

    let exit = runtime.wait_for_exit(Duration::from_secs(10));
    assert!(
        exit.success(),
        "the process exits successfully after SIGTERM: {exit}\n{}",
        runtime.diagnostics()
    );
}

/// POSTs `body` to the deployment route of the web interface and returns its JSON answer.
fn post_deploy(runtime: &Runtime, body: &Value) -> Value {
    let answer = ureq::post(&format!("{}/api/deploy", runtime.web_base()))
        .header("Content-Type", "application/json")
        .send(body.to_string())
        .expect("the web interface answers a deployment")
        .body_mut()
        .read_to_string()
        .expect("read the deployment answer");
    serde_json::from_str(&answer).expect("the deployment answer is JSON")
}

#[test]
fn the_web_deploy_while_running_answers_resource_not_running() {
    // Specification 34 §11.2 (decision Q11) and 11 §6.8.2: a deployment is accepted only while the
    // resource is Stopped. While the program runs, its answer carries the message and the code
    // that `program.install` gives for the same state, and nothing is written. Wrong behaviour: the
    // running resource accepts the deployment, or the refusal is a text of the web interface alone.
    let runtime = ExampleRuntime::new(EXAMPLE).web().start();
    runtime.wait_for_snapshot(WAIT, "a running scan", |_| true);
    let program_before = fs::read(runtime.project().join("program.stbc")).expect("program.stbc");
    let step_two = step_two_program();

    let answer = post_deploy(
        &runtime,
        &json!({"program_stbc_b64": base64::engine::general_purpose::STANDARD.encode(&step_two.program)}),
    );

    assert_eq!(answer["ok"], json!(false), "{answer}");
    assert_eq!(answer["error_code"], json!("resource_not_running"));
    let install = runtime.control_request(
        json!({"id": 5, "type": "program.install", "params": {"transfer": "0000000000000000"}}),
    );
    assert_eq!(
        install["error"], answer["error"],
        "the deployment is refused with the text of `program.install`"
    );
    assert_eq!(install["error_code"], answer["error_code"]);
    assert_eq!(
        answer["error"],
        json!("resource is running and its scheduler does not accept commands")
    );
    assert_eq!(
        fs::read(runtime.project().join("program.stbc")).expect("program.stbc"),
        program_before,
        "a refused deployment writes nothing"
    );
}

#[test]
fn the_web_rollback_while_running_answers_resource_not_running() {
    // Specification 34 §11.2 (decision Q11) and 11 §6.8.2: a rollback changes which program the
    // runtime starts next, as a deployment does, so it is accepted only while the resource is
    // Stopped, with the refusal of `program.install` for the same state. Wrong behaviour: the
    // running resource changes its links, or answers the store's own error without the code.
    let runtime = ExampleRuntime::new(EXAMPLE).web().start();
    runtime.wait_for_snapshot(WAIT, "a running scan", |_| true);
    let program_before = fs::read(runtime.project().join("program.stbc")).expect("program.stbc");

    let answer = ureq::post(&format!("{}/api/rollback", runtime.web_base()))
        .header("Content-Type", "application/json")
        .send("{}")
        .expect("the web interface answers a rollback")
        .body_mut()
        .read_to_string()
        .expect("read the rollback answer");
    let answer: Value = serde_json::from_str(&answer).expect("the rollback answer is JSON");

    assert_eq!(answer["ok"], json!(false), "{answer}");
    assert_eq!(answer["error_code"], json!("resource_not_running"));
    assert_eq!(
        answer["error"],
        json!("resource is running and its scheduler does not accept commands")
    );
    assert_eq!(
        fs::read(runtime.project().join("program.stbc")).expect("program.stbc"),
        program_before,
        "a refused rollback changes nothing"
    );
}

#[test]
fn a_program_installed_while_stopped_runs_after_restart() {
    // Specification 34 §11.2 and STATE-08: a deployment installed while the resource is Stopped
    // replaces the program, and the `restart` that follows runs the new program: the counter that
    // climbed by one in each scan now climbs by two. Wrong behaviour: the restart runs the program
    // that stopped again, or the install is refused in the Stopped state.
    let runtime = ExampleRuntime::new(EXAMPLE).web().start();
    let before = run_until_the_counter_is_high(&runtime);
    let first = runtime.wait_for_snapshot(WAIT, "a later scan", |s| s.scan() > before);
    let second = runtime.wait_for_snapshot(WAIT, "a later scan", |s| s.scan() >= first.scan() + 3);
    assert_eq!(
        second.int("%MW0") - first.int("%MW0"),
        second.scan() as i64 - first.scan() as i64,
        "the installed program climbs by one in each scan"
    );
    stop_the_program(&runtime);
    let step_two = step_two_program();

    let answer = post_deploy(
        &runtime,
        &json!({"program_stbc_b64": base64::engine::general_purpose::STANDARD.encode(&step_two.program)}),
    );
    assert_eq!(answer["ok"], json!(true), "{answer}");

    let active = runtime.control_request(json!({"id": 6, "type": "program.active"}));
    let active = &result_of(&active)["active"];
    assert_eq!(
        active["program_sha256"],
        json!(hex(&Sha256::digest(&step_two.program)))
    );
    assert_eq!(
        runtime.status().state,
        "stopped",
        "an install does not start"
    );
    start_the_program(&runtime);
    assert_climbs_by(&runtime, before, 2);
}

#[test]
fn a_program_installed_with_program_install_while_stopped_runs_after_restart() {
    // Specification 34 §11.2 (decision Q11): `program.install` of a finished upload commits while
    // the resource is Stopped, which a computer could not be before, and the `restart` that
    // follows runs the new program. Wrong behaviour: the install is refused while Stopped, or the
    // restart runs the old program.
    let runtime = ExampleRuntime::new(EXAMPLE).start();
    let before = run_until_the_counter_is_high(&runtime);
    stop_the_program(&runtime);
    let step_two = step_two_program();
    let container = encode_sequenced(1, &step_two.program, &step_two.record)
        .expect("encode the container of the second program");

    let transfer = upload(&runtime, &container);
    let installed = runtime.control_request(
        json!({"id": 20, "type": "program.install", "params": {"transfer": transfer}}),
    );

    let installed = result_of(&installed);
    assert_eq!(installed["status"], json!("installed"));
    assert_eq!(
        installed["program_sha256"],
        json!(hex(&Sha256::digest(&step_two.program)))
    );
    assert_eq!(
        runtime.status().state,
        "stopped",
        "an install does not start"
    );
    start_the_program(&runtime);
    assert_climbs_by(&runtime, before, 2);
}

#[cfg(unix)]
#[test]
fn a_program_installed_into_the_store_while_stopped_runs_after_restart() {
    // Specification 34 §11.2 and STATE-08: the installed program of a store is the bundle that
    // `current` names. `program.install` while the resource is Stopped commits a new bundle and
    // moves `current` to it, `previous` keeps the bundle that ran, and the `restart` that follows
    // loads the new bundle through the link. Wrong behaviour: the restart loads the bundle that
    // `current` named when the process started, or it reads a half-switched store.
    let runtime = ExampleRuntime::new(EXAMPLE).store().start();
    let before = run_until_the_counter_is_high(&runtime);
    stop_the_program(&runtime);
    let first_bundle = fs::read_link(runtime.project()).expect("current is a link");
    let step_two = step_two_program();
    let container = encode_sequenced(1, &step_two.program, &step_two.record)
        .expect("encode the container of the second program");

    let transfer = upload(&runtime, &container);
    let installed = runtime.control_request(
        json!({"id": 20, "type": "program.install", "params": {"transfer": transfer}}),
    );

    assert_eq!(result_of(&installed)["sequence"], json!(1));
    let second_bundle = fs::read_link(runtime.project()).expect("current is still a link");
    assert_ne!(second_bundle, first_bundle, "current names the new bundle");
    let active = runtime.control_request(json!({"id": 21, "type": "program.active"}));
    let active = result_of(&active);
    assert_eq!(active["active"]["sequence"], json!(1));
    assert_eq!(active["previous"]["sequence"], json!(0));
    start_the_program(&runtime);
    assert_climbs_by(&runtime, before, 2);
}

#[test]
fn a_restart_from_stopped_reads_the_installed_cycle_interval_again() {
    // Specification 34 STATE-08: a restart that finds the resource Stopped reads the resource's own
    // configuration again, so the cycle interval that a deployment installed while stopped is the
    // one the scan keeps, and `config.get` reports it. The example scans every 50 ms; the
    // installed configuration says 100 ms, so ten scans take about a second and not half of it.
    // Wrong behaviour: the restart keeps the interval the process started with.
    let runtime = ExampleRuntime::new(EXAMPLE).web().start();
    runtime.wait_for_snapshot(WAIT, "a running scan", |_| true);
    stop_the_program(&runtime);
    let installed =
        fs::read_to_string(runtime.project().join("runtime.toml")).expect("runtime.toml");
    assert!(installed.contains("cycle_interval_ms = 50"), "{installed}");

    let answer = post_deploy(
        &runtime,
        &json!({"runtime_toml": installed.replace("cycle_interval_ms = 50", "cycle_interval_ms = 100")}),
    );
    assert_eq!(answer["ok"], json!(true), "{answer}");
    start_the_program(&runtime);

    let config = runtime.control_request(json!({"id": 30, "type": "config.get"}));
    assert_eq!(result_of(&config)["resource.cycle_interval_ms"], json!(100));
    let first = runtime.wait_for_snapshot(WAIT, "a scan of the new run", |s| s.scan() >= 2);
    let started = std::time::Instant::now();
    runtime.wait_for_snapshot(CLIMB, "ten scans later", |s| s.scan() >= first.scan() + 10);
    assert!(
        started.elapsed() >= Duration::from_millis(750),
        "ten scans at 100 ms take about a second, they took {:?}",
        started.elapsed()
    );
}

/// A `cycle.step` request `id` with `params`.
fn cycle_step(runtime: &Runtime, id: u64, params: Value) -> Value {
    runtime.control_request(json!({"id": id, "type": "cycle.step", "params": params}))
}

#[test]
fn a_stopped_computer_steps_its_program_and_loads_an_installed_one() {
    // Specification 11 section 6.8.1: a stopped computer answers `cycle.step` as a chip does. The
    // first step restarts the example cold at time zero and writes 41 into `%MW0`, where
    // `VAR_CONFIG` locates `Counter`; the scan at 50 ms runs the task `Cycle` once: `CounterLatched`
    // takes 41 and `Counter` 42, which `%MW0` then holds (as the conformance case
    // `cfm_memory_map_var_config_sync_001` expects); the bare name `Counter` is the one program's
    // variable, `P1.CounterLatched` the qualified one, and `%QW0` holds `CounterLatched`. The next
    // step goes on from 50 ms with `Counter` 42; a time before it is refused.
    // A program installed while stopped is loaded by the next cold step, whose time starts at zero
    // again: the second program's scan at 50 ms climbs `Counter` by two from zero. The resource
    // stays Stopped throughout. Wrong behaviour: the step scans a running program, starts the
    // tasks, keeps the old program after the install, or lets the time run backwards.
    let runtime = ExampleRuntime::new(EXAMPLE).debug_enabled(true).start();
    runtime.wait_for_snapshot(CLIMB, "five scans", |s| s.scan() >= 5);
    let refused = cycle_step(&runtime, 1, json!({"time_ns": 0, "restart": "cold"}));
    assert_eq!(
        refused["error_code"],
        json!("resource_not_running"),
        "{refused}"
    );
    stop_the_program(&runtime);

    let watch = json!(["Counter", "P1.CounterLatched"]);
    let first = cycle_step(
        &runtime,
        2,
        json!({
            "time_ns": 50_000_000,
            "restart": "cold",
            "direct_inputs": {"%MW0": "WORD:41"},
            "watch": watch,
            "watch_direct": ["%MW0", "%QW0"],
        }),
    );
    let first = result_of(&first);
    assert_eq!(first["time_ns"], json!(50_000_000));
    assert_eq!(
        first["values"],
        json!({
            "Counter": {"type": "INT", "value": 42},
            "P1.CounterLatched": {"type": "INT", "value": 41},
        })
    );
    assert_eq!(
        first["direct"]["%MW0"],
        json!({"type": "WORD", "value": 42}),
        "{first}"
    );
    assert_eq!(first["direct"]["%QW0"]["value"], json!(41), "{first}");
    assert_eq!(first["errors"], json!([]));

    let back = cycle_step(&runtime, 3, json!({"time_ns": 10_000_000}));
    assert_eq!(back["error"], json!("time before the last step"));
    let second = cycle_step(&runtime, 4, json!({"time_ns": 100_000_000, "watch": watch}));
    assert_eq!(
        result_of(&second)["values"]["Counter"],
        json!({"type": "INT", "value": 43})
    );
    // `run` runs the body of `P1` once outside the schedule, as a test program runs: the marker
    // image is not sampled, so `Counter` goes on from the 43 it holds.
    let ran = cycle_step(
        &runtime,
        5,
        json!({"time_ns": 200_000_000, "run": "P1", "watch": watch}),
    );
    assert_eq!(
        result_of(&ran)["values"],
        json!({
            "Counter": {"type": "INT", "value": 44},
            "P1.CounterLatched": {"type": "INT", "value": 43},
        })
    );

    let step_two = step_two_program();
    let container = encode_sequenced(1, &step_two.program, &step_two.record)
        .expect("encode the container of the second program");
    let transfer = upload(&runtime, &container);
    let installed = runtime.control_request(
        json!({"id": 20, "type": "program.install", "params": {"transfer": transfer}}),
    );
    assert_eq!(result_of(&installed)["status"], json!("installed"));
    let loaded = cycle_step(
        &runtime,
        21,
        json!({"time_ns": 50_000_000, "restart": "cold", "watch": ["Counter"]}),
    );
    assert_eq!(
        result_of(&loaded)["values"]["Counter"],
        json!({"type": "INT", "value": 2}),
        "the installed program climbs by two"
    );
    assert_eq!(runtime.status().state, "stopped", "a step starts no task");
}

/// Uploads `bytes` over the control connection in parts, and returns the finished transfer.
fn upload(runtime: &Runtime, bytes: &[u8]) -> String {
    let begin = runtime.control_request(json!({
        "id": 10,
        "type": "transfer.begin",
        "params": {"bytes": bytes.len(), "sha256": hex(&Sha256::digest(bytes))},
    }));
    let begin = result_of(&begin);
    let transfer = begin["transfer"].as_str().expect("transfer id").to_owned();
    let part_bytes = begin["part_bytes"].as_u64().expect("part size") as usize;
    for (index, part) in bytes.chunks(part_bytes).enumerate() {
        let reply = runtime.control_request(json!({
            "id": 11 + index as u64,
            "type": "transfer.part",
            "params": {
                "transfer": transfer,
                "offset": index * part_bytes,
                "data": base64::engine::general_purpose::STANDARD.encode(part),
                "crc32": hex(&crc32fast::hash(part).to_be_bytes()),
            },
        }));
        result_of(&reply);
    }
    transfer
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
