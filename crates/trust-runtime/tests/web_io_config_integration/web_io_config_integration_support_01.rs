use super::*;

pub(super) fn runtime_settings() -> RuntimeSettings {
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

pub(super) fn source_fixture() -> &'static str {
    "PROGRAM Main\nEND_PROGRAM\n"
}

pub(super) fn control_state(source: &str) -> Arc<ControlState> {
    control_state_named(source, "RESOURCE")
}

pub(super) fn control_state_named(source: &str, resource_name: &str) -> Arc<ControlState> {
    control_state_named_with_audit(source, resource_name, None)
}

pub(super) fn control_state_named_with_audit(
    source: &str,
    resource_name: &str,
    audit_tx: Option<Sender<ControlAuditEvent>>,
) -> Arc<ControlState> {
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

    let sources = SourceRegistry::new(vec![SourceFile {
        id: 1,
        path: PathBuf::from("main.st"),
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
        audit_tx,
        metrics: Arc::new(Mutex::new(RuntimeMetrics::default())),
        events: Arc::new(Mutex::new(VecDeque::new())),
        settings: Arc::new(Mutex::new(runtime_settings())),
        realtime_status: Arc::new(Mutex::new(
            trust_runtime::linux_rt::LinuxRtRuntimeStatus::from_config(
                trust_runtime::linux_rt::LinuxRtConfig::default(),
            ),
        )),
        project_root: None,
        resource_name: SmolStr::new(resource_name),
        io_health: Arc::new(Mutex::new(Vec::new())),
        debug_enabled: Arc::new(AtomicBool::new(true)),
        debug_variables: Arc::new(Mutex::new(DebugVariableHandles::new())),
        hmi_live: Arc::new(Mutex::new(trust_runtime::hmi::HmiLiveState::default())),
        hmi_descriptor,
        historian: None,
        pairing: None,
    })
}

pub(super) fn reserve_loopback_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind local port");
    let port = listener.local_addr().expect("read local addr").port();
    drop(listener);
    port
}

pub(super) fn start_test_server(state: Arc<ControlState>, project_root: PathBuf) -> String {
    start_test_server_with_options(state, project_root, None, None, WebAuthMode::Local)
}

pub(super) fn start_test_server_with_discovery(
    state: Arc<ControlState>,
    project_root: PathBuf,
    discovery: Option<Arc<DiscoveryState>>,
) -> String {
    start_test_server_with_options(state, project_root, discovery, None, WebAuthMode::Local)
}

pub(super) fn start_test_server_with_options(
    state: Arc<ControlState>,
    project_root: PathBuf,
    discovery: Option<Arc<DiscoveryState>>,
    pairing: Option<Arc<PairingStore>>,
    auth: WebAuthMode,
) -> String {
    start_test_server_with_options_and_profile(
        state,
        project_root,
        discovery,
        pairing,
        auth,
        RuntimeCloudProfile::Dev,
    )
}
