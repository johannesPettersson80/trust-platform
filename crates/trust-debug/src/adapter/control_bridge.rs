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
    AdsDoctorJobStore, ControlEndpoint, ControlServer, ControlState, SourceRegistry,
};
use trust_runtime::debug::{DebugControl, DebugSnapshot, DebugVariableHandles, RuntimeEvent};
use trust_runtime::discovery::DiscoveryState;
use trust_runtime::error::RuntimeError;
use trust_runtime::io::IoDriverStatus;
use trust_runtime::linux_rt::{LinuxRtConfig, LinuxRtRuntimeStatus};
use trust_runtime::mesh::MeshTopologyEvidence;
use trust_runtime::metrics::RuntimeMetrics;
use trust_runtime::scheduler::{simulation_warning, ResourceCommand, ResourceControl, StdClock};
use trust_runtime::settings::{
    BaseSettings, DiscoverySettings, MeshSettings, RuntimeSettings, SimulationSettings, WebSettings,
};
use trust_runtime::value::Value;
use trust_runtime::watchdog::{FaultPolicy, RetainMode, WatchdogPolicy};

use super::core::cycle_time_hint;
use crate::runtime::DebugRuntime;
use crate::session::SimulationFacts;

#[cfg(unix)]
const DEFAULT_UNIX_ENDPOINT: &str = "/tmp/trust-debug.sock";
#[cfg(not(unix))]
const DEFAULT_TCP_ENDPOINT: &str = "127.0.0.1:9901";

pub(super) struct DebugControlServer {
    server: ControlServer,
    io_snapshot: Arc<Mutex<Option<trust_runtime::io::IoSnapshot>>>,
    io_snapshot_seen_ms: Arc<AtomicU64>,
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
        // The simulator loop paces with this speed, and `config.set` changes it (specification 33, §6.1 item 5). It is
        // attached whatever the facts are at start: a reload can change them, and `config.set` refuses a physical target.
        let resource = resource.with_simulation_speed(session.simulation_speed());
        // The display is served at the debugged program's Hmi.Display call (specification 39, §18).
        if let Ok(mut runtime) = session.runtime_handle().lock() {
            runtime.set_hmi_exchange(resource.hmi_exchange());
        }
        let debug = session.debug_control();
        let sources = SourceRegistry::new(session.control_sources());
        let io_snapshot = Arc::new(Mutex::new(None));
        let io_snapshot_seen_ms = Arc::new(AtomicU64::new(0));
        let state = Arc::new(ControlState {
            debug: debug.clone(),
            resource,
            metadata: Arc::new(Mutex::new(session.metadata().clone())),
            project_root,
            plant_service: Arc::new(trust_runtime::control::PlantControlService::default()),
            experiments: trust_runtime::control::ControlExperimentService::default(),
            controllers: trust_runtime::control::ControllerParameterService::default(),
            searches: trust_runtime::control::search::SearchService::default(),
            sources,
            io_snapshot: io_snapshot.clone(),
            io_snapshot_seen_ms: io_snapshot_seen_ms.clone(),
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
            historian: None,
            pairing: None,
            ads_doctor_jobs: Arc::new(Mutex::new(AdsDoctorJobStore::default())),
            ads_client_config: Arc::new(Mutex::new(None)),
            opcua_client_config: Arc::new(Mutex::new(None)),
            ads_server_config: Arc::new(Mutex::new(None)),
            #[cfg(feature = "ads-server")]
            ads_server_runtime: Arc::new(Mutex::new(None)),
        });
        // Experiments advance on the simulator's scans, as they do in the runtime binary (specification 35, A3.5).
        state.connect_experiments();
        let server = ControlServer::start(endpoint, state.clone())?;
        let drain = spawn_command_drain(
            cmd_rx,
            debug,
            session.runtime_handle(),
            session.simulation_handle(),
        );
        Ok(Self {
            server,
            io_snapshot,
            io_snapshot_seen_ms,
            _drain: drain,
        })
    }

    pub fn endpoint(&self) -> &ControlEndpoint {
        self.server.endpoint()
    }

    pub(super) fn io_snapshot_sink(
        &self,
    ) -> (
        Arc<Mutex<Option<trust_runtime::io::IoSnapshot>>>,
        Arc<AtomicU64>,
    ) {
        (self.io_snapshot.clone(), self.io_snapshot_seen_ms.clone())
    }

    /// The metrics that control.status reports. The simulator loop publishes its achieved speed into them.
    pub(super) fn metrics_sink(&self) -> Arc<Mutex<RuntimeMetrics>> {
        self.server.state().metrics.clone()
    }

    /// Reports the program that is now loaded (specification 35, A2.3 and A2.4): the tasks that `status` lists, the cycle that
    /// the managed runner takes from them, the simulation facts, and the program's sources, which isolated runs and searches compile
    /// and breakpoints resolve against. A reload changes all of them, as a reload in the runtime replaces the metadata that its
    /// `status` lists.
    pub(super) fn refresh_program(&self, session: &dyn DebugRuntime) {
        let state = self.server.state();
        if let Ok(mut metadata) = state.metadata.lock() {
            *metadata = session.metadata().clone();
        }
        state.sources.replace(session.control_sources());
        let Ok(mut settings) = state.settings.lock() else {
            return;
        };
        settings.cycle_interval = cycle_time_hint(session.metadata());
        settings.simulation = simulation_settings(session.simulation_facts());
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
        // `status` reports the cycle that the managed runner takes, never a constant (annex E1.3). `refresh_program` keeps it
        // current across reloads.
        cycle_time_hint(session.metadata()),
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
        simulation_settings(session.simulation_facts()),
    )
}

