use std::collections::VecDeque;
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use indexmap::IndexMap;
use serde_json::{json, Value};
use smol_str::SmolStr;
use trust_runtime::config::{ControlMode, WebAuthMode, WebConfig};
use trust_runtime::control::{ControlState, SourceFile, SourceRegistry};
use trust_runtime::debug::DebugVariableHandles;
use trust_runtime::error::RuntimeError;
use trust_runtime::harness::TestHarness;
use trust_runtime::metrics::RuntimeMetrics;
use trust_runtime::scheduler::{ResourceCommand, ResourceControl, StdClock};
use trust_runtime::settings::{
    BaseSettings, DiscoverySettings, MeshSettings, RuntimeSettings, SimulationSettings, WebSettings,
};
use trust_runtime::watchdog::{FaultPolicy, RetainMode, WatchdogPolicy};
use trust_runtime::web::start_web_server;

fn runtime_settings() -> RuntimeSettings {
    RuntimeSettings::new(
        BaseSettings {
            log_level: SmolStr::new("info"),
            watchdog: WatchdogPolicy::default(),
            fault_policy: FaultPolicy::SafeHalt,
            retain_mode: RetainMode::None,
            retain_save_interval: None,
        },
        WebSettings {
            enabled: true,
            listen: SmolStr::new("127.0.0.1:0"),
            auth: SmolStr::new("local"),
            tls: false,
        },
        DiscoverySettings {
            enabled: false,
            service_name: SmolStr::new("truST"),
            advertise: false,
            interfaces: Vec::new(),
        },
        MeshSettings {
            enabled: false,
            listen: SmolStr::new("127.0.0.1:0"),
            tls: false,
            auth_token: None,
            publish: Vec::new(),
            subscribe: IndexMap::new(),
        },
        SimulationSettings {
            enabled: false,
            time_scale: 1,
            mode_label: SmolStr::new("production"),
            warning: SmolStr::new(""),
        },
    )
}

fn source_fixture() -> &'static str {
    "PROGRAM Main\nEND_PROGRAM\n"
}

fn control_state(source: &str) -> Arc<ControlState> {
    let mut harness = TestHarness::from_source(source).expect("build test harness");
    let debug = harness.runtime_mut().enable_debug();
    harness.cycle();

    let snapshot = trust_runtime::debug::DebugSnapshot {
        storage: harness.runtime().storage().clone(),
        now: harness.runtime().current_time(),
    };

    let (resource, cmd_rx) = ResourceControl::stub(StdClock::new());
    thread::spawn(move || {
        while let Ok(command) = cmd_rx.recv() {
            match command {
                ResourceCommand::ReloadBytecode { respond_to, .. } => {
                    let _ = respond_to
                        .send(Err(RuntimeError::ControlError(SmolStr::new("unsupported"))));
                }
                ResourceCommand::MeshSnapshot { respond_to, .. } => {
                    let _ = respond_to.send(IndexMap::new());
                }
                ResourceCommand::Snapshot { respond_to } => {
                    let _ = respond_to.send(snapshot.clone());
                }
                _ => {}
            }
        }
    });

    Arc::new(ControlState {
        debug,
        resource,
        metadata: Arc::new(Mutex::new(harness.runtime().metadata_snapshot())),
        sources: SourceRegistry::new(vec![SourceFile {
            id: 1,
            path: PathBuf::from("main.st"),
            text: source.to_string(),
        }]),
        io_snapshot: Arc::new(Mutex::new(None)),
        pending_restart: Arc::new(Mutex::new(None)),
        auth_token: Arc::new(Mutex::new(None)),
        control_requires_auth: false,
        control_mode: Arc::new(Mutex::new(ControlMode::Debug)),
        audit_tx: None,
        metrics: Arc::new(Mutex::new(RuntimeMetrics::default())),
        events: Arc::new(Mutex::new(VecDeque::new())),
        settings: Arc::new(Mutex::new(runtime_settings())),
        project_root: None,
        resource_name: SmolStr::new("RESOURCE"),
        io_health: Arc::new(Mutex::new(Vec::new())),
        debug_enabled: Arc::new(AtomicBool::new(true)),
        debug_variables: Arc::new(Mutex::new(DebugVariableHandles::new())),
        hmi_live: Arc::new(Mutex::new(trust_runtime::hmi::HmiLiveState::default())),
        historian: None,
        pairing: None,
    })
}

fn reserve_loopback_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind local port");
    let port = listener.local_addr().expect("read local addr").port();
    drop(listener);
    port
}

