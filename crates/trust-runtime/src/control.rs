//! Runtime control server (JSON IPC).

#![allow(missing_docs)]

mod ads_handlers;
mod audit;
mod auth;
mod comm_handlers;
mod config_handlers;
mod connectors_handlers;
mod controller_handlers;
pub mod endpoint;
mod experiment_handlers;
pub(crate) mod fleet_handlers;
mod handlers;
mod hmi_handlers;
pub(crate) mod hmi_ports;
mod host_ports;
mod parameter_handlers;
mod plant_handlers;
mod plant_service;
mod policy;
mod program_handlers;
mod recording_handlers;
mod relay_state;
mod resource_queue;
pub mod search;
mod signal_handlers;
mod status_handlers;
mod transfer_handlers;
mod transport;
mod types;
mod variable_handlers;

use std::collections::VecDeque;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, PoisonError, RwLock};

use crate::config::ControlMode;
use crate::debug::DebugControl;
use crate::discovery::DiscoveryState;
use crate::error::RuntimeError;
use crate::io::{IdentifyHandle, IoDriverStatus, IoSnapshot};
use crate::linux_rt::LinuxRtRuntimeStatus;
use crate::mesh::MeshTopologyEvidence;
use crate::metrics::RuntimeMetrics;
use crate::scheduler::ResourceControl;
use crate::security::pairing::PairingStore;
use crate::settings::RuntimeSettings;
use crate::value::Value;
use crate::RestartMode;
use serde::Deserialize;
use serde_json::json;
use smol_str::SmolStr;

pub use self::ads_handlers::AdsDoctorJobStore;
pub use self::controller_handlers::ControllerParameterService;
pub use self::experiment_handlers::ControlExperimentService;
pub use self::plant_service::PlantControlService;
use self::policy::required_role_for_control_request;
pub(crate) use self::types::ControlResponse;
use self::types::*;
use trust_runtime_services::dispatch::{dispatch_value, invalid_line};
use trust_runtime_services::live_values::value_grammar;
use trust_runtime_services::transfer::TransferSession;

// The in-crate control tests reach these through `use super::*`.
#[cfg(test)]
use std::sync::atomic::Ordering;

fn control_event_time_now() -> crate::value::Duration {
    let timestamp_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(i64::MAX as u128) as i64;
    crate::value::Duration::from_millis(timestamp_ms)
}

/// One stopped-project plant command producer with a session-scoped replay cache.
#[derive(Debug)]
pub struct OfflinePlantSession {
    project_root: PathBuf,
    plant_service: PlantControlService,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct OfflinePlantRequest {
    id: u64,
    #[serde(rename = "type")]
    operation: String,
    params: Option<serde_json::Value>,
}

impl OfflinePlantSession {
    pub fn new(project_root: PathBuf) -> Self {
        Self {
            project_root,
            plant_service: PlantControlService::default(),
        }
    }

