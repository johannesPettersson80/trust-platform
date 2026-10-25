//! Control-server bridge for debug sessions (inline values, tooling): the control server of the
//! simulator, whose resource is trust-runtime's scheduler running the launch's compiled program
//! (specification 44 NX-16).

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
use trust_runtime::debug::{DebugControl, RuntimeEvent};
use trust_runtime::discovery::DiscoveryState;
use trust_runtime::error::RuntimeError;
use trust_runtime::io::IoDriverStatus;
use trust_runtime::linux_rt::{LinuxRtConfig, LinuxRtRuntimeStatus};
use trust_runtime::mesh::MeshTopologyEvidence;
use trust_runtime::metrics::RuntimeMetrics;
use trust_runtime::scheduler::{ResourceControl, StdClock};
use trust_runtime::settings::{
    BaseSettings, DiscoverySettings, MeshSettings, RuntimeSettings, SimulationSettings, WebSettings,
};
use trust_runtime::value::Duration;
use trust_runtime::watchdog::{FaultPolicy, RetainMode, WatchdogPolicy};

#[cfg(unix)]
const DEFAULT_UNIX_ENDPOINT: &str = "/tmp/trust-debug.sock";
#[cfg(not(unix))]
const DEFAULT_TCP_ENDPOINT: &str = "127.0.0.1:9901";

pub(super) struct DebugControlServer {
    _server: ControlServer,
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

/// The parts of a control state that the simulator gives.
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
        let settings = simulator_settings(WatchdogPolicy::default(), FaultPolicy::SafeHalt);
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
            _server: Self::serve(served, endpoint, auth_token, project_root)?,
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