fn start_test_server(state: Arc<ControlState>, project_root: PathBuf) -> String {
    let port = reserve_loopback_port();
    let listen = format!("127.0.0.1:{port}");
    let config = WebConfig {
        enabled: true,
        listen: SmolStr::new(listen.clone()),
        auth: WebAuthMode::Local,
        tls: false,
    };
    let _server = start_web_server(&config, state, None, None, Some(project_root), None)
        .expect("start web server");
    let base = format!("http://{listen}");
    wait_for_server(&base);
    base
}

fn wait_for_server(base: &str) {
    for _ in 0..80 {
        if ureq::get(&format!("{base}/api/io/config")).call().is_ok() {
            return;
        }
        thread::sleep(Duration::from_millis(25));
    }
    panic!("web server did not become reachable at {base}");
}

fn make_project(name: &str) -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("trust-runtime-web-io-{name}-{stamp}"));
    std::fs::create_dir_all(root.join("sources")).expect("create sources");
    std::fs::write(
        root.join("sources/main.st"),
        "PROGRAM Main\nVAR\nx : INT := 0;\nEND_VAR\nEND_PROGRAM\n",
    )
    .expect("write source");
    root
}

#[test]
fn io_config_endpoint_round_trips_multi_driver_payload() {
    let project = make_project("multidriver");
    let state = control_state(source_fixture());
    let base = start_test_server(state, project.clone());

    let payload = json!({
        "drivers": [
            {
                "name": "modbus-tcp",
                "params": {
                    "address": "127.0.0.1:502",
                    "unit_id": 1,
                    "input_start": 0,
                    "output_start": 0
                }
            },
            {
                "name": "mqtt",
                "params": {
                    "broker": "127.0.0.1:1883",
                    "topic_in": "trust/io/in",
                    "topic_out": "trust/io/out",
                    "reconnect_ms": 250
                }
            }
        ],
        "safe_state": [
            {
                "address": "%QX0.0",
                "value": "FALSE"
            }
        ],
        "use_system_io": false
    });

    let save_response = ureq::post(&format!("{base}/api/io/config"))
        .set("Content-Type", "application/json")
        .send_string(&payload.to_string())
        .expect("save io config")
        .into_string()
        .expect("read save response");
    assert!(
        save_response.contains("I/O config saved"),
        "expected save confirmation, got: {save_response}"
    );

    let io_toml = std::fs::read_to_string(project.join("io.toml")).expect("read io.toml");
    assert!(
        io_toml.contains("drivers"),
        "expected multi-driver array in io.toml, got:\n{io_toml}"
    );
    assert!(
        io_toml.contains("modbus-tcp"),
        "expected modbus-tcp entry in io.toml, got:\n{io_toml}"
    );
    assert!(
        io_toml.contains("mqtt"),
        "expected mqtt entry in io.toml, got:\n{io_toml}"
    );

    let get_body = ureq::get(&format!("{base}/api/io/config"))
        .call()
        .expect("load io config")
        .into_string()
        .expect("read io config body");
    let loaded: Value = serde_json::from_str(&get_body).expect("parse io config json");
    assert_eq!(
        loaded.get("source").and_then(Value::as_str),
        Some("project")
    );
    assert_eq!(
        loaded.get("use_system_io").and_then(Value::as_bool),
        Some(false)
    );
    let drivers = loaded
        .get("drivers")
        .and_then(Value::as_array)
        .expect("drivers array");
    assert_eq!(drivers.len(), 2, "expected two configured drivers");
    assert_eq!(
        drivers
            .first()
            .and_then(|entry| entry.get("name"))
            .and_then(Value::as_str),
        Some("modbus-tcp")
    );
    assert_eq!(
        drivers
            .get(1)
            .and_then(|entry| entry.get("name"))
            .and_then(Value::as_str),
        Some("mqtt")
    );
    assert_eq!(
        loaded.get("driver").and_then(Value::as_str),
        Some("modbus-tcp")
    );
    assert_eq!(
        loaded
            .get("safe_state")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(1)
    );

    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn io_config_endpoint_rejects_invalid_driver_params_shape() {
    let project = make_project("invalid");
    let state = control_state(source_fixture());
    let base = start_test_server(state, project.clone());

    let payload = json!({
        "drivers": [
            {
                "name": "mqtt",
                "params": ["not", "an", "object"]
            }
        ],
        "use_system_io": false
    });

    let response = ureq::post(&format!("{base}/api/io/config"))
        .set("Content-Type", "application/json")
        .send_string(&payload.to_string())
        .expect("post invalid io config")
        .into_string()
        .expect("read error body");
    assert!(
        response.contains("error:"),
        "expected error response for invalid payload, got: {response}"
    );
    assert!(
        response.contains("params must be a table/object"),
        "expected params shape validation error, got: {response}"
    );
    assert!(
        !project.join("io.toml").exists(),
        "invalid payload must not write io.toml"
    );

    let _ = std::fs::remove_dir_all(project);
}