    pub fn request_json(&self, request: serde_json::Value) -> serde_json::Value {
        let request = match serde_json::from_value::<OfflinePlantRequest>(request) {
            Ok(request) => request,
            Err(error) => {
                return serde_json::to_value(ControlResponse::error_with_code(
                    0,
                    format!("invalid offline plant request: {error}"),
                    "malformed_request",
                ))
                .expect("control response is serializable");
            }
        };
        let context = plant_handlers::PlantHandlerContext::offline(
            self.project_root.as_path(),
            &self.plant_service,
        );
        let mut response = plant_handlers::dispatch(
            request.id,
            request.operation.as_str(),
            request.params,
            &context,
        )
        .unwrap_or_else(|| {
            ControlResponse::error_with_code(
                request.id,
                "unsupported plant operation".to_string(),
                "unsupported_operation",
            )
        });
        if !response.ok && response.result.is_none() {
            response.result = plant_handlers::error_envelope(
                request.operation.as_str(),
                Some(self.project_root.as_path()),
            );
        }
        serde_json::to_value(response).expect("control response is serializable")
    }
}

/// The largest reply a computer holds and the largest upload it takes: 64 MiB, the largest program
/// container a computer admits (specification 34 §5.1, PORT-05).
const MAX_TRANSFER_BYTES: usize = 64 * 1024 * 1024;

/// The transfers of this process (specification 34, PORT-05). Its boot number is the process start
/// in seconds since the epoch, so a transfer id does not repeat after a restart.
#[must_use]
pub fn new_transfer_session() -> Arc<Mutex<TransferSession>> {
    let boot = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| {
            u32::try_from(elapsed.as_secs()).unwrap_or(u32::MAX)
        });
    Arc::new(Mutex::new(TransferSession::new(
        boot,
        MAX_TRANSFER_BYTES,
        MAX_TRANSFER_BYTES,
    )))
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

/// Enable the ADS client in a project's `runtime.toml` after an ADS tag import.
///
/// Returns whether `runtime.toml` changed. A missing `runtime.toml` is refused with
/// the message the VS Code port used (spec 25 §5.8; design §4.3 B7).
pub fn offline_ads_ensure_runtime_enabled(
    project_root: &Path,
    config_path: &str,
) -> Result<bool, String> {
    comm_handlers::ensure_ads_runtime_enabled(project_root, config_path)
}

/// Remove imported ADS tag points from a project's `ads.toml` and regenerate its ST.
///
/// The same function backs the `ads.import_symbols.remove` control request and
/// `trust-runtime ads remove-symbols` (design §4.3 B7).
pub fn offline_ads_remove_symbols_json(
    project_root: &Path,
    params: serde_json::Value,
) -> Result<serde_json::Value, String> {
    ads_handlers::remove_symbols_from_project(project_root, params)
}

/// Import ADS tag points into a project's files offline, as the `ads.import_symbols.apply` control
/// request and `trust-runtime ads import-symbols` do (design §4.3 B7; specification 25 §5.4).
pub fn offline_ads_import_symbols_json(
    project_root: &Path,
    params: serde_json::Value,
) -> Result<serde_json::Value, String> {
    ads_handlers::import_symbols_from_params(project_root, params)
}

/// Run a communication `comm.test` request from this host without a running runtime.
///
/// `project_root` is accepted so the command has the same shape as the runtime request;
/// `comm.test` probes its explicit parameters and reads no project file.
pub fn offline_comm_test_json(
    _project_root: &Path,
    protocol: &str,
    params: serde_json::Value,
) -> serde_json::Value {
    comm_handlers::comm_test_probe_value(protocol, params)
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
    /// The identify state of the runtime (specification 11, "Identify"), shared with the runtime
    /// whose scan and waits drive its output. Empty while the project has no
    /// `[runtime.identify] output`.
    pub identify: IdentifyHandle,
    pub resource: ResourceControl<crate::scheduler::StdClock>,
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
    pub plant_service: Arc<PlantControlService>,
    pub experiments: ControlExperimentService,
    pub controllers: ControllerParameterService,
    pub searches: search::SearchService,
    pub resource_name: SmolStr,
    pub runtime_id: SmolStr,
    pub io_health: Arc<Mutex<Vec<IoDriverStatus>>>,
    pub debug_enabled: Arc<AtomicBool>,
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
    /// The transfers of this process (specification 34, PORT-05); see [`new_transfer_session`].
    pub transfers: Arc<Mutex<TransferSession>>,
}

impl ControlState {
    /// Connects the experiments to this state's debug control, so that every published scan advances the active experiment
    /// (Specification 35, A3.5). The debug handle must be the runtime's own.
    pub fn connect_experiments(&self) {
        self.debug
            .set_scan_observer(self.experiments.scan_observer());
    }

    /// The image of the program the resource runs, whatever the debugging switch: the type and
    /// symbol tables a web route checks a request against (specification 44 §8.8). None before a
    /// program is set.
    pub(crate) fn program_image(
        &self,
    ) -> Option<Arc<trust_runtime_core::native::program::AdmittedImage>> {
        self.debug.program_image()
    }