/// The simulation settings for `facts`. The mode label and the warning are the ones the runtime binary shows in
/// simulation mode (specification 33, §6.1 item 6).
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

fn spawn_command_drain(
    rx: std::sync::mpsc::Receiver<ResourceCommand>,
    debug: DebugControl,
    runtime: Arc<Mutex<trust_runtime::Runtime>>,
    simulation: Arc<Mutex<Option<trust_runtime::simulation::SimulationController>>>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        while let Ok(command) = rx.recv() {
            match command {
                ResourceCommand::ReloadBytecode { respond_to, .. } => {
                    let _ = respond_to.send(Err(RuntimeError::ControlError(SmolStr::new(
                        "bytecode reload unavailable in debug control",
                    ))));
                }
                ResourceCommand::MeshCatalogue { respond_to, .. } => {
                    let _ = respond_to
                        .send(Err("mesh catalogue unavailable in debug control".to_owned()));
                }
                ResourceCommand::MeshSnapshot { respond_to, .. } => {
                    let _ = respond_to.send(IndexMap::<SmolStr, Value>::new());
                }
                ResourceCommand::MeshApply { .. } => {}
                ResourceCommand::Snapshot { respond_to } => {
                    let _ = respond_to.send(debug.snapshot().unwrap_or_else(empty_debug_snapshot));
                }
                ResourceCommand::SimulationSnapshot { respond_to } => {
                    let qualified = runtime
                        .lock()
                        .ok()
                        .map(|runtime| runtime.simulation_only_io());
                    let snapshot = simulation.lock().ok().and_then(|simulation| {
                        simulation.as_ref().map(|simulation| {
                            let mut snapshot = simulation.runtime_snapshot();
                            snapshot.simulation_only_io = qualified;
                            snapshot
                        })
                    });
                    let _ = respond_to.send(snapshot);
                }
                ResourceCommand::SimulationTraceClear { respond_to, .. } => {
                    let _ = respond_to.send(Err(
                        trust_runtime::simulation::PlantTraceClearError::StatusUnavailable,
                    ));
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

#[cfg(all(test, target_os = "linux"))]
mod search_provenance_tests {
    use super::*;
    use crate::DebugAdapter;
    use trust_runtime::value::Duration;

    #[test]
    fn debug_bridge_reports_the_actual_session_simulation_snapshot() {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("debug-search-host-{}-{stamp}", std::process::id()));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../tests/fixtures/control_search_worker.json"
        ))
        .unwrap();
        std::fs::write(
            root.join("src/Main.st"),
            fixture["snapshot"]["sources"][0]["text"].as_str().unwrap(),
        )
        .unwrap();
        std::fs::write(
            root.join("simulation.toml"),
            fixture["snapshot"]["simulation"].as_str().unwrap(),
        )
        .unwrap();
        let mut session = crate::DebugSession::new(trust_runtime::Runtime::new());
        session.update_source_options(crate::session::SourceOptionsUpdate {
            root: Some(root.to_string_lossy().into_owned()),
            include_globs: Some(vec!["**/*.st".into()]),
            exclude_globs: Some(vec![]),
            ignore_pragmas: None,
        });
        session
            .reload_program(Some(root.join("src/Main.st").to_string_lossy().as_ref()))
            .unwrap();
        let bridge = DebugControlServer::start(
            &session,
            ControlEndpoint::Tcp("127.0.0.1:0".parse().unwrap()),
            None,
            Some(root.clone()),
        )
        .unwrap();
        let (sender, receiver) = std::sync::mpsc::channel();
        bridge
            .server
            .state()
            .resource
            .send_command(ResourceCommand::SimulationSnapshot { respond_to: sender })
            .unwrap();
        let snapshot = receiver
            .recv_timeout(std::time::Duration::from_secs(2))
            .unwrap();
        assert!(
            snapshot.is_some(),
            "debug control bridge must expose its real simulation instead of hardcoded None"
        );
        assert!(snapshot.unwrap().status.active);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn simulation_snapshot_remains_available_between_slow_runner_scans() {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "debug-slow-simulation-snapshot-{}-{stamp}",
            std::process::id()
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
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
        std::fs::write(root.join("src/Main.st"), source).unwrap();
        std::fs::write(
            root.join("simulation.toml"),
            r#"
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
"#,
        )
        .unwrap();
        let mut session = crate::DebugSession::new(trust_runtime::Runtime::new());
        session.update_source_options(crate::session::SourceOptionsUpdate {
            root: Some(root.to_string_lossy().into_owned()),
            include_globs: Some(vec!["**/*.st".into()]),
            exclude_globs: Some(vec![]),
            ignore_pragmas: None,
        });
        session
            .reload_program(Some(root.join("src/Main.st").to_string_lossy().as_ref()))
            .unwrap();
        let mut adapter = DebugAdapter::new(session);
        adapter.control_server = Some(
            DebugControlServer::start(
                adapter.session(),
                ControlEndpoint::Tcp("127.0.0.1:0".parse().unwrap()),
                None,
                Some(root.clone()),
            )
            .unwrap(),
        );
        let resource = adapter
            .control_server
            .as_ref()
            .unwrap()
            .server
            .state()
            .resource
            .clone();
        let runtime = adapter.session().runtime_handle();
        adapter.start_runner();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while runtime.lock().unwrap().current_time() == Duration::ZERO {
            assert!(
                std::time::Instant::now() < deadline,
                "runner did not finish its first cycle"
            );
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        let (sender, receiver) = std::sync::mpsc::channel();
        resource
            .send_command(ResourceCommand::SimulationSnapshot { respond_to: sender })
            .unwrap();
        let snapshot = receiver.recv_timeout(std::time::Duration::from_millis(250));
        adapter.stop_runner();
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
        let main = root.join("src/Main.st").to_string_lossy().into_owned();
        let mut session = crate::DebugSession::new(trust_runtime::Runtime::new());
        session.update_source_options(crate::session::SourceOptionsUpdate {
            root: Some(root.to_string_lossy().into_owned()),
            include_globs: Some(vec!["**/*.st".into()]),
            exclude_globs: Some(vec![]),
            ignore_pragmas: None,
        });
        session.reload_program(Some(main.as_ref())).unwrap();
        let bridge = DebugControlServer::start(
            &session,
            ControlEndpoint::Tcp("127.0.0.1:0".parse().unwrap()),
            None,
            Some(root.clone()),
        )
        .unwrap();
        // A reload builds a new runtime; the display keeps its exchange.
        session.reload_program(Some(main.as_ref())).unwrap();
        let paths: Vec<_> = ["Main.Count", "Main.Gone"]
            .iter()
            .map(|path| trust_runtime::hmi::member_paths::MemberPath::parse(path).unwrap())
            .collect();
        let exchange = bridge.server.state().resource.hmi_exchange();
        let _screen = exchange.open_screen(paths.clone());
        session
            .runtime_handle()
            .lock()
            .unwrap()
            .execute_cycle()
            .unwrap();
        let read = exchange
            .read(&paths)
            .expect("the debugged program's Hmi.Display call served the display");
        let values: Vec<Option<serde_json::Value>> = read
            .values
            .into_iter()
            .map(|value| value.map(|wire| serde_json::to_value(wire).unwrap()))
            .collect();
        assert_eq!(values, vec![Some(serde_json::json!({ "value": 7 })), None]);
        let _ = std::fs::remove_dir_all(root);
    }
}

#[cfg(test)]
#[path = "control_bridge_mesh_tests.rs"]
mod mesh_tests;
