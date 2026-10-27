//! A project with HMI files and a running program behind the runtime's web server.

use std::collections::VecDeque;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use indexmap::IndexMap;
use serde_json::{json, Value};
use smol_str::SmolStr;
use trust_runtime::config::{ControlMode, WebAuthMode, WebConfig};
use trust_runtime::control::{ControlState, SourceFile, SourceRegistry};
use trust_runtime::harness::TestHarness;
use trust_runtime::hmi::access::config::HmiAccessConfig;
use trust_runtime::linux_rt::LinuxRtRuntimeStatus;
use trust_runtime::metrics::RuntimeMetrics;
use trust_runtime::scheduler::{ResourceHandle, ResourceRunner, StdClock};
use trust_runtime::settings::{
    BaseSettings, DiscoverySettings, MeshSettings, RuntimeSettings, SimulationSettings, WebSettings,
};
use trust_runtime::watchdog::{FaultPolicy, RetainMode, WatchdogPolicy};
use trust_runtime::web::start_web_server;

pub const PROGRAM: &str = r#"
FUNCTION_BLOCK Pump
VAR_INPUT
    StartCmd : BOOL;
END_VAR
VAR_OUTPUT
    Running : BOOL;
    Speed : REAL;
END_VAR
    Running := StartCmd;
    IF Running THEN
        Speed := 1460.0;
    END_IF;
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    P101 : Pump;
    Levels : ARRAY[1..3] OF INT := [10, 20, 30];
END_VAR
    P101(StartCmd := TRUE);
END_PROGRAM
"#;

/// The HMI library and a program calling its block, as every project with screens has: the
/// runtime serves the screens at the call (specification 39, §18 Phase 5).
const HMI_LIBRARY: &str = include_str!("../../../../libraries/hmi/src/hmi.st");
const SCREENS: &str = r#"
PROGRAM Screens
VAR
    Display : Hmi.Display;
END_VAR
    Display();
END_PROGRAM
"#;

pub const SCREEN: &str = "format = 1\nsize = [1280, 720]\ntheme = \"dark\"\nsymbols = \"flat\"\n";

pub const OVERVIEW: &str = r#"format = 1
title = "Overview"

[[object]]
id = "speed"
type = "value"
at = [40, 40]
size = [240, 96]
label = "Pump speed"

[object.bind]
value = "Main.P101.Speed"

[[object]]
id = "running"
type = "status"
at = [40, 160]
size = [240, 48]

[object.bind]
state = "Main.P101.Running"

[[object]]
id = "levels"
type = "table"
at = [320, 40]
size = [320, 200]

[object.bind]
value = "Main.Levels"

[[object]]
id = "gone"
type = "value"
at = [40, 240]
size = [240, 96]

[object.bind]
value = "Main.Gone"
"#;

pub const TRENDS: &str = "format = 1\ntitle = \"Trends\"\n";

/// A fresh project folder under the system's temporary folder.
pub fn project(name: &str, files: &[(&str, &str)]) -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "trust-hmi-display-{name}-{}-{stamp}",
        std::process::id()
    ));
    for (path, text) in files {
        let file = root.join(path);
        std::fs::create_dir_all(file.parent().expect("a parent folder")).expect("create folders");
        std::fs::write(file, text).expect("write a project file");
    }
    std::fs::create_dir_all(&root).expect("create the project folder");
    root
}

/// The usual project: a screen file and two pages.
pub fn hmi_project(name: &str) -> PathBuf {
    project(
        name,
        &[
            ("hmi/screen.toml", SCREEN),
            ("hmi/pages/overview.toml", OVERVIEW),
            ("hmi/pages/trends.toml", TRENDS),
        ],
    )
}

pub struct Running {
    pub base: String,
    pub handle: ResourceHandle<StdClock>,
    /// The runtime's control state, the one the web server reads; a test sets its settings and
    /// metrics through [`set_simulation`] and [`set_achieved_speed`].
    pub control: Arc<ControlState>,
}

impl Drop for Running {
    fn drop(&mut self) {
        self.handle.stop();
        let _ = self.handle.join();
    }
}

/// Turns simulation on or off at `time_scale` in the running runtime's settings, as its
/// `simulation` section does at launch. The stream reads the settings at its next update.
pub fn set_simulation(running: &Running, enabled: bool, time_scale: u32) {
    let mut settings = running
        .control
        .settings
        .lock()
        .expect("the runtime settings");
    settings.simulation = SimulationSettings {
        enabled,
        time_scale,
        mode_label: SmolStr::new(if enabled { "simulation" } else { "production" }),
        warning: SmolStr::new(""),
    };
}

