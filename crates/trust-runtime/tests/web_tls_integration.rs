use std::collections::VecDeque;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
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
use trust_runtime::metrics::RuntimeMetrics;
use trust_runtime::scheduler::{ResourceCommand, ResourceControl, StdClock};
use trust_runtime::security::{rustls_client_config, TlsMaterials};
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
            tls: true,
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

fn control_state(source: &str) -> Arc<ControlState> {
    let mut harness = TestHarness::from_source(source).expect("build harness");
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
        path: std::path::PathBuf::from("main.st"),
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

fn https_get_raw(listen: &str, path: &str, materials: &TlsMaterials) -> anyhow::Result<String> {
    let config = rustls_client_config(materials)
        .map_err(|err| anyhow::anyhow!("build tls client config: {err}"))?;
    let server_name =
        rustls::ServerName::try_from("localhost").map_err(|_| anyhow::anyhow!("server name"))?;
    let tcp = TcpStream::connect(listen)?;
    let connection = rustls::ClientConnection::new(config, server_name)?;
    let mut stream = rustls::StreamOwned::new(connection, tcp);
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n"
    )?;
    stream.flush()?;
    let mut response = String::new();
    stream.read_to_string(&mut response)?;
    Ok(response)
}

fn tls_materials() -> Arc<TlsMaterials> {
    let cert = include_bytes!("fixtures/tls/server-cert.pem").to_vec();
    let key = include_bytes!("fixtures/tls/server-key.pem").to_vec();
    Arc::new(TlsMaterials {
        cert_path: std::path::PathBuf::from("tests/fixtures/tls/server-cert.pem"),
        key_path: std::path::PathBuf::from("tests/fixtures/tls/server-key.pem"),
        ca_path: Some(std::path::PathBuf::from(
            "tests/fixtures/tls/server-cert.pem",
        )),
        certificate_pem: cert.clone(),
        private_key_pem: key,
        ca_pem: cert,
    })
}

#[test]
fn web_tls_handshake_and_downgrade_prevention() {
    let state = control_state("PROGRAM Main\nEND_PROGRAM\n");
    let listen = format!("127.0.0.1:{}", reserve_loopback_port());
    let config = WebConfig {
        enabled: true,
        listen: SmolStr::new(listen.clone()),
        auth: WebAuthMode::Local,
        tls: true,
    };
    let tls = tls_materials();
    let _server = start_web_server(&config, state, None, None, None, Some(tls.clone()))
        .expect("start tls web server");

    let mut response = None;
    let mut last_error = String::new();
    for _ in 0..240 {
        match https_get_raw(&listen, "/hmi", &tls) {
            Ok(body) => {
                response = Some(body);
                break;
            }
            Err(err) => {
                last_error = err.to_string();
                thread::sleep(Duration::from_millis(20));
            }
        }
    }
    let response = response.unwrap_or_else(|| panic!("https response (last error: {last_error})"));
    assert!(response.starts_with("HTTP/1.1 200"));
    assert!(response.contains("truST HMI"));

    let mut plain = TcpStream::connect(&listen).expect("connect plain http socket");
    plain
        .set_read_timeout(Some(Duration::from_millis(250)))
        .expect("set read timeout");
    plain
        .write_all(b"GET /hmi HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
        .expect("write plaintext request");
    let mut buffer = [0_u8; 32];
    match plain.read(&mut buffer) {
        Ok(0) | Err(_) => {}
        Ok(count) => {
            let text = String::from_utf8_lossy(&buffer[..count]);
            assert!(
                !text.starts_with("HTTP/1.1"),
                "plaintext downgrade unexpectedly returned HTTP response"
            );
        }
    }
}
