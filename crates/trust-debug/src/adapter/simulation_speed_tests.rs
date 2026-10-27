//! Simulation facts, speed and achieved speed of the debug simulator (specification 46, §6.1; specification 35, A2.2 to
//! A2.4). The simulator is built and started as a launch starts it (specification 44 NX-16), and its control server is
//! exercised over its TCP endpoint, as the other control tests are. The tests need the toolchain of specification 44 §4.6
//! at `TRUST_TOOLCHAIN`.

use std::fs;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration as StdDuration, Instant};

use serde_json::json;
use trust_runtime::control::{ControlEndpoint, ControlState};
use trust_runtime::debug::RuntimeEvent;
use trust_runtime::scheduler::ResourceCommand;
use trust_runtime::simulation::SimulationRuntimeSnapshot;
use trust_runtime::value::Duration;

use super::tests_part_02::{
    assert_control_ok, control_result, launch_debug_control_session, reserve_loopback_addr,
    send_control_request, unique_project_root,
};
use crate::adapter::control_bridge::start_simulator;
use crate::adapter::simulator::Simulator;
use crate::session::SimulationFacts;
use crate::{DebugAdapter, DebugSession};

const PROGRAM: &str = r#"PROGRAM Main
VAR
    Count : INT := 0;
END_VAR
    Count := Count + 1;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM Main WITH MainTask : Main;
END_RESOURCE
END_CONFIGURATION
"#;

const SIMULATED_IO: &str = "[io]\ndriver = \"simulated\"\nparams = {}\n";
/// A physical driver; no server answers it, and it warns instead of faulting the scan (`on_error`, specification 11), so
/// the simulator keeps scanning and takes a reload.
const PHYSICAL_IO: &str = "[io]\ndriver = \"modbus-tcp\"\nparams = { address = \"127.0.0.1:502\", on_error = \"warn\" }\n";
const SCALE_TEN: &str = "[simulation]\nenabled = true\ntime_scale = 10\n";
const SCALE_TEN_PLANTS_DISABLED: &str = "[simulation]\nenabled = false\ntime_scale = 10\n";
const TEN_TIMES_WARNING: &str = "Simulation mode active (time scale x10). Not for live hardware.";

/// A project folder with `program` in `main.st`, and the optional `io.toml` and `simulation.toml`.
pub(super) fn project_with(
    label: &str,
    program: &str,
    io: Option<&str>,
    simulation: Option<&str>,
) -> PathBuf {
    let root = unique_project_root(label);
    fs::write(root.join("main.st"), program).unwrap();
    if let Some(io) = io {
        fs::write(root.join("io.toml"), io).unwrap();
    }
    if let Some(simulation) = simulation {
        fs::write(root.join("simulation.toml"), simulation).unwrap();
    }
    root
}

/// A project folder with [`PROGRAM`] and the optional `io.toml` and `simulation.toml`.
fn project(label: &str, io: Option<&str>, simulation: Option<&str>) -> PathBuf {
    project_with(label, PROGRAM, io, simulation)
}

/// The simulator of the project at `root`, whose program is `main.st`, built and started as a launch starts it, with its
/// control server at `addr`, authenticated with `auth`, and its scheduler running.
pub(super) fn simulator(root: &Path, addr: SocketAddr, auth: &str) -> (DebugSession, Simulator) {
    start_simulator(
        root,
        &root.join("main.st"),
        ControlEndpoint::Tcp(addr),
        Some(auth.to_string()),
    )
}

/// The adapter after a launch of the project at `root` whose program is `main.st`, with its control server at `addr`,
/// authenticated with `auth`: a `stReload` of this adapter changes the simulator's program online.
pub(super) fn launched(root: &Path, addr: SocketAddr, auth: &str) -> DebugAdapter {
    let mut adapter = DebugAdapter::new(DebugSession::new());
    launch_debug_control_session(&mut adapter, root, &format!("tcp://{addr}"), auth);
    adapter
}

/// The control state of the adapter's simulator.
pub(super) fn launched_state(adapter: &DebugAdapter) -> std::sync::Arc<ControlState> {
    adapter
        .simulator
        .as_ref()
        .expect("the launch runs the simulator")
        .control_server()
        .state()
}

