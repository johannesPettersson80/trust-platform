//! Control-server bridge for debug sessions (inline values, tooling).

use std::collections::VecDeque;
#[cfg(unix)]
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::thread;

use indexmap::IndexMap;
use smol_str::SmolStr;

use trust_runtime::config::ControlMode;
use trust_runtime::control::{ControlEndpoint, ControlServer, ControlState, SourceRegistry};
use trust_runtime::debug::{DebugVariableHandles, RuntimeEvent};
use trust_runtime::error::RuntimeError;
use trust_runtime::io::IoDriverStatus;
use trust_runtime::metrics::RuntimeMetrics;
use trust_runtime::scheduler::{ResourceCommand, ResourceControl, StdClock};
use trust_runtime::settings::{
    BaseSettings, DiscoverySettings, MeshSettings, RuntimeSettings, SimulationSettings, WebSettings,
};
use trust_runtime::value::Value;
use trust_runtime::watchdog::{FaultPolicy, RetainMode, WatchdogPolicy};

use crate::runtime::DebugRuntime;

#[cfg(unix)]
const DEFAULT_UNIX_ENDPOINT: &str = "/tmp/trust-debug.sock";
#[cfg(not(unix))]
const DEFAULT_TCP_ENDPOINT: &str = "127.0.0.1:9901";

pub(super) struct DebugControlServer {
    server: ControlServer,
    _drain: thread::JoinHandle<()>,
}

impl DebugControlServer {
    pub fn start(
        session: &dyn DebugRuntime,
        endpoint: ControlEndpoint,
        auth_token: Option<String>,
    ) -> Result<Self, RuntimeError> {
        let (resource, cmd_rx) = ResourceControl::stub(StdClock::new());
        let state = Arc::new(ControlState {
            debug: session.debug_control(),
            resource,
            metadata: Arc::new(Mutex::new(session.metadata().clone())),
            project_root: None,
            sources: SourceRegistry::new(session.control_sources()),
            io_snapshot: Arc::new(Mutex::new(None)),
            pending_restart: Arc::new(Mutex::new(None)),
            auth_token: Arc::new(Mutex::new(auth_token.map(SmolStr::new))),
            control_requires_auth: matches!(endpoint, ControlEndpoint::Tcp(_)),
            control_mode: Arc::new(Mutex::new(ControlMode::Debug)),
            audit_tx: None,
            metrics: Arc::new(Mutex::new(RuntimeMetrics::default())),
            events: Arc::new(Mutex::new(VecDeque::<RuntimeEvent>::new())),
            settings: Arc::new(Mutex::new(default_settings(session))),
            resource_name: SmolStr::new("RESOURCE"),
            io_health: Arc::new(Mutex::new(Vec::<IoDriverStatus>::new())),
            debug_enabled: Arc::new(AtomicBool::new(true)),
            debug_variables: Arc::new(Mutex::new(DebugVariableHandles::new())),
            hmi_live: Arc::new(Mutex::new(trust_runtime::hmi::HmiLiveState::default())),
            pairing: None,
        });
        let server = ControlServer::start(endpoint, state.clone())?;
        let drain = spawn_command_drain(cmd_rx);
        Ok(Self {
            server,
            _drain: drain,
        })
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

fn default_settings(session: &dyn DebugRuntime) -> RuntimeSettings {
    let (watchdog, fault_policy) = session
        .runtime_handle()
        .lock()
        .map(|runtime| (runtime.watchdog_policy(), runtime.fault_policy()))
        .unwrap_or_else(|_| (WatchdogPolicy::default(), FaultPolicy::SafeHalt));
    RuntimeSettings::new(
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
        },
        DiscoverySettings {
            enabled: false,
            service_name: SmolStr::new("truST"),
            advertise: false,
            interfaces: Vec::new(),
        },
        MeshSettings {
            enabled: false,
            listen: SmolStr::new("0.0.0.0:5200"),
            auth_token: None,
            publish: Vec::new(),
            subscribe: IndexMap::new(),
        },
        SimulationSettings {
            enabled: false,
            time_scale: 1,
            mode_label: SmolStr::new("production"),
            warning: SmolStr::new(""),
        },
    )
}

fn spawn_command_drain(rx: std::sync::mpsc::Receiver<ResourceCommand>) -> thread::JoinHandle<()> {
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
                    let _ = respond_to.send(trust_runtime::debug::DebugSnapshot {
                        storage: trust_runtime::memory::VariableStorage::new(),
                        now: trust_runtime::value::Duration::ZERO,
                    });
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