    /// The source files of the program the resource runs, as a reload last replaced them
    /// (specification 39, §16.4).
    pub(crate) fn source_files(&self) -> Arc<Vec<SourceFile>> {
        self.sources.files()
    }
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

/// The sources of the program that runs, which the debug, breakpoint, isolated-run and search handlers read. A program reload replaces
/// them in place ([`SourceRegistry::replace`]): every clone of the registry shares them, so a handler always reads the sources of the
/// program that runs now, never those of the program the state was built with.
#[derive(Debug, Clone, Default)]
pub struct SourceRegistry {
    files: Arc<RwLock<Arc<Vec<SourceFile>>>>,
}

#[derive(Debug, Clone)]
pub struct SourceFile {
    pub id: u32,
    pub path: PathBuf,
    pub text: String,
}

impl SourceRegistry {
    pub fn new(files: Vec<SourceFile>) -> Self {
        Self {
            files: Arc::new(RwLock::new(Arc::new(files))),
        }
    }

    /// Replaces the sources with those of the program a reload loaded.
    pub fn replace(&self, files: Vec<SourceFile>) {
        *self.files.write().unwrap_or_else(PoisonError::into_inner) = Arc::new(files);
    }

    /// The sources as they are now.
    pub fn files(&self) -> Arc<Vec<SourceFile>> {
        Arc::clone(&self.files.read().unwrap_or_else(PoisonError::into_inner))
    }

    pub fn file_id_for_path(&self, path: &Path) -> Option<u32> {
        file_for_path(&self.files(), path, None).map(|file| file.id)
    }

    pub fn file_id_for_path_in_project(
        &self,
        path: &Path,
        project_root: Option<&Path>,
    ) -> Option<u32> {
        file_for_path(&self.files(), path, project_root).map(|file| file.id)
    }

    pub fn source_text(&self, file_id: u32) -> Option<String> {
        self.files()
            .iter()
            .find(|file| file.id == file_id)
            .map(|file| file.text.clone())
    }

    pub fn is_empty(&self) -> bool {
        self.files().is_empty()
    }
}

fn file_for_path<'a>(
    files: &'a [SourceFile],
    path: &Path,
    project_root: Option<&Path>,
) -> Option<&'a SourceFile> {
    if let Some(file) = files.iter().find(|file| file.path == path) {
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
        if let Some(file) = files.iter().find(|file| file.path == *candidate) {
            return Some(file);
        }
        if let Ok(canonical) = std::fs::canonicalize(candidate)
            && let Some(file) = files
                .iter()
                .find(|file| canonical_path_eq(&file.path, &canonical))
        {
            return Some(file);
        }
    }

    if path.is_relative() {
        return unique_suffix_match(files, path);
    }

    None
}

fn unique_suffix_match<'a>(files: &'a [SourceFile], suffix: &Path) -> Option<&'a SourceFile> {
    let mut matches = files.iter().filter(|file| file.path.ends_with(suffix));
    let first = matches.next()?;
    if matches.next().is_some() {
        return None;
    }
    Some(first)
}

fn canonical_path_eq(left: &Path, right: &Path) -> bool {
    std::fs::canonicalize(left).is_ok_and(|canonical| canonical == right)
}

#[derive(Debug)]
pub struct ControlServer {
    endpoint: ControlEndpoint,
    state: Arc<ControlState>,
}

impl ControlServer {
    /// Binds `endpoint` and starts serving it. The server then reports the endpoint that is
    /// bound, so a TCP port of 0 is replaced by the port the operating system chose.
    pub fn start(
        endpoint: ControlEndpoint,
        state: Arc<ControlState>,
    ) -> Result<Self, RuntimeError> {
        let bound = transport::spawn_control_server(&endpoint, state.clone())?;
        Ok(Self {
            endpoint: bound,
            state,
        })
    }

    /// The endpoint that the server listens on, with the port that was bound.
    #[must_use]
    pub fn endpoint(&self) -> &ControlEndpoint {
        &self.endpoint
    }

