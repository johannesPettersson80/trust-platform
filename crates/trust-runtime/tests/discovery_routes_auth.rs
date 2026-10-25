//! The `/api/discovery` and `/api/probe` web routes pass the web authentication, and the probe
//! takes its target and credentials in the request body (specification 11, runtime discovery).
//!
//! The tests need the toolchain of specification 44 §4.6 at TRUST_TOOLCHAIN.

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
use trust_runtime::control::{ControlState, HmiRuntimeDescriptor, SourceFile, SourceRegistry};
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

/// The web token every request of these tests carries.
const WEB_TOKEN: &str = "discovery-route-token";

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
            auth: SmolStr::new("token"),
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

fn control_state(source: &str) -> Arc<ControlState> {
    let mut harness = TestHarness::from_source(source).expect("build test harness");
    let debug = trust_runtime::debug::DebugControl::new();
    harness.runtime_mut().set_debug_control(debug.clone());
    harness.cycle();

    let snapshot = trust_runtime::debug::DebugSnapshot {
        storage: Default::default(),
        now: harness.runtime().current_time(),
        compiled: Some(Arc::new(
            harness.runtime().snapshot().expect("the program's state"),
        )),
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
                ResourceCommand::ActiveAdsDevice { respond_to, .. } => {
                    let _ = respond_to.send(None);
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
        metadata: Arc::new(Mutex::new(trust_runtime::RuntimeMetadata::default())),
        sources,
        io_snapshot: Arc::new(Mutex::new(None)),
        io_snapshot_seen_ms: Arc::new(std::sync::atomic::AtomicU64::new(0)),
        pending_restart: Arc::new(Mutex::new(None)),
        auth_token: Arc::new(Mutex::new(Some(SmolStr::new(WEB_TOKEN)))),
        control_requires_auth: false,
        control_mode: Arc::new(Mutex::new(ControlMode::Debug)),
        audit_tx: None,
        metrics: Arc::new(Mutex::new(RuntimeMetrics::default())),
        events: Arc::new(Mutex::new(VecDeque::new())),
        settings: Arc::new(Mutex::new(runtime_settings())),
        discovery: Arc::new(trust_runtime::discovery::DiscoveryState::new()),
        mesh_topology: Arc::new(Mutex::new(None)),
        realtime_status: Arc::new(Mutex::new(LinuxRtRuntimeStatus::from_config(
            trust_runtime::linux_rt::LinuxRtConfig::default(),
        ))),
        web_listener_bound: Arc::new(AtomicBool::new(false)),
        opcua_server_bound: Arc::new(AtomicBool::new(false)),
        project_root: None,
        resource_name: SmolStr::new("RESOURCE"),
        runtime_id: SmolStr::new("test-runtime-id"),
        io_health: Arc::new(Mutex::new(Vec::new())),
        debug_enabled: Arc::new(AtomicBool::new(true)),
        transfers: trust_runtime::control::new_transfer_session(),
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

fn reserve_loopback_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind local port");
    let port = listener.local_addr().expect("local addr").port();
    drop(listener);
    port
}

/// Starts one loopback web server in token mode, which needs `X-Trust-Token` on every protected
/// route, and answers its base URL.
fn start_token_server(state: Arc<ControlState>) -> String {
    for _ in 0..16 {
        let port = reserve_loopback_port();
        let listen = format!("127.0.0.1:{port}");
        let config = WebConfig {
            enabled: true,
            listen: SmolStr::new(listen.clone()),
            auth: WebAuthMode::Token,
            tls: false,
        };
        match start_web_server(&config, Arc::clone(&state), None, None, None, None) {
            Ok(_server) => return format!("http://{listen}"),
            Err(RuntimeError::ControlError(message))
                if message.contains("Address already in use") =>
            {
                thread::sleep(Duration::from_millis(10));
            }
            Err(err) => panic!("start web server: {err}"),
        }
    }
    panic!("start web server: no free loopback port after retries");
}

fn get(base: &str, path: &str, token: Option<&str>) -> (u16, String) {
    let request = ureq::get(&format!("{base}{path}"))
        .config()
        .http_status_as_error(false)
        .build();
    let request = match token {
        Some(token) => request.header("X-Trust-Token", token),
        None => request,
    };
    let mut response = request.call().expect("web response");
    let status = response.status().as_u16();
    let body = response.body_mut().read_to_string().expect("web body");
    (status, body)
}

fn post(base: &str, path: &str, token: Option<&str>, body: &str) -> (u16, String) {
    let request = ureq::post(&format!("{base}{path}"))
        .config()
        .http_status_as_error(false)
        .build();
    let request = match token {
        Some(token) => request.header("X-Trust-Token", token),
        None => request,
    };
    let mut response = request
        .header("Content-Type", "application/json")
        .send(body)
        .expect("web response");
    let status = response.status().as_u16();
    let body = response.body_mut().read_to_string().expect("web body");
    (status, body)
}

fn json_body(body: &str) -> Value {
    serde_json::from_str(body).expect("json body")
}

// Specification 11, runtime discovery: `/api/discovery` and `/api/probe` require the same web
// authentication as the other routes. It would catch a route that answers an unauthenticated
// request with its payload, or an authenticated request with a refusal.
#[test]
fn discovery_and_probe_routes_require_web_auth() {
    let base = start_token_server(control_state("PROGRAM Main\nEND_PROGRAM\n"));

    let (status, body) = get(&base, "/api/discovery", None);
    assert_eq!(status, 401, "{body}");
    assert_eq!(
        json_body(&body),
        json!({"ok": false, "error": "unauthorized"})
    );
    let (status, body) = get(&base, "/api/discovery", Some(WEB_TOKEN));
    assert_eq!(status, 200, "{body}");
    assert_eq!(json_body(&body), json!({"items": []}));

    let probe = json!({"url": "http://127.0.0.1:1"}).to_string();
    let (status, body) = post(&base, "/api/probe", None, probe.as_str());
    assert_eq!(status, 401, "{body}");
    assert_eq!(
        json_body(&body),
        json!({"ok": false, "error": "unauthorized"})
    );
    let (status, body) = post(&base, "/api/probe", Some(WEB_TOKEN), probe.as_str());
    assert_eq!(status, 200, "{body}");
    assert_eq!(
        json_body(&body),
        json!({"ok": false, "error": "unreachable"})
    );
}

// Specification 11, runtime discovery: `/api/probe` accepts exactly `POST /api/probe` with its
// target and credentials in the JSON body; a GET, or any request that carries a query string, is
// refused. It would catch a GET that still probes from query parameters, a query that is ignored
// instead of refused, or a body whose target and credentials are not the probe's.
#[test]
fn probe_takes_credentials_in_the_body() {
    let base = start_token_server(control_state("PROGRAM Main\nEND_PROGRAM\n"));

    let query = "/api/probe?url=http%3A%2F%2F127.0.0.1%3A1&username=plant&password=s3cret";
    for (status, body) in [
        get(&base, query, Some(WEB_TOKEN)),
        post(&base, query, Some(WEB_TOKEN), "{}"),
    ] {
        assert_eq!(status, 405, "{body}");
        assert_eq!(
            json_body(&body),
            json!({
                "ok": false,
                "error": "probe takes its target and credentials in the request body",
            })
        );
        assert!(
            !body.contains("s3cret"),
            "no credential in the answer: {body}"
        );
    }

    let (status, body) = post(
        &base,
        "/api/probe",
        Some(WEB_TOKEN),
        json!({
            "url": "http://127.0.0.1:1",
            "username": "plant",
            "password": "s3cret",
        })
        .to_string()
        .as_str(),
    );
    assert_eq!(status, 200, "{body}");
    assert_eq!(
        json_body(&body),
        json!({"ok": false, "error": "unreachable"})
    );

    let (status, _) = post(&base, "/api/probe", Some(WEB_TOKEN), "{}");
    assert_eq!(status, 400, "a body without a target is malformed");
    let (status, body) = post(
        &base,
        "/api/probe",
        Some(WEB_TOKEN),
        json!({"url": "127.0.0.1:8080"}).to_string().as_str(),
    );
    assert_eq!(status, 400, "{body}");
    assert_eq!(
        json_body(&body),
        json!({"ok": false, "error": "invalid url"})
    );
}
