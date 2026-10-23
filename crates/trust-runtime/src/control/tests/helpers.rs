use super::*;
use std::collections::VecDeque;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

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

pub(super) fn control_test_state(source: &str) -> ControlState {
    control_test_state_with_ads_status(source, disabled_ads_status())
}

fn control_test_state_with_ads_status(
    source: &str,
    ads_status: crate::ads::diagnostics::AdsStatusReport,
) -> ControlState {
    control_test_state_with_status_reports(source, ads_status, disabled_opcua_client_status())
}

fn control_test_state_with_opcua_client_status(
    source: &str,
    opcua_status: crate::opcua::OpcUaClientStatusReport,
) -> ControlState {
    control_test_state_with_status_reports(source, disabled_ads_status(), opcua_status)
}

fn control_test_state_with_status_reports(
    source: &str,
    ads_status: crate::ads::diagnostics::AdsStatusReport,
    opcua_status: crate::opcua::OpcUaClientStatusReport,
) -> ControlState {
    let mut harness = TestHarness::from_source(source).expect("build harness");
    let debug = harness.runtime_mut().enable_debug();
    harness.cycle();
    let snapshot = crate::debug::DebugSnapshot {
        storage_abi: Default::default(),
        storage: harness.runtime().storage().clone(),
        now: harness.runtime().current_time(),
    };
    debug.refresh_snapshot_from_storage_with_abi(
        &snapshot.storage,
        snapshot.now,
        snapshot.storage_abi,
    );

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
                ResourceCommand::MeshCatalogue { respond_to, .. } => {
                    let _ = respond_to.send(Err("unsupported in this control fixture".to_string()));
                }
                ResourceCommand::Snapshot { respond_to } => {
                    let _ = respond_to.send(snapshot.clone());
                }
                ResourceCommand::SimulationSnapshot { respond_to } => {
                    let _ = respond_to.send(None);
                }
                ResourceCommand::SimulationTraceClear { respond_to, .. } => {
                    let _ = respond_to.send(Err(
                        crate::simulation::PlantTraceClearError::StatusUnavailable,
                    ));
                }
                ResourceCommand::AdsStatus { respond_to } => {
                    let _ = respond_to.send(ads_status.clone());
                }
                ResourceCommand::OpcUaClientStatus { respond_to } => {
                    let _ = respond_to.send(opcua_status.clone());
                }
                ResourceCommand::ActiveAdsDevice { respond_to, .. } => {
                    let _ = respond_to.send(None);
                }
                ResourceCommand::MeshApply { .. }
                | ResourceCommand::Pause
                | ResourceCommand::Resume
                | ResourceCommand::UpdateSchedulerPolicies(_)
                | ResourceCommand::UpdateIoSafeState(_) => {}
            }
        }
    });
    let sources = SourceRegistry::new(vec![SourceFile {
        id: 1,
        path: std::path::PathBuf::from("main.st"),
        text: source.to_string(),
    }]);
    ControlState {
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
        discovery: Arc::new(crate::discovery::DiscoveryState::new()),
        mesh_topology: Arc::new(Mutex::new(None)),
        realtime_status: Arc::new(Mutex::new(LinuxRtRuntimeStatus::from_config(
            crate::linux_rt::LinuxRtConfig::default(),
        ))),
        web_listener_bound: Arc::new(AtomicBool::new(false)),
        opcua_server_bound: Arc::new(AtomicBool::new(false)),
        project_root: None,
        plant_service: Arc::new(PlantControlService::default()),
        experiments: ControlExperimentService::default(),
        controllers: ControllerParameterService::default(),
        searches: super::search::SearchService::default(),
        resource_name: SmolStr::new("RESOURCE"),
        io_health: Arc::new(Mutex::new(Vec::new())),
        debug_enabled: Arc::new(AtomicBool::new(true)),
        debug_variables: Arc::new(Mutex::new(DebugVariableHandles::new())),
        historian: None,
        openot_persistence_status: None,
        pairing: None,
        ads_doctor_jobs: Arc::new(Mutex::new(AdsDoctorJobStore::default())),
        ads_client_config: Arc::new(Mutex::new(None)),
        opcua_client_config: Arc::new(Mutex::new(None)),
        ads_server_config: Arc::new(Mutex::new(None)),
        #[cfg(feature = "ads-server")]
        ads_server_runtime: Arc::new(Mutex::new(None)),
    }
}

fn disabled_ads_status() -> crate::ads::diagnostics::AdsStatusReport {
    crate::ads::diagnostics::AdsStatusReport {
        schema_version: crate::ads::diagnostics::ADS_DIAGNOSTICS_SCHEMA_VERSION,
        role: crate::ads::diagnostics::DoctorRole::Client,
        overall: crate::ads::diagnostics::AdsStatusOverall::Disabled,
        runtime_identity_hash: None,
        deployed_ads_config_hash: None,
        connections: Vec::new(),
        summary: "ADS is not configured.".to_string(),
    }
}

fn disabled_opcua_client_status() -> crate::opcua::OpcUaClientStatusReport {
    crate::opcua::OpcUaClientStatusReport {
        enabled: false,
        deployed_config_hash: None,
        connections: Vec::new(),
    }
}

fn set_project_root(state: &mut ControlState, root: &Path) {
    state.project_root = Some(root.to_path_buf());
}

fn pairing_file(name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    std::env::temp_dir().join(format!("trust-pairing-control-{name}-{stamp}.json"))
}

fn engineer_token(name: &str, state: &mut ControlState) -> String {
    let store = Arc::new(PairingStore::load(pairing_file(name)));
    state.pairing = Some(store.clone());
    let code = store.start_pairing();
    store
        .claim(&code.code, Some(AccessRole::Engineer))
        .expect("engineer token")
}
