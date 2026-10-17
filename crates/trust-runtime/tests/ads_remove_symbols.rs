//! The `ads.import_symbols.remove` control request and `trust-runtime ads remove-symbols`, and
//! the `comm.test` request and `trust-runtime comm test`: each pair must give the same answer.

use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use indexmap::IndexMap;
use serde_json::{json, Value};
use smol_str::SmolStr;
use trust_ads_core::{
    AdsDataTypeDescriptor, IecDataType, SymbolDescriptor, SymbolFlag, SymbolSnapshot,
};
use trust_runtime::config::ControlMode;
use trust_runtime::control::{
    ControlEndpoint, ControlServer, ControlState, HmiRuntimeDescriptor, SourceFile, SourceRegistry,
};
use trust_runtime::debug::{DebugSnapshot, DebugVariableHandles};
use trust_runtime::error::RuntimeError;
use trust_runtime::harness::TestHarness;
use trust_runtime::metrics::RuntimeMetrics;
use trust_runtime::scheduler::{ResourceCommand, ResourceControl, StdClock};
use trust_runtime::settings::{
    BaseSettings, DiscoverySettings, MeshSettings, RuntimeSettings, SimulationSettings, WebSettings,
};
use trust_runtime::watchdog::{FaultPolicy, RetainMode, WatchdogPolicy};

const CONTROL_TOKEN: &str = "ads-remove-test-token";

/// The literal `ads.toml` of the VS Code test `ads-tag-config-mutation.test.ts` (lines 10-50).
const TAG_ADS_TOML: &str = r#"[[connections]]
name = "line_301"
target_net_id = "100.67.6.217.1.1"
host = "192.168.77.11"
ams_port = 301
local_net_id = "192.168.77.10.1.1"

[[connections.points]]
var = "var_1"
symbol = "Task 4.Inputs.Var 1"
type = "USINT"

[[connections.points]]
var = "var_2"
symbol = "Task 4.Inputs.Var 2"
type = "USINT"

[[connections]]
name = "duplicate_301"
target_net_id = "100.67.6.217.1.1"
host = "192.168.77.11"
ams_port = 301

[[connections.points]]
var = "duplicate_var_1"
symbol = "Task 4.Inputs.Var 1"
type = "USINT"

[[connections]]
name = "line_851"
target_net_id = "100.67.6.217.1.1"
host = "192.168.77.11"
ams_port = 851

[[connections.points]]
var = "other_port_var_1"
symbol = "Task 4.Inputs.Var 1"
type = "USINT"
"#;

