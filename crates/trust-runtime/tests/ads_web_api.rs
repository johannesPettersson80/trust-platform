use std::collections::VecDeque;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use indexmap::IndexMap;
use serde_json::json;
use smol_str::SmolStr;
use trust_runtime::config::{ControlMode, WebAuthMode, WebConfig};
use trust_runtime::control::{ControlState, HmiRuntimeDescriptor, SourceFile, SourceRegistry};
use trust_runtime::debug::{DebugSnapshot, DebugVariableHandles};
use trust_runtime::error::RuntimeError;
use trust_runtime::harness::TestHarness;
use trust_runtime::metrics::RuntimeMetrics;
use trust_runtime::scheduler::{ResourceCommand, ResourceControl, StdClock};
use trust_runtime::settings::{
    BaseSettings, DiscoverySettings, MeshSettings, RuntimeSettings, SimulationSettings, WebSettings,
};
use trust_runtime::watchdog::{FaultPolicy, RetainMode, WatchdogPolicy};
use trust_runtime::web::start_web_server;

#[test]
fn ads_web_status_and_import_symbols_route_to_control() {
    let state = control_state();
    let base = start_test_server(state);

    let status = get_json(&format!("{base}/api/ads/status"));
    assert_eq!(
        status.get("ok").and_then(serde_json::Value::as_bool),
        Some(true)
    );
    assert_eq!(
        status
            .get("result")
            .and_then(|result| result.get("overall"))
            .and_then(serde_json::Value::as_str),
        Some("disabled")
    );

    let imported = post_json(
        &format!("{base}/api/ads/import-symbols"),
        ads_import_symbols_params(),
    );
    assert_eq!(
        imported.get("ok").and_then(serde_json::Value::as_bool),
        Some(true)
    );
    assert_eq!(
        imported
            .get("result")
            .and_then(|result| result.get("candidates"))
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(1)
    );
}

#[test]
fn ads_web_status_uses_internal_token_for_local_web_auth() {
    let state = control_state_with_auth(Some("web-local-control-token"));
    let base = start_test_server(state);

    let status = get_json(&format!("{base}/api/ads/status"));
    assert_eq!(
        status.get("ok").and_then(serde_json::Value::as_bool),
        Some(true),
        "local web auth should satisfy token-protected control dispatch: {status:#}"
    );
    assert_eq!(
        status
            .get("result")
            .and_then(|result| result.get("overall"))
            .and_then(serde_json::Value::as_str),
        Some("disabled")
    );
}

#[test]
fn control_proxy_does_not_forward_local_internal_control_token() {
    let state = control_state_with_auth(Some("web-local-control-token"));
    let base = start_test_server(state);
    let (target, captured, join) = start_capture_server();

    let response = post_json(
        &format!("{base}/api/control/proxy"),
        json!({
            "target": target,
            "control_request": {
                "id": 1,
                "type": "status"
            }
        }),
    );

    assert_eq!(
        response.get("ok").and_then(serde_json::Value::as_bool),
        Some(true),
        "proxy response: {response:#}"
    );
    join.join().expect("capture server joined");
    let captured = captured
        .lock()
        .expect("capture lock")
        .clone()
        .expect("captured request");
    assert!(
        !captured.contains("X-Trust-Token"),
        "proxy leaked an X-Trust-Token header:\n{captured}"
    );
    assert!(
        !captured.contains("web-local-control-token"),
        "proxy leaked the internal control token:\n{captured}"
    );
}