/// The answer of the adapter to a `stReload` of the program at `program`, which succeeded.
pub(super) fn reload(adapter: &mut DebugAdapter, program: &Path) {
    let outcome = adapter.dispatch_request(crate::protocol::Request {
        seq: 1,
        message_type: crate::protocol::MessageType::Request,
        command: "stReload".to_string(),
        arguments: Some(json!({ "program": program.to_string_lossy() })),
    });
    assert_eq!(outcome.responses.len(), 1, "{:?}", outcome.responses);
    assert_eq!(
        outcome.responses[0]["success"],
        json!(true),
        "stReload failed: {}",
        outcome.responses[0]
    );
}

/// Ends the adapter's session: `disconnect` stops its simulator and closes its control server.
pub(super) fn disconnect(adapter: &mut DebugAdapter) {
    adapter.dispatch_request(crate::protocol::Request {
        seq: 2,
        message_type: crate::protocol::MessageType::Request,
        command: "disconnect".to_string(),
        arguments: Some(json!({})),
    });
    assert!(
        adapter.simulator.is_none(),
        "disconnect stops the simulator"
    );
}

/// The result of the control server's `status` request.
pub(super) fn control_status(addr: SocketAddr, auth: &str) -> serde_json::Value {
    let response = send_control_request(addr, auth, json!({ "id": 1, "type": "status" }));
    assert_control_ok(&response, "status");
    control_result(&response).clone()
}

/// The whole response to a `config.set` of the simulation speed.
fn set_time_scale(addr: SocketAddr, auth: &str, scale: u32) -> serde_json::Value {
    send_control_request(
        addr,
        auth,
        json!({ "id": 2, "type": "config.set", "params": { "simulation.time_scale": scale } }),
    )
}

/// The simulation facts the simulator runs with, which its control server reports in `status`.
fn facts(state: &ControlState) -> SimulationFacts {
    let settings = state.settings.lock().unwrap();
    SimulationFacts {
        enabled: settings.simulation.enabled,
        time_scale: settings.simulation.time_scale,
    }
}

/// The speed at which the simulator's scheduler paces its cycles: the speed of its simulation, and 1 without one, when
/// it runs on the wall clock.
fn loop_speed(state: &ControlState) -> u32 {
    state
        .resource
        .simulation_speed()
        .map_or(1, |speed| speed.get())
}

/// The simulation the simulator's scheduler runs, as it answers for it between scans; `None` when it runs no plants.
fn simulation(state: &ControlState) -> Option<SimulationRuntimeSnapshot> {
    let (sender, receiver) = std::sync::mpsc::channel();
    state
        .resource
        .send_command(ResourceCommand::SimulationSnapshot { respond_to: sender })
        .unwrap();
    receiver
        .recv_timeout(StdDuration::from_secs(2))
        .expect("the scheduler answers the simulation snapshot")
}

/// The value at the output `address` in the last I/O snapshot the simulator published.
pub(super) fn published_output(
    state: &ControlState,
    address: &str,
) -> Option<trust_runtime::value::Value> {
    let address = trust_runtime::io::IoAddress::parse(address).expect("an output address");
    let snapshot = state.io_snapshot.lock().unwrap().clone()?;
    let entry = snapshot
        .outputs
        .into_iter()
        .find(|entry| entry.address == address)?;
    match entry.value {
        trust_runtime::io::IoSnapshotValue::Value(value) => Some(value),
        _ => None,
    }
}

/// The `(cycle, process time)` of every cycle end the simulator reported, in order.
pub(super) fn cycle_ends(state: &ControlState) -> Vec<(u64, Duration)> {
    state
        .events
        .lock()
        .unwrap()
        .iter()
        .filter_map(|event| match event {
            RuntimeEvent::CycleEnd { cycle, time } => Some((*cycle, *time)),
            _ => None,
        })
        .collect()
}

