use std::collections::VecDeque;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use indexmap::IndexMap;
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
        metadata: Arc::new(Mutex::new(harness.runtime().metadata_snapshot())),
        sources,
        io_snapshot: Arc::new(Mutex::new(None)),
        io_snapshot_seen_ms: Arc::new(std::sync::atomic::AtomicU64::new(0)),
        pending_restart: Arc::new(Mutex::new(None)),
        auth_token: Arc::new(Mutex::new(None)),
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
        match start_web_server(&config, Arc::clone(&state), None, None, None, None) {
            Ok(_server) => {
                let base = format!("http://{listen}");
                wait_for_hmi_ready(&listen);
                return base;
            }
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

fn wait_for_hmi_ready(addr: &str) {
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline {
        if raw_get(addr, "/hmi", Duration::from_millis(250))
            .is_ok_and(|response| response.starts_with("HTTP/1.1 200"))
        {
            return;
        }
        thread::sleep(Duration::from_millis(25));
    }
    panic!("web server did not become reachable at {addr}");
}

fn raw_get(addr: &str, path: &str, timeout: Duration) -> std::io::Result<String> {
    let mut stream = TcpStream::connect(addr)?;
    stream.set_read_timeout(Some(timeout))?;
    stream.set_write_timeout(Some(timeout))?;
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n"
    )?;
    stream.flush()?;
    let mut response = String::new();
    stream.read_to_string(&mut response)?;
    Ok(response)
}

fn raw_post(
    addr: &str,
    path: &str,
    body: &str,
    timeout: Duration,
) -> std::io::Result<String> {
    let mut stream = TcpStream::connect(addr)?;
    stream.set_read_timeout(Some(timeout))?;
    stream.set_write_timeout(Some(timeout))?;
    write!(
        stream,
        "POST {path} HTTP/1.1\r\n\
         Host: {addr}\r\n\
         Content-Type: application/json\r\n\
         Content-Length: {}\r\n\
         Connection: close\r\n\
         \r\n\
         {body}",
        body.len()
    )?;
    stream.flush()?;
    let mut response = String::new();
    stream.read_to_string(&mut response)?;
    Ok(response)
}

fn open_incomplete_pair_claim(addr: &str) -> TcpStream {
    let mut stream = TcpStream::connect(addr).expect("connect slow request");
    stream
        .set_write_timeout(Some(Duration::from_millis(250)))
        .expect("set slow request write timeout");
    write!(
        stream,
        "POST /api/pair/claim HTTP/1.1\r\n\
         Host: {addr}\r\n\
         Content-Type: application/json\r\n\
         Content-Length: 4096\r\n\
         Connection: keep-alive\r\n\
         \r\n\
         {{\"code\":\""
    )
    .expect("write incomplete pair claim");
    stream.flush().expect("flush incomplete pair claim");
    stream
}

#[test]
fn incomplete_body_does_not_block_unrelated_hmi_request() {
    let state = control_state("PROGRAM Main\nEND_PROGRAM\n");
    let base = start_test_server(state);
    let addr = base
        .strip_prefix("http://")
        .expect("test server base should be http");

    let slow_stream = open_incomplete_pair_claim(addr);
    thread::sleep(Duration::from_millis(100));

    let response = raw_get(addr, "/hmi", Duration::from_secs(1));
    assert!(
        response
            .as_ref()
            .is_ok_and(|response| response.starts_with("HTTP/1.1 200")),
        "incomplete POST body must not block unrelated /hmi traffic, got {response:?}"
    );

    drop(slow_stream);
    wait_for_hmi_ready(addr);
}

#[test]
fn saturated_body_lane_rejects_promptly_without_blocking_hmi_and_recovers() {
    let state = control_state("PROGRAM Main\nEND_PROGRAM\n");
    let base = start_test_server(state);
    let addr = base
        .strip_prefix("http://")
        .expect("test server base should be http");

    let slow_streams = (0..4)
        .map(|_| open_incomplete_pair_claim(addr))
        .collect::<Vec<_>>();
    thread::sleep(Duration::from_millis(100));

    let overloaded = raw_post(
        addr,
        "/api/pair/claim",
        r#"{"code":"invalid"}"#,
        Duration::from_secs(1),
    )
    .expect("saturated body lane must respond instead of blocking");
    assert!(
        overloaded.starts_with("HTTP/1.1 503"),
        "saturated body lane must return 503, got {overloaded:?}"
    );
    assert!(
        overloaded.contains("\"denial_code\":\"server_busy\""),
        "saturated response must carry server_busy, got {overloaded:?}"
    );

    let hmi = raw_get(addr, "/hmi", Duration::from_secs(1))
        .expect("read lane must remain responsive while body lane is saturated");
    assert!(
        hmi.starts_with("HTTP/1.1 200"),
        "body saturation must not block /hmi, got {hmi:?}"
    );

    drop(slow_streams);
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        let response = raw_post(
            addr,
            "/api/pair/claim",
            r#"{"code":"invalid"}"#,
            Duration::from_millis(500),
        );
        if response
            .as_ref()
            .is_ok_and(|response| response.starts_with("HTTP/1.1 200"))
        {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "body lane did not recover after slow clients disconnected: {response:?}"
        );
        thread::sleep(Duration::from_millis(25));
    }
}