#[test]
fn ads_setup_page_assets_are_served_without_runtime_chooser() {
    let state = control_state();
    let base = start_test_server(state);

    let setup = get_text(&format!("{base}/setup/ads"));
    assert!(setup.contains("Beckhoff ADS Setup"));
    assert!(setup.contains("This runtime host"));
    assert!(setup.contains("Open IDE Deploy"));
    assert!(setup.contains("ADS Server"));
    assert!(setup.contains("Expected AMS Net ID"));
    assert!(setup.contains("href=\"/ide\""));
    assert!(setup.contains("Production-ready requires"));
    assert!(!setup.contains("runtime selector"));

    let js = get_text(&format!("{base}/setup/ads.js"));
    assert!(js.contains("/api/ads/status"));
    assert!(js.contains("/api/ads/server/status"));
    assert!(js.contains("serverClientTomlSnippet"));
    assert!(js.contains("doctorAfterDeployBtn"));
    assert!(js.contains("Deploy/reload the generated bundle"));
    assert!(!js.contains("MAIN.Temperature"));
    assert!(!js.contains("runtimeTarget"));
}

#[cfg(feature = "ads-server")]
#[test]
fn ads_server_web_status_symbols_and_route_plan_route_to_control() {
    let state = control_state();
    let base = start_test_server(state);

    let status = get_json(&format!("{base}/api/ads/server/status"));
    assert_eq!(
        status.get("ok").and_then(serde_json::Value::as_bool),
        Some(true)
    );
    assert_eq!(
        status
            .get("result")
            .and_then(|result| result.get("role"))
            .and_then(serde_json::Value::as_str),
        Some("server")
    );
    assert_eq!(
        status
            .get("result")
            .and_then(|result| result.get("exposed_count"))
            .and_then(serde_json::Value::as_u64),
        Some(1)
    );

    let symbols = get_json(&format!("{base}/api/ads/server/symbols"));
    assert_eq!(
        symbols.get("ok").and_then(serde_json::Value::as_bool),
        Some(true)
    );
    assert_eq!(
        symbols
            .get("result")
            .and_then(|result| result.get("symbols"))
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(1)
    );

    let route_plan = post_json(
        &format!("{base}/api/ads/server/route-plan"),
        ads_server_route_plan_params(),
    );
    assert_eq!(
        route_plan.get("ok").and_then(serde_json::Value::as_bool),
        Some(true),
        "route plan response: {route_plan:#}"
    );
    let manual = route_plan
        .get("result")
        .and_then(|result| result.get("artifacts"))
        .and_then(serde_json::Value::as_array)
        .expect("artifacts")
        .iter()
        .find(|artifact| {
            artifact.get("kind").and_then(serde_json::Value::as_str) == Some("manual_steps")
        })
        .and_then(|artifact| artifact.get("content"))
        .and_then(serde_json::Value::as_str)
        .expect("manual route content");
    assert!(manual.contains("truST ADS server"));
    assert!(!manual.contains("ADS Error 1861"));
}

#[cfg(feature = "ads-server")]
#[test]
fn ads_server_web_doctor_job_start_and_status_route_to_control_job_store() {
    let state = control_state();
    let base = start_test_server(state);

    let started = post_json(&format!("{base}/api/ads/server/doctor/start"), json!({}));
    assert_eq!(
        started.get("ok").and_then(serde_json::Value::as_bool),
        Some(true)
    );
    let job_id = started
        .get("result")
        .and_then(|result| result.get("job_id"))
        .and_then(serde_json::Value::as_str)
        .expect("job id")
        .to_string();
    assert!(job_id.starts_with("ads-server-doctor-"));

    let mut final_status = None;
    for _ in 0..30 {
        let status = get_json(&format!(
            "{base}/api/ads/server/doctor/status?job_id={job_id}"
        ));
        assert_eq!(
            status.get("ok").and_then(serde_json::Value::as_bool),
            Some(true)
        );
        let result = status.get("result").cloned().expect("job status");
        match result.get("state").and_then(serde_json::Value::as_str) {
            Some("complete" | "failed") => {
                final_status = Some(result);
                break;
            }
            _ => thread::sleep(Duration::from_millis(10)),
        }
    }

    let final_status = final_status.expect("server doctor job should finish");
    assert_eq!(
        final_status
            .get("state")
            .and_then(serde_json::Value::as_str),
        Some("complete")
    );
    assert_eq!(
        final_status
            .get("report")
            .and_then(|report| report.get("role"))
            .and_then(serde_json::Value::as_str),
        Some("server")
    );
    assert_eq!(
        final_status
            .get("report")
            .and_then(|report| report.get("production_ready"))
            .and_then(serde_json::Value::as_bool),
        Some(false)
    );
}

