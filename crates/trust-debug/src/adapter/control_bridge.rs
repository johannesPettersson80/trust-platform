//! Control-server bridge for debug sessions (inline values, tooling): the control server of the
//! bytecode engine's simulator, whose resource is a stub the adapter's runner stands behind, and of
//! the compiled simulator, whose resource is trust-runtime's scheduler (specification 44 NX-16).

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
    SourceFile as ControlSourceFile, SourceRegistry,
};
use trust_runtime::debug::{DebugControl, DebugSnapshot, DebugVariableHandles, RuntimeEvent};
use trust_runtime::discovery::DiscoveryState;
use trust_runtime::error::RuntimeError;
use trust_runtime::io::IoDriverStatus;
use trust_runtime::linux_rt::{LinuxRtConfig, LinuxRtRuntimeStatus};
use trust_runtime::mesh::MeshTopologyEvidence;
use trust_runtime::metrics::RuntimeMetrics;
use trust_runtime::scheduler::{
    ResourceCommand, ResourceControl, ResourceState, StdClock, StepRefusal,
};
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
    /// The thread that answers the stub resource's commands, for the bytecode engine's simulator.
    _drain: Option<thread::JoinHandle<()>>,
}

/// What a compiled program reports to the control server through: its debugger, which also
/// carries its I/O snapshots and runtime events, its metrics and its drivers' health.
pub(super) struct ServedProgram {
    pub(super) debug: DebugControl,
    pub(super) metrics: Arc<Mutex<RuntimeMetrics>>,
    pub(super) io_health: Arc<Mutex<Vec<IoDriverStatus>>>,
}

