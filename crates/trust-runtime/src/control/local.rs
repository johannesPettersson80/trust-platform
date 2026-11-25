//! A control state for a runtime whose client is on this computer and needs no token: the simulator
//! of a machine test (specification 11 §6.8.1, "Simulator for machine tests") and the configuration
//! UI. Its optional services start empty: no discovery, mesh, pairing, historian, audit log, ADS or
//! OPC UA.

use super::*;

/// What a [`ControlState::local`] shares with its runtime and program: the handles the program is
/// wired with, its sources and settings, and its names.
pub struct LocalControl {
    /// The program's debugger.
    pub debug: DebugControl,
    /// The identify output of the resource.
    pub identify: IdentifyHandle,
    /// The resource the requests control.
    pub resource: ResourceControl<crate::scheduler::StdClock>,
    /// The program's sources.
    pub sources: SourceRegistry,
    /// The I/O snapshot the debugger publishes, and when it was last seen.
    pub io_snapshot: Arc<Mutex<Option<IoSnapshot>>>,
    /// When the I/O snapshot was last seen, in Unix milliseconds.
    pub io_snapshot_seen_ms: Arc<AtomicU64>,
    /// The restart that the scan takes at its next boundary.
    pub pending_restart: Arc<Mutex<Option<RestartMode>>>,
    /// The control mode the requests run in.
    pub control_mode: ControlMode,
    /// The program's metrics.
    pub metrics: Arc<Mutex<RuntimeMetrics>>,
    /// The runtime's events.
    pub events: Arc<Mutex<VecDeque<crate::debug::RuntimeEvent>>>,
    /// The runtime's settings.
    pub settings: RuntimeSettings,
    /// The project the runtime runs.
    pub project_root: Option<PathBuf>,
    /// The resource's name.
    pub resource_name: SmolStr,
    /// The runtime's identity.
    pub runtime_id: SmolStr,
    /// The health of the I/O drivers.
    pub io_health: Arc<Mutex<Vec<IoDriverStatus>>>,
    /// Whether the debug surface is on.
    pub debug_enabled: Arc<AtomicBool>,
    /// The slot of the cycle traces that the control link reads, shared with the runtime's flight
    /// recorder (specification 11 §6.8.1, "Cycle traces").
    pub traces: crate::runtime::SharedTraces,
}

impl ControlState {
    /// A control state whose client is on this computer and needs no token, so that a request
    /// without one has the Admin role (`trust_runtime_services::access::resolve_role`), with the
    /// optional services of a run empty.
    #[must_use]
    pub fn local(parts: LocalControl) -> Self {
        let state = Self {
            transfers: new_transfer_session(),
            traces: parts.traces,
            debug: parts.debug,
            identify: parts.identify,
            resource: parts.resource,
            sources: parts.sources,
            io_snapshot: parts.io_snapshot,
            io_snapshot_seen_ms: parts.io_snapshot_seen_ms,
            pending_restart: parts.pending_restart,
            auth_token: Arc::new(Mutex::new(None)),
            control_requires_auth: false,
            control_mode: Arc::new(Mutex::new(parts.control_mode)),
            audit_tx: None,
            metrics: parts.metrics,
            events: parts.events,
            settings: Arc::new(Mutex::new(parts.settings)),
            discovery: Arc::new(DiscoveryState::new()),
            mesh_topology: Arc::new(Mutex::new(None)),
            realtime_status: Arc::new(Mutex::new(LinuxRtRuntimeStatus::from_config(
                crate::linux_rt::LinuxRtConfig::default(),
            ))),
            web_listener_bound: Arc::new(AtomicBool::new(false)),
            opcua_server_bound: Arc::new(AtomicBool::new(false)),
            project_root: parts.project_root,
            plant_service: Arc::new(PlantControlService::default()),
            experiments: ControlExperimentService::default(),
            controllers: ControllerParameterService::default(),
            searches: search::SearchService::default(),
            resource_name: parts.resource_name,
            runtime_id: parts.runtime_id,
            io_health: parts.io_health,
            debug_enabled: parts.debug_enabled,
            historian: None,
            openot_persistence_status: None,
            pairing: None,
            search: Default::default(),
            ads_doctor_jobs: Arc::new(Mutex::new(AdsDoctorJobStore::default())),
            ads_client_config: Arc::new(Mutex::new(None)),
            opcua_client_config: Arc::new(Mutex::new(None)),
            ads_server_config: Arc::new(Mutex::new(None)),
            #[cfg(feature = "ads-server")]
            ads_server_runtime: Arc::new(Mutex::new(None)),
        };
        state.connect_experiments();
        state
    }
}