#[cfg(not(feature = "ads-wire"))]
#[test]
fn ads_web_doctor_job_start_and_status_route_to_control_job_store() {
    let state = control_state();
    let base = start_test_server(state);

    let started = post_json(&format!("{base}/api/ads/doctor/start"), ads_doctor_params());
    assert_eq!(
        started.get("ok").and_then(serde_json::Value::as_bool),
        Some(true)
    );
    let job_id = started
        .get("result")
        .and_then(|result| result.get("job_id"))
        .and_then(serde_json::Value::as_str)
        .expect("job id")
        .to_string();

    let mut final_status = None;
    for _ in 0..30 {
        let status = get_json(&format!("{base}/api/ads/doctor/status?job_id={job_id}"));
        assert_eq!(
            status.get("ok").and_then(serde_json::Value::as_bool),
            Some(true)
        );
        let result = status.get("result").cloned().expect("job status");
        if result.get("state").and_then(serde_json::Value::as_str) == Some("failed") {
            final_status = Some(result);
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }

    let final_status = final_status.expect("doctor job should fail without ads-wire");
    assert!(final_status
        .get("error")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .contains("ads-wire build"));
}

#[cfg(not(feature = "ads-wire"))]
#[test]
fn ads_web_route_add_derives_local_trusted_channel_and_does_not_echo_password() {
    let state = control_state();
    let base = start_test_server(state);

    let response = post_json(&format!("{base}/api/ads/route-add"), ads_route_add_params());
    assert_eq!(
        response.get("ok").and_then(serde_json::Value::as_bool),
        Some(false)
    );
    assert_ne!(
        response
            .get("error_code")
            .and_then(serde_json::Value::as_str),
        Some("untrusted_credential_channel")
    );
    assert!(response
        .get("error")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .contains("ads-wire build"));
    let serialized = serde_json::to_string(&response).expect("serialize response");
    assert!(!serialized.contains("not-persisted"));
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

fn control_state() -> Arc<ControlState> {
    control_state_with_auth(None)
}

fn control_state_with_auth(auth_token: Option<&str>) -> Arc<ControlState> {
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
                ResourceCommand::AdsLiveValues { respond_to } => {
                    let _ = respond_to.send(trust_runtime::ads::AdsLiveValuesSnapshot::new(
                        1,
                        Vec::new(),
                    ));
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
                | ResourceCommand::UpdateWatchdog(_)
                | ResourceCommand::UpdateFaultPolicy(_)
                | ResourceCommand::UpdateRetainSaveInterval(_)
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
        auth_token: Arc::new(Mutex::new(auth_token.map(SmolStr::new))),
        control_requires_auth: auth_token.is_some(),
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
        project_root: None,
        resource_name: SmolStr::new("RESOURCE"),
        io_health: Arc::new(Mutex::new(Vec::new())),
        debug_enabled: Arc::new(AtomicBool::new(true)),
        debug_variables: Arc::new(Mutex::new(DebugVariableHandles::new())),
        hmi_live: Arc::new(Mutex::new(trust_runtime::hmi::HmiLiveState::default())),
        hmi_persistence: None,
        hmi_descriptor,
        historian: None,
        pairing: None,
        ads_doctor_jobs: Arc::new(Mutex::new(
            trust_runtime::control::AdsDoctorJobStore::default(),
        )),
        ads_client_config: Arc::new(Mutex::new(None)),
        opcua_client_config: Arc::new(Mutex::new(None)),
        ads_server_config: Arc::new(Mutex::new(Some(ads_server_runtime_config()))),
        #[cfg(feature = "ads-server")]
        ads_server_runtime: Arc::new(Mutex::new(None)),
    })
}

fn ads_server_runtime_config() -> trust_runtime::ads::server::AdsServerRuntimeConfig {
    trust_runtime::ads::server::AdsServerRuntimeConfig {
        enabled: true,
        listen: Some(SmolStr::new("127.0.0.1")),
        ads_port: 851,
        ams_net_id: Some(trust_ads_core::AmsNetId::new("127.0.0.1.1.1")),
        insecure_transport: true,
        writes_enabled: true,
        expose: vec![SmolStr::new("global.*")],
        writable: vec![SmolStr::new("global.setpoint")],
        clients: vec![trust_runtime::ads::server::AdsServerClientConfig {
            ams_net_id: trust_ads_core::AmsNetId::new("5.23.91.12.1.1"),
            source: trust_runtime::ads::server::AdsServerSourcePin::Cidr(SmolStr::new(
                "127.0.0.0/8",
            )),
        }],
        ..trust_runtime::ads::server::AdsServerRuntimeConfig::default()
    }
}

fn reserve_loopback_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind local port");
    let port = listener.local_addr().expect("read local addr").port();
    drop(listener);
    port
}

