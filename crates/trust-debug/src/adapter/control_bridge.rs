//! Control-server bridge for debug sessions (inline values, tooling): the control server of the
//! simulator, whose resource is trust-runtime's scheduler running the launch's compiled program
//! (specification 44 NX-16).

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use indexmap::IndexMap;
use smol_str::SmolStr;

use trust_runtime::config::ControlMode;
use trust_runtime::control::{
    AdsDoctorJobStore, ControlEndpoint, ControlServer, ControlState, SourceRegistry,
};
use trust_runtime::debug::{DebugControl, RuntimeEvent};
use trust_runtime::discovery::DiscoveryState;
use trust_runtime::error::RuntimeError;
use trust_runtime::io::IoDriverStatus;
use trust_runtime::linux_rt::{LinuxRtConfig, LinuxRtRuntimeStatus};
use trust_runtime::mesh::MeshTopologyEvidence;
use trust_runtime::metrics::RuntimeMetrics;
use trust_runtime::scheduler::{simulation_warning, ResourceControl, StdClock};
use trust_runtime::settings::{
    BaseSettings, DiscoverySettings, MeshSettings, RuntimeSettings, SimulationSettings, WebSettings,
};
use trust_runtime::value::Duration;
use trust_runtime::watchdog::{FaultPolicy, RetainMode, WatchdogPolicy};
use trust_runtime::RestartMode;

use crate::runtime::DebugRuntime;
use crate::session::SimulationFacts;

#[cfg(unix)]
const DEFAULT_UNIX_ENDPOINT: &str = "/tmp/trust-debug.sock";
#[cfg(not(unix))]
const DEFAULT_TCP_ENDPOINT: &str = "127.0.0.1:9901";

pub(super) struct DebugControlServer {
    server: ControlServer,
}

/// What a compiled program reports to the control server through: its debugger, which also
/// carries its I/O snapshots and runtime events, its metrics and its drivers' health; and the
/// restart signal its scheduler takes the control server's `restart` requests from.
pub(super) struct ServedProgram {
    pub(super) debug: DebugControl,
    pub(super) metrics: Arc<Mutex<RuntimeMetrics>>,
    pub(super) io_health: Arc<Mutex<Vec<IoDriverStatus>>>,
    pub(super) pending_restart: Arc<Mutex<Option<RestartMode>>>,
}

impl Default for ServedProgram {
    /// Reports that nothing has filled yet, and no restart is queued.
    fn default() -> Self {
        Self {
            debug: DebugControl::new(),
            metrics: Arc::new(Mutex::new(RuntimeMetrics::default())),
            io_health: Arc::new(Mutex::new(Vec::new())),
            pending_restart: Arc::new(Mutex::new(None)),
        }
    }
}

/// What the simulator runs its program with, which `status` and `config.get` report in place of fixed values
/// (specification 35, A2.3 and A2.4; specification 46, §6.1 item 6): the cycle its scheduler takes, the watchdog and
/// fault policies of `runtime.toml`, and the simulation facts of its I/O drivers and its `simulation.toml`.
#[derive(Debug, Clone, Copy)]
pub(super) struct SimulatorRun {
    pub(super) cycle_interval: Duration,
    pub(super) watchdog: WatchdogPolicy,
    pub(super) fault_policy: FaultPolicy,
    pub(super) simulation: SimulationFacts,
}

/// The parts of a control state that the simulator gives.
struct Served {
    debug: DebugControl,
    resource: ResourceControl<StdClock>,
    sources: SourceRegistry,
    settings: RuntimeSettings,
    metrics: Arc<Mutex<RuntimeMetrics>>,
    io_health: Arc<Mutex<Vec<IoDriverStatus>>>,
    io_snapshot: Arc<Mutex<Option<trust_runtime::io::IoSnapshot>>>,
    io_snapshot_seen_ms: Arc<AtomicU64>,
    events: Arc<Mutex<VecDeque<RuntimeEvent>>>,
    pending_restart: Arc<Mutex<Option<RestartMode>>>,
}

