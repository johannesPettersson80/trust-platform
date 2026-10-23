//! Simulation facts, speed and achieved speed of the debug simulator (specification 46, §6.1; specification 35, A2.2 to
//! A2.4). The control server is exercised over its TCP endpoint, as the other control tests are.

use std::fs;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration as StdDuration, Instant};

use serde_json::json;
use trust_runtime::control::ControlEndpoint;
use trust_runtime::value::Duration;

use super::tests_part_02::{
    assert_control_ok, control_result, reserve_loopback_addr, send_control_request,
    unique_project_root,
};
use crate::adapter::control_bridge::DebugControlServer;
use crate::adapter::core::execute_simulator_cycle;
use crate::adapter::cycle_pacing::SimulatorClock;
use crate::runtime::DebugRuntime;
use crate::session::{SimulationFacts, SourceOptionsUpdate};
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
const PHYSICAL_IO: &str =
    "[io]\ndriver = \"modbus-tcp\"\nparams = { address = \"127.0.0.1:502\" }\n";
const SCALE_TEN: &str = "[simulation]\nenabled = true\ntime_scale = 10\n";
const SCALE_TEN_PLANTS_DISABLED: &str = "[simulation]\nenabled = false\ntime_scale = 10\n";
const TEN_TIMES_WARNING: &str = "Simulation mode active (time scale x10). Not for live hardware.";

pub(super) fn source_options(root: &Path) -> SourceOptionsUpdate {
    SourceOptionsUpdate {
        root: Some(root.to_string_lossy().into_owned()),
        include_globs: Some(vec!["**/*.st".to_string()]),
        exclude_globs: Some(Vec::new()),
        ignore_pragmas: None,
    }
}

/// Writes a project with its program and the optional `io.toml` and `simulation.toml`, and loads it.
fn loaded_session(
    label: &str,
    io: Option<&str>,
    simulation: Option<&str>,
) -> (DebugSession, PathBuf) {
    let root = unique_project_root(label);
    let main = root.join("main.st");
    fs::write(&main, PROGRAM).unwrap();
    if let Some(io) = io {
        fs::write(root.join("io.toml"), io).unwrap();
    }
    if let Some(simulation) = simulation {
        fs::write(root.join("simulation.toml"), simulation).unwrap();
    }
    let mut session = DebugSession::new(
        trust_runtime::RuntimeBuilder::new()
            .build()
            .expect("admit an empty runtime"),
    );
    session.update_source_options(source_options(&root));
    session
        .reload_program(Some(main.to_string_lossy().as_ref()))
        .unwrap();
    (session, root)
}