/// Sets the achieved simulation speed of the runtime's one resource; `None` clears it, as the
/// scheduler does until it has measured a speed (specification 46, §6.1, item 3).
pub fn set_achieved_speed(running: &Running, achieved: Option<f64>) {
    running
        .control
        .metrics
        .lock()
        .expect("the runtime metrics")
        .set_simulation_achieved_time_scale(1, achieved);
}

/// Starts the program on a real resource with a 10 ms scan and the web server on the project.
pub fn start(project_root: &Path, auth: WebAuthMode, auth_token: Option<&str>) -> Running {
    start_with_access(project_root, auth, auth_token, HmiAccessConfig::default())
}

/// Like [`start`], with HMI users, session times and a write policy (§12.8).
pub fn start_with_access(
    project_root: &Path,
    auth: WebAuthMode,
    auth_token: Option<&str>,
    access: HmiAccessConfig,
) -> Running {
    start_with(project_root, auth, auth_token, access, PROGRAM, None)
}

/// Like [`start_with_access`], with another program and the runtime's audit records sent to
/// `audit`.
pub fn start_with(
    project_root: &Path,
    auth: WebAuthMode,
    auth_token: Option<&str>,
    access: HmiAccessConfig,
    program: &str,
    audit: Option<std::sync::mpsc::Sender<trust_runtime::control::ControlAuditEvent>>,
) -> Running {
    start_inner(
        project_root,
        auth,
        auth_token,
        access,
        &[program],
        audit,
        false,
    )
}

/// Like [`start`], with another program and the runtime's historian recording every value each
/// 100 ms into the project's `history/historian.jsonl`.
pub fn start_with_historian(project_root: &Path, program: &str) -> Running {
    start_inner(
        project_root,
        WebAuthMode::Local,
        None,
        HmiAccessConfig::default(),
        &[program],
        None,
        true,
    )
}

/// Keeps library and application USING scopes separate for multi-source fixtures.
pub fn start_with_sources(
    project_root: &Path,
    auth: WebAuthMode,
    auth_token: Option<&str>,
    access: HmiAccessConfig,
    programs: &[&str],
    audit: Option<std::sync::mpsc::Sender<trust_runtime::control::ControlAuditEvent>>,
) -> Running {
    start_inner(
        project_root,
        auth,
        auth_token,
        access,
        programs,
        audit,
        false,
    )
}

