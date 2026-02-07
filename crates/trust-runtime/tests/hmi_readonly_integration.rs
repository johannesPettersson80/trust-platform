use std::collections::VecDeque;
use std::net::TcpListener;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use indexmap::IndexMap;
use serde_json::json;
use smol_str::SmolStr;
use trust_runtime::config::{ControlMode, WebAuthMode, WebConfig};
use trust_runtime::control::{ControlState, SourceRegistry};
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

fn hmi_control_state(source: &str) -> Arc<ControlState> {
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

    Arc::new(ControlState {
        debug,
        resource,
        metadata: Arc::new(Mutex::new(harness.runtime().metadata_snapshot())),
        sources: SourceRegistry::new(vec![trust_runtime::control::SourceFile {
            id: 1,
            path: std::path::PathBuf::from("main.st"),
            text: source.to_string(),
        }]),
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
        pairing: None,
    })
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
    };
    let _server = start_web_server(&config, state, None, None, None).expect("start web server");
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

    let hmi_css = ureq::get(&format!("{base}/hmi/styles.css"))
        .call()
        .expect("get /hmi/styles.css")
        .into_string()
        .expect("read /hmi/styles.css body");
    assert!(hmi_css.contains(".card"));
    assert!(hmi_css.contains("viewport-kiosk"));
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
fn hmi_standalone_export_bundle_contains_assets_routes_and_config() {
    let state = hmi_control_state(hmi_fixture_source());
    let base = start_test_server(state);

    let export = ureq::get(&format!("{base}/hmi/export.json"))
        .call()
        .expect("get /hmi/export.json")
        .into_string()
        .expect("read export body");
    let payload: serde_json::Value = serde_json::from_str(&export).expect("parse export body");

    assert_eq!(payload.get("version").and_then(|v| v.as_u64()), Some(1));
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
        }));
    assert!(payload
        .get("assets")
        .and_then(|v| v.as_object())
        .is_some_and(|assets| {
            assets.contains_key("hmi/index.html")
                && assets.contains_key("hmi/styles.css")
                && assets.contains_key("hmi/app.js")
        }));
    assert!(payload
        .get("config")
        .and_then(|v| v.get("schema"))
        .and_then(|v| v.get("widgets"))
        .and_then(|v| v.as_array())
        .is_some_and(|widgets| !widgets.is_empty()));
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
