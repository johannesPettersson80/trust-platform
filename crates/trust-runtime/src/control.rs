//! Runtime control server (JSON IPC).

#![allow(missing_docs)]

mod ads_handlers;
mod audit;
mod auth;
mod breakpoint_handlers;
mod comm_handlers;
mod config_handlers;
mod connectors_handlers;
mod debug_handlers;
mod fleet_handlers;
mod handlers;
mod hmi_handlers;
mod host_ports;
mod io_handlers;
mod policy;
mod program_handlers;
mod resource_queue;
mod status_handlers;
mod transport;
mod types;
mod variable_handlers;

use std::collections::VecDeque;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64};
use std::sync::mpsc::{RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::config::ControlMode;
use crate::debug::{DebugControl, DebugVariableHandles};
use crate::discovery::DiscoveryState;
use crate::error::RuntimeError;
use crate::io::{IoDriverStatus, IoSnapshot};
use crate::linux_rt::LinuxRtRuntimeStatus;
use crate::mesh::MeshTopologyEvidence;
use crate::metrics::RuntimeMetrics;
use crate::runtime::RuntimeMetadata;
use crate::scheduler::ResourceControl;
use crate::security::pairing::PairingStore;
use crate::settings::RuntimeSettings;
use crate::value::Value;
use crate::RestartMode;
use notify::{Event, RecursiveMode, Watcher};
use smol_str::SmolStr;
use tracing::warn;

pub use self::ads_handlers::AdsDoctorJobStore;
use self::policy::required_role_for_control_request;
pub(crate) use self::types::ControlResponse;
use self::types::*;
use trust_runtime_services::dispatch::{dispatch_value, prepare_line};

// The in-crate control tests reach these through `use super::*`.
#[cfg(test)]
use std::sync::atomic::Ordering;

const HMI_DESCRIPTOR_WATCH_DEBOUNCE: Duration = Duration::from_millis(250);

fn hmi_descriptor_watch_startup_timeout() -> Duration {
    if cfg!(test) {
        Duration::from_secs(10)
    } else {
        Duration::from_secs(1)
    }
}

fn control_event_time_now() -> crate::value::Duration {
    let timestamp_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(i64::MAX as u128) as i64;
    crate::value::Duration::from_millis(timestamp_ms)
}

/// Build the communication setup schema without connecting to a running runtime.
pub fn offline_comm_schema_json(
    params: Option<&serde_json::Value>,
) -> Result<serde_json::Value, String> {
    comm_handlers::static_comm_schema_value(params)
}

/// Apply a communication setup change directly to project files.
pub fn offline_comm_apply_json(
    project_root: &Path,
    params: serde_json::Value,
) -> Result<serde_json::Value, String> {
    comm_handlers::apply_project_value(project_root, params)
}

/// Discover communication targets from the current host without a running runtime.
pub fn offline_comm_discover_json(params: serde_json::Value) -> Result<serde_json::Value, String> {
    comm_handlers::discover_value(params, None)
}

/// Browse symbols/nodes/channels from the current host without a running runtime.
pub fn offline_comm_browse_symbols_json(
    params: serde_json::Value,
) -> Result<serde_json::Value, String> {
    comm_handlers::browse_symbols_value(params, None, None)
}

/// Browse symbols/nodes/channels from a stopped project without a running runtime.
pub fn offline_comm_browse_symbols_project_json(
    project_root: &Path,
    params: serde_json::Value,
) -> Result<serde_json::Value, String> {
    comm_handlers::browse_symbols_value(params, None, Some(project_root))
}

/// Build a config-derived fleet topology without connecting to a running runtime.
pub fn offline_fleet_topology_json(project_root: &Path) -> Result<serde_json::Value, String> {
    fleet_handlers::build_project_fleet_topology_value(project_root)
}

#[derive(Debug, Clone)]
pub enum ControlEndpoint {
    Tcp(SocketAddr),
    #[cfg(unix)]
    Unix(PathBuf),
}

impl ControlEndpoint {
    pub fn parse(text: &str) -> Result<Self, RuntimeError> {
        if let Some(rest) = text.strip_prefix("tcp://") {
            let addr = rest.parse::<SocketAddr>().map_err(|err| {
                RuntimeError::ControlError(format!("invalid tcp endpoint: {err}").into())
            })?;
            if !addr.ip().is_loopback() {
                return Err(RuntimeError::ControlError(
                    "tcp endpoint must be loopback (use unix:// for local sockets)".into(),
                ));
            }
            return Ok(Self::Tcp(addr));
        }
        #[cfg(unix)]
        if let Some(rest) = text.strip_prefix("unix://") {
            if rest.is_empty() {
                return Err(RuntimeError::ControlError(
                    "unix endpoint path must not be empty".into(),
                ));
            }
            if text.trim() != text {
                return Err(RuntimeError::ControlError(
                    "unix endpoint must not contain surrounding whitespace".into(),
                ));
            }
            return Ok(Self::Unix(PathBuf::from(rest)));
        }
        Err(RuntimeError::ControlError(
            format!("unsupported endpoint '{text}'").into(),
        ))
    }
}

#[derive(Debug, Clone)]
pub struct ControlState {
    pub debug: DebugControl,
    pub resource: ResourceControl<crate::scheduler::StdClock>,
    pub metadata: Arc<Mutex<RuntimeMetadata>>,
    pub sources: SourceRegistry,
    pub io_snapshot: Arc<Mutex<Option<IoSnapshot>>>,
    pub io_snapshot_seen_ms: Arc<AtomicU64>,
    pub pending_restart: Arc<Mutex<Option<RestartMode>>>,
    pub auth_token: Arc<Mutex<Option<SmolStr>>>,
    pub control_requires_auth: bool,
    pub control_mode: Arc<Mutex<ControlMode>>,
    pub audit_tx: Option<Sender<ControlAuditEvent>>,
    pub metrics: Arc<Mutex<RuntimeMetrics>>,
    pub events: Arc<Mutex<VecDeque<crate::debug::RuntimeEvent>>>,
    pub settings: Arc<Mutex<RuntimeSettings>>,
    pub discovery: Arc<DiscoveryState>,
    pub mesh_topology: Arc<Mutex<Option<MeshTopologyEvidence>>>,
    pub realtime_status: Arc<Mutex<LinuxRtRuntimeStatus>>,
    pub web_listener_bound: Arc<AtomicBool>,
    pub opcua_server_bound: Arc<AtomicBool>,
    pub project_root: Option<PathBuf>,
    pub resource_name: SmolStr,
    pub io_health: Arc<Mutex<Vec<IoDriverStatus>>>,
    pub debug_enabled: Arc<AtomicBool>,
    pub debug_variables: Arc<Mutex<DebugVariableHandles>>,
    pub hmi_live: Arc<Mutex<crate::hmi::HmiLiveState>>,
    pub hmi_persistence: Option<Arc<crate::hmi::HmiPersistenceService>>,
    pub hmi_descriptor: Arc<Mutex<HmiRuntimeDescriptor>>,
    pub historian: Option<Arc<crate::historian::HistorianService>>,
    pub openot_persistence_status:
        Option<Arc<Mutex<crate::openot_persistence::OpenOtPersistenceStatus>>>,
    pub pairing: Option<Arc<PairingStore>>,
    pub ads_doctor_jobs: Arc<Mutex<AdsDoctorJobStore>>,
    /// The searches that `comm.search.*` started in this runtime.
    pub search: Arc<crate::search::SearchRegistry>,
    pub ads_client_config: Arc<Mutex<Option<crate::ads::AdsClientConfig>>>,
    pub opcua_client_config: Arc<Mutex<Option<crate::opcua::OpcUaClientConfig>>>,
    pub ads_server_config: Arc<Mutex<Option<crate::ads::server::AdsServerRuntimeConfig>>>,
    #[cfg(feature = "ads-server")]
    pub ads_server_runtime: Arc<Mutex<Option<crate::ads::server::AdsServerRuntime>>>,
}

#[derive(Debug, Clone)]
pub struct ControlAuditEvent {
    pub event_id: SmolStr,
    pub timestamp_ms: u128,
    pub request_id: u64,
    pub request_type: SmolStr,
    pub correlation_id: Option<SmolStr>,
    pub ok: bool,
    pub error: Option<SmolStr>,
    pub auth_present: bool,
    pub client: Option<SmolStr>,
    pub details: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Default)]