#[test]
fn remove_symbols_cli_matches_the_control_request() {
    let cli_root = unique_temp_dir("ads-remove-cli");
    let request_root = unique_temp_dir("ads-remove-request");
    write_tag_project(&cli_root);
    write_tag_project(&request_root);

    let output = Command::new(
        std::env::var_os("CARGO_BIN_EXE_trust-runtime")
            .expect("Cargo must provide trust-runtime binary while executing integration tests"),
    )
    .args([
        "ads",
        "remove-symbols",
        "--project",
        cli_root.to_str().expect("cli project path"),
        "--host",
        "192.168.77.11",
        "--target-net-id",
        "100.67.6.217.1.1",
        "--ams-port",
        "301",
        "--path",
        "Task 4.Inputs.Var 1",
        "--json",
    ])
    .output()
    .expect("run trust-runtime ads remove-symbols");
    assert!(
        output.status.success(),
        "ads remove-symbols failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let cli_answer: Value =
        serde_json::from_slice(&output.stdout).expect("ads remove-symbols --json stdout");

    let state = control_state(&request_root);
    let request_answer = send_control_request(
        start_control_server(state),
        &json!({
            "id": 1,
            "type": "ads.import_symbols.remove",
            "auth": CONTROL_TOKEN,
            "params": {
                "host": "192.168.77.11",
                "target_net_id": "100.67.6.217.1.1",
                "ams_port": 301,
                "paths": ["Task 4.Inputs.Var 1"],
            },
        }),
    )
    .get("result")
    .cloned()
    .expect("result");

    assert_eq!(cli_answer, request_answer);
    assert_eq!(cli_answer["removed_count"], json!(2));
    assert_eq!(
        cli_answer["files_written"],
        json!(["ads.toml", "src/generated/ads_generated.st"])
    );
    assert_eq!(cli_answer["matched"], json!(true));

    let generated = Path::new("src/generated/ads_generated.st");
    assert_eq!(
        std::fs::read_to_string(cli_root.join("ads.toml")).expect("cli ads.toml"),
        std::fs::read_to_string(request_root.join("ads.toml")).expect("request ads.toml")
    );
    assert_eq!(
        std::fs::read_to_string(cli_root.join(generated)).expect("cli generated ST"),
        std::fs::read_to_string(request_root.join(generated)).expect("request generated ST")
    );

    let _ = std::fs::remove_dir_all(cli_root);
    let _ = std::fs::remove_dir_all(request_root);
}

#[test]
fn comm_test_cli_matches_the_control_request() {
    let root = unique_temp_dir("comm-test-parity");
    let state = control_state(&root);
    let request_answer = send_control_request(
        start_control_server(state),
        &json!({
            "id": 1,
            "type": "comm.test",
            "auth": CONTROL_TOKEN,
            "params": { "protocol": "simulated", "params": {} },
        }),
    )
    .get("result")
    .cloned()
    .expect("result");

    let output = Command::new(
        std::env::var_os("CARGO_BIN_EXE_trust-runtime")
            .expect("Cargo must provide trust-runtime binary while executing integration tests"),
    )
    .args([
        "comm",
        "test",
        "--project",
        root.to_str().expect("project path"),
        "--protocol",
        "simulated",
        "--params",
        "{}",
        "--json",
    ])
    .output()
    .expect("run trust-runtime comm test");
    assert!(
        output.status.success(),
        "comm test failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let cli_answer: Value =
        serde_json::from_slice(&output.stdout).expect("comm test --json stdout");

    assert_eq!(cli_answer, request_answer);
    assert_eq!(cli_answer["protocol"], json!("simulated"));
    assert_eq!(cli_answer["supported"], json!(true));
    assert_eq!(cli_answer["ok"], json!(true));

    let _ = std::fs::remove_dir_all(root);
}

fn write_tag_project(root: &Path) {
    let snapshots = root.join("ads/snapshots");
    std::fs::create_dir_all(&snapshots).expect("create snapshot directory");
    std::fs::write(root.join("ads.toml"), TAG_ADS_TOML).expect("write ads.toml");
    std::fs::write(
        snapshots.join("line_301.symbols.json"),
        snapshot_json("line_301", "Task 4.Inputs.Var 2"),
    )
    .expect("write line_301 snapshot");
    std::fs::write(
        snapshots.join("line_851.symbols.json"),
        snapshot_json("line_851", "Task 4.Inputs.Var 1"),
    )
    .expect("write line_851 snapshot");
}

fn snapshot_json(route_name: &str, symbol_name: &str) -> String {
    let symbol = SymbolDescriptor::new(
        symbol_name,
        AdsDataTypeDescriptor::scalar("USINT", IecDataType::Usint),
        0x4020,
        0,
        1,
    )
    .with_flag(SymbolFlag::Read);
    SymbolSnapshot::new(route_name, vec![symbol])
        .to_deterministic_json()
        .expect("snapshot json")
}

fn reserve_loopback_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback port");
    listener.local_addr().expect("read loopback address").port()
}

fn start_control_server(state: Arc<ControlState>) -> SocketAddr {
    for _ in 0..16 {
        let addr = SocketAddr::from(([127, 0, 0, 1], reserve_loopback_port()));
        if ControlServer::start(ControlEndpoint::Tcp(addr), Arc::clone(&state)).is_ok() {
            return addr;
        }
        thread::sleep(Duration::from_millis(10));
    }
    panic!("start control server: no free loopback port after retries");
}

fn send_control_request(addr: SocketAddr, request: &Value) -> Value {
    let mut stream = TcpStream::connect(addr).expect("connect control server");
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("set control read timeout");
    writeln!(stream, "{request}").expect("write control request");
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line).expect("read control response");
    let response: Value = serde_json::from_str(line.trim_end()).expect("parse control response");
    assert_eq!(response["ok"], json!(true), "{response}");
    response
}

fn runtime_settings() -> RuntimeSettings {
    RuntimeSettings::new(
        trust_runtime::value::Duration::from_millis(10),
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
            host_group: None,
        },
        MeshSettings {
            enabled: false,
            role: trust_runtime::config::MeshRole::Peer,
            listen: SmolStr::new("127.0.0.1:0"),
            connect: Vec::new(),
            tls: false,
            auth_token: None,
            publish: Vec::new(),
            subscribe: IndexMap::new(),
            zenohd_version: SmolStr::new("1.7.2"),
            plugin_versions: IndexMap::new(),
        },
        SimulationSettings {
            enabled: false,
            time_scale: 1,
            mode_label: SmolStr::new("production"),
            warning: SmolStr::new(""),
        },
    )
}

