use super::*;
use std::collections::VecDeque;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use indexmap::IndexMap;
use serde_json::json;

use crate::debug::{DebugVariableHandles, PendingVarTarget};
use crate::error::RuntimeError;
use crate::harness::TestHarness;
use crate::historian::{AlertRule, HistorianConfig, HistorianService, RecordingMode};
use crate::linux_rt::LinuxRtRuntimeStatus;
use crate::metrics::RuntimeMetrics;
use crate::scheduler::{ResourceCommand, ResourceControl, StdClock};
use crate::security::{pairing::PairingStore, AccessRole};
use crate::settings::{
    BaseSettings, DiscoverySettings, MeshSettings, RuntimeSettings, SimulationSettings, WebSettings,
};
use crate::watchdog::{FaultPolicy, RetainMode, WatchdogPolicy};

fn temp_history_path(name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    std::env::temp_dir().join(format!("trust-control-{name}-{stamp}.jsonl"))
}

fn temp_dir(name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("trust-control-{name}-{stamp}"));
    fs::create_dir_all(&path).expect("create temp dir");
    path
}

fn write_file(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent");
    }
    fs::write(path, content).expect("write file");
}

fn runtime_settings() -> RuntimeSettings {
    RuntimeSettings::new(
        crate::value::Duration::from_millis(10),
        BaseSettings {
            log_level: SmolStr::new("info"),
            watchdog: WatchdogPolicy::default(),
            fault_policy: FaultPolicy::SafeHalt,
            retain_mode: RetainMode::None,
            retain_save_interval: None,
        },
        WebSettings {
            enabled: false,
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
            role: crate::config::MeshRole::Peer,
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

fn hmi_test_state(source: &str) -> ControlState {
    let mut harness = TestHarness::from_source(source).expect("build harness");
    let debug = harness.runtime_mut().enable_debug();
    harness.cycle();
    let snapshot = crate::debug::DebugSnapshot {
        storage: harness.runtime().storage().clone(),
        now: harness.runtime().current_time(),
    };
    debug.refresh_snapshot_from_storage(&snapshot.storage, snapshot.now);

    let (resource, cmd_rx) = ResourceControl::stub(StdClock::new());
    std::thread::spawn(move || {
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
        path: std::path::PathBuf::from("main.st"),
        text: source.to_string(),
    }]);
    let hmi_descriptor = Arc::new(Mutex::new(HmiRuntimeDescriptor::from_sources(
        None, &sources,
    )));
    ControlState {
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
        realtime_status: Arc::new(Mutex::new(LinuxRtRuntimeStatus::from_config(
            crate::linux_rt::LinuxRtConfig::default(),
        ))),
        project_root: None,
        resource_name: SmolStr::new("RESOURCE"),
        io_health: Arc::new(Mutex::new(Vec::new())),
        debug_enabled: Arc::new(AtomicBool::new(true)),
        debug_variables: Arc::new(Mutex::new(DebugVariableHandles::new())),
        hmi_live: Arc::new(Mutex::new(crate::hmi::HmiLiveState::default())),
        hmi_descriptor,
        historian: None,
        pairing: None,
    }
}

fn set_hmi_project_root(state: &mut ControlState, root: &Path) {
    state.project_root = Some(root.to_path_buf());
    state.hmi_descriptor = Arc::new(Mutex::new(HmiRuntimeDescriptor::from_sources(
        state.project_root.as_deref(),
        &state.sources,
    )));
}

fn hmi_schema_result(state: &ControlState) -> serde_json::Value {
    let response = handle_request_value(json!({"id": 999, "type": "hmi.schema.get"}), state, None);
    assert!(response.ok, "schema response failed: {:?}", response.error);
    response.result.expect("schema result")
}

fn hmi_schema_revision_and_speed_label(state: &ControlState) -> (u64, String) {
    let result = hmi_schema_result(state);
    let revision = result
        .get("schema_revision")
        .and_then(serde_json::Value::as_u64)
        .expect("schema revision");
    let label = result
        .get("widgets")
        .and_then(serde_json::Value::as_array)
        .and_then(|widgets| {
            widgets.iter().find_map(|widget| {
                if widget.get("path").and_then(serde_json::Value::as_str) == Some("Main.speed") {
                    widget
                        .get("label")
                        .and_then(serde_json::Value::as_str)
                        .map(ToOwned::to_owned)
                } else {
                    None
                }
            })
        })
        .expect("speed label");
    (revision, label)
}

fn wait_for_schema_revision(
    state: &ControlState,
    min_revision: u64,
    timeout: Duration,
) -> (u64, String) {
    let deadline = Instant::now() + timeout;
    loop {
        let current = hmi_schema_revision_and_speed_label(state);
        if current.0 >= min_revision {
            return current;
        }
        if Instant::now() >= deadline {
            panic!(
                "schema revision did not reach {min_revision}; last seen {:?}",
                current
            );
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn wait_for_descriptor_error(state: &ControlState, timeout: Duration) -> serde_json::Value {
    let deadline = Instant::now() + timeout;
    loop {
        let schema = hmi_schema_result(state);
        if schema
            .get("descriptor_error")
            .and_then(serde_json::Value::as_str)
            .is_some()
        {
            return schema;
        }
        if Instant::now() >= deadline {
            panic!("descriptor_error was not present before timeout");
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn wait_for_descriptor_error_clear(state: &ControlState, timeout: Duration) -> serde_json::Value {
    let deadline = Instant::now() + timeout;
    loop {
        let schema = hmi_schema_result(state);
        if schema.get("descriptor_error").is_none() {
            return schema;
        }
        if Instant::now() >= deadline {
            panic!("descriptor_error was not cleared before timeout");
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn pairing_file(name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    std::env::temp_dir().join(format!("trust-pairing-control-{name}-{stamp}.json"))
}