pub struct SourceRegistry {
    files: Vec<SourceFile>,
}

#[derive(Debug, Clone)]
pub struct SourceFile {
    pub id: u32,
    pub path: PathBuf,
    pub text: String,
}

impl SourceRegistry {
    pub fn new(files: Vec<SourceFile>) -> Self {
        Self { files }
    }

    pub fn files(&self) -> &[SourceFile] {
        &self.files
    }

    pub fn file_id_for_path(&self, path: &Path) -> Option<u32> {
        self.file_for_path(path, None).map(|file| file.id)
    }

    pub fn file_id_for_path_in_project(
        &self,
        path: &Path,
        project_root: Option<&Path>,
    ) -> Option<u32> {
        self.file_for_path(path, project_root).map(|file| file.id)
    }

    pub fn source_text(&self, file_id: u32) -> Option<&str> {
        self.files
            .iter()
            .find(|file| file.id == file_id)
            .map(|file| file.text.as_str())
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    fn file_for_path(&self, path: &Path, project_root: Option<&Path>) -> Option<&SourceFile> {
        if let Some(file) = self.files.iter().find(|file| file.path == path) {
            return Some(file);
        }

        let mut candidates = Vec::new();
        candidates.push(path.to_path_buf());
        if let Some(root) = project_root
            && path.is_relative()
        {
            candidates.push(root.join(path));
            if path.components().count() == 1 {
                candidates.push(root.join("src").join(path));
            }
        }

        for candidate in &candidates {
            if let Some(file) = self.files.iter().find(|file| file.path == *candidate) {
                return Some(file);
            }
            if let Ok(canonical) = std::fs::canonicalize(candidate)
                && let Some(file) = self
                    .files
                    .iter()
                    .find(|file| canonical_path_eq(&file.path, &canonical))
            {
                return Some(file);
            }
        }

        if path.is_relative() {
            return self.unique_suffix_match(path);
        }

        None
    }

    fn unique_suffix_match(&self, suffix: &Path) -> Option<&SourceFile> {
        let mut matches = self.files.iter().filter(|file| file.path.ends_with(suffix));
        let first = matches.next()?;
        if matches.next().is_some() {
            return None;
        }
        Some(first)
    }
}

fn canonical_path_eq(left: &Path, right: &Path) -> bool {
    std::fs::canonicalize(left).is_ok_and(|canonical| canonical == right)
}

#[derive(Debug, Clone)]
pub struct HmiRuntimeDescriptor {
    pub customization: crate::hmi::HmiCustomization,
    pub schema_revision: u64,
    pub last_error: Option<String>,
}

impl HmiRuntimeDescriptor {
    #[must_use]
    pub fn from_sources(project_root: Option<&Path>, sources: &SourceRegistry) -> Self {
        Self {
            customization: hmi_handlers::load_hmi_customization_from_sources(project_root, sources),
            schema_revision: 0,
            last_error: None,
        }
    }
}

#[derive(Debug)]
pub struct ControlServer {
    endpoint: ControlEndpoint,
    state: Arc<ControlState>,
}

impl ControlServer {
    pub fn start(
        endpoint: ControlEndpoint,
        state: Arc<ControlState>,
    ) -> Result<Self, RuntimeError> {
        transport::spawn_control_server(&endpoint, state.clone())?;
        Ok(Self { endpoint, state })
    }

