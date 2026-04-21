use std::collections::VecDeque;
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use indexmap::IndexMap;
use serde_json::{json, Value};
use smol_str::SmolStr;
use trust_runtime::config::{ControlMode, WebAuthMode, WebConfig};
use trust_runtime::control::{ControlState, HmiRuntimeDescriptor, SourceFile, SourceRegistry};
use trust_runtime::debug::DebugVariableHandles;
use trust_runtime::error::RuntimeError;
use trust_runtime::harness::TestHarness;
use trust_runtime::linux_rt::LinuxRtRuntimeStatus;
use trust_runtime::metrics::RuntimeMetrics;
use trust_runtime::scheduler::{ResourceCommand, ResourceControl, StdClock};
use trust_runtime::settings::{
    BaseSettings, DiscoverySettings, MeshSettings, RuntimeSettings, SimulationSettings, WebSettings,
};
use trust_runtime::watchdog::{FaultPolicy, RetainMode, WatchdogPolicy};
use trust_runtime::web::start_web_server;

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

fn control_state(source: &str, mode: ControlMode, auth_token: Option<&str>) -> Arc<ControlState> {
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

    let sources = SourceRegistry::new(vec![SourceFile {
        id: 1,
        path: PathBuf::from("main.st"),
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
        pending_restart: Arc::new(Mutex::new(None)),
        auth_token: Arc::new(Mutex::new(auth_token.map(SmolStr::new))),
        control_requires_auth: auth_token.is_some(),
        control_mode: Arc::new(Mutex::new(mode)),
        audit_tx: None,
        metrics: Arc::new(Mutex::new(RuntimeMetrics::default())),
        events: Arc::new(Mutex::new(VecDeque::new())),
        settings: Arc::new(Mutex::new(runtime_settings())),
        realtime_status: Arc::new(Mutex::new(LinuxRtRuntimeStatus::from_config(
            trust_runtime::linux_rt::LinuxRtConfig::default(),
        ))),
        project_root: None,
        resource_name: SmolStr::new("RESOURCE"),
        io_health: Arc::new(Mutex::new(Vec::new())),
        debug_enabled: Arc::new(AtomicBool::new(true)),
        debug_variables: Arc::new(Mutex::new(DebugVariableHandles::new())),
        hmi_live: Arc::new(Mutex::new(trust_runtime::hmi::HmiLiveState::default())),
        hmi_descriptor,
        historian: None,
        pairing: None,
    })
}

fn reserve_loopback_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind local port");
    let port = listener.local_addr().expect("local addr").port();
    drop(listener);
    port
}

fn start_test_server_with_root(
    state: Arc<ControlState>,
    project_root: Option<PathBuf>,
    auth: WebAuthMode,
) -> String {
    let port = reserve_loopback_port();
    let listen = format!("127.0.0.1:{port}");
    let config = WebConfig {
        enabled: true,
        listen: SmolStr::new(listen.clone()),
        auth,
        tls: false,
    };
    let _server =
        start_web_server(&config, state, None, None, project_root, None).expect("start server");
    let base = format!("http://{listen}");
    wait_for_server(&base);
    base
}

fn start_test_server(state: Arc<ControlState>, project_root: PathBuf, auth: WebAuthMode) -> String {
    start_test_server_with_root(state, Some(project_root), auth)
}

fn wait_for_server(base: &str) {
    for _ in 0..100 {
        if ureq::get(&format!("{base}/ide")).call().is_ok() {
            return;
        }
        thread::sleep(Duration::from_millis(25));
    }
    panic!("web server did not become reachable at {base}");
}

fn request_json(
    method: &str,
    url: &str,
    payload: Option<Value>,
    headers: &[(&str, &str)],
) -> (u16, Value) {
    match method {
        "GET" => {
            let mut request = ureq::get(url);
            for &(name, value) in headers {
                request = request.header(name, value);
            }
            let mut response = request
                .config()
                .http_status_as_error(false)
                .build()
                .call()
                .unwrap_or_else(|err| panic!("request failed: {err}"));
            let status = response.status().as_u16();
            let body = response
                .body_mut()
                .read_to_string()
                .expect("read response body");
            let json = serde_json::from_str(&body).unwrap_or_else(|_| json!({}));
            (status, json)
        }
        "POST" => {
            let mut request = ureq::post(url).header("Content-Type", "application/json");
            for &(name, value) in headers {
                request = request.header(name, value);
            }
            let mut response = request
                .config()
                .http_status_as_error(false)
                .build()
                .send(payload.unwrap_or_else(|| json!({})).to_string().as_str())
                .unwrap_or_else(|err| panic!("request failed: {err}"));
            let status = response.status().as_u16();
            let body = response
                .body_mut()
                .read_to_string()
                .expect("read response body");
            let json = serde_json::from_str(&body).unwrap_or_else(|_| json!({}));
            (status, json)
        }
        other => panic!("unsupported method {other}"),
    }
}

fn make_project(name: &str) -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("trust-runtime-web-ide-{name}-{stamp}"));
    std::fs::create_dir_all(&root).expect("create project dir");
    std::fs::write(
        root.join("main.st"),
        "PROGRAM Main\nVAR\nCounter : INT := 1;\nEND_VAR\nEND_PROGRAM\n",
    )
    .expect("write source");
    root
}

fn source_fixture() -> &'static str {
    "PROGRAM Main\nEND_PROGRAM\n"
}

fn p95(samples: &[Duration]) -> Duration {
    let mut sorted = samples.to_vec();
    sorted.sort_unstable();
    let idx = ((sorted.len() as f64) * 0.95).ceil() as usize;
    sorted[idx.saturating_sub(1).min(sorted.len().saturating_sub(1))]
}

fn position_for(text: &str, needle: &str) -> (u32, u32) {
    let byte_index = text.find(needle).expect("needle should exist");
    let before = &text[..byte_index];
    let line = before.bytes().filter(|byte| *byte == b'\n').count() as u32;
    let character = before
        .rsplit_once('\n')
        .map(|(_, tail)| tail.chars().count() as u32)
        .unwrap_or_else(|| before.chars().count() as u32);
    (line, character)
}

#[path = "web_ide_integration/web_ide_integration_part_01.rs"]
mod web_ide_integration_part_01;
#[path = "web_ide_integration/web_ide_integration_part_02.rs"]
mod web_ide_integration_part_02;
#[path = "web_ide_integration/web_ide_integration_part_03.rs"]
mod web_ide_integration_part_03;
#[path = "web_ide_integration/web_ide_integration_part_04.rs"]
mod web_ide_integration_part_04;
#[path = "web_ide_integration/web_ide_integration_part_05.rs"]
mod web_ide_integration_part_05;
#[path = "web_ide_integration/web_ide_integration_part_06.rs"]
mod web_ide_integration_part_06;
#[path = "web_ide_integration/web_ide_integration_part_07.rs"]
mod web_ide_integration_part_07;
#[path = "web_ide_integration/web_ide_integration_part_08.rs"]
mod web_ide_integration_part_08;
#[path = "web_ide_integration/web_ide_integration_part_09.rs"]
mod web_ide_integration_part_09;