impl Default for ServedProgram {
    /// Reports that nothing has filled yet.
    fn default() -> Self {
        Self {
            debug: DebugControl::new(),
            metrics: Arc::new(Mutex::new(RuntimeMetrics::default())),
            io_health: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

/// The parts of a control state that differ between the two simulators.
struct Served {
    debug: DebugControl,
    resource: ResourceControl<StdClock>,
    metadata: trust_runtime::RuntimeMetadata,
    sources: SourceRegistry,
    settings: RuntimeSettings,
    metrics: Arc<Mutex<RuntimeMetrics>>,
    io_health: Arc<Mutex<Vec<IoDriverStatus>>>,
    io_snapshot: Arc<Mutex<Option<trust_runtime::io::IoSnapshot>>>,
    events: Arc<Mutex<VecDeque<RuntimeEvent>>>,
}

impl DebugControlServer {
    /// The control server of the bytecode engine's simulator, whose stub resource the adapter's
    /// runner stands behind.
    pub fn start(
        session: &dyn DebugRuntime,
        endpoint: ControlEndpoint,
        auth_token: Option<String>,
        project_root: Option<PathBuf>,
    ) -> Result<Self, RuntimeError> {
        let (resource, cmd_rx) = ResourceControl::stub(StdClock::new());
        let debug = session.debug_control();
        let served = Served {
            debug: debug.clone(),
            resource,
            metadata: session.metadata().clone(),
            sources: SourceRegistry::new(session.control_sources()),
            settings: default_settings(session),
            metrics: Arc::new(Mutex::new(RuntimeMetrics::default())),
            io_health: Arc::new(Mutex::new(Vec::new())),
            io_snapshot: Arc::new(Mutex::new(None)),
            events: Arc::new(Mutex::new(VecDeque::new())),
        };
        let server = Self::serve(served, endpoint, auth_token, project_root)?;
        let drain = spawn_command_drain(cmd_rx, debug, session.runtime_handle());
        Ok(Self {
            server,
            _drain: Some(drain),
        })
    }

    /// The control server of a compiled program that trust-runtime's scheduler runs under
    /// `resource` (specification 44 NX-16), as trust-runtime serves it: the I/O snapshots and the
    /// runtime events its debugger sends are kept for `io.read` and `events.tail`.
    pub fn start_compiled(
        program: ServedProgram,
        resource: ResourceControl<StdClock>,
        sources: Vec<ControlSourceFile>,
        endpoint: ControlEndpoint,
        auth_token: Option<String>,
        project_root: Option<PathBuf>,
    ) -> Result<Self, RuntimeError> {
        let io_snapshot = Arc::new(Mutex::new(None));
        let (io_tx, io_rx) = std::sync::mpsc::channel();
        program.debug.set_io_sender(io_tx);
        {
            let io_snapshot = io_snapshot.clone();
            thread::spawn(move || {
                for snapshot in io_rx {
                    if let Ok(mut guard) = io_snapshot.lock() {
                        *guard = Some(snapshot);
                    }
                }
            });
        }
        let events = Arc::new(Mutex::new(VecDeque::new()));
        let (event_tx, event_rx) = std::sync::mpsc::channel();
        program.debug.set_runtime_sender(event_tx);
        {
            let events = events.clone();
            thread::spawn(move || {
                for event in event_rx {
                    if let Ok(mut guard) = events.lock() {
                        if guard.len() >= SIMULATOR_EVENTS {
                            guard.pop_front();
                        }
                        guard.push_back(event);
                    }
                }
            });
        }
        // `status` names the backend, by which a client sends compiled expressions.
        let mut settings = simulator_settings(WatchdogPolicy::default(), FaultPolicy::SafeHalt);
        settings.execution_backend = trust_runtime::execution_backend::ExecutionBackend::Native;
        let served = Served {
            debug: program.debug,
            resource,
            metadata: trust_runtime::RuntimeMetadata::default(),
            sources: SourceRegistry::new(sources),
            settings,
            metrics: program.metrics,
            io_health: program.io_health,
            io_snapshot,
            events,
        };
        Ok(Self {
            server: Self::serve(served, endpoint, auth_token, project_root)?,
            _drain: None,
        })
    }

    fn serve(
        served: Served,
        endpoint: ControlEndpoint,
        auth_token: Option<String>,
        project_root: Option<PathBuf>,
    ) -> Result<ControlServer, RuntimeError> {
        let Served {
            debug,
            resource,
            metadata,
            sources,
            settings,
            metrics,
            io_health,
            io_snapshot,
            events,
        } = served;
        let hmi_descriptor = Arc::new(Mutex::new(HmiRuntimeDescriptor::from_sources(
            project_root.as_deref(),
            &sources,
        )));
        let state = Arc::new(ControlState {
            transfers: trust_runtime::control::new_transfer_session(),
            debug,
            resource,
            metadata: Arc::new(Mutex::new(metadata)),
            project_root,
            sources,
            io_snapshot,
            io_snapshot_seen_ms: Arc::new(AtomicU64::new(0)),
            pending_restart: Arc::new(Mutex::new(None)),
            auth_token: Arc::new(Mutex::new(auth_token.map(SmolStr::new))),
            control_requires_auth: matches!(endpoint, ControlEndpoint::Tcp(_)),
            control_mode: Arc::new(Mutex::new(ControlMode::Debug)),
            audit_tx: None,
            metrics,
            events,
            settings: Arc::new(Mutex::new(settings)),
            discovery: Arc::new(DiscoveryState::new()),
            mesh_topology: Arc::new(Mutex::new(None::<MeshTopologyEvidence>)),
            web_listener_bound: Arc::new(AtomicBool::new(false)),
            opcua_server_bound: Arc::new(AtomicBool::new(false)),
            resource_name: SmolStr::new("RESOURCE"),
            runtime_id: SmolStr::new("RESOURCE"),
            io_health,
            realtime_status: Arc::new(Mutex::new(LinuxRtRuntimeStatus::from_config(
                LinuxRtConfig::default(),
            ))),
            debug_enabled: Arc::new(AtomicBool::new(true)),
            debug_variables: Arc::new(Mutex::new(DebugVariableHandles::new())),
            hmi_live: Arc::new(Mutex::new(trust_runtime::hmi::HmiLiveState::default())),
            hmi_persistence: None,
            openot_persistence_status: None,
            hmi_descriptor,
            historian: None,
            pairing: None,
            search: Default::default(),
            ads_doctor_jobs: Arc::new(Mutex::new(AdsDoctorJobStore::default())),
            ads_client_config: Arc::new(Mutex::new(None)),
            opcua_client_config: Arc::new(Mutex::new(None)),
            ads_server_config: Arc::new(Mutex::new(None)),
            #[cfg(feature = "ads-server")]
            ads_server_runtime: Arc::new(Mutex::new(None)),
        });
        trust_runtime::control::spawn_hmi_descriptor_watcher(state.clone());
        ControlServer::start(endpoint, state)
    }

    pub fn endpoint(&self) -> &ControlEndpoint {
        self.server.endpoint()
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

/// The runtime events the compiled simulator keeps for `events.tail`.
const SIMULATOR_EVENTS: usize = 1_000;

fn default_settings(session: &dyn DebugRuntime) -> RuntimeSettings {
    let (watchdog, fault_policy) = session
        .runtime_handle()
        .lock()
        .map(|runtime| (runtime.watchdog_policy(), runtime.fault_policy()))
        .unwrap_or_else(|_| (WatchdogPolicy::default(), FaultPolicy::SafeHalt));
    simulator_settings(watchdog, fault_policy)
}

/// The settings a simulator reports: a 10 ms cycle, no retain file, and no web, discovery, mesh
/// or simulation service.
fn simulator_settings(watchdog: WatchdogPolicy, fault_policy: FaultPolicy) -> RuntimeSettings {
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
                | ResourceCommand::UpdateSchedulerPolicies(_)
                | ResourceCommand::UpdateIoSafeState(_)
                // A debug session installs no program.
                | ResourceCommand::ProgramInstalled => {}
                // The debugger runs the program, so it takes no step, as a running resource
                // refuses one (specification 11 section 6.8.1).
                ResourceCommand::StepCycle { respond_to, .. } => {
                    let _ = respond_to.send(Err(StepRefusal::NotStopped(ResourceState::Running)));
                }
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
        storage_abi: Default::default(),
        storage: trust_runtime::memory::VariableStorage::new(),
        now: trust_runtime::value::Duration::ZERO,
        compiled: None,
    }
}