/// Waits until the simulator has reported the end of the cycle `cycle`.
pub(super) fn wait_for_cycle(state: &ControlState, cycle: u64) {
    let deadline = Instant::now() + StdDuration::from_secs(10);
    while !cycle_ends(state).iter().any(|(ended, _)| *ended >= cycle) {
        assert!(
            Instant::now() < deadline,
            "the simulator did not end cycle {cycle}: {:?}",
            cycle_ends(state)
        );
        thread::sleep(StdDuration::from_millis(5));
    }
}

/// The next `count` cycle ends the simulator reports from now on: the ones it reported before are dropped, so that a
/// counter that a reload started again (specification 11 §6.7.1) is not mixed with the one before it.
pub(super) fn next_cycle_ends(state: &ControlState, count: usize) -> Vec<(u64, Duration)> {
    state.events.lock().unwrap().clear();
    let deadline = Instant::now() + StdDuration::from_secs(10);
    loop {
        let cycles = cycle_ends(state);
        if cycles.len() >= count {
            return cycles;
        }
        assert!(
            Instant::now() < deadline,
            "the simulator did not end {count} cycles: {cycles:?}"
        );
        thread::sleep(StdDuration::from_millis(5));
    }
}

/// Asserts that each cycle in `cycles` advanced process time by exactly `step` over the one before it (specification 46,
/// §6.1 item 1): consecutive cycles, `step` apart.
pub(super) fn assert_cycles_step(cycles: &[(u64, Duration)], step: Duration) {
    assert!(cycles.len() >= 2, "too few cycles ended: {cycles:?}");
    for pair in cycles.windows(2) {
        let ((first, first_time), (second, second_time)) = (pair[0], pair[1]);
        assert_eq!(second, first + 1, "cycles end in order: {cycles:?}");
        assert_eq!(
            second_time.as_nanos() - first_time.as_nanos(),
            step.as_nanos(),
            "cycle {second} follows cycle {first} by one cycle of process time: {cycles:?}"
        );
    }
}

#[test]
fn a_simulated_target_reports_the_project_speed_when_no_physical_driver_is_attached() {
    let root = project(
        "simulated-facts",
        Some(SIMULATED_IO),
        Some(SCALE_TEN_PLANTS_DISABLED),
    );
    let addr = reserve_loopback_addr();
    let auth = "simulation-facts-token";
    let (_session, simulator) = simulator(&root, addr, auth);

    let status = control_status(addr, auth);
    simulator.stop();
    assert_eq!(status["simulation_enabled"], json!(true));
    assert_eq!(status["simulation_time_scale"], json!(10));
    assert_eq!(status["simulation_mode"], json!("simulation"));
    assert_eq!(status["simulation_warning"], json!(TEN_TIMES_WARNING));
    fs::remove_dir_all(root).ok();
}

#[test]
fn a_physical_driver_makes_the_target_physical_at_speed_one() {
    let root = project("physical-facts", Some(PHYSICAL_IO), Some(SCALE_TEN));
    let addr = reserve_loopback_addr();
    let auth = "physical-facts-token";
    let (_session, simulator) = simulator(&root, addr, auth);
    let state = simulator.control_server().state();
    assert_eq!(
        facts(&state),
        SimulationFacts {
            enabled: false,
            time_scale: 1,
        }
    );

    let status = control_status(addr, auth);
    simulator.stop();
    assert_eq!(status["simulation_enabled"], json!(false));
    assert_eq!(status["simulation_time_scale"], json!(1));
    assert_eq!(status["simulation_mode"], json!("production"));
    assert_eq!(status["simulation_warning"], json!(""));
    fs::remove_dir_all(root).ok();
}

#[test]
fn a_project_without_a_simulation_file_runs_at_speed_one() {
    let root = project("no-simulation-file", None, None);
    let (_session, simulator) = simulator(&root, reserve_loopback_addr(), "no-simulation-token");
    let state = simulator.control_server().state();
    let facts = facts(&state);
    let plants = simulation(&state);
    simulator.stop();
    assert_eq!(
        facts,
        SimulationFacts {
            enabled: true,
            time_scale: 1,
        }
    );
    assert!(plants.is_none(), "no plant controller runs: {plants:?}");
    fs::remove_dir_all(root).ok();
}

