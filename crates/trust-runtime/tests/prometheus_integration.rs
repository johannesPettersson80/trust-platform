use std::collections::VecDeque;
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use indexmap::IndexMap;
use smol_str::SmolStr;
use trust_runtime::config::{ControlMode, WebAuthMode, WebConfig};
use trust_runtime::control::{ControlState, HmiRuntimeDescriptor, SourceFile, SourceRegistry};
use trust_runtime::debug::DebugVariableHandles;
use trust_runtime::error::RuntimeError;
use trust_runtime::harness::TestHarness;
use trust_runtime::historian::{HistorianConfig, HistorianService, RecordingMode};
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

fn temp_path(name: &str) -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!("trust-runtime-prom-{name}-{stamp}.jsonl"))
}

fn make_historian() -> Arc<HistorianService> {
    let config = HistorianConfig {
        enabled: true,
        sample_interval_ms: 1,
        mode: RecordingMode::All,
        include: Vec::new(),
        history_path: temp_path("history"),
        max_entries: 1_000,
        prometheus_enabled: true,
        prometheus_path: SmolStr::new("/metrics"),
        alerts: Vec::new(),
    };
    HistorianService::new(config, None).expect("historian service")
}

fn control_state(
    source: &str,
    auth_token: Option<&str>,
    historian: Option<Arc<HistorianService>>,
) -> Arc<ControlState> {
    let mut harness = TestHarness::from_source(source).expect("build test harness");
    let debug = harness.runtime_mut().enable_debug();
    harness.cycle();
    let snapshot = trust_runtime::debug::DebugSnapshot {
        storage: harness.runtime().storage().clone(),
        now: harness.runtime().current_time(),
    };
    if let Some(service) = historian.as_ref() {
        service
            .capture_snapshot_at(&snapshot, 10_000)
            .expect("capture initial snapshot");
    }

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

    let mut metrics = RuntimeMetrics::default();
    metrics.record_cycle(Duration::from_millis(10));

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
        control_mode: Arc::new(Mutex::new(ControlMode::Debug)),
        audit_tx: None,
        metrics: Arc::new(Mutex::new(metrics)),
        events: Arc::new(Mutex::new(VecDeque::new())),
        settings: Arc::new(Mutex::new(runtime_settings())),
        project_root: None,
        resource_name: SmolStr::new("RESOURCE"),
        io_health: Arc::new(Mutex::new(Vec::new())),
        debug_enabled: Arc::new(AtomicBool::new(true)),
        debug_variables: Arc::new(Mutex::new(DebugVariableHandles::new())),
        hmi_live: Arc::new(Mutex::new(trust_runtime::hmi::HmiLiveState::default())),
        hmi_descriptor,
        historian,
        pairing: None,
    })
}

fn reserve_loopback_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind local port");
    let port = listener.local_addr().expect("local addr").port();
    drop(listener);
    port
}

fn start_test_server(state: Arc<ControlState>, auth: WebAuthMode) -> String {
    let port = reserve_loopback_port();
    let listen = format!("127.0.0.1:{port}");
    let config = WebConfig {
        enabled: true,
        listen: SmolStr::new(listen.clone()),
        auth,
        tls: false,
    };
    let _server =
        start_web_server(&config, state, None, None, None, None).expect("start web server");
    let base = format!("http://{listen}");
    for _ in 0..80 {
        if ureq::get(&format!("{base}/")).call().is_ok() {
            return base;
        }
        thread::sleep(Duration::from_millis(25));
    }
    panic!("web server did not start at {base}");
}

#[test]
fn prometheus_endpoint_exposes_runtime_and_historian_metrics() {
    let historian = make_historian();
    let state = control_state("PROGRAM Main\nEND_PROGRAM\n", None, Some(historian));
    let base = start_test_server(state, WebAuthMode::Local);

    let response = ureq::get(&format!("{base}/metrics"))
        .call()
        .expect("metrics response");
    assert_eq!(response.status(), 200);
    let body = response.into_string().expect("metrics body");
    assert!(body.contains("trust_runtime_uptime_ms"));
    assert!(body.contains("trust_runtime_cycle_last_ms"));
    assert!(body.contains("trust_runtime_historian_samples_total"));
}

#[test]
fn prometheus_endpoint_requires_auth_when_token_mode_enabled() {
    let historian = make_historian();
    let state = control_state(
        "PROGRAM Main\nEND_PROGRAM\n",
        Some("secret"),
        Some(historian),
    );
    let base = start_test_server(state, WebAuthMode::Token);

    let unauthorized = ureq::get(&format!("{base}/metrics")).call();
    match unauthorized {
        Ok(response) => panic!("expected 401, got {}", response.status()),
        Err(ureq::Error::Status(status, _)) => assert_eq!(status, 401),
        Err(err) => panic!("request failed: {err}"),
    }

    let authorized = ureq::get(&format!("{base}/metrics"))
        .set("X-Trust-Token", "secret")
        .call()
        .expect("authorized metrics");
    assert_eq!(authorized.status(), 200);
}