fn start_test_server(state: Arc<ControlState>) -> String {
    for _ in 0..16 {
        let port = reserve_loopback_port();
        let listen = format!("127.0.0.1:{port}");
        let config = WebConfig {
            enabled: true,
            listen: SmolStr::new(listen.clone()),
            auth: WebAuthMode::Local,
            tls: false,
        };
        if start_web_server(&config, Arc::clone(&state), None, None, None, None).is_ok() {
            let base = format!("http://{listen}");
            wait_for_server(base.as_str());
            return base;
        }
        thread::sleep(Duration::from_millis(10));
    }
    panic!("start web server: no free loopback port after retries");
}

fn wait_for_server(base: &str) {
    for _ in 0..80 {
        if ureq::get(&format!("{base}/api/ads/status")).call().is_ok() {
            return;
        }
        thread::sleep(Duration::from_millis(25));
    }
    panic!("timed out waiting for web server");
}

fn get_json(url: &str) -> serde_json::Value {
    let mut response = ureq::get(url).call().expect("GET request");
    let body = response.body_mut().read_to_string().expect("read body");
    serde_json::from_str(&body).expect("parse json body")
}

fn get_text(url: &str) -> String {
    let mut response = ureq::get(url).call().expect("GET request");
    response.body_mut().read_to_string().expect("read body")
}

fn post_json(url: &str, payload: serde_json::Value) -> serde_json::Value {
    let mut response = ureq::post(url)
        .header("Content-Type", "application/json")
        .send(payload.to_string())
        .expect("POST request");
    let body = response.body_mut().read_to_string().expect("read body");
    serde_json::from_str(&body).expect("parse json body")
}

fn start_capture_server() -> (String, Arc<Mutex<Option<String>>>, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind capture server");
    let addr = listener.local_addr().expect("capture addr");
    let captured = Arc::new(Mutex::new(None));
    let captured_thread = Arc::clone(&captured);
    let join = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("capture request");
        let request = read_capture_request(&mut stream);
        *captured_thread.lock().expect("capture lock") = Some(request);
        let body = br#"{"ok":true,"result":{"proxied":true}}"#;
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        stream
            .write_all(response.as_bytes())
            .expect("write capture response headers");
        stream.write_all(body).expect("write capture response body");
        stream.flush().expect("flush capture response");
    });
    (format!("http://{addr}"), captured, join)
}