#[test]
fn disabled_plants_keep_the_project_speed_but_create_no_plant_controller() {
    let root = project("plants-disabled", None, Some(SCALE_TEN_PLANTS_DISABLED));
    let (_session, simulator) = simulator(&root, reserve_loopback_addr(), "plants-disabled-token");
    let state = simulator.control_server().state();
    let facts = facts(&state);
    let plants = simulation(&state);
    simulator.stop();
    assert_eq!(
        facts,
        SimulationFacts {
            enabled: true,
            time_scale: 10,
        }
    );
    assert!(plants.is_none(), "no plant controller runs: {plants:?}");
    fs::remove_dir_all(root).ok();
}

/// Specification 13 (Reload / Hot Reload): the drivers of `io.toml` are the launch's, so each driver name is launched.
#[test]
fn the_simulated_driver_names_of_the_runtime_binary_keep_the_target_simulated() {
    let root = project("simulated-names", Some(SIMULATED_IO), None);
    for name in ["simulated", "sim", "noop", "loopback"] {
        fs::write(
            root.join("io.toml"),
            format!("[io]\ndriver = \"{name}\"\nparams = {{}}\n"),
        )
        .unwrap();
        let (_session, simulator) = simulator(&root, reserve_loopback_addr(), "names-token");
        let facts = facts(&simulator.control_server().state());
        simulator.stop();
        assert!(facts.enabled, "the driver '{name}' is simulated");
    }
    fs::remove_dir_all(root).ok();
}

#[test]
fn config_set_changes_the_speed_that_the_simulator_loop_reads() {
    let root = project("config-set-simulated", Some(SIMULATED_IO), None);
    let addr = reserve_loopback_addr();
    let auth = "config-set-token";
    let (_session, simulator) = simulator(&root, addr, auth);
    let state = simulator.control_server().state();

    assert_control_ok(
        &set_time_scale(addr, auth, 10),
        "config.set simulation.time_scale",
    );
    let speed = loop_speed(&state);
    let reported = control_status(addr, auth)["simulation_time_scale"].clone();
    simulator.stop();
    assert_eq!(speed, 10);
    assert_eq!(reported, json!(10));
    fs::remove_dir_all(root).ok();
}

#[test]
fn config_set_is_refused_for_a_physical_target() {
    let root = project("config-set-physical", Some(PHYSICAL_IO), Some(SCALE_TEN));
    let addr = reserve_loopback_addr();
    let auth = "config-set-physical-token";
    let (_session, simulator) = simulator(&root, addr, auth);
    let state = simulator.control_server().state();

    let response = set_time_scale(addr, auth, 10);
    let speed = loop_speed(&state);
    let reported = control_status(addr, auth)["simulation_time_scale"].clone();
    simulator.stop();
    assert_eq!(
        response.get("ok").and_then(serde_json::Value::as_bool),
        Some(false),
        "a physical target refuses simulation.time_scale: {response}"
    );
    assert_eq!(speed, 1);
    assert_eq!(reported, json!(1));
    fs::remove_dir_all(root).ok();
}

/// Specification 13 (Reload / Hot Reload): a reload is an online change, which keeps the drivers the launch attached, so
/// an `io.toml` that now attaches a simulated driver takes effect at the next launch, which `status` then reports.
#[test]
fn a_relaunch_that_attaches_a_simulated_driver_updates_what_control_status_reports() {
    let root = project("reload-facts", Some(PHYSICAL_IO), None);
    let main = root.join("main.st");
    let addr = reserve_loopback_addr();
    let auth = "reload-facts-token";
    let mut adapter = launched(&root, addr, auth);
    assert_eq!(
        control_status(addr, auth)["simulation_enabled"],
        json!(false)
    );

    fs::write(root.join("io.toml"), SIMULATED_IO).unwrap();
    reload(&mut adapter, &main);
    assert_eq!(
        control_status(addr, auth)["simulation_enabled"],
        json!(false),
        "a reload keeps the physical driver of the launch"
    );
    disconnect(&mut adapter);

    let addr = reserve_loopback_addr();
    let mut adapter = launched(&root, addr, auth);
    let status = control_status(addr, auth);
    disconnect(&mut adapter);
    assert_eq!(status["simulation_enabled"], json!(true));
    assert_eq!(status["simulation_mode"], json!("simulation"));
    fs::remove_dir_all(root).ok();
}