/// Starts the control server of `adapter` at `addr`, with `auth` as its token.
pub(super) fn start_control_server(
    adapter: &mut DebugAdapter,
    root: &Path,
    addr: SocketAddr,
    auth: &str,
) {
    adapter.control_server = Some(
        DebugControlServer::start(
            adapter.session(),
            ControlEndpoint::Tcp(addr),
            Some(auth.to_string()),
            Some(root.to_path_buf()),
        )
        .unwrap(),
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

/// Runs one cycle of the simulator loop on the runtime of `session`.
fn run_one_cycle(session: &DebugSession, cycle_time: Duration) {
    let runtime_handle = session.runtime_handle();
    let simulation_handle = session.simulation_handle();
    let mut runtime = runtime_handle.lock().unwrap();
    let mut simulation = simulation_handle.lock().unwrap();
    execute_simulator_cycle(&mut runtime, &mut simulation, cycle_time);
}

/// The process time of the runtime of `session`.
fn process_time(session: &DebugSession) -> Duration {
    let runtime_handle = session.runtime_handle();
    let runtime = runtime_handle.lock().unwrap();
    runtime.current_time()
}

#[test]
fn a_simulated_target_reports_the_project_speed_when_no_physical_driver_is_attached() {
    let (session, root) = loaded_session(
        "simulated-facts",
        Some(SIMULATED_IO),
        Some(SCALE_TEN_PLANTS_DISABLED),
    );
    let mut adapter = DebugAdapter::new(session);
    let addr = reserve_loopback_addr();
    let auth = "simulation-facts-token";
    start_control_server(&mut adapter, &root, addr, auth);

    let status = control_status(addr, auth);
    assert_eq!(status["simulation_enabled"], json!(true));
    assert_eq!(status["simulation_time_scale"], json!(10));
    assert_eq!(status["simulation_mode"], json!("simulation"));
    assert_eq!(status["simulation_warning"], json!(TEN_TIMES_WARNING));
    fs::remove_dir_all(root).ok();
}

#[test]
fn a_physical_driver_makes_the_target_physical_at_speed_one() {
    let (session, root) = loaded_session("physical-facts", Some(PHYSICAL_IO), Some(SCALE_TEN));
    assert_eq!(
        session.simulation_facts(),
        SimulationFacts {
            enabled: false,
            time_scale: 1,
        }
    );
    let mut adapter = DebugAdapter::new(session);
    let addr = reserve_loopback_addr();
    let auth = "physical-facts-token";
    start_control_server(&mut adapter, &root, addr, auth);

    let status = control_status(addr, auth);
    assert_eq!(status["simulation_enabled"], json!(false));
    assert_eq!(status["simulation_time_scale"], json!(1));
    assert_eq!(status["simulation_mode"], json!("production"));
    assert_eq!(status["simulation_warning"], json!(""));
    fs::remove_dir_all(root).ok();
}

#[test]
fn a_project_without_a_simulation_file_runs_at_speed_one() {
    let (session, root) = loaded_session("no-simulation-file", None, None);
    assert_eq!(
        session.simulation_facts(),
        SimulationFacts {
            enabled: true,
            time_scale: 1,
        }
    );
    assert!(session.simulation_handle().lock().unwrap().is_none());
    fs::remove_dir_all(root).ok();
}

#[test]
fn disabled_plants_keep_the_project_speed_but_create_no_plant_controller() {
    let (session, root) = loaded_session("plants-disabled", None, Some(SCALE_TEN_PLANTS_DISABLED));
    assert_eq!(
        session.simulation_facts(),
        SimulationFacts {
            enabled: true,
            time_scale: 10,
        }
    );
    assert!(session.simulation_handle().lock().unwrap().is_none());
    fs::remove_dir_all(root).ok();
}

#[test]
fn the_simulated_driver_names_of_the_runtime_binary_keep_the_target_simulated() {
    let (mut session, root) = loaded_session("simulated-names", Some(SIMULATED_IO), None);
    let main = root.join("main.st");
    for name in ["simulated", "sim", "noop", "loopback"] {
        fs::write(
            root.join("io.toml"),
            format!("[io]\ndriver = \"{name}\"\nparams = {{}}\n"),
        )
        .unwrap();
        session
            .reload_program(Some(main.to_string_lossy().as_ref()))
            .unwrap();
        assert!(
            session.simulation_facts().enabled,
            "the driver '{name}' is simulated"
        );
    }
    fs::remove_dir_all(root).ok();
}

#[test]
fn config_set_changes_the_speed_that_the_simulator_loop_reads() {
    let (session, root) = loaded_session("config-set-simulated", Some(SIMULATED_IO), None);
    let speed = session.simulation_speed();
    let mut adapter = DebugAdapter::new(session);
    let addr = reserve_loopback_addr();
    let auth = "config-set-token";
    start_control_server(&mut adapter, &root, addr, auth);

    assert_control_ok(
        &set_time_scale(addr, auth, 10),
        "config.set simulation.time_scale",
    );
    assert_eq!(speed.get(), 10);
    assert_eq!(
        control_status(addr, auth)["simulation_time_scale"],
        json!(10)
    );
    fs::remove_dir_all(root).ok();
}

#[test]
fn config_set_is_refused_for_a_physical_target() {
    let (session, root) = loaded_session("config-set-physical", Some(PHYSICAL_IO), Some(SCALE_TEN));
    let speed = session.simulation_speed();
    let mut adapter = DebugAdapter::new(session);
    let addr = reserve_loopback_addr();
    let auth = "config-set-physical-token";
    start_control_server(&mut adapter, &root, addr, auth);

    let response = set_time_scale(addr, auth, 10);
    assert_eq!(
        response.get("ok").and_then(serde_json::Value::as_bool),
        Some(false),
        "a physical target refuses simulation.time_scale: {response}"
    );
    assert_eq!(speed.get(), 1);
    assert_eq!(
        control_status(addr, auth)["simulation_time_scale"],
        json!(1)
    );
    fs::remove_dir_all(root).ok();
}

#[test]
fn a_reload_that_attaches_a_simulated_driver_updates_what_control_status_reports() {
    let (session, root) = loaded_session("reload-facts", Some(PHYSICAL_IO), None);
    let main = root.join("main.st");
    let mut adapter = DebugAdapter::new(session);
    let addr = reserve_loopback_addr();
    let auth = "reload-facts-token";
    start_control_server(&mut adapter, &root, addr, auth);
    assert_eq!(
        control_status(addr, auth)["simulation_enabled"],
        json!(false)
    );

    fs::write(root.join("io.toml"), SIMULATED_IO).unwrap();
    adapter
        .session_mut()
        .reload_program(Some(main.to_string_lossy().as_ref()))
        .unwrap();
    adapter.sync_control_program();

    let status = control_status(addr, auth);
    assert_eq!(status["simulation_enabled"], json!(true));
    assert_eq!(status["simulation_mode"], json!("simulation"));
    fs::remove_dir_all(root).ok();
}

#[test]
fn a_reload_replaces_the_sources_that_isolated_runs_and_searches_compile() {
    // Wrong behaviour caught: the control state kept the sources the simulator started with, so after an edit and a reload,
    // Validate's isolated runs and a search compiled the program as it was before the edit (specification 35, B5.3 and C5).
    let (session, root) = loaded_session("reload-sources", Some(SIMULATED_IO), None);
    let main = root.join("main.st");
    let mut adapter = DebugAdapter::new(session);
    let addr = reserve_loopback_addr();
    let auth = "reload-sources-token";
    start_control_server(&mut adapter, &root, addr, auth);
    let edited = PROGRAM.replace("Count := Count + 1;", "Count := Count + 2;");
    fs::write(&main, &edited).unwrap();
    adapter
        .session_mut()
        .reload_program(Some(main.to_string_lossy().as_ref()))
        .unwrap();
    adapter.sync_control_program();

    let texts = adapter
        .control_server
        .as_ref()
        .expect("the control server runs")
        .source_texts();
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
    let (session, root) = loaded_session("reload-epoch", Some(SIMULATED_IO), None);
    let main = root.join("main.st");
    let mut adapter = DebugAdapter::new(session);
    let addr = reserve_loopback_addr();
    let auth = "reload-epoch-token";
    start_control_server(&mut adapter, &root, addr, auth);
    let before = control_status(addr, auth)["session_epoch"].clone();

    adapter
        .session_mut()
        .reload_program(Some(main.to_string_lossy().as_ref()))
        .unwrap();

    let after = control_status(addr, auth)["session_epoch"].clone();
    assert_ne!(
        before, after,
        "a program reload starts a new session epoch (specification 35, A2.2 and A6.7)"
    );
    fs::remove_dir_all(root).ok();
}

#[test]
fn a_speed_change_restarts_the_schedule_without_moving_process_time() {
    let (session, root) = loaded_session("speed-change", Some(SIMULATED_IO), None);
    let cycle = Duration::from_millis(10);
    let origin = Instant::now();
    let mut clock = SimulatorClock::new(StdDuration::from_millis(10), 1, None, origin);
    for step in 0..3_u64 {
        let started = origin + StdDuration::from_millis(10 * step);
        clock.begin(started, 1, false);
        run_one_cycle(&session, cycle);
        clock.next();
    }
    assert_eq!(process_time(&session), Duration::from_millis(30));

    // The speed becomes 10 at 30 ms of wall time. The change moves no process time. The next cycle starts at once,
    // and the one after it is due one paced interval (1 ms) later.
    let change = origin + StdDuration::from_millis(30);
    clock.begin(change, 10, false);
    assert_eq!(process_time(&session), Duration::from_millis(30));
    assert_eq!(clock.next(), change + StdDuration::from_millis(1));
    run_one_cycle(&session, cycle);
    assert_eq!(process_time(&session), Duration::from_millis(40));
    fs::remove_dir_all(root).ok();
}

#[test]
fn the_achieved_speed_of_the_running_simulator_is_reported_in_status() {
    let (session, root) = loaded_session("achieved-speed", Some(SIMULATED_IO), Some(SCALE_TEN));
    let mut adapter = DebugAdapter::new(session);
    let addr = reserve_loopback_addr();
    let auth = "achieved-speed-token";
    start_control_server(&mut adapter, &root, addr, auth);

    adapter.start_runner();
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
    adapter.stop_runner();

    assert!(
        achieved > 1.0 && achieved < 12.0,
        "the simulator runs at speed 10, achieved {achieved}"
    );
    fs::remove_dir_all(root).ok();
}