    #[must_use]
    pub fn endpoint(&self) -> &ControlEndpoint {
        &self.endpoint
    }

    #[must_use]
    pub fn state(&self) -> Arc<ControlState> {
        self.state.clone()
    }
}

pub fn spawn_hmi_descriptor_watcher(state: Arc<ControlState>) {
    let Some(project_root) = state.project_root.clone() else {
        return;
    };
    let project_root_for_thread = project_root.clone();
    let state_for_thread = state.clone();
    let (ready_tx, ready_rx) = std::sync::mpsc::channel::<Result<(), String>>();
    std::thread::spawn(move || {
        let (tx, rx) = std::sync::mpsc::channel::<notify::Result<Event>>();
        let mut watcher = match notify::recommended_watcher(move |result| {
            if tx.send(result).is_err() {
                warn!("hmi watcher event channel closed");
            }
        }) {
            Ok(watcher) => watcher,
            Err(err) => {
                if ready_tx.send(Err(err.to_string())).is_err() {
                    warn!("hmi watcher ready channel closed after init failure");
                }
                warn!("hmi watcher init failed: {err}");
                return;
            }
        };

        if let Err(err) = watcher.watch(project_root_for_thread.as_path(), RecursiveMode::Recursive)
        {
            if ready_tx.send(Err(err.to_string())).is_err() {
                warn!("hmi watcher ready channel closed after watch failure");
            }
            warn!(
                "hmi watcher failed to watch '{}': {err}",
                project_root_for_thread.display()
            );
            return;
        }
        if ready_tx.send(Ok(())).is_err() {
            warn!("hmi watcher ready channel closed after startup");
        }

        loop {
            let mut should_reload = match rx.recv() {
                Ok(Ok(event)) => hmi_handlers::hmi_event_matches_descriptor(
                    &event,
                    project_root_for_thread.as_path(),
                ),
                Ok(Err(err)) => {
                    warn!("hmi watcher event error: {err}");
                    false
                }
                Err(_) => break,
            };
            if !should_reload {
                continue;
            }

            let mut deadline = Instant::now() + HMI_DESCRIPTOR_WATCH_DEBOUNCE;
            loop {
                let now = Instant::now();
                let Some(timeout) = deadline.checked_duration_since(now) else {
                    break;
                };
                match rx.recv_timeout(timeout) {
                    Ok(Ok(event)) => {
                        if hmi_handlers::hmi_event_matches_descriptor(
                            &event,
                            project_root_for_thread.as_path(),
                        ) {
                            should_reload = true;
                            deadline = Instant::now() + HMI_DESCRIPTOR_WATCH_DEBOUNCE;
                        }
                    }
                    Ok(Err(err)) => {
                        warn!("hmi watcher event error: {err}");
                    }
                    Err(RecvTimeoutError::Timeout) => break,
                    Err(RecvTimeoutError::Disconnected) => return,
                }
            }

            if !should_reload {
                continue;
            }

            if let Err(err) = hmi_handlers::reload_hmi_descriptor_state(&state_for_thread) {
                warn!("hmi descriptor reload failed: {err}");
            }
        }
    });
    match ready_rx.recv_timeout(hmi_descriptor_watch_startup_timeout()) {
        Ok(Ok(())) => {}
        Ok(Err(err)) => warn!("hmi watcher startup failed: {err}"),
        Err(RecvTimeoutError::Timeout) => {
            warn!(
                "hmi watcher startup timed out for '{}'",
                project_root.display()
            );
        }
        Err(RecvTimeoutError::Disconnected) => {
            warn!(
                "hmi watcher startup channel disconnected for '{}'",
                project_root.display()
            );
        }
    }
}

pub(crate) fn hmi_asset_project_root_port(state: &ControlState) -> Option<PathBuf> {
    state.project_root.clone()
}

pub(crate) fn runtime_resource_name_port(state: &ControlState) -> SmolStr {
    state.resource_name.clone()
}

pub(crate) fn control_request_required_role_port(
    kind: &str,
    params: Option<&serde_json::Value>,
) -> crate::security::AccessRole {
    required_role_for_control_request(kind, params)
}

pub(crate) fn communication_probe_value_port(
    protocol: &str,
    params: serde_json::Value,
) -> serde_json::Value {
    comm_handlers::comm_test_probe_value(protocol, params)
}

/// Answers one `comm.discover` request for a source of the one search (`crate::search`), reading
/// the project at `project_root` as the runtime that runs the search does. The sources wrap these
/// per-protocol searches until they move into the search engine (V7 slice C1b).
pub(crate) fn comm_discover_port(
    params: serde_json::Value,
    project_root: Option<&Path>,
) -> Result<serde_json::Value, String> {
    comm_handlers::discover_for_search(params, project_root)
}

/// The hosts that a `comm.discover` CIDR scan probes, by its rules; an error for a malformed CIDR
/// or one broader than `/24`.
pub(crate) fn comm_discover_cidr_hosts_port(cidr: &str) -> Result<Vec<std::net::Ipv4Addr>, String> {
    comm_handlers::discover_cidr_hosts(cidr)
}

pub(crate) fn dispatch_web_control_request_port(
    mut payload: serde_json::Value,
    state: &ControlState,
    client: Option<&str>,
    request_token: Option<&str>,
) -> ControlResponse {
    if payload.get("auth").is_none()
        && let Some(token) = request_token
    {
        payload["auth"] = serde_json::Value::String(token.to_string());
    }
    prepare_request_value(payload, state, client)
}

pub(crate) fn complete_web_control_response_port(
    response: &mut ControlResponse,
    state: &ControlState,
) {
    let after_response = response.after_response.take();
    apply_after_response(after_response, state);
}

pub(crate) fn write_then_complete_web_control_responses_port<R>(
    responses: &mut [ControlResponse],
    state: &ControlState,
    write: impl FnOnce(&[ControlResponse]) -> R,
) -> R {
    let result = write(responses);
    for response in responses {
        complete_web_control_response_port(response, state);
    }
    result
}

#[cfg(test)]
pub(crate) fn handle_request_line(
    line: &str,
    state: &ControlState,
    client: Option<&str>,
) -> Option<String> {
    let prepared = prepare_request_line(line, state, client)?;
    let response = prepared.response.clone();
    prepared.complete(state);
    Some(response)
}

pub(crate) struct PreparedControlLine {
    response: String,
    after_response: Option<ControlAfterResponse>,
}

impl PreparedControlLine {
    pub(crate) fn response(&self) -> &str {
        self.response.as_str()
    }

