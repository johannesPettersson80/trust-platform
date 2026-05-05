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

pub(super) fn hmi_control_state_with_root(
    source: &str,
    project_root: Option<PathBuf>,
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
        realtime_status: Arc::new(Mutex::new(
            trust_runtime::linux_rt::LinuxRtRuntimeStatus::from_config(
                trust_runtime::linux_rt::LinuxRtConfig::default(),
            ),
        )),
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

pub(super) fn hmi_control_state(source: &str) -> Arc<ControlState> {
    hmi_control_state_with_root(source, None)
}

pub(super) fn write_file(path: &Path, text: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent directories");
    }
    fs::write(path, text).expect("write file");
}

pub(super) fn temp_dir(prefix: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("trust-hmi-readonly-{prefix}-{stamp}"));
    fs::create_dir_all(&root).expect("create temp dir");
    root
}

pub(super) fn reserve_loopback_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind local port");
    let port = listener.local_addr().expect("read local addr").port();
    drop(listener);
    port
}

pub(super) fn start_test_server(state: Arc<ControlState>) -> String {
    for _ in 0..16 {
        let port = reserve_loopback_port();
        let listen = format!("127.0.0.1:{port}");
        let config = WebConfig {
            enabled: true,
            listen: SmolStr::new(listen.clone()),
            auth: WebAuthMode::Local,
            tls: false,
        };
        match start_web_server(&config, Arc::clone(&state), None, None, None, None) {
            Ok(_server) => {
                let base = format!("http://{listen}");
                wait_for_server(&base);
                return base;
            }
            Err(RuntimeError::ControlError(message))
                if message.contains("Address already in use") =>
            {
                thread::sleep(Duration::from_millis(10));
            }
            Err(err) => panic!("start web server: {err}"),
        }
    }
    panic!("start web server: no free loopback port after retries");
}

pub(super) fn wait_for_server(base: &str) {
    for _ in 0..80 {
        if ureq::get(&format!("{base}/hmi")).call().is_ok() {
            return;
        }
        thread::sleep(Duration::from_millis(25));
    }
    panic!("web server did not become reachable at {base}");
}