fn read_capture_request(stream: &mut TcpStream) -> String {
    const MAX_CAPTURE_REQUEST_BYTES: usize = 8192;

    // Drain the declared body before closing so Windows does not reset the connection.
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .expect("set capture read timeout");
    let mut request = Vec::new();
    let mut expected_len = None;
    while expected_len.is_none_or(|len| request.len() < len) {
        let mut chunk = [0_u8; 1024];
        let read = stream.read(&mut chunk).expect("read capture request");
        assert!(read > 0, "capture request ended before its body");
        request.extend_from_slice(&chunk[..read]);
        assert!(
            request.len() <= MAX_CAPTURE_REQUEST_BYTES,
            "capture request exceeded {MAX_CAPTURE_REQUEST_BYTES} bytes"
        );

        if expected_len.is_none() {
            let Some(header_end) = request.windows(4).position(|window| window == b"\r\n\r\n")
            else {
                continue;
            };
            let headers = String::from_utf8_lossy(&request[..header_end]);
            let content_length = headers
                .lines()
                .filter_map(|line| line.split_once(':'))
                .find(|(name, _)| name.eq_ignore_ascii_case("Content-Length"))
                .map(|(_, value)| {
                    value
                        .trim()
                        .parse::<usize>()
                        .expect("parse capture Content-Length")
                })
                .unwrap_or(0);
            expected_len = Some(header_end + 4 + content_length);
        }
    }
    String::from_utf8_lossy(&request).to_string()
}

fn ads_import_symbols_params() -> serde_json::Value {
    json!({
        "connection_name": "line1",
        "name_prefix": "line1_",
        "snapshot": {
            "schema_version": 1,
            "route_name": "line1",
            "symbols": [
                {
                    "name": "MAIN.Temperature",
                    "data_type": {
                        "source_name": "REAL",
                        "iec_type": "REAL"
                    },
                    "index_group": 16416,
                    "index_offset": 0,
                    "byte_size": 4,
                    "flags": ["read"]
                }
            ]
        }
    })
}

#[cfg(feature = "ads-server")]
fn ads_server_route_plan_params() -> serde_json::Value {
    json!({
        "route_name": "trust-runtime-server",
        "target": {
            "name": "trust-runtime",
            "ip": "192.168.10.20",
            "ams_net_id": "192.168.10.20.1.1",
            "ams_port": 851,
        },
        "local": {
            "host_name": "line-controller-1",
            "chosen_ip": "192.168.10.20",
            "ams_net_id": "192.168.10.20.1.1",
            "nic": "eth0",
            "candidates": [],
            "classification": "lan",
        },
        "channel": "local_cli_direct_add_route",
    })
}

#[cfg(not(feature = "ads-wire"))]
fn ads_doctor_params() -> serde_json::Value {
    json!({
        "target_ip": "192.168.10.5",
        "target_identity": {
            "name": "CX-1234",
            "ip": "192.168.10.5",
            "ams_net_id": "5.23.91.12.1.1",
            "ams_port": 851,
            "tc_version": "3.1.4024",
        },
        "local_identity": {
            "host_name": "line-controller-1",
            "chosen_ip": "192.168.10.20",
            "ams_net_id": "192.168.10.20.1.1",
            "nic": "eth0",
            "candidates": [],
            "classification": "lan",
        },
        "selected_symbol": "MAIN.Temperature",
    })
}

#[cfg(not(feature = "ads-wire"))]
fn ads_route_add_params() -> serde_json::Value {
    let mut params = ads_route_plan_params();
    params.as_object_mut().expect("route params").insert(
        "credentials".to_string(),
        json!({
            "username": "Administrator",
            "password": "not-persisted",
        }),
    );
    params
}

#[cfg(not(feature = "ads-wire"))]
fn ads_route_plan_params() -> serde_json::Value {
    json!({
        "route_name": "trust-runtime-line-controller-1",
        "target": {
            "name": "CX-1234",
            "ip": "192.168.10.5",
            "ams_net_id": "5.23.91.12.1.1",
            "ams_port": 851,
            "tc_version": "3.1.4024",
        },
        "local": {
            "host_name": "line-controller-1",
            "chosen_ip": "192.168.10.20",
            "ams_net_id": "192.168.10.20.1.1",
            "nic": "eth0",
            "candidates": [],
            "classification": "lan",
        }
    })
}