fn control_state(project_root: &Path) -> Arc<ControlState> {
    let source = r#"
VAR_GLOBAL
    setpoint : REAL := 12.5;
END_VAR
PROGRAM Main
END_PROGRAM
"#;
    let mut harness = TestHarness::from_source(source).expect("build harness");
    let debug = harness.runtime_mut().enable_debug();
    harness.cycle();
    let snapshot = DebugSnapshot {
        storage_abi: Default::default(),
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
                ResourceCommand::AdsStatus { respond_to } => {
                    let _ = respond_to.send(trust_runtime::ads::diagnostics::AdsStatusReport {
                        schema_version:
                            trust_runtime::ads::diagnostics::ADS_DIAGNOSTICS_SCHEMA_VERSION,
                        role: trust_runtime::ads::diagnostics::DoctorRole::Client,
                        overall: trust_runtime::ads::diagnostics::AdsStatusOverall::Disabled,
                        runtime_identity_hash: None,
                        deployed_ads_config_hash: None,
                        connections: Vec::new(),
                        summary: "ADS is not configured.".to_string(),
                    });
                }
                ResourceCommand::OpcUaClientStatus { respond_to } => {
                    let _ = respond_to.send(trust_runtime::opcua::OpcUaClientStatusReport {
                        enabled: false,
                        deployed_config_hash: None,
                        connections: Vec::new(),
                    });
                }
                ResourceCommand::ActiveAdsDevice { respond_to, .. } => {
                    let _ = respond_to.send(None);
                }
                ResourceCommand::MeshApply { .. }
                | ResourceCommand::Pause
                | ResourceCommand::Resume
                | ResourceCommand::UpdateSchedulerPolicies(_)
                | ResourceCommand::UpdateIoSafeState(_) => {}
            }
        }
    });

    let sources = SourceRegistry::new(vec![SourceFile {
        id: 1,
        path: "main.st".into(),
        text: source.to_string(),
    }]);
    let hmi_descriptor = Arc::new(Mutex::new(HmiRuntimeDescriptor::from_sources(
        None, &sources,
    )));

    Arc::new(ControlState {
        debug,
        resource,
        metadata: Arc::new(Mutex::new(harness.runtime().metadata_snapshot())),
        sources,
        io_snapshot: Arc::new(Mutex::new(None)),
        io_snapshot_seen_ms: Arc::new(std::sync::atomic::AtomicU64::new(0)),
        pending_restart: Arc::new(Mutex::new(None)),
        auth_token: Arc::new(Mutex::new(Some(SmolStr::new(CONTROL_TOKEN)))),
        control_requires_auth: true,
        control_mode: Arc::new(Mutex::new(ControlMode::Debug)),
        audit_tx: None,
        metrics: Arc::new(Mutex::new(RuntimeMetrics::default())),
        events: Arc::new(Mutex::new(VecDeque::new())),
        settings: Arc::new(Mutex::new(runtime_settings())),
        discovery: Arc::new(trust_runtime::discovery::DiscoveryState::new()),
        mesh_topology: Arc::new(Mutex::new(None)),
        realtime_status: Arc::new(Mutex::new(
            trust_runtime::linux_rt::LinuxRtRuntimeStatus::from_config(
                trust_runtime::linux_rt::LinuxRtConfig::default(),
            ),
        )),
        web_listener_bound: Arc::new(AtomicBool::new(false)),
        opcua_server_bound: Arc::new(AtomicBool::new(false)),
        project_root: Some(project_root.to_path_buf()),
        resource_name: SmolStr::new("RESOURCE"),
        io_health: Arc::new(Mutex::new(Vec::new())),
        debug_enabled: Arc::new(AtomicBool::new(true)),
        debug_variables: Arc::new(Mutex::new(DebugVariableHandles::new())),
        hmi_live: Arc::new(Mutex::new(trust_runtime::hmi::HmiLiveState::default())),
        hmi_persistence: None,
        hmi_descriptor,
        historian: None,
        openot_persistence_status: None,
        pairing: None,
        search: Default::default(),
        ads_doctor_jobs: Arc::new(Mutex::new(
            trust_runtime::control::AdsDoctorJobStore::default(),
        )),
        ads_client_config: Arc::new(Mutex::new(None)),
        opcua_client_config: Arc::new(Mutex::new(None)),
        ads_server_config: Arc::new(Mutex::new(None)),
        #[cfg(feature = "ads-server")]
        ads_server_runtime: Arc::new(Mutex::new(None)),
    })
}

fn unique_temp_dir(prefix: &str) -> PathBuf {
    static TEMP_COUNTER: AtomicU64 = AtomicU64::new(1);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time before unix epoch")
        .as_nanos();
    let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "trust-runtime-{prefix}-{}-{nanos}-{counter}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).expect("create temp dir");
    root
}