    #[must_use]
    pub fn state(&self) -> Arc<ControlState> {
        self.state.clone()
    }
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
    // ports (`control/host_ports.rs`). The host reads the line as a JSON value, as the shared
    // pipeline reads a line that it cannot read straight into a request, and answers it as it
    // answers a request of the web, so that it completes the reply (`HostCompletion`). A line that
    // is not JSON gets the shared reply: `invalid request` with id 0 and no audit record.
    let value = match serde_json::from_str::<serde_json::Value>(line) {
        Ok(value) => value,
        Err(error) => {
            let invalid = invalid_line(&error.to_string());
            return Some(PreparedControlLine {
                response: invalid.response,
                after_response: invalid.after_response,
            });
        }
    };
    let mut response = prepare_request_value(value, state, client);
    let after_response = response.after_response.take();
    // A response that cannot be serialized is answered as the shared pipeline answers it.
    let response = serde_json::to_string(&response).unwrap_or_else(|error| {
        json!({
            "id": 0_u64,
            "ok": false,
            "error": format!("response serialization failed: {error}"),
        })
        .to_string()
    });
    Some(PreparedControlLine {
        response,
        after_response,
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
    // The operation is read before the shared pipeline takes the request, so that the host can
    // complete the reply (`HostCompletion`).
    let completion = HostCompletion::of(&value);
    let mut response = dispatch_value(value, state, client);
    if let Some(completion) = completion {
        completion.apply(&mut response, state);
    }
    response
}

// The shared pipeline answers `status` and refuses a request before the host's handlers see it, so
// the host completes those replies after the pipeline, for the control connection and the web alike
// (`prepare_request_line`, `prepare_request_value`): the runtime facts that Control Studio reads, and
// the plant contract's error envelope. A `status` whose facts cannot be read (a poisoned lock) is
// answered `<value> unavailable` under the audit record of the successful shared `status`. Moving
// the facts into the shared reply through a `StatusPort` method, so that a chip reports them too, is
// on the chips' parity list (specification 34; the runtime portability checklist).
/// What the host adds to the reply of an operation that only it completes, read from the request
/// before the shared pipeline answers it.
enum HostCompletion {
    /// `status` carries the runtime facts that Control Studio reads
    /// (control-studio-runtime-protocol E1.3).
    Status,
    /// A failed `plant.*` reply carries the plant contract's error envelope, a refusal of the
    /// shared pipeline included (specification 47 §20.1).
    Plant(String),
}

impl HostCompletion {
    /// The completion of the request `value`: none for any other operation, nor for a plant request
    /// that is not a request at all, which is answered `invalid request` as any other.
    fn of(value: &serde_json::Value) -> Option<Self> {
        let kind = value.get("type")?.as_str()?;
        if kind == "status" {
            return Some(Self::Status);
        }
        (kind.starts_with("plant.") && ControlRequest::deserialize(value).is_ok())
            .then(|| Self::Plant(kind.to_string()))
    }

    fn apply(self, response: &mut ControlResponse, state: &ControlState) {
        match self {
            Self::Status => status_handlers::attach_runtime_facts(response, state),
            Self::Plant(operation) => attach_plant_error_contract(response, &operation, state),
        }
    }
}

/// A failed `plant.*` reply without a result carries the plant contract's error envelope: the
/// contract, its schema version, the operation and the project's identity (specification 47
/// §20.1).
fn attach_plant_error_contract(
    response: &mut ControlResponse,
    operation: &str,
    state: &ControlState,
) {
    if response.ok || response.result.is_some() {
        return;
    }
    response.result = plant_handlers::error_envelope(operation, state.project_root.as_deref());
}

/// Runs the work a reply left for after it was written. `shutdown` stops the program and leaves the
/// runtime running in the Stopped state, and the explicit exit also ends the process (specification
/// 34, STATE-08).
fn apply_after_response(action: Option<ControlAfterResponse>, state: &ControlState) {
    match action {
        Some(ControlAfterResponse::StopResource) => state.resource.stop_program(),
        Some(ControlAfterResponse::ExitRuntime) => state.resource.stop(),
        None => {}
    }
}

/// An experiment value given as text: a boolean or a signed 64-bit integer, read by the Live
/// Values grammar that every control value shares (specification 11, "Live Values"; specification
/// 34 §4.3.1).
fn parse_value(text: &str) -> Result<Value, RuntimeError> {
    value_grammar::parse_value(text).map_err(|error| RuntimeError::ControlError(error.into()))
}

#[cfg(test)]
mod tests;
