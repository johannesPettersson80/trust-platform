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

fn start_test_server(state: Arc<ControlState>, project_root: PathBuf, auth: WebAuthMode) -> String {
    let port = reserve_loopback_port();
    let listen = format!("127.0.0.1:{port}");
    let config = WebConfig {
        enabled: true,
        listen: SmolStr::new(listen.clone()),
        auth,
        tls: false,
    };
    let _server = start_web_server(&config, state, None, None, Some(project_root), None)
        .expect("start server");
    let base = format!("http://{listen}");
    wait_for_server(&base);
    base
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
    let mut request = match method {
        "GET" => ureq::get(url),
        "POST" => ureq::post(url),
        other => panic!("unsupported method {other}"),
    };
    for (name, value) in headers {
        request = request.set(name, value);
    }

    let result = match (method, payload) {
        ("POST", Some(body)) => request
            .set("Content-Type", "application/json")
            .send_string(&body.to_string()),
        ("POST", None) => request.send_string("{}"),
        ("GET", _) => request.call(),
        _ => unreachable!(),
    };

    match result {
        Ok(response) => {
            let status = response.status();
            let body = response.into_string().expect("read success body");
            let json = serde_json::from_str(&body).unwrap_or_else(|_| json!({}));
            (status, json)
        }
        Err(ureq::Error::Status(status, response)) => {
            let body = response.into_string().expect("read error body");
            let json = serde_json::from_str(&body).unwrap_or_else(|_| json!({}));
            (status, json)
        }
        Err(err) => panic!("request failed: {err}"),
    }
}

fn make_project(name: &str) -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("trust-runtime-web-ide-{name}-{stamp}"));
    std::fs::create_dir_all(root.join("sources")).expect("create sources");
    std::fs::write(
        root.join("sources/main.st"),
        "PROGRAM Main\nVAR\nCounter : INT := 1;\nEND_VAR\nEND_PROGRAM\n",
    )
    .expect("write source");
    root
}

fn source_fixture() -> &'static str {
    "PROGRAM Main\nEND_PROGRAM\n"
}