#[test]
fn a_reload_replaces_the_sources_that_isolated_runs_and_searches_compile() {
    // Wrong behaviour caught: the control state kept the sources the simulator started with, so after an edit and a reload,
    // Validate's isolated runs and a search compiled the program as it was before the edit (specification 35, B5.3 and C5).
    let root = project("reload-sources", Some(SIMULATED_IO), None);
    let main = root.join("main.st");
    let mut adapter = launched(&root, reserve_loopback_addr(), "reload-sources-token");
    let edited = PROGRAM.replace("Count := Count + 1;", "Count := Count + 2;");
    fs::write(&main, &edited).unwrap();
    reload(&mut adapter, &main);

    let texts = adapter
        .simulator
        .as_ref()
        .expect("the simulator runs")
        .control_server()
        .source_texts();
    disconnect(&mut adapter);
    assert!(
        texts
            .iter()
            .any(|text| text.contains("Count := Count + 2;")),
        "the control state holds the reloaded program: {texts:?}"
    );
    assert!(
        texts
            .iter()
            .all(|text| !text.contains("Count := Count + 1;")),
        "and no longer the program from before the edit"
    );
    fs::remove_dir_all(root).ok();
}

#[test]
fn a_reload_starts_a_new_runtime_epoch_in_control_status() {
    let root = project("reload-epoch", Some(SIMULATED_IO), None);
    let main = root.join("main.st");
    let addr = reserve_loopback_addr();
    let auth = "reload-epoch-token";
    let mut adapter = launched(&root, addr, auth);
    let before = control_status(addr, auth)["session_epoch"].clone();

    reload(&mut adapter, &main);

    let after = control_status(addr, auth)["session_epoch"].clone();
    disconnect(&mut adapter);
    assert_ne!(
        before, after,
        "a program reload starts a new session epoch (specification 35, A2.2 and A6.7)"
    );
    fs::remove_dir_all(root).ok();
}

/// Specification 46, §6.1 items 1 and 5: each executed cycle advances simulation time by exactly one cycle, the project's
/// 10 ms without a `runtime.toml` (specification 13), and a change of speed applies from the next cycle without moving
/// simulation time. That the schedule restarts at the new pace is the scheduler's own rule
/// (`scheduler/simulated_time_tests.rs`); the achieved speed below shows the simulator follows it.
#[test]
fn a_speed_change_restarts_the_schedule_without_moving_process_time() {
    let root = project("speed-change", Some(SIMULATED_IO), None);
    let addr = reserve_loopback_addr();
    let auth = "speed-change-token";
    let (_session, simulator) = simulator(&root, addr, auth);
    let state = simulator.control_server().state();
    wait_for_cycle(&state, 3);

    assert_control_ok(
        &set_time_scale(addr, auth, 10),
        "config.set simulation.time_scale",
    );
    let changed_after = cycle_ends(&state).last().map_or(0, |(cycle, _)| *cycle);
    wait_for_cycle(&state, changed_after + 30);
    simulator.stop();

    assert_cycles_step(&cycle_ends(&state), Duration::from_millis(10));
    fs::remove_dir_all(root).ok();
}

#[test]
fn the_achieved_speed_of_the_running_simulator_is_reported_in_status() {
    let root = project("achieved-speed", Some(SIMULATED_IO), Some(SCALE_TEN));
    let addr = reserve_loopback_addr();
    let auth = "achieved-speed-token";
    let (_session, simulator) = simulator(&root, addr, auth);

    let deadline = Instant::now() + StdDuration::from_secs(6);
    let achieved = loop {
        let status = control_status(addr, auth);
        if let Some(value) = status["simulation_achieved_time_scale"].as_f64() {
            break value;
        }
        assert!(
            Instant::now() < deadline,
            "no achieved speed was reported after 6 s: {status}"
        );
        thread::sleep(StdDuration::from_millis(100));
    };
    simulator.stop();

    assert!(
        achieved > 1.0 && achieved < 12.0,
        "the simulator runs at speed 10, achieved {achieved}"
    );
    fs::remove_dir_all(root).ok();
}