fn start_inner(
    project_root: &Path,
    auth: WebAuthMode,
    auth_token: Option<&str>,
    access: HmiAccessConfig,
    programs: &[&str],
    audit: Option<std::sync::mpsc::Sender<trust_runtime::control::ControlAuditEvent>>,
    historian: bool,
) -> Running {
    let mut sources = vec![HMI_LIBRARY];
    sources.extend_from_slice(programs);
    sources.push(SCREENS);
    let mut harness = TestHarness::from_sources(&sources).expect("the program compiles");
    let debug = trust_runtime::debug::DebugControl::new();
    harness.runtime_mut().set_debug_control(debug.clone());
    let historian = historian.then(|| {
        let service = trust_runtime::historian::HistorianService::new(
            trust_runtime::historian::HistorianConfig {
                enabled: true,
                sample_interval_ms: 100,
                history_path: project_root.join("history/historian.jsonl"),
                prometheus_enabled: false,
                ..trust_runtime::historian::HistorianConfig::default()
            },
            Some(project_root),
        )
        .expect("the historian starts");
        service.clone().start_sampler(debug.clone());
        service
    });
    let handle = ResourceRunner::new(
        harness.into_runtime(),
        StdClock::new(),
        trust_runtime::value::Duration::from_millis(10),
    )
    .spawn("hmi-display-test")
    .expect("the resource starts");
    let state = Arc::new(ControlState {
        debug,
        identify: trust_runtime::io::IdentifyHandle::default(),
        resource: handle.control(),
        // Empty, as `trust-runtime run` gives it: the web server reads the running program's
        // image, never a description of its sources (specification 39, §21.2, §21.4).
        sources: SourceRegistry::new(
            programs
                .iter()
                .enumerate()
                .map(|(index, program)| SourceFile {
                    id: (index + 1) as u32,
                    path: PathBuf::from(format!("source-{index}.st")),
                    text: (*program).to_string(),
                })
                .collect(),
        ),
        io_snapshot: Arc::new(Mutex::new(None)),
        io_snapshot_seen_ms: Arc::new(std::sync::atomic::AtomicU64::new(0)),
        pending_restart: Arc::new(Mutex::new(None)),
        auth_token: Arc::new(Mutex::new(auth_token.map(SmolStr::new))),
        control_requires_auth: auth_token.is_some(),
        control_mode: Arc::new(Mutex::new(ControlMode::Debug)),
        audit_tx: audit,
        metrics: Arc::new(Mutex::new(RuntimeMetrics::default())),
        events: Arc::new(Mutex::new(VecDeque::new())),
        settings: Arc::new(Mutex::new(settings())),
        discovery: Arc::new(trust_runtime::discovery::DiscoveryState::new()),
        mesh_topology: Arc::new(Mutex::new(None)),
        realtime_status: Arc::new(Mutex::new(LinuxRtRuntimeStatus::from_config(
            trust_runtime::linux_rt::LinuxRtConfig::default(),
        ))),
        web_listener_bound: Arc::new(AtomicBool::new(false)),
        opcua_server_bound: Arc::new(AtomicBool::new(false)),
        project_root: Some(project_root.to_path_buf()),
        plant_service: Arc::new(trust_runtime::control::PlantControlService::default()),
        experiments: trust_runtime::control::ControlExperimentService::default(),
        controllers: trust_runtime::control::ControllerParameterService::default(),
        searches: trust_runtime::control::search::SearchService::default(),
        resource_name: SmolStr::new("RESOURCE"),
        runtime_id: SmolStr::new("hmi-display-test"),
        io_health: Arc::new(Mutex::new(Vec::new())),
        debug_enabled: Arc::new(AtomicBool::new(true)),
        historian,
        openot_persistence_status: None,
        pairing: None,
        ads_doctor_jobs: Arc::new(Mutex::new(
            trust_runtime::control::AdsDoctorJobStore::default(),
        )),
        search: Default::default(),
        ads_client_config: Arc::new(Mutex::new(None)),
        opcua_client_config: Arc::new(Mutex::new(None)),
        ads_server_config: Arc::new(Mutex::new(None)),
        #[cfg(feature = "ads-server")]
        ads_server_runtime: Arc::new(Mutex::new(None)),
        transfers: trust_runtime::control::new_transfer_session(),
    });
    let control = Arc::clone(&state);
    let listen = format!("127.0.0.1:{}", free_port());
    let config = WebConfig {
        enabled: true,
        listen: SmolStr::new(listen.clone()),
        auth,
        tls: false,
        hmi: access,
    };
    let _server = start_web_server(
        &config,
        state,
        None,
        None,
        Some(project_root.to_path_buf()),
        None,
    )
    .expect("the web server starts");
    let base = format!("http://{listen}");
    for _ in 0..80 {
        if ureq::get(&format!("{base}/")).call().is_ok() {
            return Running {
                base,
                handle,
                control,
            };
        }
        thread::sleep(Duration::from_millis(25));
    }
    panic!("the web server did not start at {base}");
}

fn free_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind a local port");
    listener.local_addr().expect("the local address").port()
}

fn settings() -> RuntimeSettings {
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

/// A JSON request; the status, the JSON answer (null when there is none) and any session
/// cookie set.
pub fn http_request(
    method: &str,
    base: &str,
    path: &str,
    cookie: Option<&str>,
    body: Option<Value>,
) -> (u16, Value, String) {
    let url = format!("{base}{path}");
    let mut response = if method == "POST" {
        // The HMI write routes take JSON only (specification 39, §21.5 item 3).
        let mut request = ureq::post(&url)
            .config()
            .http_status_as_error(false)
            .build()
            .header("Content-Type", "application/json");
        if let Some(cookie) = cookie {
            request = request.header("Cookie", cookie);
        }
        request.send(body.unwrap_or(json!({})).to_string())
    } else {
        let mut request = ureq::get(&url).config().http_status_as_error(false).build();
        if let Some(cookie) = cookie {
            request = request.header("Cookie", cookie);
        }
        request.call()
    }
    .unwrap_or_else(|error| panic!("{method} {path}: {error}"));
    let status = response.status().as_u16();
    let set_cookie = response
        .headers()
        .get("Set-Cookie")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string();
    let text = response.body_mut().read_to_string().unwrap_or_default();
    (
        status,
        serde_json::from_str(&text).unwrap_or(Value::Null),
        set_cookie,
    )
}

/// Logs a user in with the password "secret" and returns the session cookie.
pub fn session_cookie(base: &str, user: &str) -> String {
    let (status, body, set_cookie) = http_request(
        "POST",
        base,
        "/api/hmi/login",
        None,
        Some(json!({ "user": user, "password": "secret" })),
    );
    assert_eq!(status, 200, "{body}");
    set_cookie.split(';').next().unwrap_or_default().to_string()
}