impl DebugControlServer {
    /// The control server of a compiled program that trust-runtime's scheduler runs under
    /// `resource` (specification 44 NX-16), as trust-runtime serves it: the I/O snapshots and the
    /// runtime events its debugger sends are kept for `io.read`, `fleet.topology` and
    /// `events.tail`. `status` reports what the program runs with (`run`) and the tasks of the
    /// session's program, and the isolated runs, searches and breakpoints take the session's
    /// sources (specification 35, A2.3 and A2.4).
    pub fn start_compiled(
        program: ServedProgram,
        run: SimulatorRun,
        resource: ResourceControl<StdClock>,
        session: &dyn DebugRuntime,
        endpoint: ControlEndpoint,
        auth_token: Option<String>,
        project_root: Option<PathBuf>,
    ) -> Result<Self, RuntimeError> {
        let io_snapshot = Arc::new(Mutex::new(None));
        // When the last snapshot arrived, as the runtime binary keeps it: `fleet.topology` shows the
        // simulator's I/O live from it (Devices & Connections, specification 47).
        let io_snapshot_seen_ms = Arc::new(AtomicU64::new(0));
        let (io_tx, io_rx) = std::sync::mpsc::channel();
        program.debug.set_io_sender(io_tx);
        {
            let io_snapshot = io_snapshot.clone();
            let io_snapshot_seen_ms = io_snapshot_seen_ms.clone();
            thread::spawn(move || {
                for snapshot in io_rx {
                    if let Ok(mut guard) = io_snapshot.lock() {
                        *guard = Some(snapshot);
                        io_snapshot_seen_ms.store(
                            std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .map(|duration| duration.as_millis() as u64)
                                .unwrap_or(0),
                            Ordering::Relaxed,
                        );
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
        let served = Served {
            debug: program.debug,
            resource,
            sources: SourceRegistry::new(session.control_sources()),
            settings: simulator_settings(&run),
            metrics: program.metrics,
            io_health: program.io_health,
            io_snapshot,
            io_snapshot_seen_ms,
            events,
            pending_restart: program.pending_restart,
        };
        Ok(Self {
            server: Self::serve(served, endpoint, auth_token, project_root)?,
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
            sources,
            settings,
            metrics,
            io_health,
            io_snapshot,
            io_snapshot_seen_ms,
            events,
            pending_restart,
        } = served;
        let state = Arc::new(ControlState {
            transfers: trust_runtime::control::new_transfer_session(),
            debug,
            identify: trust_runtime::io::IdentifyHandle::default(),
            resource,
            project_root,
            plant_service: Arc::new(trust_runtime::control::PlantControlService::default()),
            experiments: trust_runtime::control::ControlExperimentService::default(),
            controllers: trust_runtime::control::ControllerParameterService::default(),
            searches: trust_runtime::control::search::SearchService::default(),
            sources,
            io_snapshot,
            io_snapshot_seen_ms,
            pending_restart,
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
            // The program's driver health, which `fleet.topology` reports as the runtime binary
            // does (specification 47; Devices & Connections shows the simulator's I/O live).
            io_health,
            realtime_status: Arc::new(Mutex::new(LinuxRtRuntimeStatus::from_config(
                LinuxRtConfig::default(),
            ))),
            debug_enabled: Arc::new(AtomicBool::new(true)),
            openot_persistence_status: None,
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
        // Experiments advance on the simulator's scans, as they do in the runtime binary (specification 35, A3.5).
        state.connect_experiments();
        ControlServer::start(endpoint, state)
    }

    /// Reports the program that a reload changed (specification 35, A2.3 and A2.4): the program's sources, which
    /// isolated runs and searches compile and breakpoints resolve against; the tasks that `status` lists come from the
    /// image the reloaded program runs. A simulated target takes `time_scale`, the speed of
    /// the project's `simulation.toml`, which the simulator reads again at a reload (specification 46, §6.1 item 5).
    /// The change keeps the drivers, the plants and the cycle of the scheduler (specification 44 §12), so the target
    /// stays simulated or physical and `status` keeps its cycle.
    pub(super) fn refresh_program(&self, session: &dyn DebugRuntime, time_scale: u32) {
        let state = self.server.state();
        state.sources.replace(session.control_sources());
        let Ok(mut settings) = state.settings.lock() else {
            return;
        };
        if !settings.simulation.enabled {
            return;
        }
        let facts = SimulationFacts::for_io(false, time_scale);
        settings.simulation = simulation_settings(facts);
        if let Some(speed) = state.resource.simulation_speed() {
            speed.set(facts.time_scale);
        }
    }
}

#[cfg(test)]
impl DebugControlServer {
    /// The texts of the sources the control state holds now, in order.
    pub(super) fn source_texts(&self) -> Vec<String> {
        self.server
            .state()
            .sources
            .files()
            .iter()
            .map(|file| file.text.clone())
            .collect()
    }

    /// The control state the server serves.
    pub(super) fn state(&self) -> Arc<ControlState> {
        self.server.state()
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

/// The settings a simulator reports: the cycle its scheduler takes, the watchdog and fault policies and the simulation
/// facts it runs with (`run`), no retain file, and no web, discovery or mesh service.
fn simulator_settings(run: &SimulatorRun) -> RuntimeSettings {
    RuntimeSettings::new(
        // `status` reports the cycle that the scheduler takes, never a constant (annex E1.3).
        run.cycle_interval,
        BaseSettings {
            log_level: SmolStr::new("info"),
            watchdog: run.watchdog,
            fault_policy: run.fault_policy,
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
        simulation_settings(run.simulation),
    )
}

/// The simulation settings for `facts`. The mode label and the warning are the ones the runtime binary shows in
/// simulation mode (specification 46, §6.1 item 6).
fn simulation_settings(facts: SimulationFacts) -> SimulationSettings {
    let warning = if facts.enabled {
        simulation_warning(facts.time_scale)
    } else {
        String::new()
    };
    SimulationSettings {
        enabled: facts.enabled,
        time_scale: facts.time_scale,
        mode_label: SmolStr::new(if facts.enabled {
            "simulation"
        } else {
            "production"
        }),
        warning: SmolStr::new(warning),
    }
}

/// The simulator of the project folder `root` whose program is `main`, built and started as a launch builds and starts
/// it (specification 44 NX-16), with its control server at `endpoint` and its scheduler running, and the session that
/// holds the program's sources. It needs the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.
#[cfg(test)]
pub(super) fn start_simulator(
    root: &std::path::Path,
    main: &std::path::Path,
    endpoint: ControlEndpoint,
    auth_token: Option<String>,
) -> (crate::DebugSession, super::simulator::Simulator) {
    trust_runtime::native::Toolchain::find()
        .expect("TRUST_TOOLCHAIN names the toolchain of specification 44 §4.6");
    let mut session = crate::DebugSession::new();
    session.update_source_options(crate::session::SourceOptionsUpdate {
        root: Some(root.to_string_lossy().into_owned()),
        include_globs: Some(vec!["**/*.st".into()]),
        exclude_globs: Some(vec![]),
        ignore_pragmas: None,
    });
    let program = main.to_string_lossy().into_owned();
    let sources = session
        .launch_sources(&program)
        .expect("the project's sources");
    let image = super::simulator::build_image(root, &sources, session.conversion_profile())
        .expect("the project builds");
    let simulator =
        super::simulator::Simulator::start(image, &program, root, &session, endpoint, auth_token)
            .expect("the simulator starts");
    simulator.open();
    (session, simulator)
}

#[cfg(all(test, target_os = "linux"))]
mod search_provenance_tests {
    use super::*;
    use std::sync::mpsc::RecvTimeoutError;
    use trust_runtime::scheduler::{ResourceCommand, ResourceState};
    use trust_runtime::simulation::SimulationRuntimeSnapshot;

    /// A project folder labelled `label` with `main` as its program in `src/Main.st` and `simulation` as its
    /// `simulation.toml`.
    fn project(label: &str, main: &str, simulation: &str) -> PathBuf {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("{label}-{}-{stamp}", std::process::id()));
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join("src/Main.st"), main).unwrap();
        std::fs::write(root.join("simulation.toml"), simulation).unwrap();
        root
    }

    /// The simulation snapshot that the resource the control server serves answers within `wait`.
    fn simulation_snapshot(
        resource: &ResourceControl<StdClock>,
        wait: std::time::Duration,
    ) -> Result<Option<SimulationRuntimeSnapshot>, RecvTimeoutError> {
        let (sender, receiver) = std::sync::mpsc::channel();
        resource
            .send_command(ResourceCommand::SimulationSnapshot { respond_to: sender })
            .unwrap();
        receiver.recv_timeout(wait)
    }

    /// Specification 47 (`plant.status`): the control server answers with the simulation that the simulator runs, from
    /// the project's `simulation.toml`. Catches a control server that answers with no simulation while the plants run.
    #[test]
    fn debug_bridge_reports_the_actual_session_simulation_snapshot() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../tests/fixtures/control_search_worker.json"
        ))
        .unwrap();
        let root = project(
            "debug-search-host",
            fixture["snapshot"]["sources"][0]["text"].as_str().unwrap(),
            fixture["snapshot"]["simulation"].as_str().unwrap(),
        );
        let (_session, simulator) = start_simulator(
            &root,
            &root.join("src/Main.st"),
            ControlEndpoint::Tcp("127.0.0.1:0".parse().unwrap()),
            None,
        );
        let snapshot = simulation_snapshot(
            &simulator.control_server().server.state().resource,
            std::time::Duration::from_secs(2),
        );
        simulator.stop();
        let snapshot = snapshot.unwrap();
        assert!(
            snapshot.is_some(),
            "debug control bridge must expose its real simulation instead of hardcoded None"
        );
        assert!(snapshot.unwrap().status.active);
        let _ = std::fs::remove_dir_all(root);
    }

    /// Specification 47 (`plant.status`): a program whose task runs once a second still has its simulation snapshot
    /// answered between scans. Catches a snapshot that waits for the program's next one-second execution.
    #[test]
    fn simulation_snapshot_remains_available_between_slow_runner_scans() {
        let source = r#"
PROGRAM Main
VAR
    PV AT %IL0 : LREAL;
    MV AT %QL0 : LREAL := LREAL#0.0;
END_VAR
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#1s, PRIORITY := 1);
    PROGRAM Main WITH MainTask : Main;
END_RESOURCE
END_CONFIGURATION
"#;
        let simulation = r#"
[simulation]
enabled = true
plant_step_ms = 1000
[[plants]]
id = "slow-plant"
model = "first_order"
command_sources = ["%QL0"]
feedback_targets = ["%IL0"]
initial_state = [0.0]
[plants.parameters]
gain = 1.0
time_constant_s = 1.0
"#;
        let root = project("debug-slow-simulation-snapshot", source, simulation);
        let (_session, simulator) = start_simulator(
            &root,
            &root.join("src/Main.st"),
            ControlEndpoint::Tcp("127.0.0.1:0".parse().unwrap()),
            None,
        );
        let resource = simulator.control_server().server.state().resource.clone();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while !matches!(resource.state(), ResourceState::Running) {
            assert!(
                std::time::Instant::now() < deadline,
                "runner did not start its cycles"
            );
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        let snapshot = simulation_snapshot(&resource, std::time::Duration::from_millis(250));
        simulator.stop();
        let snapshot = snapshot
            .expect("active simulation snapshot must not wait for the next one-second scan")
            .expect("active simulated session must report a snapshot between scans");
        assert!(snapshot.status.active);
        let _ = std::fs::remove_dir_all(root);
    }
}

#[cfg(test)]
mod display_read_tests {
    use super::*;
    use trust_runtime::scheduler::{ReloadClaim, ResourceCommand};

    /// Specification 39, §11.1 and §18 Phase 5: the runtime display shows the debugged program's
    /// values from its `Hmi.Display` call, also after the program is reloaded.
    #[test]
    fn the_runtime_display_reads_the_debugged_programs_values() {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("debug-hmi-read-{}-{stamp}", std::process::id()));
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(
            root.join("src/hmi.st"),
            include_str!("../../../../libraries/hmi/src/hmi.st"),
        )
        .unwrap();
        std::fs::write(
            root.join("src/Main.st"),
            "PROGRAM Main\nVAR\n    Count : INT := 7;\n    Screens : Hmi.Display;\nEND_VAR\n    Screens();\nEND_PROGRAM\n",
        )
        .unwrap();
        let main = root.join("src/Main.st");
        let (mut session, simulator) = start_simulator(
            &root,
            &main,
            ControlEndpoint::Tcp("127.0.0.1:0".parse().unwrap()),
            None,
        );
        let resource = simulator.control_server().server.state().resource.clone();
        // A reload changes the program online (specification 44 §12); the display keeps its exchange.
        let sources = session
            .launch_sources(main.to_string_lossy().as_ref())
            .unwrap();
        let image = super::super::simulator::build_image(
            &root,
            &sources,
            crate::runtime::DebugRuntime::conversion_profile(&session),
        )
        .unwrap();
        let (sender, receiver) = std::sync::mpsc::channel();
        resource
            .send_command(ResourceCommand::ReloadBytecode {
                bytes: image,
                claim: ReloadClaim::default(),
                respond_to: sender,
            })
            .unwrap();
        receiver
            .recv_timeout(std::time::Duration::from_secs(10))
            .expect("the reload is answered")
            .expect("the program changes online");
        let paths: Vec<_> = ["Main.Count", "Main.Gone"]
            .iter()
            .map(|path| trust_runtime::hmi::member_paths::MemberPath::parse(path).unwrap())
            .collect();
        let exchange = resource.hmi_exchange();
        let _screen = exchange.open_screen(paths.clone());
        // The compiled runtime serves the exchange at the program's `Hmi.Display` call (specification 39, §18 Phase 5).
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        let values = loop {
            let values: Option<Vec<Option<serde_json::Value>>> =
                exchange.read(&paths).map(|read| {
                    read.values
                        .into_iter()
                        .map(|value| value.map(|wire| serde_json::to_value(wire).unwrap()))
                        .collect()
                });
            if let Some(values) = values
                && values.first().is_some_and(Option::is_some)
            {
                break values;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "the debugged program's Hmi.Display call served the display"
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        };
        simulator.stop();
        assert_eq!(values, vec![Some(serde_json::json!({ "value": 7 })), None]);
        let _ = std::fs::remove_dir_all(root);
    }
}

// The simulator's scheduler answers `MeshCatalogue`, so the test sends the request to the resource of a started simulator.
#[cfg(test)]
#[path = "control_bridge_mesh_tests.rs"]
mod mesh_tests;
