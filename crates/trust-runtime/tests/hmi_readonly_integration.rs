use std::collections::{BTreeSet, VecDeque};
use std::fs;
use std::io::ErrorKind;
use std::io::{Read, Write};
use std::net::Shutdown;
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use indexmap::IndexMap;
use serde_json::json;
use smol_str::SmolStr;
use trust_runtime::config::{ControlMode, WebAuthMode, WebConfig};
use trust_runtime::control::{ControlState, HmiRuntimeDescriptor, SourceRegistry};
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

fn hmi_control_state_with_root(source: &str, project_root: Option<PathBuf>) -> Arc<ControlState> {
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

    let sources = SourceRegistry::new(vec![trust_runtime::control::SourceFile {
        id: 1,
        path: std::path::PathBuf::from("main.st"),
        text: source.to_string(),
    }]);
    let hmi_descriptor = Arc::new(Mutex::new(HmiRuntimeDescriptor::from_sources(
        project_root.as_deref(),
        &sources,
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
        project_root,
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

fn hmi_control_state(source: &str) -> Arc<ControlState> {
    hmi_control_state_with_root(source, None)
}

fn write_file(path: &Path, text: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent directories");
    }
    fs::write(path, text).expect("write file");
}

fn temp_dir(prefix: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("trust-hmi-readonly-{prefix}-{stamp}"));
    fs::create_dir_all(&root).expect("create temp dir");
    root
}

fn reserve_loopback_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind local port");
    let port = listener.local_addr().expect("read local addr").port();
    drop(listener);
    port
}

fn start_test_server(state: Arc<ControlState>) -> String {
    let port = reserve_loopback_port();
    let listen = format!("127.0.0.1:{port}");
    let config = WebConfig {
        enabled: true,
        listen: SmolStr::new(listen.clone()),
        auth: WebAuthMode::Local,
        tls: false,
    };
    let _server =
        start_web_server(&config, state, None, None, None, None).expect("start web server");
    let base = format!("http://{listen}");
    wait_for_server(&base);
    base
}

fn wait_for_server(base: &str) {
    for _ in 0..80 {
        if ureq::get(&format!("{base}/hmi")).call().is_ok() {
            return;
        }
        thread::sleep(Duration::from_millis(25));
    }
    panic!("web server did not become reachable at {base}");
}

fn post_control(
    base: &str,
    request_type: &str,
    params: Option<serde_json::Value>,
) -> serde_json::Value {
    let mut payload = json!({
        "id": 1u64,
        "type": request_type,
    });
    if let Some(params) = params {
        payload["params"] = params;
    }
    let response = ureq::post(&format!("{base}/api/control"))
        .set("Content-Type", "application/json")
        .send_string(&payload.to_string())
        .expect("post control request");
    let body = response.into_string().expect("read control response body");
    serde_json::from_str(&body).expect("parse control response body")
}

fn websocket_url(base: &str) -> String {
    let authority = base.strip_prefix("http://").unwrap_or(base);
    format!("ws://{authority}/ws/hmi")
}

fn wait_for_ws_event<S>(
    socket: &mut tungstenite::WebSocket<S>,
    expected_type: &str,
    timeout: Duration,
) -> serde_json::Value
where
    S: Read + Write,
{
    let deadline = Instant::now() + timeout;
    loop {
        let message = match socket.read() {
            Ok(message) => message,
            Err(tungstenite::Error::Io(err))
                if matches!(err.kind(), ErrorKind::TimedOut | ErrorKind::WouldBlock) =>
            {
                if Instant::now() >= deadline {
                    break;
                }
                continue;
            }
            Err(err) => panic!("read websocket message: {err}"),
        };
        if !message.is_text() {
            if Instant::now() >= deadline {
                break;
            }
            continue;
        }
        let payload: serde_json::Value = serde_json::from_str(
            message
                .into_text()
                .expect("websocket text payload")
                .as_str(),
        )
        .expect("parse websocket payload");
        if payload
            .get("type")
            .and_then(|value| value.as_str())
            .is_some_and(|value| value == expected_type)
        {
            return payload;
        }
        if Instant::now() >= deadline {
            break;
        }
    }
    panic!("timed out waiting for websocket event type {expected_type}");
}

fn configure_ws_read_timeout(
    socket: &mut tungstenite::WebSocket<tungstenite::stream::MaybeTlsStream<TcpStream>>,
) {
    if let tungstenite::stream::MaybeTlsStream::Plain(stream) = socket.get_mut() {
        stream
            .set_read_timeout(Some(Duration::from_millis(200)))
            .expect("set websocket read timeout");
    }
}

fn percentile_ms(samples: &[u128], percentile: usize) -> u128 {
    assert!(!samples.is_empty(), "samples must not be empty");
    assert!(percentile <= 100, "percentile must be <= 100");
    let mut sorted = samples.to_vec();
    sorted.sort_unstable();
    let rank = ((sorted.len() - 1) * percentile) / 100;
    sorted[rank]
}

fn hmi_fixture_source() -> &'static str {
    r#"
TYPE MODE : (OFF, AUTO); END_TYPE

PROGRAM Main
VAR
    run : BOOL := TRUE;
    // @hmi(min=0, max=100)
    speed : REAL := 42.5;
    mode : MODE := MODE#AUTO;
    name : STRING := 'pump';
END_VAR
END_PROGRAM
"#
}