    pub(crate) fn complete(self, state: &ControlState) {
        apply_after_response(self.after_response, state);
    }
}

pub(crate) fn prepare_request_line(
    line: &str,
    state: &ControlState,
    client: Option<&str>,
) -> Option<PreparedControlLine> {
    // The request pipeline is shared runtime code (spec 34 ARCH-11); `ControlState` supplies its
    // ports (`control/host_ports.rs`).
    let prepared = prepare_line(line, state, client);
    Some(PreparedControlLine {
        response: prepared.response,
        after_response: prepared.after_response,
    })
}

#[cfg(test)]
pub(crate) fn handle_request_value(
    value: serde_json::Value,
    state: &ControlState,
    client: Option<&str>,
) -> ControlResponse {
    let mut response = prepare_request_value(value, state, client);
    let after_response = response.after_response.take();
    apply_after_response(after_response, state);
    response
}

fn prepare_request_value(
    value: serde_json::Value,
    state: &ControlState,
    client: Option<&str>,
) -> ControlResponse {
    dispatch_value(value, state, client)
}

fn apply_after_response(action: Option<ControlAfterResponse>, state: &ControlState) {
    if matches!(action, Some(ControlAfterResponse::StopResource)) {
        state.resource.stop();
    }
}

fn parse_value(text: &str) -> Result<Value, RuntimeError> {
    let upper = text.trim().to_ascii_uppercase();
    if upper == "TRUE" {
        return Ok(Value::Bool(true));
    }
    if upper == "FALSE" {
        return Ok(Value::Bool(false));
    }
    if let Ok(int_val) = upper.parse::<i64>() {
        return Ok(Value::LInt(int_val));
    }
    Err(RuntimeError::ControlError(
        format!("unsupported value '{text}'").into(),
    ))
}

#[cfg(test)]
mod tests;