#[test]
fn web_ide_auth_and_session_contract() {
    let project = make_project("auth");
    let state = control_state(source_fixture(), ControlMode::Debug, Some("secret-token"));
    let base = start_test_server(state, project.clone(), WebAuthMode::Token);

    let (status, unauthorized) = request_json(
        "POST",
        &format!("{base}/api/ide/session"),
        Some(json!({ "role": "editor" })),
        &[],
    );
    assert_eq!(status, 401);
    assert_eq!(
        unauthorized.get("error").and_then(Value::as_str),
        Some("unauthorized")
    );

    let (status, session_body) = request_json(
        "POST",
        &format!("{base}/api/ide/session"),
        Some(json!({ "role": "editor" })),
        &[("X-Trust-Token", "secret-token")],
    );
    assert_eq!(status, 200);
    let session_token = session_body
        .get("result")
        .and_then(|v| v.get("token"))
        .and_then(Value::as_str)
        .expect("session token");

    let (status, files) = request_json(
        "GET",
        &format!("{base}/api/ide/files"),
        None,
        &[("X-Trust-Ide-Session", session_token)],
    );
    assert_eq!(status, 200);
    assert!(files
        .get("result")
        .and_then(|v| v.get("files"))
        .and_then(Value::as_array)
        .is_some_and(|items| items.iter().any(|item| item.as_str() == Some("main.st"))));

    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn web_ide_collaborative_conflict_contract() {
    let project = make_project("conflict");
    let state = control_state(source_fixture(), ControlMode::Debug, None);
    let base = start_test_server(state, project.clone(), WebAuthMode::Local);

    let (_, s1) = request_json(
        "POST",
        &format!("{base}/api/ide/session"),
        Some(json!({ "role": "editor" })),
        &[],
    );
    let (_, s2) = request_json(
        "POST",
        &format!("{base}/api/ide/session"),
        Some(json!({ "role": "editor" })),
        &[],
    );
    let token_a = s1
        .get("result")
        .and_then(|v| v.get("token"))
        .and_then(Value::as_str)
        .expect("token a");
    let token_b = s2
        .get("result")
        .and_then(|v| v.get("token"))
        .and_then(Value::as_str)
        .expect("token b");

    let (_, a_open) = request_json(
        "GET",
        &format!("{base}/api/ide/file?path=main.st"),
        None,
        &[("X-Trust-Ide-Session", token_a)],
    );
    let (_, b_open) = request_json(
        "GET",
        &format!("{base}/api/ide/file?path=main.st"),
        None,
        &[("X-Trust-Ide-Session", token_b)],
    );

    let version_a = a_open
        .get("result")
        .and_then(|v| v.get("version"))
        .and_then(Value::as_u64)
        .expect("version a");
    let version_b = b_open
        .get("result")
        .and_then(|v| v.get("version"))
        .and_then(Value::as_u64)
        .expect("version b");
    assert_eq!(version_a, version_b);

    let (status, first_write) = request_json(
        "POST",
        &format!("{base}/api/ide/file"),
        Some(json!({
            "path": "main.st",
            "expected_version": version_a,
            "content": "PROGRAM Main\nVAR\nCounter : INT := 2;\nEND_VAR\nEND_PROGRAM\n"
        })),
        &[("X-Trust-Ide-Session", token_a)],
    );
    assert_eq!(status, 200);
    let current_version = first_write
        .get("result")
        .and_then(|v| v.get("version"))
        .and_then(Value::as_u64)
        .expect("current version");

    let (status, conflict) = request_json(
        "POST",
        &format!("{base}/api/ide/file"),
        Some(json!({
            "path": "main.st",
            "expected_version": version_b,
            "content": "PROGRAM Main\nVAR\nCounter : INT := 9;\nEND_VAR\nEND_PROGRAM\n"
        })),
        &[("X-Trust-Ide-Session", token_b)],
    );
    assert_eq!(status, 409);
    assert_eq!(
        conflict.get("current_version").and_then(Value::as_u64),
        Some(current_version)
    );

    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn web_ide_writes_require_debug_mode() {
    let project = make_project("mode");
    let state = control_state(source_fixture(), ControlMode::Production, None);
    let base = start_test_server(state, project.clone(), WebAuthMode::Local);

    let (_, caps) = request_json("GET", &format!("{base}/api/ide/capabilities"), None, &[]);
    assert_eq!(
        caps.get("result")
            .and_then(|v| v.get("mode"))
            .and_then(Value::as_str),
        Some("read_only")
    );

    let (_, session) = request_json(
        "POST",
        &format!("{base}/api/ide/session"),
        Some(json!({ "role": "editor" })),
        &[],
    );
    let token = session
        .get("result")
        .and_then(|v| v.get("token"))
        .and_then(Value::as_str)
        .expect("session token");

    let (_, opened) = request_json(
        "GET",
        &format!("{base}/api/ide/file?path=main.st"),
        None,
        &[("X-Trust-Ide-Session", token)],
    );
    assert_eq!(
        opened
            .get("result")
            .and_then(|v| v.get("read_only"))
            .and_then(Value::as_bool),
        Some(true)
    );
    let version = opened
        .get("result")
        .and_then(|v| v.get("version"))
        .and_then(Value::as_u64)
        .expect("version");

    let (status, denied) = request_json(
        "POST",
        &format!("{base}/api/ide/file"),
        Some(json!({
            "path": "main.st",
            "expected_version": version,
            "content": "PROGRAM Main\nEND_PROGRAM\n"
        })),
        &[("X-Trust-Ide-Session", token)],
    );
    assert_eq!(status, 403);
    assert!(denied
        .get("error")
        .and_then(Value::as_str)
        .is_some_and(|message| message.contains("authoring is disabled")));

    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn web_ide_latency_and_resource_budget_contract() {
    let project = make_project("budget");
    let state = control_state(source_fixture(), ControlMode::Debug, None);
    let base = start_test_server(state, project.clone(), WebAuthMode::Local);

    let (_, caps) = request_json("GET", &format!("{base}/api/ide/capabilities"), None, &[]);
    let max_file_bytes = caps
        .get("result")
        .and_then(|v| v.get("limits"))
        .and_then(|v| v.get("max_file_bytes"))
        .and_then(Value::as_u64)
        .expect("max_file_bytes");

    let (_, session) = request_json(
        "POST",
        &format!("{base}/api/ide/session"),
        Some(json!({ "role": "editor" })),
        &[],
    );
    let token = session
        .get("result")
        .and_then(|v| v.get("token"))
        .and_then(Value::as_str)
        .expect("session token");

    let (_, opened) = request_json(
        "GET",
        &format!("{base}/api/ide/file?path=main.st"),
        None,
        &[("X-Trust-Ide-Session", token)],
    );
    let mut version = opened
        .get("result")
        .and_then(|v| v.get("version"))
        .and_then(Value::as_u64)
        .expect("initial version");

    let runs: u32 = 60;
    let mut total = Duration::ZERO;
    let mut max = Duration::ZERO;
    for idx in 0..runs {
        let started = Instant::now();
        let (status, result) = request_json(
            "POST",
            &format!("{base}/api/ide/file"),
            Some(json!({
                "path": "main.st",
                "expected_version": version,
                "content": format!(
                    "PROGRAM Main\\nVAR\\nCounter : INT := {};\\nEND_VAR\\nEND_PROGRAM\\n",
                    idx
                )
            })),
            &[("X-Trust-Ide-Session", token)],
        );
        let elapsed = started.elapsed();
        total += elapsed;
        max = max.max(elapsed);
        assert_eq!(status, 200);
        version = result
            .get("result")
            .and_then(|v| v.get("version"))
            .and_then(Value::as_u64)
            .expect("next version");
    }

    let avg = total / runs;
    assert!(
        max < Duration::from_millis(250),
        "max latency {:?} exceeded budget",
        max
    );
    assert!(
        avg < Duration::from_millis(50),
        "avg latency {:?} exceeded budget",
        avg
    );

    let too_large = "X".repeat(max_file_bytes as usize + 1);
    let (status, body) = request_json(
        "POST",
        &format!("{base}/api/ide/file"),
        Some(json!({
            "path": "main.st",
            "expected_version": version,
            "content": too_large
        })),
        &[("X-Trust-Ide-Session", token)],
    );
    assert_eq!(status, 413);
    assert!(body
        .get("error")
        .and_then(Value::as_str)
        .is_some_and(|message| message.contains("exceeds limit")));

    let _ = std::fs::remove_dir_all(project);
}