fn run_node_hmi_script(js_path: &Path, script: &str, context: &str) {
    let output = Command::new("node")
        .arg("-e")
        .arg(script)
        .env("HMI_JS_PATH", js_path)
        .output()
        .expect("run node script");
    assert!(
        output.status.success(),
        "node script failed ({context}): status={:?}, stdout={}, stderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn extract_svg_ids(svg: &str) -> BTreeSet<String> {
    svg.split("id=\"")
        .skip(1)
        .filter_map(|tail| tail.split('"').next())
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn extract_quoted_values_from_lines(text: &str, prefix: &str) -> Vec<String> {
    text.lines()
        .filter_map(|line| {
            let line = line.trim();
            if !line.starts_with(prefix) {
                return None;
            }
            let mut parts = line.splitn(2, '"');
            let _ = parts.next();
            let tail = parts.next()?;
            let value = tail.split('"').next()?.trim();
            if value.is_empty() {
                None
            } else {
                Some(value.to_string())
            }
        })
        .collect()
}

#[test]
fn hmi_dashboard_routes_render_without_manual_layout() {
    let state = hmi_control_state(hmi_fixture_source());
    let base = start_test_server(state);

    let hmi_html = ureq::get(&format!("{base}/hmi"))
        .call()
        .expect("get /hmi")
        .into_string()
        .expect("read /hmi body");
    assert!(hmi_html.contains("truST HMI"));
    assert!(hmi_html.contains("id=\"hmiGroups\""));
    assert!(hmi_html.contains("id=\"pageSidebar\""));

    let hmi_js = ureq::get(&format!("{base}/hmi/app.js"))
        .call()
        .expect("get /hmi/app.js")
        .into_string()
        .expect("read /hmi/app.js body");
    assert!(hmi_js.contains("hmi.schema.get"));
    assert!(hmi_js.contains("hmi.values.get"));
    assert!(hmi_js.contains("hmi.trends.get"));
    assert!(hmi_js.contains("hmi.alarms.get"));
    assert!(hmi_js.contains("hmi.alarm.ack"));
    assert!(hmi_js.contains("connectWebSocketTransport"));
    assert!(hmi_js.contains("/ws/hmi"));
    assert!(hmi_js.contains("hmi.values.delta"));
    assert!(hmi_js.contains("hmi.schema.revision"));
    assert!(hmi_js.contains("renderProcessPage"));
    assert!(hmi_js.contains("/hmi/assets/"));
    assert!(hmi_js.contains("section-grid"));
    assert!(hmi_js.contains("section-widget-grid"));
    assert!(hmi_js.contains("createGaugeRenderer"));
    assert!(hmi_js.contains("createSparklineRenderer"));
    assert!(hmi_js.contains("createBarRenderer"));
    assert!(hmi_js.contains("createTankRenderer"));
    assert!(hmi_js.contains("createIndicatorRenderer"));
    assert!(hmi_js.contains("createToggleRenderer"));
    assert!(hmi_js.contains("createSliderRenderer"));

    let hmi_css = ureq::get(&format!("{base}/hmi/styles.css"))
        .call()
        .expect("get /hmi/styles.css")
        .into_string()
        .expect("read /hmi/styles.css body");
    assert!(hmi_css.contains(".card"));
    assert!(hmi_css.contains(".section-grid"));
    assert!(hmi_css.contains(".hmi-section"));
    assert!(hmi_css.contains(".section-widget-grid"));
    assert!(hmi_css.contains(".widget-gauge"));
    assert!(hmi_css.contains(".widget-sparkline"));
    assert!(hmi_css.contains(".widget-bar"));
    assert!(hmi_css.contains(".widget-tank"));
    assert!(hmi_css.contains(".widget-indicator"));
    assert!(hmi_css.contains(".widget-toggle-control"));
    assert!(hmi_css.contains(".widget-slider-control"));
    assert!(hmi_css.contains("viewport-kiosk"));
    assert!(hmi_css.contains("prefers-color-scheme: dark"));
    assert!(hmi_css.contains("@media (max-width: 680px)"));
    assert!(hmi_css.contains("@media (max-width: 1024px)"));

    let schema = post_control(&base, "hmi.schema.get", None);
    assert_eq!(schema.get("ok").and_then(|v| v.as_bool()), Some(true));
    let widgets = schema
        .get("result")
        .and_then(|v| v.get("widgets"))
        .and_then(|v| v.as_array())
        .expect("schema widgets");
    assert!(
        !widgets.is_empty(),
        "schema should return discovered widgets"
    );
    assert!(schema
        .get("result")
        .and_then(|v| v.get("theme"))
        .and_then(|v| v.get("style"))
        .and_then(|v| v.as_str())
        .is_some());
    assert!(schema
        .get("result")
        .and_then(|v| v.get("pages"))
        .and_then(|v| v.as_array())
        .is_some());
    assert_eq!(
        schema
            .get("result")
            .and_then(|v| v.get("responsive"))
            .and_then(|v| v.get("mode"))
            .and_then(|v| v.as_str()),
        Some("auto")
    );
    assert_eq!(
        schema
            .get("result")
            .and_then(|v| v.get("export"))
            .and_then(|v| v.get("enabled"))
            .and_then(|v| v.as_bool()),
        Some(true)
    );
    assert!(schema
        .get("result")
        .and_then(|v| v.get("pages"))
        .and_then(|v| v.as_array())
        .is_some_and(|pages| pages.iter().any(|page| {
            page.get("kind")
                .and_then(|v| v.as_str())
                .is_some_and(|kind| kind == "trend" || kind == "alarm")
        })));
    let ids = widgets
        .iter()
        .filter_map(|widget| widget.get("id").and_then(|v| v.as_str()))
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let values = post_control(&base, "hmi.values.get", Some(json!({ "ids": ids })));
    assert_eq!(values.get("ok").and_then(|v| v.as_bool()), Some(true));
    assert_eq!(
        values
            .get("result")
            .and_then(|v| v.get("connected"))
            .and_then(|v| v.as_bool()),
        Some(true)
    );

    let trends = post_control(
        &base,
        "hmi.trends.get",
        Some(json!({ "duration_ms": 60_000, "buckets": 32 })),
    );
    assert_eq!(trends.get("ok").and_then(|v| v.as_bool()), Some(true));
    assert!(trends
        .get("result")
        .and_then(|v| v.get("series"))
        .and_then(|v| v.as_array())
        .is_some_and(|series| !series.is_empty()));

    let alarms = post_control(&base, "hmi.alarms.get", Some(json!({ "limit": 10 })));
    assert_eq!(alarms.get("ok").and_then(|v| v.as_bool()), Some(true));
}

#[test]
fn hmi_schema_exposes_section_spans_and_widget_spans_for_web_layout() {
    let root = temp_dir("section-layout");
    write_file(
        &root.join("hmi/overview.toml"),
        r##"
title = "Overview"
kind = "dashboard"

[[section]]
title = "Drive Controls"
span = 8

[[section.widget]]
type = "gauge"
bind = "Main.speed"
label = "Speed"
span = 6
min = 0
max = 100

[[section.widget]]
type = "indicator"
bind = "Main.run"
label = "Running"
span = 3
on_color = "#22c55e"
off_color = "#94a3b8"
"##,
    );

    let state = hmi_control_state_with_root(hmi_fixture_source(), Some(root.clone()));
    let base = start_test_server(state);
    let schema = post_control(&base, "hmi.schema.get", None);
    assert_eq!(schema.get("ok").and_then(|v| v.as_bool()), Some(true));

    let result = schema.get("result").expect("schema result");
    let overview = result
        .get("pages")
        .and_then(|v| v.as_array())
        .and_then(|pages| {
            pages
                .iter()
                .find(|page| page.get("id").and_then(|v| v.as_str()) == Some("overview"))
        })
        .expect("overview page");
    let first_section = overview
        .get("sections")
        .and_then(|v| v.as_array())
        .and_then(|sections| sections.first())
        .expect("overview section");
    assert_eq!(
        first_section.get("title").and_then(|v| v.as_str()),
        Some("Drive Controls")
    );
    assert_eq!(first_section.get("span").and_then(|v| v.as_u64()), Some(8));
    assert!(first_section
        .get("widget_ids")
        .and_then(|v| v.as_array())
        .is_some_and(|ids| ids.len() == 2));

    let speed_widget = result
        .get("widgets")
        .and_then(|v| v.as_array())
        .and_then(|widgets| {
            widgets
                .iter()
                .find(|widget| widget.get("path").and_then(|v| v.as_str()) == Some("Main.speed"))
        })
        .expect("speed widget");
    assert_eq!(
        speed_widget.get("section_title").and_then(|v| v.as_str()),
        Some("Drive Controls")
    );
    assert_eq!(
        speed_widget.get("widget_span").and_then(|v| v.as_u64()),
        Some(6)
    );

    fs::remove_dir_all(root).ok();
}

#[test]
fn hmi_standalone_export_bundle_contains_assets_routes_and_config() {
    let state = hmi_control_state(hmi_fixture_source());
    let base = start_test_server(state);

    let export = ureq::get(&format!("{base}/hmi/export.json"))
        .call()
        .expect("get /hmi/export.json")
        .into_string()
        .expect("read export body");
    let payload: serde_json::Value = serde_json::from_str(&export).expect("parse export body");

    assert_eq!(payload.get("version").and_then(|v| v.as_u64()), Some(2));
    assert_eq!(
        payload.get("entrypoint").and_then(|v| v.as_str()),
        Some("hmi/index.html")
    );
    assert!(payload
        .get("routes")
        .and_then(|v| v.as_array())
        .is_some_and(|routes| {
            routes.iter().any(|route| route.as_str() == Some("/hmi"))
                && routes
                    .iter()
                    .any(|route| route.as_str() == Some("/hmi/app.js"))
                && routes.iter().any(|route| route.as_str() == Some("/ws/hmi"))
        }));
    assert_eq!(
        payload
            .get("config")
            .and_then(|v| v.get("ws_route"))
            .and_then(|v| v.as_str()),
        Some("/ws/hmi")
    );
    assert!(payload
        .get("config")
        .and_then(|v| v.get("descriptor"))
        .is_some_and(serde_json::Value::is_null));
    assert!(payload
        .get("assets")
        .and_then(|v| v.as_object())
        .is_some_and(|assets| {
            assets.contains_key("hmi/index.html")
                && assets.contains_key("hmi/styles.css")
                && assets.contains_key("hmi/app.js")
        }));
    let app_js = payload
        .get("assets")
        .and_then(|v| v.get("hmi/app.js"))
        .and_then(|v| v.as_str())
        .expect("hmi app js");
    assert!(
        app_js.contains("function renderProcessPage")
            && app_js.contains("createGaugeRenderer")
            && app_js.contains("kind === 'sparkline'"),
        "exported app.js should include process-page and rich-widget renderers"
    );
    assert!(payload
        .get("config")
        .and_then(|v| v.get("schema"))
        .and_then(|v| v.get("widgets"))
        .and_then(|v| v.as_array())
        .is_some_and(|widgets| !widgets.is_empty()));
}

#[test]
fn hmi_standalone_export_bundle_includes_resolved_descriptor_when_hmi_dir_present() {
    let root = temp_dir("export-descriptor");
    write_file(
        &root.join("hmi/_config.toml"),
        r##"
[theme]
style = "industrial"
accent = "#ff6b00"

[write]
enabled = true
allow = ["Main.run"]
"##,
    );
    write_file(
        &root.join("hmi/overview.toml"),
        r##"
title = "Overview"
kind = "dashboard"
order = 0

[[section]]
title = "Drive"
span = 8

[[section.widget]]
type = "gauge"
bind = "Main.speed"
label = "Speed"
min = 0
max = 100
"##,
    );

    let state = hmi_control_state_with_root(hmi_fixture_source(), Some(root.clone()));
    let base = start_test_server(state);

    let export = ureq::get(&format!("{base}/hmi/export.json"))
        .call()
        .expect("get /hmi/export.json")
        .into_string()
        .expect("read export body");
    let payload: serde_json::Value = serde_json::from_str(&export).expect("parse export body");

    assert_eq!(payload.get("version").and_then(|v| v.as_u64()), Some(2));
    let descriptor = payload
        .get("config")
        .and_then(|v| v.get("descriptor"))
        .expect("descriptor field");
    assert!(descriptor.is_object(), "descriptor should be object");

    assert_eq!(
        descriptor
            .get("config")
            .and_then(|v| v.get("theme"))
            .and_then(|v| v.get("style"))
            .and_then(|v| v.as_str()),
        Some("industrial")
    );
    assert!(descriptor
        .get("config")
        .and_then(|v| v.get("write"))
        .and_then(|v| v.get("allow"))
        .and_then(|v| v.as_array())
        .is_some_and(|allow| allow.iter().any(|entry| entry.as_str() == Some("Main.run"))));
    assert!(descriptor
        .get("pages")
        .and_then(|v| v.as_array())
        .is_some_and(|pages| pages.iter().any(|page| {
            page.get("id").and_then(|v| v.as_str()) == Some("overview")
                && page.get("kind").and_then(|v| v.as_str()) == Some("dashboard")
                && page
                    .get("sections")
                    .and_then(|v| v.as_array())
                    .is_some_and(|sections| {
                        sections.iter().any(|section| {
                            section.get("title").and_then(|v| v.as_str()) == Some("Drive")
                        })
                    })
        })));

    fs::remove_dir_all(root).ok();
}

#[test]
fn hmi_standalone_export_bundle_validates_offline_bootstrap_with_embedded_schema() {
    let state = hmi_control_state(hmi_fixture_source());
    let base = start_test_server(state);

    let export = ureq::get(&format!("{base}/hmi/export.json"))
        .call()
        .expect("get /hmi/export.json")
        .into_string()
        .expect("read export body");
    let root = temp_dir("export-offline-run");
    let export_path = root.join("trust-hmi-export.json");
    write_file(&export_path, export.as_str());

    let script = r#"
const fs = require('fs');
const vm = require('vm');
const assert = require('assert');

class ClassList {
  constructor() { this.values = new Set(); }
  add(...names) { for (const name of names) { if (name) { this.values.add(String(name)); } } }
  remove(...names) { for (const name of names) { this.values.delete(String(name)); } }
  contains(name) { return this.values.has(String(name)); }
  toggle(name, force) {
    const value = String(name);
    if (force === true) { this.values.add(value); return true; }
    if (force === false) { this.values.delete(value); return false; }
    if (this.values.has(value)) { this.values.delete(value); return false; }
    this.values.add(value);
    return true;
  }
}

class StyleDecl {
  setProperty(key, value) {
    this[String(key)] = String(value);
  }
}

class FakeElement {
  constructor(tag) {
    this.tagName = String(tag || 'div').toUpperCase();
    this.children = [];
    this.attrs = {};
    this.style = new StyleDecl();
    this.dataset = {};
    this.className = '';
    this.classList = new ClassList();
    this.listeners = new Map();
    this.textContent = '';
    this.type = '';
    this.min = '';
    this.max = '';
    this.step = '';
    this.value = '';
    this.disabled = false;
    this.href = '';
    this._innerHTML = '';
  }
  appendChild(child) {
    this.children.push(child);
    return child;
  }
  setAttribute(key, value) {
    this.attrs[String(key)] = String(value);
  }
  getAttribute(key) {
    return this.attrs[String(key)];
  }
  addEventListener(event, handler) {
    const key = String(event);
    if (!this.listeners.has(key)) {
      this.listeners.set(key, []);
    }
    this.listeners.get(key).push(handler);
  }
  dispatch(event, payload = {}) {
    const handlers = this.listeners.get(String(event)) || [];
    for (const handler of handlers) {
      handler({ target: this, ...payload });
    }
  }
  querySelector(selector) {
    const match = (node) => {
      if (!(node instanceof FakeElement)) {
        return false;
      }
      if (selector.startsWith('.')) {
        const className = selector.slice(1);
        const classes = `${node.className || ''} ${node.attrs.class || ''}`
          .split(/\s+/)
          .filter(Boolean);
        return node.classList.contains(className) || classes.includes(className);
      }
      if (selector.startsWith('#')) {
        return node.attrs.id === selector.slice(1);
      }
      return node.tagName.toLowerCase() === selector.toLowerCase();
    };
    const stack = [...this.children];
    while (stack.length > 0) {
      const node = stack.shift();
      if (match(node)) {
        return node;
      }
      if (node && Array.isArray(node.children)) {
        stack.unshift(...node.children);
      }
    }
    return null;
  }
  get childElementCount() {
    return this.children.length;
  }
  set innerHTML(value) {
    this._innerHTML = String(value || '');
    this.children = [];
  }
  get innerHTML() {
    return this._innerHTML;
  }
}

function responseJson(payload, status = 200) {
  return {
    ok: status >= 200 && status < 300,
    status,
    json: async () => payload,
    text: async () => JSON.stringify(payload),
  };
}

(async () => {
  const bundlePath = process.env.HMI_EXPORT_BUNDLE_PATH;
  assert(bundlePath, 'missing HMI_EXPORT_BUNDLE_PATH');
  const bundle = JSON.parse(fs.readFileSync(bundlePath, 'utf8'));
  assert.strictEqual(bundle.version, 2);
  assert.strictEqual(bundle.entrypoint, 'hmi/index.html');

  const assets = bundle.assets || {};
  const appSource = assets['hmi/app.js'];
  const indexHtml = assets['hmi/index.html'];
  assert(typeof appSource === 'string' && appSource.length > 0, 'missing exported hmi/app.js');
  assert(typeof indexHtml === 'string' && indexHtml.includes('id=\"hmiGroups\"'), 'missing exported hmi/index.html shell');

  const schema = bundle.config && bundle.config.schema;
  assert(schema && Array.isArray(schema.widgets) && schema.widgets.length > 0, 'missing schema widgets');

  const valuesById = {};
  for (const widget of schema.widgets) {
    const type = String(widget.data_type || '').toUpperCase();
    let value = 0;
    if (type === 'BOOL') {
      value = false;
    } else if (type.includes('STRING')) {
      value = 'ok';
    } else if (type.includes('REAL') || type.includes('INT') || type.includes('WORD') || type.includes('BYTE') || type.includes('TIME')) {
      value = 1;
    }
    valuesById[widget.id] = { v: value, q: 'good', ts_ms: Date.now() };
  }

  const ids = [
    ['resourceName', 'p'],
    ['connectionState', 'div'],
    ['freshnessState', 'div'],
    ['modeLabel', 'div'],
    ['themeLabel', 'div'],
    ['exportLink', 'a'],
    ['pageSidebar', 'aside'],
    ['pageContent', 'section'],
    ['hmiGroups', 'section'],
    ['trendPanel', 'section'],
    ['alarmPanel', 'section'],
    ['emptyState', 'section'],
  ];
  const elements = new Map();
  for (const [id, tag] of ids) {
    const node = new FakeElement(tag);
    node.setAttribute('id', id);
    elements.set(id, node);
  }

  const listeners = new Map();
  const noop = () => {};
  const context = {
    console,
    URLSearchParams,
    window: {
      location: { protocol: 'http:', host: 'offline.local', search: '' },
      addEventListener: (event, handler) => {
        listeners.set(String(event), handler);
      },
      setInterval: () => 1,
      clearInterval: noop,
      setTimeout: () => 1,
      clearTimeout: noop,
      innerWidth: 1280,
    },
    document: {
      createElement: (tag) => new FakeElement(tag),
      createElementNS: (_ns, tag) => new FakeElement(tag),
      getElementById: (id) => elements.get(String(id)) || null,
      body: new FakeElement('body'),
      documentElement: { style: new StyleDecl() },
    },
    fetch: async (url, init = {}) => {
      if (url !== '/api/control') {
        return responseJson({ ok: false, error: `unexpected route ${url}` }, 404);
      }
      const payload = JSON.parse(init.body || '{}');
      const type = payload.type;
      if (type === 'hmi.schema.get') {
        return responseJson({ ok: true, result: schema });
      }
      if (type === 'hmi.values.get') {
        const requested = Array.isArray(payload.params?.ids)
          ? payload.params.ids
          : Object.keys(valuesById);
        const values = {};
        for (const id of requested) {
          if (Object.prototype.hasOwnProperty.call(valuesById, id)) {
            values[id] = valuesById[id];
          }
        }
        return responseJson({
          ok: true,
          result: { connected: true, timestamp_ms: Date.now(), values },
        });
      }
      if (type === 'hmi.trends.get') {
        return responseJson({
          ok: true,
          result: { connected: true, timestamp_ms: Date.now(), duration_ms: 60000, buckets: 16, series: [] },
        });
      }
      if (type === 'hmi.alarms.get') {
        return responseJson({
          ok: true,
          result: { connected: true, timestamp_ms: Date.now(), active: [], history: [] },
        });
      }
      if (type === 'hmi.alarm.ack') {
        return responseJson({ ok: true, result: { acknowledged: true } });
      }
      return responseJson({ ok: false, error: `unsupported request type ${type}` }, 400);
    },
    DOMParser: class {
      parseFromString() {
        return { querySelector: () => null, documentElement: null };
      }
    },
  };
  context.window.document = context.document;
  vm.createContext(context);

  const source = `${appSource}\n;globalThis.__hmi_test__ = { state };`;
  vm.runInContext(source, context, { filename: 'exported-hmi-app.js' });

  const ready = listeners.get('DOMContentLoaded');
  assert.strictEqual(typeof ready, 'function', 'DOMContentLoaded init handler not registered');
  await ready();

  const test = context.__hmi_test__;
  assert(test && test.state && test.state.schema, 'schema should load during standalone bootstrap');
  assert.strictEqual(test.state.schema.version, schema.version, 'schema version mismatch');
  assert(test.state.cards.size > 0, 'widget cards should render in standalone bootstrap');
  assert.strictEqual(elements.get('connectionState').textContent, 'Connected');
  assert(
    typeof elements.get('freshnessState').textContent === 'string'
      && elements.get('freshnessState').textContent.includes('freshness:'),
    'freshness badge did not render'
  );
  console.log('ok');
})().catch((error) => {
  console.error(error && error.stack ? error.stack : String(error));
  process.exit(1);
});
"#;

    let output = Command::new("node")
        .arg("-e")
        .arg(script)
        .env("HMI_EXPORT_BUNDLE_PATH", &export_path)
        .output()
        .expect("run node standalone bootstrap validation");
    assert!(
        output.status.success(),
        "standalone bootstrap validation failed: status={:?}, stdout={}, stderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );

    fs::remove_dir_all(root).ok();
}

#[test]
fn hmi_websocket_pushes_values_schema_revision_and_alarm_events() {
    let state = hmi_control_state(hmi_fixture_source());
    let base = start_test_server(state.clone());

    let (mut socket, response) =
        tungstenite::connect(websocket_url(&base)).expect("connect websocket");
    assert_eq!(
        response.status(),
        tungstenite::http::StatusCode::SWITCHING_PROTOCOLS
    );
    configure_ws_read_timeout(&mut socket);

    let value_event = wait_for_ws_event(&mut socket, "hmi.values.delta", Duration::from_secs(3));
    assert!(value_event
        .get("result")
        .and_then(|value| value.get("values"))
        .and_then(|value| value.as_object())
        .is_some_and(|values| !values.is_empty()));

    {
        let mut descriptor = state.hmi_descriptor.lock().expect("lock hmi descriptor");
        descriptor.schema_revision = descriptor.schema_revision.saturating_add(1);
    }

    let revision_event =
        wait_for_ws_event(&mut socket, "hmi.schema.revision", Duration::from_secs(3));
    assert!(revision_event
        .get("result")
        .and_then(|value| value.get("schema_revision"))
        .and_then(|value| value.as_u64())
        .is_some_and(|revision| revision >= 1));

    let alarm_event = wait_for_ws_event(&mut socket, "hmi.alarms.event", Duration::from_secs(3));
    assert!(alarm_event.get("result").is_some());

    let _ = socket.close(None);
}

#[test]
fn hmi_websocket_value_push_meets_local_latency_slo() {
    let state = hmi_control_state(hmi_fixture_source());
    let base = start_test_server(state);
    let mut latencies_ms = Vec::new();
    let samples = 40_u32;

    for _ in 0..samples {
        let (mut socket, response) =
            tungstenite::connect(websocket_url(&base)).expect("connect websocket");
        assert_eq!(
            response.status(),
            tungstenite::http::StatusCode::SWITCHING_PROTOCOLS
        );
        configure_ws_read_timeout(&mut socket);

        let started = Instant::now();
        let payload = wait_for_ws_event(&mut socket, "hmi.values.delta", Duration::from_secs(3));
        let elapsed = started.elapsed();
        assert!(payload
            .get("result")
            .and_then(|value| value.get("values"))
            .and_then(|value| value.as_object())
            .is_some_and(|values| !values.is_empty()));
        latencies_ms.push(elapsed.as_millis());

        let _ = socket.close(None);
    }

    let p95 = percentile_ms(&latencies_ms, 95);
    let p99 = percentile_ms(&latencies_ms, 99);
    assert!(
        p95 <= 100,
        "websocket value push p95 {}ms exceeded 100ms budget",
        p95
    );
    assert!(
        p99 <= 250,
        "websocket value push p99 {}ms exceeded 250ms budget",
        p99
    );
}

#[test]
fn hmi_websocket_forced_failure_polling_recovers_within_one_interval() {
    let state = hmi_control_state(hmi_fixture_source());
    let base = start_test_server(state);
    let schema = post_control(&base, "hmi.schema.get", None);
    let ids = schema
        .get("result")
        .and_then(|value| value.get("widgets"))
        .and_then(|value| value.as_array())
        .expect("schema widgets")
        .iter()
        .filter_map(|widget| widget.get("id").and_then(|value| value.as_str()))
        .map(str::to_owned)
        .collect::<Vec<_>>();
    assert!(!ids.is_empty(), "ids must not be empty");

    let (mut socket, response) =
        tungstenite::connect(websocket_url(&base)).expect("connect websocket");
    assert_eq!(
        response.status(),
        tungstenite::http::StatusCode::SWITCHING_PROTOCOLS
    );
    configure_ws_read_timeout(&mut socket);

    let _ = wait_for_ws_event(&mut socket, "hmi.values.delta", Duration::from_secs(3));
    if let tungstenite::stream::MaybeTlsStream::Plain(stream) = socket.get_mut() {
        let _ = stream.shutdown(Shutdown::Both);
    }
    let _ = socket.close(None);

    let started = Instant::now();
    let values = post_control(&base, "hmi.values.get", Some(json!({ "ids": ids })));
    let elapsed = started.elapsed();
    assert_eq!(values.get("ok").and_then(|v| v.as_bool()), Some(true));
    assert!(
        elapsed <= Duration::from_millis(500),
        "polling fallback recovery exceeded one poll interval: {:?}",
        elapsed
    );
}

#[test]
fn hmi_websocket_reconnect_churn_remains_stable() {
    let state = hmi_control_state(hmi_fixture_source());
    let base = start_test_server(state);
    let churn_attempts = 50_u32;

    for attempt in 0..churn_attempts {
        let (mut socket, response) =
            tungstenite::connect(websocket_url(&base)).expect("connect websocket");
        assert_eq!(
            response.status(),
            tungstenite::http::StatusCode::SWITCHING_PROTOCOLS
        );
        configure_ws_read_timeout(&mut socket);
        let _ = wait_for_ws_event(&mut socket, "hmi.values.delta", Duration::from_secs(3));
        let _ = socket.close(None);

        if attempt % 10 == 0 {
            let schema = post_control(&base, "hmi.schema.get", None);
            assert_eq!(schema.get("ok").and_then(|v| v.as_bool()), Some(true));
        }
    }

    let schema = post_control(&base, "hmi.schema.get", None);
    assert_eq!(schema.get("ok").and_then(|v| v.as_bool()), Some(true));
    let values = post_control(&base, "hmi.values.get", Some(json!({})));
    assert_eq!(values.get("ok").and_then(|v| v.as_bool()), Some(true));
}

#[test]
fn hmi_websocket_slow_consumers_do_not_block_control_plane() {
    let state = hmi_control_state(hmi_fixture_source());
    let base = start_test_server(state);

    let mut slow_sockets = Vec::new();
    for _ in 0..12_u32 {
        let (mut socket, response) =
            tungstenite::connect(websocket_url(&base)).expect("connect websocket");
        assert_eq!(
            response.status(),
            tungstenite::http::StatusCode::SWITCHING_PROTOCOLS
        );
        configure_ws_read_timeout(&mut socket);
        slow_sockets.push(socket);
    }

    let schema = post_control(&base, "hmi.schema.get", None);
    let ids = schema
        .get("result")
        .and_then(|value| value.get("widgets"))
        .and_then(|value| value.as_array())
        .expect("schema widgets")
        .iter()
        .filter_map(|widget| widget.get("id").and_then(|value| value.as_str()))
        .map(str::to_owned)
        .collect::<Vec<_>>();
    assert!(!ids.is_empty(), "ids must not be empty");

    let control_started = Instant::now();
    for _ in 0..120_u32 {
        let values = post_control(&base, "hmi.values.get", Some(json!({ "ids": ids.clone() })));
        assert_eq!(values.get("ok").and_then(|v| v.as_bool()), Some(true));
    }
    let control_elapsed = control_started.elapsed();
    assert!(
        control_elapsed < Duration::from_secs(4),
        "control plane stalled under websocket slow-consumer load: {:?}",
        control_elapsed
    );

    let (mut probe_socket, response) =
        tungstenite::connect(websocket_url(&base)).expect("connect probe websocket");
    assert_eq!(
        response.status(),
        tungstenite::http::StatusCode::SWITCHING_PROTOCOLS
    );
    configure_ws_read_timeout(&mut probe_socket);
    let ws_started = Instant::now();
    let _ = wait_for_ws_event(
        &mut probe_socket,
        "hmi.values.delta",
        Duration::from_secs(3),
    );
    assert!(
        ws_started.elapsed() <= Duration::from_secs(1),
        "probe websocket did not receive value event quickly under slow-consumer load",
    );

    for socket in &mut slow_sockets {
        if let tungstenite::stream::MaybeTlsStream::Plain(stream) = socket.get_mut() {
            let _ = stream.shutdown(Shutdown::Both);
        }
        let _ = socket.close(None);
    }
    let _ = probe_socket.close(None);
}

#[test]
fn hmi_process_page_schema_and_svg_asset_route_render() {
    let root = temp_dir("process-page");
    write_file(
        &root.join("hmi/plant.toml"),
        r##"
title = "Plant"
kind = "process"
order = 70
svg = "plant.svg"

[[bind]]
selector = "#pump1-status"
attribute = "fill"
source = "Main.run"
map = { true = "#22c55e", false = "#94a3b8" }

[[bind]]
selector = "#tank1-level"
attribute = "height"
source = "Main.speed"
scale = { min = 0, max = 100, output_min = 0, output_max = 180 }

[[bind]]
selector = "svg #unsafe"
attribute = "fill"
source = "Main.run"

[[bind]]
attribute = "opacity"
source = "Main.speed"
"##,
    );
    write_file(
        &root.join("hmi/plant.svg"),
        r##"<svg viewBox="0 0 200 100" xmlns="http://www.w3.org/2000/svg"><circle id="pump1-status" cx="20" cy="20" r="10" fill="#999"/><rect id="tank1-level" x="80" y="10" width="20" height="0"/></svg>"##,
    );

    let state = hmi_control_state_with_root(hmi_fixture_source(), Some(root.clone()));
    let base = start_test_server(state);

    let svg = ureq::get(&format!("{base}/hmi/assets/plant.svg"))
        .call()
        .expect("get process svg asset")
        .into_string()
        .expect("read svg asset body");
    assert!(svg.contains("pump1-status"));
    assert!(svg.contains("tank1-level"));

    let schema = post_control(&base, "hmi.schema.get", None);
    assert_eq!(schema.get("ok").and_then(|v| v.as_bool()), Some(true));
    let page = schema
        .get("result")
        .and_then(|v| v.get("pages"))
        .and_then(|v| v.as_array())
        .and_then(|pages| {
            pages
                .iter()
                .find(|page| page.get("id").and_then(|v| v.as_str()) == Some("plant"))
        })
        .expect("plant page");
    assert_eq!(page.get("kind").and_then(|v| v.as_str()), Some("process"));
    assert_eq!(page.get("svg").and_then(|v| v.as_str()), Some("plant.svg"));
    let bindings = page
        .get("bindings")
        .and_then(|v| v.as_array())
        .expect("process bindings");
    assert_eq!(bindings.len(), 2);
    assert!(bindings.iter().any(|entry| {
        entry.get("selector").and_then(|v| v.as_str()) == Some("#pump1-status")
            && entry.get("attribute").and_then(|v| v.as_str()) == Some("fill")
            && entry
                .get("map")
                .and_then(|v| v.get("true"))
                .and_then(|v| v.as_str())
                == Some("#22c55e")
    }));
    assert!(bindings.iter().any(|entry| {
        entry.get("selector").and_then(|v| v.as_str()) == Some("#tank1-level")
            && entry.get("attribute").and_then(|v| v.as_str()) == Some("height")
            && entry
                .get("scale")
                .and_then(|v| v.get("output_max"))
                .and_then(|v| v.as_f64())
                == Some(180.0)
    }));

    fs::remove_dir_all(root).ok();
}

#[test]
fn hmi_process_binding_transforms_update_fill_opacity_text_y_and_height() {
    let js_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/web/ui/hmi.js");
    let script = r#"
const fs = require('fs');
const vm = require('vm');
const assert = require('assert');

const sourcePath = process.env.HMI_JS_PATH;
const source = fs.readFileSync(sourcePath, 'utf8') + '\n;globalThis.__hmi_test__ = { state, applyProcessValueEntries };';
const noop = () => {};
const context = {
  console,
  URLSearchParams,
  window: {
    location: { protocol: 'http:', host: '127.0.0.1:7777', search: '' },
    addEventListener: noop,
    setInterval: () => 1,
    clearInterval: noop,
    setTimeout: () => 1,
    clearTimeout: noop,
    innerWidth: 1280,
  },
  document: {
    getElementById: () => null,
    body: { classList: { add: noop, remove: noop } },
    documentElement: { style: { setProperty: noop } },
  },
  fetch: async () => { throw new Error('unexpected fetch'); },
  DOMParser: class {
    parseFromString() {
      return { querySelector: () => null, documentElement: null };
    }
  },
};
vm.createContext(context);
vm.runInContext(source, context, { filename: 'hmi.js' });

const test = context.__hmi_test__;
const fillTarget = { attrs: {}, setAttribute(k, v) { this.attrs[k] = String(v); }, textContent: '' };
const opacityTarget = { attrs: {}, setAttribute(k, v) { this.attrs[k] = String(v); }, textContent: '' };
const textTarget = { attrs: {}, setAttribute(k, v) { this.attrs[k] = String(v); }, textContent: '' };
const yTarget = { attrs: {}, setAttribute(k, v) { this.attrs[k] = String(v); }, textContent: '' };
const heightTarget = { attrs: {}, setAttribute(k, v) { this.attrs[k] = String(v); }, textContent: '' };

test.state.processView = {
  bindingsByWidgetId: new Map([
    ['run', [{ target: fillTarget, attribute: 'fill', format: null, map: { true: '#22c55e', false: '#94a3b8' }, scale: null }]],
    ['pressure', [
      { target: opacityTarget, attribute: 'opacity', format: null, map: null, scale: null },
      { target: textTarget, attribute: 'text', format: '{:.1f} bar', map: null, scale: null },
    ]],
    ['level', [{ target: yTarget, attribute: 'y', format: null, map: null, scale: { min: 0, max: 100, output_min: 0, output_max: 180 } }]],
    ['volume', [{ target: heightTarget, attribute: 'height', format: null, map: null, scale: { min: 0, max: 100, output_min: 0, output_max: 240 } }]],
  ]),
};

test.applyProcessValueEntries({
  run: { v: true, q: 'good', ts_ms: 1 },
  pressure: { v: 42.34, q: 'good', ts_ms: 1 },
  level: { v: 50, q: 'good', ts_ms: 1 },
  volume: { v: 25, q: 'good', ts_ms: 1 },
}, 1234);

assert.strictEqual(fillTarget.attrs.fill, '#22c55e');
assert.strictEqual(opacityTarget.attrs.opacity, '42.34');
assert.strictEqual(textTarget.textContent, '42.3 bar');
assert.strictEqual(yTarget.attrs.y, '90');
assert.strictEqual(heightTarget.attrs.height, '60');
console.log('ok');
"#;
    run_node_hmi_script(&js_path, script, "process transform");
}

#[test]
fn hmi_process_renderer_handles_malformed_svg_without_crash() {
    let js_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/web/ui/hmi.js");
    let script = r#"
const fs = require('fs');
const vm = require('vm');
const assert = require('assert');

class ClassList {
  constructor() { this.values = new Set(); }
  add(...names) { for (const name of names) { if (name) { this.values.add(String(name)); } } }
  remove(...names) { for (const name of names) { this.values.delete(String(name)); } }
  contains(name) { return this.values.has(String(name)); }
}

class FakeElement {
  constructor(tag) {
    this.tagName = String(tag || 'div').toUpperCase();
    this.children = [];
    this.attrs = {};
    this.className = '';
    this.classList = new ClassList();
    this.textContent = '';
    this.innerHTML = '';
  }
  appendChild(child) {
    this.children.push(child);
    return child;
  }
  setAttribute(key, value) {
    this.attrs[String(key)] = String(value);
  }
}

(async () => {
  const sourcePath = process.env.HMI_JS_PATH;
  const source = fs.readFileSync(sourcePath, 'utf8') + '\n;globalThis.__hmi_test__ = { state, renderProcessPage };';
  const elements = new Map();
  const noop = () => {};
  function element(id, tag = 'div') {
    const node = new FakeElement(tag);
    node.setAttribute('id', id);
    elements.set(id, node);
    return node;
  }
  const groups = element('hmiGroups');
  const empty = element('emptyState');
  element('connectionState');
  element('freshnessState');
  const context = {
    console,
    URLSearchParams,
    window: {
      location: { protocol: 'http:', host: '127.0.0.1:7777', search: '' },
      addEventListener: noop,
      setInterval: () => 1,
      clearInterval: noop,
      setTimeout: () => 1,
      clearTimeout: noop,
      innerWidth: 1200,
    },
    document: {
      createElement: (tag) => new FakeElement(tag),
      createElementNS: (_ns, tag) => new FakeElement(tag),
      getElementById: (id) => elements.get(id) || null,
      body: { classList: new ClassList() },
      documentElement: { style: { setProperty: noop } },
    },
    fetch: async () => ({
      ok: true,
      text: async () => '<svg><broken',
    }),
    DOMParser: class {
      parseFromString() {
        return {
          querySelector(selector) {
            return selector === 'parsererror' ? { textContent: 'invalid svg' } : null;
          },
          documentElement: null,
        };
      }
    },
  };
  vm.createContext(context);
  vm.runInContext(source, context, { filename: 'hmi.js' });

  const test = context.__hmi_test__;
  test.state.currentPage = 'process';
  await test.renderProcessPage({
    id: 'process',
    title: 'Process',
    svg: 'malformed.svg',
    bindings: [],
  });
  assert.strictEqual(test.state.processView, null);
  assert.strictEqual(groups.classList.contains('hidden'), false);
  assert.ok(empty.textContent.includes('Process view unavailable'));
  console.log('ok');
})().catch((error) => {
  console.error(error);
  process.exit(1);
});
"#;
    run_node_hmi_script(&js_path, script, "malformed process svg");
}

#[test]
fn hmi_process_renderer_rewrites_relative_svg_asset_references() {
    let js_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/web/ui/hmi.js");
    let script = r#"
const fs = require('fs');
const vm = require('vm');
const assert = require('assert');

const sourcePath = process.env.HMI_JS_PATH;
const source = fs.readFileSync(sourcePath, 'utf8') + '\n;globalThis.__hmi_test__ = { rewriteProcessAssetReferences };';
const noop = () => {};
const context = {
  console,
  URLSearchParams,
  window: {
    location: { protocol: 'http:', host: '127.0.0.1:7777', search: '' },
    addEventListener: noop,
    setInterval: () => 1,
    clearInterval: noop,
    setTimeout: () => 1,
    clearTimeout: noop,
    innerWidth: 1280,
  },
  document: {
    getElementById: () => null,
    body: { classList: { add: noop, remove: noop } },
    documentElement: { style: { setProperty: noop } },
  },
  fetch: async () => { throw new Error('unexpected fetch'); },
  DOMParser: class {
    parseFromString() {
      return { querySelector: () => null, documentElement: null };
    }
  },
};
vm.createContext(context);
vm.runInContext(source, context, { filename: 'hmi.js' });

const test = context.__hmi_test__;
function node(attrs) {
  return {
    attrs: { ...attrs },
    getAttribute(name) {
      return Object.prototype.hasOwnProperty.call(this.attrs, name) ? this.attrs[name] : null;
    },
    setAttribute(name, value) {
      this.attrs[name] = String(value);
    },
  };
}

const relative = node({ href: 'pid-symbols/PP001A.svg' });
const relativeParent = node({ href: '../pid-symbols/PT005A.svg' });
const xlinkRelative = node({ 'xlink:href': 'pid-symbols/PV022A.svg' });
const localRef = node({ href: '#tank-001' });
const absoluteRef = node({ href: '/hmi/assets/pid-symbols%2FPP001A.svg' });
const externalRef = node({ href: 'https://example.com/symbol.svg' });
const dataRef = node({ href: 'data:image/svg+xml;base64,AAAA' });

const svgRoot = {
  querySelectorAll() {
    return [relative, relativeParent, xlinkRelative, localRef, absoluteRef, externalRef, dataRef];
  },
};

test.rewriteProcessAssetReferences(svgRoot, 'nested/plant.svg');
assert.strictEqual(relative.attrs.href, '/hmi/assets/nested%2Fpid-symbols%2FPP001A.svg');
assert.strictEqual(relativeParent.attrs.href, '/hmi/assets/pid-symbols%2FPT005A.svg');
assert.strictEqual(xlinkRelative.attrs['xlink:href'], '/hmi/assets/nested%2Fpid-symbols%2FPV022A.svg');
assert.strictEqual(localRef.attrs.href, '#tank-001');
assert.strictEqual(absoluteRef.attrs.href, '/hmi/assets/pid-symbols%2FPP001A.svg');
assert.strictEqual(externalRef.attrs.href, 'https://example.com/symbol.svg');
assert.strictEqual(dataRef.attrs.href, 'data:image/svg+xml;base64,AAAA');
console.log('ok');
"#;
    run_node_hmi_script(&js_path, script, "process asset rewrite");
}

#[test]
fn hmi_widget_renderers_handle_null_stale_and_good_values() {
    let js_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/web/ui/hmi.js");
    let script = r#"
const fs = require('fs');
const vm = require('vm');
const assert = require('assert');

class ClassList {
  constructor() { this.values = new Set(); }
  add(...names) { for (const name of names) { if (name) { this.values.add(String(name)); } } }
  remove(...names) { for (const name of names) { this.values.delete(String(name)); } }
  contains(name) { return this.values.has(String(name)); }
  toggle(name, force) {
    const value = String(name);
    if (force === true) { this.values.add(value); return true; }
    if (force === false) { this.values.delete(value); return false; }
    if (this.values.has(value)) { this.values.delete(value); return false; }
    this.values.add(value);
    return true;
  }
}

class FakeElement {
  constructor(tag) {
    this.tagName = String(tag || 'div').toUpperCase();
    this.children = [];
    this.attrs = {};
    this.style = {};
    this.dataset = {};
    this.className = '';
    this.classList = new ClassList();
    this.listeners = new Map();
    this.textContent = '';
    this.type = '';
    this.min = '';
    this.max = '';
    this.step = '';
    this.value = '';
    this.disabled = false;
  }
  appendChild(child) {
    this.children.push(child);
    return child;
  }
  setAttribute(key, value) {
    this.attrs[String(key)] = String(value);
  }
  getAttribute(key) {
    return this.attrs[String(key)];
  }
  addEventListener(event, handler) {
    const key = String(event);
    if (!this.listeners.has(key)) {
      this.listeners.set(key, []);
    }
    this.listeners.get(key).push(handler);
  }
  dispatch(event, payload = {}) {
    const handlers = this.listeners.get(String(event)) || [];
    for (const handler of handlers) {
      handler({ target: this, ...payload });
    }
  }
  querySelector(selector) {
    const match = (node) => {
      if (!(node instanceof FakeElement)) {
        return false;
      }
      if (selector.startsWith('.')) {
        const className = selector.slice(1);
        const classes = `${node.className || ''} ${node.attrs.class || ''}`
          .split(/\s+/)
          .filter(Boolean);
        return node.classList.contains(className) || classes.includes(className);
      }
      if (selector.startsWith('#')) {
        return node.attrs.id === selector.slice(1);
      }
      return node.tagName.toLowerCase() === selector.toLowerCase();
    };
    const stack = [...this.children];
    while (stack.length > 0) {
      const node = stack.shift();
      if (match(node)) {
        return node;
      }
      if (node && Array.isArray(node.children)) {
        stack.unshift(...node.children);
      }
    }
    return null;
  }
}

const noop = () => {};
const sourcePath = process.env.HMI_JS_PATH;
const source = fs.readFileSync(sourcePath, 'utf8') + '\n;globalThis.__hmi_test__ = { state, createWidgetRenderer, applyValueDelta };';
const context = {
  console,
  URLSearchParams,
  window: {
    location: { protocol: 'http:', host: '127.0.0.1:7777', search: '' },
    addEventListener: noop,
    setInterval: () => 1,
    clearInterval: noop,
    setTimeout: () => 1,
    clearTimeout: noop,
    innerWidth: 1280,
  },
  document: {
    createElement: (tag) => new FakeElement(tag),
    createElementNS: (_ns, tag) => new FakeElement(tag),
    getElementById: () => null,
    body: { classList: new ClassList() },
    documentElement: { style: { setProperty: noop } },
  },
  fetch: async () => { throw new Error('unexpected fetch'); },
  DOMParser: class {
    parseFromString() {
      return { querySelector: () => null, documentElement: null };
    }
  },
};
vm.createContext(context);
vm.runInContext(source, context, { filename: 'hmi.js' });

const test = context.__hmi_test__;
test.state.schema = { read_only: true };

function firstByClass(host, className) {
  const node = host.querySelector(`.${className}`);
  assert(node, `expected .${className}`);
  return node;
}

const gaugeHost = new FakeElement('div');
const gaugeApply = test.createWidgetRenderer(
  { id: 'gauge', widget: 'gauge', label: 'Pump Speed', data_type: 'REAL', writable: false, min: 0, max: 100, unit: 'rpm', zones: [] },
  gaugeHost,
);
gaugeApply(null);
assert.strictEqual(firstByClass(gaugeHost, 'widget-gauge-label').textContent, 'Pump Speed');
assert.strictEqual(firstByClass(gaugeHost, 'widget-gauge-center-value').textContent, '--');
gaugeApply({ v: 64, q: 'good', ts_ms: 1 });
assert.strictEqual(firstByClass(gaugeHost, 'widget-gauge-center-value').textContent, '64');
assert.strictEqual(firstByClass(gaugeHost, 'widget-gauge-unit').textContent, 'rpm');
gaugeApply({ v: 12, q: 'stale', ts_ms: 2 });
assert.strictEqual(firstByClass(gaugeHost, 'widget-gauge-center-value').textContent, '12');
assert.strictEqual(firstByClass(gaugeHost, 'widget-gauge-unit').textContent, 'rpm');
assert.notStrictEqual(firstByClass(gaugeHost, 'widget-gauge-value').attrs.d, '');

const sparkHost = new FakeElement('div');
const sparkApply = test.createWidgetRenderer(
  { id: 'spark', widget: 'sparkline', data_type: 'REAL', writable: false, unit: 'bar' },
  sparkHost,
);
sparkApply(null);
assert.strictEqual(firstByClass(sparkHost, 'widget-sparkline-label').textContent, '--');
sparkApply({ v: 10, q: 'good', ts_ms: 1 });
sparkApply({ v: 15, q: 'stale', ts_ms: 2 });
sparkApply({ v: 20, q: 'good', ts_ms: 3 });
assert.strictEqual(firstByClass(sparkHost, 'widget-sparkline-label').textContent, '20 bar');
assert.ok((firstByClass(sparkHost, 'widget-sparkline-line').attrs.points || '').length > 0);
assert.ok(sparkHost.querySelector('.widget-sparkline-area'), 'sparkline area should exist');

const barHost = new FakeElement('div');
const barApply = test.createWidgetRenderer(
  { id: 'bar', widget: 'bar', data_type: 'REAL', writable: false, min: 0, max: 100, unit: '%' },
  barHost,
);
barApply(null);
assert.strictEqual(firstByClass(barHost, 'widget-bar-label').textContent, '--');
barApply({ v: 45, q: 'good', ts_ms: 1 });
assert.strictEqual(firstByClass(barHost, 'widget-bar-label').textContent, '45 %');
barApply({ v: 80, q: 'stale', ts_ms: 2 });
assert.strictEqual(firstByClass(barHost, 'widget-bar-label').textContent, '80 %');
assert.notStrictEqual(firstByClass(barHost, 'widget-bar-fill').style.width, '0%');

const tankHost = new FakeElement('div');
const tankApply = test.createWidgetRenderer(
  { id: 'tank', widget: 'tank', data_type: 'REAL', writable: false, min: 0, max: 100, unit: '%' },
  tankHost,
);
tankApply(null);
assert.strictEqual(firstByClass(tankHost, 'widget-tank-label').textContent, '--');
tankApply({ v: 33, q: 'good', ts_ms: 1 });
assert.strictEqual(firstByClass(tankHost, 'widget-tank-label').textContent, '33 %');
tankApply({ v: 66, q: 'stale', ts_ms: 2 });
assert.strictEqual(firstByClass(tankHost, 'widget-tank-label').textContent, '66 %');
assert.ok(Number(firstByClass(tankHost, 'widget-tank-fill').attrs.height) > 0);

const indicatorHost = new FakeElement('div');
const indicatorApply = test.createWidgetRenderer(
  { id: 'indicator', widget: 'indicator', data_type: 'BOOL', writable: false },
  indicatorHost,
);
indicatorApply(null);
assert.strictEqual(firstByClass(indicatorHost, 'widget-indicator-label').textContent, '--');
indicatorApply({ v: true, q: 'good', ts_ms: 1 });
assert.strictEqual(firstByClass(indicatorHost, 'widget-indicator-label').textContent, 'ON');
indicatorApply({ v: false, q: 'stale', ts_ms: 2 });
assert.strictEqual(firstByClass(indicatorHost, 'widget-indicator-label').textContent, 'OFF');
assert.strictEqual(firstByClass(indicatorHost, 'widget-indicator-dot').classList.contains('active'), false);

const toggleHost = new FakeElement('div');
const toggleApply = test.createWidgetRenderer(
  { id: 'toggle', widget: 'toggle', data_type: 'BOOL', writable: true },
  toggleHost,
);
toggleApply(null);
assert.strictEqual(firstByClass(toggleHost, 'widget-toggle-label').textContent, '--');
toggleApply({ v: true, q: 'good', ts_ms: 1 });
assert.strictEqual(firstByClass(toggleHost, 'widget-toggle-label').textContent, 'ON');
toggleApply({ v: false, q: 'stale', ts_ms: 2 });
assert.strictEqual(firstByClass(toggleHost, 'widget-toggle-label').textContent, 'OFF');
assert.strictEqual(firstByClass(toggleHost, 'widget-toggle-control').disabled, true);

const sliderHost = new FakeElement('div');
const sliderApply = test.createWidgetRenderer(
  { id: 'slider', widget: 'slider', data_type: 'REAL', writable: true, min: 0, max: 100, unit: '%' },
  sliderHost,
);
sliderApply(null);
assert.strictEqual(firstByClass(sliderHost, 'widget-slider-label').textContent, '--');
sliderApply({ v: 25.5, q: 'good', ts_ms: 1 });
assert.strictEqual(firstByClass(sliderHost, 'widget-slider-label').textContent, '25.5 %');
sliderApply({ v: 60, q: 'stale', ts_ms: 2 });
assert.strictEqual(firstByClass(sliderHost, 'widget-slider-label').textContent, '60 %');
assert.strictEqual(firstByClass(sliderHost, 'widget-slider-control').disabled, true);

const card = new FakeElement('article');
const cardValue = new FakeElement('div');
const seen = [];
test.state.cards = new Map([
  ['widget-1', { card, value: cardValue, apply: (entry) => seen.push(entry ? entry.v : null) }],
]);
test.applyValueDelta({
  connected: true,
  timestamp_ms: 10,
  values: { 'widget-1': { v: 3, q: 'good', ts_ms: 10 } },
});
assert.strictEqual(card.dataset.quality, 'good');
test.applyValueDelta({
  connected: true,
  timestamp_ms: 11,
  values: { 'widget-1': { v: 4, q: 'stale', ts_ms: 11 } },
});
assert.strictEqual(card.dataset.quality, 'stale');
test.applyValueDelta({
  connected: true,
  timestamp_ms: 12,
  values: { 'widget-1': null },
});
assert.strictEqual(card.dataset.quality, 'stale');
assert.deepStrictEqual(seen, [3, 4, null]);
assert.strictEqual(cardValue.classList.contains('value-updated'), true);
console.log('ok');
"#;
    run_node_hmi_script(&js_path, script, "widget renderer null/stale/good coverage");
}

#[test]
fn hmi_responsive_layout_breakpoint_classes_cover_mobile_tablet_desktop() {
    let js_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/web/ui/hmi.js");
    let script = r#"
const fs = require('fs');
const vm = require('vm');
const assert = require('assert');

class ClassList {
  constructor() { this.values = new Set(); }
  add(...names) { for (const name of names) { if (name) { this.values.add(String(name)); } } }
  remove(...names) { for (const name of names) { this.values.delete(String(name)); } }
  contains(name) { return this.values.has(String(name)); }
  entries() { return Array.from(this.values.values()); }
}

const noop = () => {};
const sourcePath = process.env.HMI_JS_PATH;
const source = fs.readFileSync(sourcePath, 'utf8') + '\n;globalThis.__hmi_test__ = { state, applyResponsiveLayout, viewportForWidth };';
const bodyClassList = new ClassList();
const context = {
  console,
  URLSearchParams,
  window: {
    location: { protocol: 'http:', host: '127.0.0.1:7777', search: '' },
    addEventListener: noop,
    setInterval: () => 1,
    clearInterval: noop,
    setTimeout: () => 1,
    clearTimeout: noop,
    innerWidth: 1280,
  },
  document: {
    getElementById: () => null,
    body: { classList: bodyClassList },
    documentElement: { style: { setProperty: noop } },
  },
  fetch: async () => { throw new Error('unexpected fetch'); },
  DOMParser: class {
    parseFromString() {
      return { querySelector: () => null, documentElement: null };
    }
  },
};
vm.createContext(context);
vm.runInContext(source, context, { filename: 'hmi.js' });

const test = context.__hmi_test__;
test.state.schema = {
  responsive: {
    mode: 'auto',
    mobile_max_px: 680,
    tablet_max_px: 1024,
  },
};

assert.strictEqual(test.viewportForWidth(500, 680, 1024), 'mobile');
assert.strictEqual(test.viewportForWidth(900, 680, 1024), 'tablet');
assert.strictEqual(test.viewportForWidth(1400, 680, 1024), 'desktop');

context.window.innerWidth = 520;
test.applyResponsiveLayout();
assert.strictEqual(bodyClassList.contains('viewport-mobile'), true);
assert.strictEqual(bodyClassList.contains('viewport-tablet'), false);
assert.strictEqual(bodyClassList.contains('viewport-kiosk'), false);

context.window.innerWidth = 900;
test.applyResponsiveLayout();
assert.strictEqual(bodyClassList.contains('viewport-mobile'), false);
assert.strictEqual(bodyClassList.contains('viewport-tablet'), true);
assert.strictEqual(bodyClassList.contains('viewport-kiosk'), false);

context.window.innerWidth = 1440;
test.applyResponsiveLayout();
assert.strictEqual(bodyClassList.contains('viewport-mobile'), false);
assert.strictEqual(bodyClassList.contains('viewport-tablet'), false);
assert.strictEqual(bodyClassList.contains('viewport-kiosk'), false);

test.state.schema.responsive.mode = 'kiosk';
test.applyResponsiveLayout();
assert.strictEqual(bodyClassList.contains('viewport-kiosk'), true);
console.log('ok');
"#;
    run_node_hmi_script(&js_path, script, "responsive breakpoint classes");
}

#[test]
fn hmi_process_asset_pack_templates_and_bindings_align() {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let hmi_root = repo_root.join("hmi");
    assert!(hmi_root.is_dir(), "hmi/ directory is missing");

    let symbols_root = hmi_root.join("pid-symbols");
    assert!(
        symbols_root.is_dir(),
        "hmi/pid-symbols/ directory is missing"
    );
    assert!(
        symbols_root
            .join("LICENSE-EQUINOR-ENGINEERING-SYMBOLS.txt")
            .is_file(),
        "symbol library license file is missing"
    );

    let symbol_svg_count = fs::read_dir(&symbols_root)
        .expect("read hmi/pid-symbols directory")
        .flatten()
        .filter(|entry| {
            entry
                .path()
                .extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| ext.eq_ignore_ascii_case("svg"))
        })
        .count();
    assert!(
        symbol_svg_count >= 20,
        "expected symbol library to contain many SVGs, found {symbol_svg_count}"
    );

    let plant_svg = fs::read_to_string(hmi_root.join("plant.svg")).expect("read hmi/plant.svg");
    let minimal_svg =
        fs::read_to_string(hmi_root.join("plant-minimal.svg")).expect("read hmi/plant-minimal.svg");
    let bindings_example = fs::read_to_string(hmi_root.join("plant.bindings.example.toml"))
        .expect("read hmi/plant.bindings.example.toml");

    assert!(
        bindings_example.contains("svg = \"plant.svg\""),
        "bindings example must target hmi/plant.svg"
    );

    let plant_ids = extract_svg_ids(&plant_svg);
    let minimal_ids = extract_svg_ids(&minimal_svg);
    let required_ids = [
        "pid-tank-001-level-fill",
        "pid-tank-002-level-fill",
        "pid-pump-001-status",
        "pid-valve-001-status",
        "pid-line-002",
        "pid-tag-fit-001-pv",
        "pid-tag-pt-001-pv",
        "pid-tag-tank-001-level",
        "pid-tag-tank-002-level",
        "pid-tag-pump-001-state",
        "pid-tag-valve-001-position",
        "pid-banner-alarm-001",
        "pid-banner-alarm-001-text",
    ];
    for required_id in required_ids {
        assert!(
            plant_ids.contains(required_id),
            "plant.svg is missing stable id '{required_id}'"
        );
        assert!(
            minimal_ids.contains(required_id),
            "plant-minimal.svg is missing stable id '{required_id}'"
        );
    }

    let selectors = extract_quoted_values_from_lines(&bindings_example, "selector = ");
    assert!(
        !selectors.is_empty(),
        "bindings example selectors must not be empty"
    );
    for selector in selectors {
        if let Some(id) = selector.strip_prefix('#') {
            assert!(
                plant_ids.contains(id),
                "binding selector '{selector}' is missing in plant.svg"
            );
            assert!(
                minimal_ids.contains(id),
                "binding selector '{selector}' is missing in plant-minimal.svg"
            );
        }
    }

    let sources = extract_quoted_values_from_lines(&bindings_example, "source = ");
    assert!(
        !sources.is_empty(),
        "bindings example must include bind source paths"
    );
    for source in sources {
        assert!(
            source.contains('.'),
            "source '{source}' should use canonical Program.field or global.name form"
        );
    }
}

#[test]
fn hmi_polling_stays_under_cycle_budget() {
    let state = hmi_control_state(hmi_fixture_source());
    let base = start_test_server(state);
    let schema = post_control(&base, "hmi.schema.get", None);
    let widgets = schema
        .get("result")
        .and_then(|v| v.get("widgets"))
        .and_then(|v| v.as_array())
        .expect("schema widgets");
    let ids = widgets
        .iter()
        .filter_map(|widget| widget.get("id").and_then(|v| v.as_str()))
        .map(str::to_owned)
        .collect::<Vec<_>>();
    assert!(!ids.is_empty(), "ids must not be empty");

    let cycle_budget = Duration::from_millis(100);
    let mut total = Duration::ZERO;
    let mut max = Duration::ZERO;
    let polls: u32 = 240;

    for _ in 0..polls {
        let started = Instant::now();
        let values = post_control(&base, "hmi.values.get", Some(json!({ "ids": ids.clone() })));
        let elapsed = started.elapsed();
        total += elapsed;
        max = max.max(elapsed);
        assert_eq!(values.get("ok").and_then(|v| v.as_bool()), Some(true));
    }

    let avg = total / polls;
    assert!(
        max < cycle_budget,
        "max hmi.values.get latency {:?} exceeded cycle budget {:?}",
        max,
        cycle_budget
    );
    assert!(
        avg < Duration::from_millis(30),
        "average hmi.values.get latency {:?} exceeded expected polling overhead",
        avg
    );
}

#[test]
fn hmi_polling_soak_remains_stable() {
    let state = hmi_control_state(hmi_fixture_source());
    let base = start_test_server(state);
    let schema = post_control(&base, "hmi.schema.get", None);
    let widgets = schema
        .get("result")
        .and_then(|v| v.get("widgets"))
        .and_then(|v| v.as_array())
        .expect("schema widgets");
    let ids = widgets
        .iter()
        .filter_map(|widget| widget.get("id").and_then(|v| v.as_str()))
        .map(str::to_owned)
        .collect::<Vec<_>>();
    assert!(!ids.is_empty(), "ids must not be empty");

    let mut previous_timestamp = 0_u64;
    for _ in 0..1200 {
        let values = post_control(&base, "hmi.values.get", Some(json!({ "ids": ids.clone() })));
        assert_eq!(values.get("ok").and_then(|v| v.as_bool()), Some(true));

        let result = values.get("result").expect("values result");
        assert_eq!(
            result.get("connected").and_then(|v| v.as_bool()),
            Some(true)
        );

        let timestamp = result
            .get("timestamp_ms")
            .and_then(|v| v.as_u64())
            .expect("timestamp_ms");
        assert!(
            timestamp >= previous_timestamp,
            "timestamp drift detected: {} -> {}",
            previous_timestamp,
            timestamp
        );
        previous_timestamp = timestamp;

        let map = result
            .get("values")
            .and_then(|v| v.as_object())
            .expect("values object");
        assert_eq!(map.len(), ids.len(), "values cardinality drift");
        for id in &ids {
            let entry = map.get(id).unwrap_or_else(|| panic!("missing id {id}"));
            let quality = entry.get("q").and_then(|v| v.as_str()).unwrap_or("bad");
            assert_eq!(quality, "good", "quality drift for {id}: {quality}");
        }
    }
}
