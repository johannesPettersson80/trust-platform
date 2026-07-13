//! Control-server bridge for debug sessions (inline values, tooling).

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64};
use std::sync::{Arc, Mutex};
use std::thread;

use indexmap::IndexMap;
use smol_str::SmolStr;

use trust_runtime::config::ControlMode;
use trust_runtime::control::{
    AdsDoctorJobStore, ControlEndpoint, ControlServer, ControlState, HmiRuntimeDescriptor,
    SourceRegistry,
};
use trust_runtime::debug::{DebugControl, DebugSnapshot, DebugVariableHandles, RuntimeEvent};
use trust_runtime::discovery::DiscoveryState;
use trust_runtime::error::RuntimeError;
use trust_runtime::io::IoDriverStatus;
use trust_runtime::linux_rt::{LinuxRtConfig, LinuxRtRuntimeStatus};
use trust_runtime::mesh::MeshTopologyEvidence;
use trust_runtime::metrics::RuntimeMetrics;
use trust_runtime::scheduler::{ResourceCommand, ResourceControl, StdClock};
use trust_runtime::settings::{
    BaseSettings, DiscoverySettings, MeshSettings, RuntimeSettings, SimulationSettings, WebSettings,
};
use trust_runtime::value::{Duration, Value};
use trust_runtime::watchdog::{FaultPolicy, RetainMode, WatchdogPolicy};

use crate::runtime::DebugRuntime;

#[cfg(unix)]
const DEFAULT_UNIX_ENDPOINT: &str = "/tmp/trust-debug.sock";
#[cfg(not(unix))]
const DEFAULT_TCP_ENDPOINT: &str = "127.0.0.1:9901";

pub(super) struct DebugControlServer {
    server: ControlServer,
    resource: ResourceControl<StdClock>,
    _drain: thread::JoinHandle<()>,
}

impl DebugControlServer {
    pub fn start(
        session: &dyn DebugRuntime,
        endpoint: ControlEndpoint,
        auth_token: Option<String>,
        project_root: Option<PathBuf>,
    ) -> Result<Self, RuntimeError> {
        let (resource, cmd_rx) = ResourceControl::stub(StdClock::new());
        if let Ok(runtime) = session.runtime_handle().lock() {
            let _ = resource.publish_ads_live_values_snapshot(runtime.ads_live_values_snapshot());
        }
        let debug = session.debug_control();
        let sources = SourceRegistry::new(session.control_sources());
        let hmi_descriptor = Arc::new(Mutex::new(HmiRuntimeDescriptor::from_sources(
            project_root.as_deref(),
            &sources,
        )));
        let state = Arc::new(ControlState {
            debug: debug.clone(),
            resource: resource.clone(),
            metadata: Arc::new(Mutex::new(session.metadata().clone())),
            project_root,
            sources,
            io_snapshot: Arc::new(Mutex::new(None)),
            io_snapshot_seen_ms: Arc::new(AtomicU64::new(0)),
            pending_restart: Arc::new(Mutex::new(None)),
            auth_token: Arc::new(Mutex::new(auth_token.map(SmolStr::new))),
            control_requires_auth: matches!(endpoint, ControlEndpoint::Tcp(_)),
            control_mode: Arc::new(Mutex::new(ControlMode::Debug)),
            audit_tx: None,
            metrics: Arc::new(Mutex::new(RuntimeMetrics::default())),
            events: Arc::new(Mutex::new(VecDeque::<RuntimeEvent>::new())),
            settings: Arc::new(Mutex::new(default_settings(session))),
            discovery: Arc::new(DiscoveryState::new()),
            mesh_topology: Arc::new(Mutex::new(None::<MeshTopologyEvidence>)),
            web_listener_bound: Arc::new(AtomicBool::new(false)),
            opcua_server_bound: Arc::new(AtomicBool::new(false)),
            resource_name: SmolStr::new("RESOURCE"),
            io_health: Arc::new(Mutex::new(Vec::<IoDriverStatus>::new())),
            realtime_status: Arc::new(Mutex::new(LinuxRtRuntimeStatus::from_config(
                LinuxRtConfig::default(),
            ))),
            debug_enabled: Arc::new(AtomicBool::new(true)),
            debug_variables: Arc::new(Mutex::new(DebugVariableHandles::new())),
            hmi_live: Arc::new(Mutex::new(trust_runtime::hmi::HmiLiveState::default())),
            hmi_persistence: None,
            hmi_descriptor,
            historian: None,
            pairing: None,
            ads_doctor_jobs: Arc::new(Mutex::new(AdsDoctorJobStore::default())),
            ads_client_config: Arc::new(Mutex::new(None)),
            opcua_client_config: Arc::new(Mutex::new(None)),
            ads_server_config: Arc::new(Mutex::new(None)),
            #[cfg(feature = "ads-server")]
            ads_server_runtime: Arc::new(Mutex::new(None)),
        });
        trust_runtime::control::spawn_hmi_descriptor_watcher(state.clone());
        let server = ControlServer::start(endpoint, state.clone())?;
        let drain = spawn_command_drain(cmd_rx, debug, session.runtime_handle());
        Ok(Self {
            server,
            resource,
            _drain: drain,
        })
    }

    pub fn endpoint(&self) -> &ControlEndpoint {
        self.server.endpoint()
    }

    pub fn resource_control(&self) -> ResourceControl<StdClock> {
        self.resource.clone()
    }
}

pub(super) fn default_control_endpoint() -> ControlEndpoint {
    #[cfg(unix)]
    {
        ControlEndpoint::Unix(PathBuf::from(DEFAULT_UNIX_ENDPOINT))
    }
    #[cfg(not(unix))]
    {
        ControlEndpoint::Tcp(DEFAULT_TCP_ENDPOINT.parse().expect("default tcp endpoint"))
    }
}

fn default_settings(session: &dyn DebugRuntime) -> RuntimeSettings {
    let (watchdog, fault_policy) = session
        .runtime_handle()
        .lock()
        .map(|runtime| (runtime.watchdog_policy(), runtime.fault_policy()))
        .unwrap_or_else(|_| (WatchdogPolicy::default(), FaultPolicy::SafeHalt));
    RuntimeSettings::new(
        Duration::from_millis(10),
        BaseSettings {
            log_level: SmolStr::new("info"),
            watchdog,
            fault_policy,
            retain_mode: RetainMode::None,
            retain_save_interval: None,
        },
        WebSettings {
            enabled: false,
            listen: SmolStr::new("0.0.0.0:8080"),
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
            listen: SmolStr::new("0.0.0.0:5200"),
            connect: Vec::new(),
            tls: false,
            auth_token: None,
            publish: Vec::new(),
            subscribe: IndexMap::new(),
            zenohd_version: SmolStr::new(trust_runtime::mesh::ZENOHD_BASELINE_VERSION),
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

fn spawn_command_drain(
    rx: std::sync::mpsc::Receiver<ResourceCommand>,
    debug: DebugControl,
    runtime: Arc<Mutex<trust_runtime::Runtime>>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        while let Ok(command) = rx.recv() {
            match command {
                ResourceCommand::ReloadBytecode { respond_to, .. } => {
                    let _ = respond_to.send(Err(RuntimeError::ControlError(SmolStr::new(
                        "bytecode reload unavailable in debug control",
                    ))));
                }
                ResourceCommand::MeshSnapshot { respond_to, .. } => {
                    let _ = respond_to.send(IndexMap::<SmolStr, Value>::new());
                }
                ResourceCommand::MeshApply { .. } => {}
                ResourceCommand::Snapshot { respond_to } => {
                    let _ = respond_to.send(debug.snapshot().unwrap_or_else(empty_debug_snapshot));
                }
                ResourceCommand::AdsStatus { respond_to } => {
                    let report = runtime
                        .lock()
                        .map(|runtime| runtime.ads_status_report())
                        .unwrap_or_else(|_| disabled_ads_status("ADS status unavailable."));
                    let _ = respond_to.send(report);
                }
                ResourceCommand::AdsLiveValues { respond_to } => {
                    let snapshot = runtime
                        .lock()
                        .map(|runtime| runtime.ads_live_values_snapshot())
                        .unwrap_or_else(|_| {
                            trust_runtime::ads::AdsLiveValuesSnapshot::new(0, Vec::new())
                        });
                    let _ = respond_to.send(snapshot);
                }
                ResourceCommand::OpcUaClientStatus { respond_to } => {
                    let _ = respond_to.send(trust_runtime::opcua::OpcUaClientStatusReport {
                        enabled: false,
                        deployed_config_hash: None,
                        connections: Vec::new(),
                    });
                }
                ResourceCommand::ActiveAdsDevice {
                    target,
                    local,
                    respond_to,
                } => {
                    let snapshot = runtime.lock().ok().and_then(|runtime| {
                        runtime.active_ads_device_snapshot(&target, local.as_ref())
                    });
                    let _ = respond_to.send(snapshot);
                }
                ResourceCommand::Pause
                | ResourceCommand::Resume
                | ResourceCommand::UpdateWatchdog(_)
                | ResourceCommand::UpdateFaultPolicy(_)
                | ResourceCommand::UpdateRetainSaveInterval(_)
                | ResourceCommand::UpdateIoSafeState(_) => {}
            }
        }
    })
}

fn disabled_ads_status(summary: &str) -> trust_runtime::ads::diagnostics::AdsStatusReport {
    trust_runtime::ads::diagnostics::AdsStatusReport {
        schema_version: trust_runtime::ads::diagnostics::ADS_DIAGNOSTICS_SCHEMA_VERSION,
        role: trust_runtime::ads::diagnostics::DoctorRole::Client,
        overall: trust_runtime::ads::diagnostics::AdsStatusOverall::Disabled,
        runtime_identity_hash: None,
        deployed_ads_config_hash: None,
        connections: Vec::new(),
        summary: summary.to_string(),
    }
}

fn empty_debug_snapshot() -> DebugSnapshot {
    DebugSnapshot {
        storage: trust_runtime::memory::VariableStorage::new(),
        now: trust_runtime::value::Duration::ZERO,
    }
}
