//! Runtime control server (JSON IPC).

#![allow(missing_docs)]

mod handlers;
mod transport;

use std::collections::{BTreeMap, VecDeque};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::config::ControlMode;
use crate::debug::{
    location_to_line_col, DebugBreakpoint, DebugControl, DebugScope, DebugSource, DebugVariable,
    DebugVariableHandles, VariableHandle,
};
use crate::error::RuntimeError;
use crate::io::{IoAddress, IoDriverHealth, IoDriverStatus, IoSnapshot};
use crate::metrics::RuntimeMetrics;
use crate::runtime::RuntimeMetadata;
use crate::scheduler::{ResourceCommand, ResourceControl};
use crate::security::AccessRole;
use crate::settings::RuntimeSettings;
use crate::value::Value;
use crate::web::pairing::PairingStore;
use crate::RestartMode;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use notify::{Event, EventKind, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use serde_json::json;
use smol_str::SmolStr;
use tracing::{debug, warn};

const HMI_DESCRIPTOR_WATCH_DEBOUNCE: Duration = Duration::from_millis(250);

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
    pub pending_restart: Arc<Mutex<Option<RestartMode>>>,
    pub auth_token: Arc<Mutex<Option<SmolStr>>>,
    pub control_requires_auth: bool,
    pub control_mode: Arc<Mutex<ControlMode>>,
    pub audit_tx: Option<Sender<ControlAuditEvent>>,
    pub metrics: Arc<Mutex<RuntimeMetrics>>,
    pub events: Arc<Mutex<VecDeque<crate::debug::RuntimeEvent>>>,
    pub settings: Arc<Mutex<RuntimeSettings>>,
    pub project_root: Option<PathBuf>,
    pub resource_name: SmolStr,
    pub io_health: Arc<Mutex<Vec<IoDriverStatus>>>,
    pub debug_enabled: Arc<AtomicBool>,
    pub debug_variables: Arc<Mutex<DebugVariableHandles>>,
    pub hmi_live: Arc<Mutex<crate::hmi::HmiLiveState>>,
    pub hmi_descriptor: Arc<Mutex<HmiRuntimeDescriptor>>,
    pub historian: Option<Arc<crate::historian::HistorianService>>,
    pub pairing: Option<Arc<PairingStore>>,
}

#[derive(Debug, Clone)]
pub struct ControlAuditEvent {
    pub timestamp_ms: u128,
    pub request_id: u64,
    pub request_type: SmolStr,
    pub ok: bool,
    pub error: Option<SmolStr>,
    pub auth_present: bool,
    pub client: Option<SmolStr>,
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
        self.files
            .iter()
            .find(|file| file.path == path)
            .map(|file| file.id)
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
            customization: load_hmi_customization_from_sources(project_root, sources),
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
            let _ = tx.send(result);
        }) {
            Ok(watcher) => watcher,
            Err(err) => {
                let _ = ready_tx.send(Err(err.to_string()));
                warn!("hmi watcher init failed: {err}");
                return;
            }
        };

        if let Err(err) = watcher.watch(project_root_for_thread.as_path(), RecursiveMode::Recursive)
        {
            let _ = ready_tx.send(Err(err.to_string()));
            warn!(
                "hmi watcher failed to watch '{}': {err}",
                project_root_for_thread.display()
            );
            return;
        }
        let _ = ready_tx.send(Ok(()));

        loop {
            let mut should_reload = match rx.recv() {
                Ok(Ok(event)) => {
                    hmi_event_matches_descriptor(&event, project_root_for_thread.as_path())
                }
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
                        if hmi_event_matches_descriptor(&event, project_root_for_thread.as_path()) {
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

            if let Err(err) = reload_hmi_descriptor_state(&state_for_thread) {
                warn!("hmi descriptor reload failed: {err}");
            }
        }
    });
    match ready_rx.recv_timeout(Duration::from_secs(1)) {
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

pub(crate) fn handle_request_line(
    line: &str,
    state: &ControlState,
    client: Option<&str>,
) -> Option<String> {
    let response = match serde_json::from_str::<serde_json::Value>(line) {
        Ok(value) => handle_request_value(value, state, client),
        Err(err) => ControlResponse::error(0, format!("invalid request: {err}")),
    };
    serde_json::to_string(&response).ok()
}

pub(crate) fn handle_request_value(
    value: serde_json::Value,
    state: &ControlState,
    client: Option<&str>,
) -> ControlResponse {
    let request: ControlRequest = match serde_json::from_value(value) {
        Ok(req) => req,
        Err(err) => {
            let response = ControlResponse::error(0, format!("invalid request: {err}"));
            record_audit(
                state,
                0,
                SmolStr::new("invalid"),
                false,
                Some(SmolStr::new(format!("invalid request: {err}"))),
                false,
                client,
            );
            return response;
        }
    };
    let request_role = match resolve_request_role(&request, state) {
        Ok(role) => role,
        Err(error) => {
            let response = ControlResponse::error(request.id, error.to_string());
            record_audit(
                state,
                request.id,
                SmolStr::new(request.r#type.as_str()),
                false,
                Some(SmolStr::new(error)),
                request.auth.is_some(),
                client,
            );
            return response;
        }
    };
    let required_role =
        required_role_for_control_request(request.r#type.as_str(), request.params.as_ref());
    if !request_role.allows(required_role) {
        let error = format!("forbidden: requires role {}", required_role.as_str());
        let response = ControlResponse::error(request.id, error.clone());
        record_audit(
            state,
            request.id,
            SmolStr::new(request.r#type.as_str()),
            false,
            Some(SmolStr::new(error)),
            request.auth.is_some(),
            client,
        );
        return response;
    }
    if !state.debug_enabled.load(Ordering::Relaxed) && is_debug_request(request.r#type.as_str()) {
        let response = ControlResponse::error(request.id, "debug disabled".into());
        record_audit(
            state,
            request.id,
            SmolStr::new(request.r#type.as_str()),
            false,
            Some(SmolStr::new("debug disabled")),
            request.auth.is_some(),
            client,
        );
        return response;
    }
    let response = handlers::dispatch(&request, state)
        .unwrap_or_else(|| ControlResponse::error(request.id, "unsupported request".into()));
    record_audit(
        state,
        request.id,
        SmolStr::new(request.r#type.as_str()),
        response.ok,
        response.error.as_ref().map(SmolStr::new),
        request.auth.is_some(),
        client,
    );
    response
}

fn record_audit(
    state: &ControlState,
    request_id: u64,
    request_type: SmolStr,
    ok: bool,
    error: Option<SmolStr>,
    auth_present: bool,
    client: Option<&str>,
) {
    let Some(sender) = &state.audit_tx else {
        return;
    };
    let timestamp_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let event = ControlAuditEvent {
        timestamp_ms,
        request_id,
        request_type,
        ok,
        error,
        auth_present,
        client: client.map(SmolStr::new),
    };
    let _ = sender.send(event);
}

fn is_debug_request(kind: &str) -> bool {
    matches!(
        kind,
        "pause"
            | "resume"
            | "step_in"
            | "step_over"
            | "step_out"
            | "breakpoints.set"
            | "breakpoints.clear"
            | "breakpoints.clear_all"
            | "breakpoints.clear_id"
            | "breakpoints.list"
            | "eval"
            | "set"
            | "var.force"
            | "var.unforce"
            | "var.forced"
            | "debug.state"
            | "debug.stops"
            | "debug.stack"
            | "debug.scopes"
            | "debug.variables"
            | "debug.evaluate"
            | "debug.breakpoint_locations"
    )
}

fn resolve_request_role(
    request: &ControlRequest,
    state: &ControlState,
) -> Result<AccessRole, &'static str> {
    let provided = request.auth.as_deref();
    let expected = state.auth_token.lock().ok().and_then(|guard| guard.clone());
    if let Some(expected) = expected {
        if provided == Some(expected.as_str()) {
            return Ok(AccessRole::Admin);
        }
        if let Some(token) = provided {
            if let Some(store) = state.pairing.as_ref() {
                if let Some(role) = store.validate_with_role(token) {
                    return Ok(role);
                }
            }
        }
        return Err("unauthorized");
    }
    if let Some(token) = provided {
        if let Some(store) = state.pairing.as_ref() {
            if let Some(role) = store.validate_with_role(token) {
                return Ok(role);
            }
        }
    }
    Ok(AccessRole::Admin)
}

fn required_role_for_control_request(kind: &str, params: Option<&serde_json::Value>) -> AccessRole {
    match kind {
        "status"
        | "health"
        | "tasks.stats"
        | "events.tail"
        | "events"
        | "faults"
        | "config.get"
        | "io.list"
        | "io.read"
        | "hmi.schema.get"
        | "hmi.values.get"
        | "hmi.trends.get"
        | "hmi.alarms.get"
        | "hmi.descriptor.get"
        | "historian.query"
        | "historian.alerts"
        | "debug.state"
        | "debug.stops"
        | "debug.stack"
        | "debug.scopes"
        | "debug.variables"
        | "debug.breakpoint_locations"
        | "breakpoints.list"
        | "var.forced" => AccessRole::Viewer,
        "pause" | "resume" | "restart" | "hmi.alarm.ack" | "pair.claim" => AccessRole::Operator,
        "step_in"
        | "step_over"
        | "step_out"
        | "breakpoints.set"
        | "breakpoints.clear"
        | "breakpoints.clear_all"
        | "breakpoints.clear_id"
        | "eval"
        | "set"
        | "var.force"
        | "var.unforce"
        | "io.write"
        | "io.force"
        | "io.unforce"
        | "debug.evaluate"
        | "hmi.write"
        | "hmi.descriptor.update"
        | "hmi.scaffold.reset" => AccessRole::Engineer,
        "config.set" => required_role_for_config_set(params),
        "shutdown" | "bytecode.reload" | "pair.start" | "pair.list" | "pair.revoke" => {
            AccessRole::Admin
        }
        _ => AccessRole::Viewer,
    }
}

fn required_role_for_config_set(params: Option<&serde_json::Value>) -> AccessRole {
    let Some(params) = params.and_then(serde_json::Value::as_object) else {
        return AccessRole::Engineer;
    };
    let requires_admin = params.keys().any(|key| {
        matches!(
            key.as_str(),
            "control.auth_token" | "mesh.auth_token" | "control.mode" | "web.auth"
        )
    });
    if requires_admin {
        AccessRole::Admin
    } else {
        AccessRole::Engineer
    }
}

fn handle_status(id: u64, state: &ControlState) -> ControlResponse {
    let status = state.resource.state();
    let error = state.resource.last_error().map(|err| err.to_string());
    let simulation = state
        .settings
        .lock()
        .ok()
        .map(|guard| guard.simulation.clone());
    let io_health = state
        .io_health
        .lock()
        .ok()
        .map(|guard| guard.iter().map(io_health_to_json).collect::<Vec<_>>())
        .unwrap_or_default();
    let metrics = state
        .metrics
        .lock()
        .ok()
        .map(|guard| guard.snapshot())
        .unwrap_or_default();
    ControlResponse::ok(
        id,
        json!({
            "state": format!("{status:?}").to_ascii_lowercase(),
            "fault": error,
            "resource": state.resource_name.as_str(),
            "plc_name": state.resource_name.as_str(),
            "uptime_ms": metrics.uptime_ms,
            "debug_enabled": state.debug_enabled.load(Ordering::Relaxed),
            "control_mode": state
                .control_mode
                .lock()
                .map(|mode| format!("{:?}", *mode).to_ascii_lowercase())
                .unwrap_or_else(|_| "production".to_string()),
            "simulation_mode": simulation
                .as_ref()
                .map(|cfg| cfg.mode_label.as_str())
                .unwrap_or("production"),
            "simulation_enabled": simulation.as_ref().map(|cfg| cfg.enabled).unwrap_or(false),
            "simulation_time_scale": simulation.as_ref().map(|cfg| cfg.time_scale).unwrap_or(1),
            "simulation_warning": simulation
                .as_ref()
                .map(|cfg| cfg.warning.as_str())
                .unwrap_or(""),
            "hmi_read_only": true,
            "metrics": {
                "cycle_ms": {
                    "min": metrics.cycle.min_ms,
                    "avg": metrics.cycle.avg_ms,
                    "max": metrics.cycle.max_ms,
                    "last": metrics.cycle.last_ms,
                },
                "overruns": metrics.overruns,
                "faults": metrics.faults,
                "profiling": {
                    "enabled": metrics.profiling.enabled,
                    "top": metrics
                        .profiling
                        .top_contributors
                        .iter()
                        .map(|entry| {
                            json!({
                                "key": entry.key.as_str(),
                                "kind": entry.kind.as_str(),
                                "name": entry.name.as_str(),
                                "avg_cycle_ms": entry.avg_cycle_ms,
                                "cycle_pct": entry.cycle_pct,
                                "last_ms": entry.last_ms,
                                "last_cycle_pct": entry.last_cycle_pct,
                            })
                        })
                        .collect::<Vec<_>>(),
                },
            },
            "io_drivers": io_health,
        }),
    )
}

fn handle_health(id: u64, state: &ControlState) -> ControlResponse {
    let status = state.resource.state();
    let error = state.resource.last_error().map(|err| err.to_string());
    let io_health = state
        .io_health
        .lock()
        .ok()
        .map(|guard| guard.clone())
        .unwrap_or_default();
    let has_faulted_driver = io_health
        .iter()
        .any(|entry| matches!(entry.health, IoDriverHealth::Faulted { .. }));
    let ok = matches!(
        status,
        crate::scheduler::ResourceState::Running
            | crate::scheduler::ResourceState::Ready
            | crate::scheduler::ResourceState::Paused
    ) && error.is_none()
        && !has_faulted_driver;
    ControlResponse::ok(
        id,
        json!({
            "ok": ok,
            "state": format!("{status:?}").to_ascii_lowercase(),
            "fault": error,
            "io_drivers": io_health.iter().map(io_health_to_json).collect::<Vec<_>>(),
        }),
    )
}

fn handle_task_stats(id: u64, state: &ControlState) -> ControlResponse {
    let metrics = state
        .metrics
        .lock()
        .ok()
        .map(|guard| guard.snapshot())
        .unwrap_or_default();
    let tasks = metrics
        .tasks
        .iter()
        .map(|task| {
            json!({
                "name": task.name.as_str(),
                "min_ms": task.min_ms,
                "avg_ms": task.avg_ms,
                "max_ms": task.max_ms,
                "last_ms": task.last_ms,
                "overruns": task.overruns,
            })
        })
        .collect::<Vec<_>>();
    let top_contributors = metrics
        .profiling
        .top_contributors
        .iter()
        .map(|entry| {
            json!({
                "key": entry.key.as_str(),
                "kind": entry.kind.as_str(),
                "name": entry.name.as_str(),
                "avg_cycle_ms": entry.avg_cycle_ms,
                "cycle_pct": entry.cycle_pct,
                "last_ms": entry.last_ms,
                "last_cycle_pct": entry.last_cycle_pct,
            })
        })
        .collect::<Vec<_>>();
    ControlResponse::ok(
        id,
        json!({
            "tasks": tasks,
            "profiling_enabled": metrics.profiling.enabled,
            "top_contributors": top_contributors,
        }),
    )
}

fn handle_io_list(id: u64, state: &ControlState) -> ControlResponse {
    let snapshot = match state.io_snapshot.lock() {
        Ok(guard) => guard.clone(),
        Err(_) => None,
    };
    match snapshot {
        Some(snapshot) => ControlResponse::ok(id, snapshot.into_json()),
        None => ControlResponse::error(id, "no snapshot available".into()),
    }
}

fn handle_hmi_schema_get(id: u64, state: &ControlState) -> ControlResponse {
    let metadata = match state.metadata.lock() {
        Ok(guard) => guard,
        Err(_) => return ControlResponse::error(id, "metadata unavailable".into()),
    };
    let snapshot = load_runtime_snapshot(state);
    let descriptor = hmi_descriptor_snapshot(state);
    let mut result = crate::hmi::build_schema(
        state.resource_name.as_str(),
        &metadata,
        snapshot.as_ref(),
        true,
        Some(&descriptor.customization),
    );
    result.schema_revision = descriptor.schema_revision;
    result.descriptor_error = descriptor.last_error.clone();
    ControlResponse::ok(
        id,
        serde_json::to_value(result).expect("serialize hmi.schema.get"),
    )
}

fn handle_hmi_values_get(
    id: u64,
    params: Option<serde_json::Value>,
    state: &ControlState,
) -> ControlResponse {
    let params = match params {
        Some(value) => match serde_json::from_value::<HmiValuesParams>(value) {
            Ok(parsed) => parsed,
            Err(err) => return ControlResponse::error(id, format!("invalid params: {err}")),
        },
        None => HmiValuesParams { ids: None },
    };
    let metadata = match state.metadata.lock() {
        Ok(guard) => guard,
        Err(_) => return ControlResponse::error(id, "metadata unavailable".into()),
    };
    let snapshot = load_runtime_snapshot(state);
    let descriptor = hmi_descriptor_snapshot(state);
    let schema = crate::hmi::build_schema(
        state.resource_name.as_str(),
        &metadata,
        snapshot.as_ref(),
        true,
        Some(&descriptor.customization),
    );
    let result = crate::hmi::build_values(
        state.resource_name.as_str(),
        &metadata,
        snapshot.as_ref(),
        true,
        params.ids.as_deref(),
    );
    if let Ok(mut live) = state.hmi_live.lock() {
        crate::hmi::update_live_state(&mut live, &schema, &result);
    }
    ControlResponse::ok(
        id,
        serde_json::to_value(result).expect("serialize hmi.values.get"),
    )
}

fn handle_hmi_trends_get(
    id: u64,
    params: Option<serde_json::Value>,
    state: &ControlState,
) -> ControlResponse {
    let params = match params {
        Some(value) => match serde_json::from_value::<HmiTrendsParams>(value) {
            Ok(parsed) => parsed,
            Err(err) => return ControlResponse::error(id, format!("invalid params: {err}")),
        },
        None => HmiTrendsParams::default(),
    };
    let metadata = match state.metadata.lock() {
        Ok(guard) => guard,
        Err(_) => return ControlResponse::error(id, "metadata unavailable".into()),
    };
    let snapshot = load_runtime_snapshot(state);
    let descriptor = hmi_descriptor_snapshot(state);
    let schema = crate::hmi::build_schema(
        state.resource_name.as_str(),
        &metadata,
        snapshot.as_ref(),
        true,
        Some(&descriptor.customization),
    );
    let values = crate::hmi::build_values(
        state.resource_name.as_str(),
        &metadata,
        snapshot.as_ref(),
        true,
        params.ids.as_deref(),
    );
    let result = match state.hmi_live.lock() {
        Ok(mut live) => {
            crate::hmi::update_live_state(&mut live, &schema, &values);
            crate::hmi::build_trends(
                &live,
                &schema,
                params.ids.as_deref(),
                params.duration_ms.unwrap_or(10 * 60 * 1_000),
                params.buckets.unwrap_or(120),
            )
        }
        Err(_) => return ControlResponse::error(id, "hmi state unavailable".into()),
    };
    ControlResponse::ok(
        id,
        serde_json::to_value(result).expect("serialize hmi.trends.get"),
    )
}

fn handle_hmi_alarms_get(
    id: u64,
    params: Option<serde_json::Value>,
    state: &ControlState,
) -> ControlResponse {
    let params = match params {
        Some(value) => match serde_json::from_value::<HmiAlarmsParams>(value) {
            Ok(parsed) => parsed,
            Err(err) => return ControlResponse::error(id, format!("invalid params: {err}")),
        },
        None => HmiAlarmsParams::default(),
    };
    let metadata = match state.metadata.lock() {
        Ok(guard) => guard,
        Err(_) => return ControlResponse::error(id, "metadata unavailable".into()),
    };
    let snapshot = load_runtime_snapshot(state);
    let descriptor = hmi_descriptor_snapshot(state);
    let schema = crate::hmi::build_schema(
        state.resource_name.as_str(),
        &metadata,
        snapshot.as_ref(),
        true,
        Some(&descriptor.customization),
    );
    let values = crate::hmi::build_values(
        state.resource_name.as_str(),
        &metadata,
        snapshot.as_ref(),
        true,
        None,
    );
    let result = match state.hmi_live.lock() {
        Ok(mut live) => {
            crate::hmi::update_live_state(&mut live, &schema, &values);
            crate::hmi::build_alarm_view(&live, params.limit.unwrap_or(100))
        }
        Err(_) => return ControlResponse::error(id, "hmi state unavailable".into()),
    };
    ControlResponse::ok(
        id,
        serde_json::to_value(result).expect("serialize hmi.alarms.get"),
    )
}

fn handle_hmi_descriptor_get(id: u64, state: &ControlState) -> ControlResponse {
    let descriptor = hmi_descriptor_snapshot(state);
    if let Some(dir) = descriptor.customization.dir_descriptor().cloned() {
        return ControlResponse::ok(
            id,
            serde_json::to_value(dir).expect("serialize hmi.descriptor.get"),
        );
    }

    let metadata = match state.metadata.lock() {
        Ok(guard) => guard,
        Err(_) => return ControlResponse::error(id, "metadata unavailable".into()),
    };
    let snapshot = load_runtime_snapshot(state);
    let schema = crate::hmi::build_schema(
        state.resource_name.as_str(),
        &metadata,
        snapshot.as_ref(),
        true,
        Some(&descriptor.customization),
    );
    let inferred = descriptor_from_schema(&schema);
    ControlResponse::ok(
        id,
        serde_json::to_value(inferred).expect("serialize inferred hmi descriptor"),
    )
}

fn handle_hmi_descriptor_update(
    id: u64,
    params: Option<serde_json::Value>,
    state: &ControlState,
) -> ControlResponse {
    let params = match params {
        Some(value) => match serde_json::from_value::<HmiDescriptorUpdateParams>(value) {
            Ok(parsed) => parsed,
            Err(err) => return ControlResponse::error(id, format!("invalid params: {err}")),
        },
        None => return ControlResponse::error(id, "missing params".into()),
    };
    let project_root = match state.project_root.as_ref() {
        Some(path) => path,
        None => {
            return ControlResponse::error(
                id,
                "hmi.descriptor.update requires a project bundle".into(),
            )
        }
    };

    let metadata = match state.metadata.lock() {
        Ok(guard) => guard,
        Err(_) => return ControlResponse::error(id, "metadata unavailable".into()),
    };
    let snapshot = load_runtime_snapshot(state);
    let diagnostics = crate::hmi::validate_hmi_bindings(
        state.resource_name.as_str(),
        &metadata,
        snapshot.as_ref(),
        &params.descriptor,
    );
    if !diagnostics.is_empty() {
        return ControlResponse::error(
            id,
            format!(
                "descriptor validation failed ({} issue(s), first: {} [{}])",
                diagnostics.len(),
                diagnostics[0].message,
                diagnostics[0].code
            ),
        );
    }
    drop(metadata);

    let files = match crate::hmi::write_hmi_dir_descriptor(project_root, &params.descriptor) {
        Ok(files) => files,
        Err(err) => {
            return ControlResponse::error(id, format!("descriptor write failed: {err}"));
        }
    };
    let revision = match reload_hmi_descriptor_state(state) {
        Ok(revision) => revision,
        Err(err) => return ControlResponse::error(id, format!("descriptor reload failed: {err}")),
    };
    ControlResponse::ok(
        id,
        json!({
            "status": "updated",
            "schema_revision": revision,
            "files": files,
        }),
    )
}

fn handle_hmi_scaffold_reset(
    id: u64,
    params: Option<serde_json::Value>,
    state: &ControlState,
) -> ControlResponse {
    let params = match params {
        Some(value) => match serde_json::from_value::<HmiScaffoldResetParams>(value) {
            Ok(parsed) => parsed,
            Err(err) => return ControlResponse::error(id, format!("invalid params: {err}")),
        },
        None => HmiScaffoldResetParams::default(),
    };
    let project_root = match state.project_root.as_ref() {
        Some(path) => path,
        None => {
            return ControlResponse::error(
                id,
                "hmi.scaffold.reset requires a project bundle".into(),
            )
        }
    };

    let mode = match params
        .mode
        .as_deref()
        .map(|value| value.trim().to_ascii_lowercase())
    {
        Some(mode) if mode == "update" => crate::hmi::HmiScaffoldMode::Update,
        Some(mode) if mode == "reset" || mode.is_empty() => crate::hmi::HmiScaffoldMode::Reset,
        Some(mode) => {
            return ControlResponse::error(
                id,
                format!("invalid scaffold mode '{mode}' (expected update|reset)"),
            )
        }
        None => crate::hmi::HmiScaffoldMode::Reset,
    };
    let style = params
        .style
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| {
            hmi_descriptor_snapshot(state)
                .customization
                .dir_descriptor()
                .and_then(|descriptor| descriptor.config.theme.style.clone())
        })
        .unwrap_or_else(|| "industrial".to_string());

    let metadata = match state.metadata.lock() {
        Ok(guard) => guard,
        Err(_) => return ControlResponse::error(id, "metadata unavailable".into()),
    };
    let snapshot = load_runtime_snapshot(state);
    let source_refs = state
        .sources
        .files()
        .iter()
        .map(|file| crate::hmi::HmiSourceRef {
            path: file.path.as_path(),
            text: file.text.as_str(),
        })
        .collect::<Vec<_>>();
    let summary = match crate::hmi::scaffold_hmi_dir_with_sources_mode(
        project_root,
        &metadata,
        snapshot.as_ref(),
        &source_refs,
        style.as_str(),
        mode,
        false,
    ) {
        Ok(summary) => summary,
        Err(err) => {
            return ControlResponse::error(id, format!("failed to reset scaffold: {err}"));
        }
    };
    drop(metadata);

    let revision = match reload_hmi_descriptor_state(state) {
        Ok(revision) => revision,
        Err(err) => return ControlResponse::error(id, format!("descriptor reload failed: {err}")),
    };

    ControlResponse::ok(
        id,
        json!({
            "status": "updated",
            "mode": mode.as_str(),
            "style": summary.style,
            "schema_revision": revision,
            "files": summary.files,
        }),
    )
}

fn handle_hmi_alarm_ack(
    id: u64,
    params: Option<serde_json::Value>,
    state: &ControlState,
) -> ControlResponse {
    let params = match params {
        Some(value) => match serde_json::from_value::<HmiAlarmAckParams>(value) {
            Ok(parsed) => parsed,
            Err(err) => return ControlResponse::error(id, format!("invalid params: {err}")),
        },
        None => return ControlResponse::error(id, "missing params".into()),
    };
    let timestamp_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let result = match state.hmi_live.lock() {
        Ok(mut live) => {
            match crate::hmi::acknowledge_alarm(&mut live, params.id.as_str(), timestamp_ms) {
                Ok(()) => crate::hmi::build_alarm_view(&live, 100),
                Err(err) => return ControlResponse::error(id, err),
            }
        }
        Err(_) => return ControlResponse::error(id, "hmi state unavailable".into()),
    };
    ControlResponse::ok(
        id,
        serde_json::to_value(result).expect("serialize hmi.alarm.ack"),
    )
}

fn handle_hmi_write(
    id: u64,
    params: Option<serde_json::Value>,
    state: &ControlState,
) -> ControlResponse {
    let params = match params {
        Some(value) => match serde_json::from_value::<HmiWriteParams>(value) {
            Ok(parsed) => parsed,
            Err(err) => return ControlResponse::error(id, format!("invalid params: {err}")),
        },
        None => return ControlResponse::error(id, "missing params".into()),
    };
    let target = params.id.trim();
    if target.is_empty() {
        return ControlResponse::error(id, "missing params.id".into());
    }

    let descriptor = hmi_descriptor_snapshot(state);
    let customization = descriptor.customization;
    if !customization.write_enabled() {
        return ControlResponse::error(id, "hmi.write disabled in read-only mode".into());
    }
    if customization.write_allowlist().is_empty() {
        return ControlResponse::error(id, "hmi.write allowlist is empty".into());
    }

    let metadata = match state.metadata.lock() {
        Ok(guard) => guard,
        Err(_) => return ControlResponse::error(id, "metadata unavailable".into()),
    };
    let snapshot = match load_runtime_snapshot(state) {
        Some(snapshot) => snapshot,
        None => return ControlResponse::error(id, "runtime snapshot unavailable".into()),
    };
    let point = match crate::hmi::resolve_write_point(
        state.resource_name.as_str(),
        &metadata,
        Some(&snapshot),
        target,
    ) {
        Some(point) => point,
        None => return ControlResponse::error(id, format!("unknown hmi target '{target}'")),
    };
    let allowed = customization.write_target_allowed(point.id.as_str())
        || customization.write_target_allowed(point.path.as_str());
    if !allowed {
        return ControlResponse::error(id, "hmi.write target is not in allowlist".into());
    }
    let template = match crate::hmi::resolve_write_value_template(&point, &snapshot) {
        Some(value) => value,
        None => {
            return ControlResponse::error(
                id,
                format!("hmi.write target '{}' is currently unavailable", point.id),
            )
        }
    };
    let value = match parse_hmi_write_value(&params.value, &template) {
        Some(value) => value,
        None => {
            return ControlResponse::error(
                id,
                format!("invalid hmi.write value for target '{}'", point.id),
            )
        }
    };

    match &point.binding {
        crate::hmi::HmiWriteBinding::ProgramVar { program, variable } => {
            let instance_id = match snapshot.storage.get_global(program.as_str()) {
                Some(Value::Instance(instance_id)) => *instance_id,
                _ => {
                    return ControlResponse::error(
                        id,
                        format!("hmi.write target '{}' is currently unavailable", point.id),
                    )
                }
            };
            state
                .debug
                .enqueue_instance_write(instance_id, variable.clone(), value);
        }
        crate::hmi::HmiWriteBinding::Global { name } => {
            state.debug.enqueue_global_write(name.clone(), value);
        }
    }

    ControlResponse::ok(
        id,
        json!({
            "status": "queued",
            "id": point.id,
            "path": point.path,
        }),
    )
}

fn hmi_descriptor_snapshot(state: &ControlState) -> HmiRuntimeDescriptor {
    state
        .hmi_descriptor
        .lock()
        .map(|guard| guard.clone())
        .unwrap_or_else(|_| {
            HmiRuntimeDescriptor::from_sources(state.project_root.as_deref(), &state.sources)
        })
}

fn reload_hmi_descriptor_state(state: &ControlState) -> Result<u64, String> {
    let customization = match load_hmi_customization_strict_from_sources(
        state.project_root.as_deref(),
        &state.sources,
    ) {
        Ok(customization) => customization,
        Err(err) => {
            if let Ok(mut descriptor) = state.hmi_descriptor.lock() {
                descriptor.last_error = Some(err.clone());
            }
            return Err(err);
        }
    };
    let mut descriptor = state
        .hmi_descriptor
        .lock()
        .map_err(|_| "hmi descriptor state unavailable".to_string())?;
    descriptor.customization = customization;
    descriptor.schema_revision = descriptor.schema_revision.saturating_add(1);
    descriptor.last_error = None;
    Ok(descriptor.schema_revision)
}

fn load_hmi_customization_from_sources(
    project_root: Option<&Path>,
    sources: &SourceRegistry,
) -> crate::hmi::HmiCustomization {
    let source_refs = sources
        .files()
        .iter()
        .map(|file| crate::hmi::HmiSourceRef {
            path: &file.path,
            text: file.text.as_str(),
        })
        .collect::<Vec<_>>();
    crate::hmi::load_customization(project_root, &source_refs)
}

fn load_hmi_customization_strict_from_sources(
    project_root: Option<&Path>,
    sources: &SourceRegistry,
) -> Result<crate::hmi::HmiCustomization, String> {
    let source_refs = sources
        .files()
        .iter()
        .map(|file| crate::hmi::HmiSourceRef {
            path: &file.path,
            text: file.text.as_str(),
        })
        .collect::<Vec<_>>();
    crate::hmi::try_load_customization(project_root, &source_refs).map_err(|err| err.to_string())
}

fn hmi_event_matches_descriptor(event: &Event, project_root: &Path) -> bool {
    if !matches!(
        event.kind,
        EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
    ) {
        return false;
    }
    let hmi_dir = project_root.join("hmi");
    event.paths.iter().any(|path| {
        path.starts_with(&hmi_dir)
            && path
                .extension()
                .and_then(|value| value.to_str())
                .is_some_and(|ext| ext.eq_ignore_ascii_case("toml"))
    })
}

fn load_runtime_snapshot(state: &ControlState) -> Option<crate::debug::DebugSnapshot> {
    let (tx, rx) = std::sync::mpsc::channel();
    let request = ResourceCommand::Snapshot { respond_to: tx };
    if state.resource.send_command(request).is_ok() {
        if let Ok(snapshot) = rx.recv_timeout(std::time::Duration::from_millis(250)) {
            return Some(snapshot);
        }
    }
    state.debug.snapshot()
}

fn descriptor_from_schema(schema: &crate::hmi::HmiSchemaResult) -> crate::hmi::HmiDirDescriptor {
    let mut pages = Vec::new();
    let widgets_by_page = schema.widgets.iter().fold(
        BTreeMap::<String, Vec<&crate::hmi::HmiWidgetSchema>>::new(),
        |mut acc, widget| {
            acc.entry(widget.page.clone()).or_default().push(widget);
            acc
        },
    );
    let widget_by_id = schema
        .widgets
        .iter()
        .map(|widget| (widget.id.as_str(), widget))
        .collect::<BTreeMap<_, _>>();

    for page in &schema.pages {
        let page_widgets = widgets_by_page
            .get(page.id.as_str())
            .cloned()
            .unwrap_or_default();
        let mut sections = Vec::new();
        if !page.sections.is_empty() {
            for section in &page.sections {
                let widgets = section
                    .widget_ids
                    .iter()
                    .filter_map(|id| widget_by_id.get(id.as_str()).copied())
                    .map(|widget| crate::hmi::HmiDirWidget {
                        widget_type: Some(widget.widget.clone()),
                        bind: widget.path.clone(),
                        label: Some(widget.label.clone()),
                        unit: widget.unit.clone(),
                        min: widget.min,
                        max: widget.max,
                        span: widget.widget_span,
                        on_color: widget.on_color.clone(),
                        off_color: widget.off_color.clone(),
                        inferred_interface: widget.inferred_interface.then_some(true),
                        detail_page: widget.detail_page.clone(),
                        zones: widget.zones.clone(),
                    })
                    .collect::<Vec<_>>();
                if widgets.is_empty() {
                    continue;
                }
                sections.push(crate::hmi::HmiDirSection {
                    title: section.title.clone(),
                    span: section.span.clamp(1, 12),
                    tier: section.tier.clone(),
                    widgets,
                });
            }
        }
        if sections.is_empty() {
            let mut grouped = BTreeMap::<String, Vec<&crate::hmi::HmiWidgetSchema>>::new();
            for widget in &page_widgets {
                grouped
                    .entry(widget.group.clone())
                    .or_default()
                    .push(*widget);
            }
            for (group, widgets) in grouped {
                let mapped = widgets
                    .into_iter()
                    .map(|widget| crate::hmi::HmiDirWidget {
                        widget_type: Some(widget.widget.clone()),
                        bind: widget.path.clone(),
                        label: Some(widget.label.clone()),
                        unit: widget.unit.clone(),
                        min: widget.min,
                        max: widget.max,
                        span: widget.widget_span,
                        on_color: widget.on_color.clone(),
                        off_color: widget.off_color.clone(),
                        inferred_interface: widget.inferred_interface.then_some(true),
                        detail_page: widget.detail_page.clone(),
                        zones: widget.zones.clone(),
                    })
                    .collect::<Vec<_>>();
                if mapped.is_empty() {
                    continue;
                }
                sections.push(crate::hmi::HmiDirSection {
                    title: if group.trim().is_empty() {
                        "General".to_string()
                    } else {
                        group
                    },
                    span: 12,
                    tier: None,
                    widgets: mapped,
                });
            }
        }

        pages.push(crate::hmi::HmiDirPage {
            id: page.id.clone(),
            title: page.title.clone(),
            icon: page.icon.clone(),
            order: page.order,
            kind: page.kind.clone(),
            duration_ms: page.duration_ms,
            svg: page.svg.clone(),
            hidden: page.hidden,
            signals: page.signals.clone(),
            sections,
            bindings: page
                .bindings
                .iter()
                .map(|binding| crate::hmi::HmiDirProcessBinding {
                    selector: binding.selector.clone(),
                    attribute: binding.attribute.clone(),
                    source: binding.source.clone(),
                    format: binding.format.clone(),
                    map: binding.map.clone(),
                    scale: binding.scale.clone(),
                })
                .collect(),
        });
    }

    crate::hmi::HmiDirDescriptor {
        config: crate::hmi::HmiDirConfig {
            version: Some(1),
            theme: crate::hmi::HmiDirTheme {
                style: Some(schema.theme.style.clone()),
                accent: Some(schema.theme.accent.clone()),
            },
            layout: crate::hmi::HmiDirLayout::default(),
            write: crate::hmi::HmiDirWrite::default(),
            alarms: Vec::new(),
        },
        pages,
    }
}

fn handle_events_tail(
    id: u64,
    params: Option<serde_json::Value>,
    state: &ControlState,
) -> ControlResponse {
    let limit = params
        .and_then(|value| value.get("limit").cloned())
        .and_then(|value| value.as_u64())
        .unwrap_or(50) as usize;
    let events = state
        .events
        .lock()
        .map(|guard| guard.iter().rev().take(limit).cloned().collect::<Vec<_>>())
        .unwrap_or_default();
    let payload = events
        .into_iter()
        .map(runtime_event_to_json)
        .collect::<Vec<_>>();
    ControlResponse::ok(id, json!({ "events": payload }))
}

fn handle_faults(
    id: u64,
    params: Option<serde_json::Value>,
    state: &ControlState,
) -> ControlResponse {
    let limit = params
        .and_then(|value| value.get("limit").cloned())
        .and_then(|value| value.as_u64())
        .unwrap_or(50) as usize;
    let events = state
        .events
        .lock()
        .map(|guard| guard.iter().rev().take(limit).cloned().collect::<Vec<_>>())
        .unwrap_or_default();
    let faults = events
        .into_iter()
        .filter(|event| matches!(event, crate::debug::RuntimeEvent::Fault { .. }))
        .map(runtime_event_to_json)
        .collect::<Vec<_>>();
    ControlResponse::ok(id, json!({ "faults": faults }))
}

fn handle_historian_query(
    id: u64,
    params: Option<serde_json::Value>,
    state: &ControlState,
) -> ControlResponse {
    let Some(historian) = state.historian.as_ref() else {
        return ControlResponse::error(id, "historian disabled".into());
    };
    let params = match params {
        Some(value) => match serde_json::from_value::<HistorianQueryParams>(value) {
            Ok(parsed) => parsed,
            Err(err) => return ControlResponse::error(id, format!("invalid params: {err}")),
        },
        None => HistorianQueryParams::default(),
    };
    let items = historian.query(
        params.variable.as_deref(),
        params.since_ms,
        params.limit.unwrap_or(250),
    );
    ControlResponse::ok(id, json!({ "items": items }))
}

fn handle_historian_alerts(
    id: u64,
    params: Option<serde_json::Value>,
    state: &ControlState,
) -> ControlResponse {
    let Some(historian) = state.historian.as_ref() else {
        return ControlResponse::error(id, "historian disabled".into());
    };
    let params = match params {
        Some(value) => match serde_json::from_value::<HistorianAlertsParams>(value) {
            Ok(parsed) => parsed,
            Err(err) => return ControlResponse::error(id, format!("invalid params: {err}")),
        },
        None => HistorianAlertsParams::default(),
    };
    let items = historian.alerts(params.limit.unwrap_or(200));
    ControlResponse::ok(id, json!({ "items": items }))
}

fn handle_config_get(id: u64, state: &ControlState) -> ControlResponse {
    let settings = match state.settings.lock() {
        Ok(guard) => guard.clone(),
        Err(_) => return ControlResponse::error(id, "settings unavailable".into()),
    };
    let auth = state.auth_token.lock().ok();
    let auth_set = auth
        .as_ref()
        .and_then(|value| value.as_ref())
        .map(|value| value.len())
        .unwrap_or(0);
    let observability = state.historian.as_ref().map(|hist| hist.config().clone());
    let observability_alerts = observability
        .as_ref()
        .map(|cfg| {
            cfg.alerts
                .iter()
                .map(|rule| {
                    let mut item = serde_json::Map::new();
                    item.insert(
                        "name".to_string(),
                        serde_json::Value::String(rule.name.to_string()),
                    );
                    item.insert(
                        "variable".to_string(),
                        serde_json::Value::String(rule.variable.to_string()),
                    );
                    item.insert(
                        "above".to_string(),
                        rule.above
                            .and_then(serde_json::Number::from_f64)
                            .map(serde_json::Value::Number)
                            .unwrap_or(serde_json::Value::Null),
                    );
                    item.insert(
                        "below".to_string(),
                        rule.below
                            .and_then(serde_json::Number::from_f64)
                            .map(serde_json::Value::Number)
                            .unwrap_or(serde_json::Value::Null),
                    );
                    item.insert(
                        "debounce_samples".to_string(),
                        serde_json::Value::Number(serde_json::Number::from(rule.debounce_samples)),
                    );
                    item.insert(
                        "hook".to_string(),
                        rule.hook
                            .as_ref()
                            .map(|value| serde_json::Value::String(value.to_string()))
                            .unwrap_or(serde_json::Value::Null),
                    );
                    serde_json::Value::Object(item)
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    ControlResponse::ok(
        id,
        json!({
            "log.level": settings.log_level.as_str(),
            "watchdog.enabled": settings.watchdog.enabled,
            "watchdog.timeout_ms": settings.watchdog.timeout.as_millis(),
            "watchdog.action": format!("{:?}", settings.watchdog.action),
            "fault.policy": format!("{:?}", settings.fault_policy),
            "retain.mode": format!("{:?}", settings.retain_mode),
            "retain.save_interval_ms": settings.retain_save_interval.map(|val| val.as_millis()),
            "web.enabled": settings.web.enabled,
            "web.listen": settings.web.listen.as_str(),
            "web.auth": settings.web.auth.as_str(),
            "web.tls": settings.web.tls,
            "discovery.enabled": settings.discovery.enabled,
            "discovery.service_name": settings.discovery.service_name.as_str(),
            "discovery.advertise": settings.discovery.advertise,
            "discovery.interfaces": settings.discovery.interfaces.iter().map(|v| v.as_str()).collect::<Vec<_>>(),
            "mesh.enabled": settings.mesh.enabled,
            "mesh.listen": settings.mesh.listen.as_str(),
            "mesh.tls": settings.mesh.tls,
            "mesh.auth_token_set": settings.mesh.auth_token.as_ref().map(|t| t.len()).unwrap_or(0) > 0,
            "mesh.publish": settings.mesh.publish.iter().map(|v| v.as_str()).collect::<Vec<_>>(),
            "mesh.subscribe": settings
                .mesh
                .subscribe
                .iter()
                .map(|(k, v)| {
                    (
                        k.as_str().to_string(),
                        serde_json::Value::String(v.as_str().to_string()),
                    )
                })
                .collect::<serde_json::Map<_, _>>(),
            "opcua.enabled": settings.opcua.enabled,
            "opcua.listen": settings.opcua.listen.as_str(),
            "opcua.endpoint_path": settings.opcua.endpoint_path.as_str(),
            "opcua.namespace_uri": settings.opcua.namespace_uri.as_str(),
            "opcua.publish_interval_ms": settings.opcua.publish_interval_ms,
            "opcua.max_nodes": settings.opcua.max_nodes,
            "opcua.expose": settings.opcua.expose.iter().map(|v| v.as_str()).collect::<Vec<_>>(),
            "opcua.security_policy": settings.opcua.security_policy.as_str(),
            "opcua.security_mode": settings.opcua.security_mode.as_str(),
            "opcua.allow_anonymous": settings.opcua.allow_anonymous,
            "opcua.username_set": settings.opcua.username_set,
            "control.auth_token_set": auth_set > 0,
            "control.auth_token_length": if auth_set > 0 { Some(auth_set) } else { None },
            "control.debug_enabled": state.debug_enabled.load(Ordering::Relaxed),
            "control.mode": state
                .control_mode
                .lock()
                .map(|mode| format!("{:?}", *mode))
                .unwrap_or_else(|_| "Production".to_string()),
            "simulation.enabled": settings.simulation.enabled,
            "simulation.time_scale": settings.simulation.time_scale,
            "simulation.mode": settings.simulation.mode_label.as_str(),
            "simulation.warning": settings.simulation.warning.as_str(),
            "observability.enabled": observability.as_ref().map(|cfg| cfg.enabled).unwrap_or(false),
            "observability.sample_interval_ms": observability.as_ref().map(|cfg| cfg.sample_interval_ms),
            "observability.mode": observability.as_ref().map(|cfg| match cfg.mode {
                crate::historian::RecordingMode::All => "all",
                crate::historian::RecordingMode::Allowlist => "allowlist",
            }),
            "observability.include": observability
                .as_ref()
                .map(|cfg| cfg.include.iter().map(|entry| entry.as_str()).collect::<Vec<_>>())
                .unwrap_or_default(),
            "observability.history_path": observability.as_ref().map(|cfg| cfg.history_path.display().to_string()),
            "observability.max_entries": observability.as_ref().map(|cfg| cfg.max_entries),
            "observability.prometheus_enabled": observability.as_ref().map(|cfg| cfg.prometheus_enabled),
            "observability.prometheus_path": observability.as_ref().map(|cfg| cfg.prometheus_path.to_string()),
            "observability.alerts": observability_alerts,
            "hmi.read_only": true,
        }),
    )
}

fn handle_config_set(
    id: u64,
    params: Option<serde_json::Value>,
    state: &ControlState,
) -> ControlResponse {
    macro_rules! parse_or_error {
        ($expr:expr) => {
            match $expr {
                Ok(value) => value,
                Err(error) => return ControlResponse::error(id, error),
            }
        };
    }

    let params = match params {
        Some(params) => params,
        None => return ControlResponse::error(id, "missing params".into()),
    };
    let params = match params.as_object() {
        Some(params) => params,
        None => {
            return ControlResponse::error(
                id,
                "invalid config payload: params must be an object".into(),
            )
        }
    };
    let mut settings_guard = match state.settings.lock() {
        Ok(guard) => guard,
        Err(_) => return ControlResponse::error(id, "settings unavailable".into()),
    };
    let mut settings = settings_guard.clone();
    let mut updated = Vec::new();
    let mut restart_required = Vec::new();
    let mut auth_token = match state.auth_token.lock() {
        Ok(guard) => guard.clone(),
        Err(_) => return ControlResponse::error(id, "auth token unavailable".into()),
    };
    let mut auth_changed = false;
    if let Some(value) = params.get("control.auth_token") {
        if value.is_null() {
            if state.control_requires_auth {
                return ControlResponse::error(id, "auth token required for tcp endpoints".into());
            }
            auth_token = None;
            auth_changed = true;
            updated.push("control.auth_token");
        } else if let Some(token) = value.as_str() {
            let token = token.trim();
            if token.is_empty() {
                return ControlResponse::error(
                    id,
                    config_value_error("control.auth_token", "must not be empty"),
                );
            }
            auth_token = Some(SmolStr::new(token));
            auth_changed = true;
            updated.push("control.auth_token");
        } else {
            return ControlResponse::error(
                id,
                config_type_error("control.auth_token", "string or null"),
            );
        }
    }

    let mut control_mode = match state.control_mode.lock() {
        Ok(guard) => *guard,
        Err(_) => return ControlResponse::error(id, "control mode unavailable".into()),
    };
    let mut control_mode_changed = false;
    let mut debug_enabled = state.debug_enabled.load(Ordering::Relaxed);
    let mut debug_enabled_changed = false;

    for (key, value) in params {
        match key.as_str() {
            "control.auth_token" => {}
            "log.level" => {
                let level = parse_or_error!(expect_non_empty_string(key, value));
                settings.log_level = SmolStr::new(level);
                updated.push("log.level");
            }
            "watchdog.enabled" => {
                settings.watchdog.enabled = parse_or_error!(expect_bool(key, value));
                updated.push("watchdog.enabled");
            }
            "watchdog.timeout_ms" => {
                let timeout = parse_or_error!(expect_positive_i64(key, value));
                settings.watchdog.timeout = crate::value::Duration::from_millis(timeout);
                updated.push("watchdog.timeout_ms");
            }
            "watchdog.action" => {
                let action = parse_or_error!(expect_non_empty_string(key, value));
                settings.watchdog.action =
                    parse_or_error!(crate::watchdog::WatchdogAction::parse(action)
                        .map_err(|err| config_value_error(key, &err.to_string())));
                updated.push("watchdog.action");
            }
            "fault.policy" => {
                let policy = parse_or_error!(expect_non_empty_string(key, value));
                settings.fault_policy =
                    parse_or_error!(crate::watchdog::FaultPolicy::parse(policy)
                        .map_err(|err| config_value_error(key, &err.to_string())));
                updated.push("fault.policy");
            }
            "retain.save_interval_ms" => {
                let interval = parse_or_error!(expect_positive_i64(key, value));
                settings.retain_save_interval = Some(crate::value::Duration::from_millis(interval));
                updated.push("retain.save_interval_ms");
            }
            "retain.mode" => {
                let mode = parse_or_error!(expect_non_empty_string(key, value));
                settings.retain_mode = parse_or_error!(crate::watchdog::RetainMode::parse(mode)
                    .map_err(|err| config_value_error(key, &err.to_string())));
                updated.push("retain.mode");
                restart_required.push("retain.mode");
            }
            "web.enabled" => {
                settings.web.enabled = parse_or_error!(expect_bool(key, value));
                updated.push("web.enabled");
                restart_required.push("web.enabled");
            }
            "web.listen" => {
                let listen = parse_or_error!(expect_non_empty_string(key, value));
                settings.web.listen = SmolStr::new(listen);
                updated.push("web.listen");
                restart_required.push("web.listen");
            }
            "web.auth" => {
                let auth = parse_or_error!(expect_non_empty_string(key, value));
                if auth.eq_ignore_ascii_case("token") && auth_token.is_none() {
                    return ControlResponse::error(
                        id,
                        config_value_error("web.auth", "token mode requires control.auth_token"),
                    );
                }
                if !(auth.eq_ignore_ascii_case("local") || auth.eq_ignore_ascii_case("token")) {
                    return ControlResponse::error(
                        id,
                        config_value_error("web.auth", "expected 'local' or 'token'"),
                    );
                }
                settings.web.auth = SmolStr::new(auth.to_ascii_lowercase());
                updated.push("web.auth");
                restart_required.push("web.auth");
            }
            "web.tls" => {
                settings.web.tls = parse_or_error!(expect_bool(key, value));
                updated.push("web.tls");
                restart_required.push("web.tls");
            }
            "discovery.enabled" => {
                settings.discovery.enabled = parse_or_error!(expect_bool(key, value));
                updated.push("discovery.enabled");
                restart_required.push("discovery.enabled");
            }
            "discovery.service_name" => {
                let service_name = parse_or_error!(expect_non_empty_string(key, value));
                settings.discovery.service_name = SmolStr::new(service_name);
                updated.push("discovery.service_name");
                restart_required.push("discovery.service_name");
            }
            "discovery.advertise" => {
                settings.discovery.advertise = parse_or_error!(expect_bool(key, value));
                updated.push("discovery.advertise");
                restart_required.push("discovery.advertise");
            }
            "discovery.interfaces" => {
                settings.discovery.interfaces = parse_or_error!(expect_string_array(key, value))
                    .into_iter()
                    .map(SmolStr::new)
                    .collect();
                updated.push("discovery.interfaces");
                restart_required.push("discovery.interfaces");
            }
            "mesh.enabled" => {
                settings.mesh.enabled = parse_or_error!(expect_bool(key, value));
                updated.push("mesh.enabled");
                restart_required.push("mesh.enabled");
            }
            "mesh.listen" => {
                let listen = parse_or_error!(expect_non_empty_string(key, value));
                settings.mesh.listen = SmolStr::new(listen);
                updated.push("mesh.listen");
                restart_required.push("mesh.listen");
            }
            "mesh.tls" => {
                settings.mesh.tls = parse_or_error!(expect_bool(key, value));
                updated.push("mesh.tls");
                restart_required.push("mesh.tls");
            }
            "mesh.publish" => {
                settings.mesh.publish = parse_or_error!(expect_string_array(key, value))
                    .into_iter()
                    .map(SmolStr::new)
                    .collect();
                updated.push("mesh.publish");
                restart_required.push("mesh.publish");
            }
            "mesh.subscribe" => {
                settings.mesh.subscribe = parse_or_error!(expect_string_map(key, value))
                    .into_iter()
                    .map(|(topic, alias)| (SmolStr::new(topic), SmolStr::new(alias)))
                    .collect();
                updated.push("mesh.subscribe");
                restart_required.push("mesh.subscribe");
            }
            "mesh.auth_token" => {
                if value.is_null() {
                    settings.mesh.auth_token = None;
                } else if let Some(token) = value.as_str() {
                    let token = token.trim();
                    if token.is_empty() {
                        return ControlResponse::error(
                            id,
                            config_value_error("mesh.auth_token", "must not be empty"),
                        );
                    }
                    settings.mesh.auth_token = Some(SmolStr::new(token));
                } else {
                    return ControlResponse::error(
                        id,
                        config_type_error("mesh.auth_token", "string or null"),
                    );
                }
                updated.push("mesh.auth_token");
                restart_required.push("mesh.auth_token");
            }
            "control.debug_enabled" => {
                debug_enabled = parse_or_error!(expect_bool(key, value));
                debug_enabled_changed = true;
                updated.push("control.debug_enabled");
            }
            "control.mode" => {
                let mode = parse_or_error!(expect_non_empty_string(key, value));
                control_mode = match mode.to_ascii_lowercase().as_str() {
                    "production" => ControlMode::Production,
                    "debug" => ControlMode::Debug,
                    _ => {
                        return ControlResponse::error(
                            id,
                            config_value_error("control.mode", "expected 'production' or 'debug'"),
                        )
                    }
                };
                control_mode_changed = true;
                updated.push("control.mode");
                restart_required.push("control.mode");
            }
            _ => {
                return ControlResponse::error(id, format!("unknown config key '{key}'"));
            }
        }
    }

    *settings_guard = settings.clone();

    if auth_changed {
        if let Ok(mut guard) = state.auth_token.lock() {
            *guard = auth_token;
        } else {
            return ControlResponse::error(id, "auth token unavailable".into());
        }
    }
    if control_mode_changed {
        if let Ok(mut guard) = state.control_mode.lock() {
            *guard = control_mode;
        } else {
            return ControlResponse::error(id, "control mode unavailable".into());
        }
    }
    if debug_enabled_changed {
        state.debug_enabled.store(debug_enabled, Ordering::Relaxed);
    }

    let _ = state
        .resource
        .send_command(crate::scheduler::ResourceCommand::UpdateWatchdog(
            settings_guard.watchdog,
        ));
    let _ = state
        .resource
        .send_command(crate::scheduler::ResourceCommand::UpdateFaultPolicy(
            settings_guard.fault_policy,
        ));
    let _ =
        state
            .resource
            .send_command(crate::scheduler::ResourceCommand::UpdateRetainSaveInterval(
                settings_guard.retain_save_interval,
            ));

    ControlResponse::ok(
        id,
        json!({ "updated": updated, "restart_required": restart_required }),
    )
}

fn config_type_error(key: &str, expected: &str) -> String {
    format!("invalid config value for '{key}': expected {expected}")
}

fn config_value_error(key: &str, message: &str) -> String {
    format!("invalid config value for '{key}': {message}")
}

fn expect_bool(key: &str, value: &serde_json::Value) -> Result<bool, String> {
    value
        .as_bool()
        .ok_or_else(|| config_type_error(key, "boolean"))
}

fn expect_non_empty_string<'a>(key: &str, value: &'a serde_json::Value) -> Result<&'a str, String> {
    let value = value
        .as_str()
        .ok_or_else(|| config_type_error(key, "string"))?;
    let value = value.trim();
    if value.is_empty() {
        return Err(config_value_error(key, "must not be empty"));
    }
    Ok(value)
}

fn expect_positive_i64(key: &str, value: &serde_json::Value) -> Result<i64, String> {
    let number = value
        .as_i64()
        .or_else(|| value.as_u64().and_then(|n| i64::try_from(n).ok()))
        .ok_or_else(|| config_type_error(key, "integer >= 1"))?;
    if number < 1 {
        return Err(config_value_error(key, "must be >= 1"));
    }
    Ok(number)
}

fn expect_string_array(key: &str, value: &serde_json::Value) -> Result<Vec<String>, String> {
    let values = value
        .as_array()
        .ok_or_else(|| config_type_error(key, "array of strings"))?;
    let mut output = Vec::with_capacity(values.len());
    for (index, item) in values.iter().enumerate() {
        let Some(text) = item.as_str() else {
            return Err(config_value_error(
                key,
                &format!("entry {index} must be a string"),
            ));
        };
        let text = text.trim();
        if text.is_empty() {
            return Err(config_value_error(
                key,
                &format!("entry {index} must not be empty"),
            ));
        }
        output.push(text.to_string());
    }
    Ok(output)
}

fn expect_string_map(
    key: &str,
    value: &serde_json::Value,
) -> Result<Vec<(String, String)>, String> {
    let values = value
        .as_object()
        .ok_or_else(|| config_type_error(key, "object of strings"))?;
    let mut output = Vec::with_capacity(values.len());
    for (map_key, map_value) in values {
        if map_key.trim().is_empty() {
            return Err(config_value_error(key, "map keys must not be empty"));
        }
        let Some(text) = map_value.as_str() else {
            return Err(config_value_error(
                key,
                &format!("entry '{map_key}' must be a string"),
            ));
        };
        let text = text.trim();
        if text.is_empty() {
            return Err(config_value_error(
                key,
                &format!("entry '{map_key}' must not be empty"),
            ));
        }
        output.push((map_key.clone(), text.to_string()));
    }
    Ok(output)
}

fn handle_pause(id: u64, state: &ControlState) -> ControlResponse {
    let mode = state
        .control_mode
        .lock()
        .map(|value| *value)
        .unwrap_or(ControlMode::Production);
    if matches!(mode, ControlMode::Debug) {
        let _ = state
            .debug
            .apply_action(crate::debug::ControlAction::Pause(None));
    } else {
        let _ = state.resource.pause();
    }
    ControlResponse::ok(id, json!({"status": "paused"}))
}

fn handle_resume(id: u64, state: &ControlState) -> ControlResponse {
    let mode = state
        .control_mode
        .lock()
        .map(|value| *value)
        .unwrap_or(ControlMode::Production);
    if matches!(mode, ControlMode::Debug) {
        let _ = state
            .debug
            .apply_action(crate::debug::ControlAction::Continue);
    } else {
        let _ = state.resource.resume();
    }
    ControlResponse::ok(id, json!({"status": "running"}))
}

#[derive(Debug, Clone, Copy)]
enum StepKind {
    In,
    Over,
    Out,
}

fn handle_step(id: u64, state: &ControlState, kind: StepKind) -> ControlResponse {
    let action = match kind {
        StepKind::In => crate::debug::ControlAction::StepIn(None),
        StepKind::Over => crate::debug::ControlAction::StepOver(None),
        StepKind::Out => crate::debug::ControlAction::StepOut(None),
    };
    let _ = state.debug.apply_action(action);
    ControlResponse::ok(id, json!({"status": "stepping"}))
}

fn handle_debug_state(id: u64, state: &ControlState) -> ControlResponse {
    let paused = state.debug.is_paused();
    let last_stop = state
        .debug
        .last_stop()
        .and_then(|stop| debug_stop_to_json(stop, state));
    ControlResponse::ok(
        id,
        json!({
            "paused": paused,
            "last_stop": last_stop,
        }),
    )
}

fn handle_debug_stops(id: u64, state: &ControlState) -> ControlResponse {
    let stops = state
        .debug
        .drain_stops()
        .into_iter()
        .filter_map(|stop| debug_stop_to_json(stop, state))
        .collect::<Vec<_>>();
    ControlResponse::ok(id, json!({ "stops": stops }))
}

fn handle_debug_stack(id: u64, state: &ControlState) -> ControlResponse {
    let snapshot = match state.debug.snapshot() {
        Some(snapshot) => snapshot,
        None => return ControlResponse::error(id, "no snapshot available".into()),
    };
    let frames = snapshot.storage.frames();
    let frame_locations = state.debug.frame_locations();
    let fallback = state.debug.last_stop().and_then(|stop| stop.location);
    let mut stack_frames = Vec::new();
    if frames.is_empty() {
        if let Some(location) = fallback {
            if let Some(frame) = location_to_stack_frame(0, "Main", &location, state) {
                stack_frames.push(frame);
            }
        }
    } else {
        for frame in frames.iter().rev() {
            let resolved = frame_locations.get(&frame.id).copied().or(fallback);
            let frame_name = frame.owner.as_str();
            if let Some(location) = resolved {
                if let Some(frame_json) =
                    location_to_stack_frame(frame.id.0, frame_name, &location, state)
                {
                    stack_frames.push(frame_json);
                }
            }
        }
    }
    ControlResponse::ok(
        id,
        json!({
            "stack_frames": stack_frames,
            "total_frames": stack_frames.len(),
        }),
    )
}

fn handle_debug_scopes(
    id: u64,
    params: Option<serde_json::Value>,
    state: &ControlState,
) -> ControlResponse {
    let params: DebugScopesParams = match params {
        Some(value) => match serde_json::from_value(value) {
            Ok(parsed) => parsed,
            Err(err) => return ControlResponse::error(id, format!("invalid params: {err}")),
        },
        None => return ControlResponse::error(id, "missing params".into()),
    };
    debug!("control debug.scopes frame_id={}", params.frame_id);
    let snapshot = match state.debug.snapshot() {
        Some(snapshot) => snapshot,
        None => return ControlResponse::error(id, "no snapshot available".into()),
    };
    let requested_frame = crate::memory::FrameId(params.frame_id);
    let current_frame = snapshot.storage.current_frame().map(|frame| frame.id);
    let has_requested_frame = snapshot
        .storage
        .frames()
        .iter()
        .any(|frame| frame.id == requested_frame);
    let frame_id = if has_requested_frame {
        requested_frame
    } else {
        current_frame.unwrap_or(requested_frame)
    };
    let location = state
        .debug
        .frame_location(frame_id)
        .or_else(|| state.debug.last_stop().and_then(|stop| stop.location))
        .and_then(|loc| location_to_source(&loc, state));
    let has_frame = has_requested_frame || current_frame.is_some();
    let (has_globals, has_retain, has_instances) = (
        !snapshot.storage.globals().is_empty(),
        !snapshot.storage.retain().is_empty(),
        !snapshot.storage.instances().is_empty(),
    );
    debug!(
        "control debug.scopes has_frame={} current_frame={:?} requested_frame={} has_globals={} has_retain={} has_instances={}",
        has_frame,
        current_frame,
        params.frame_id,
        has_globals,
        has_retain,
        has_instances
    );
    let io_snapshot = state
        .io_snapshot
        .lock()
        .ok()
        .and_then(|guard| guard.clone());
    let has_io = crate::debug::dap::io_scope_available(io_snapshot.as_ref());

    let mut handles = match state.debug_variables.lock() {
        Ok(guard) => guard,
        Err(_) => return ControlResponse::error(id, "debug variables unavailable".into()),
    };
    handles.clear();

    let mut scopes = Vec::new();
    if has_frame {
        let locals_ref = handles.alloc(VariableHandle::Locals(frame_id));
        scopes.push(DebugScope {
            name: "Locals".to_string(),
            variables_reference: locals_ref,
            expensive: false,
            source: location.as_ref().map(|(source, _, _)| source.clone()),
            line: location.as_ref().map(|(_, line, _)| *line),
            column: location.as_ref().map(|(_, _, column)| *column),
            end_line: None,
            end_column: None,
        });
    }
    if has_globals {
        let globals_ref = handles.alloc(VariableHandle::Globals);
        scopes.push(DebugScope {
            name: "Globals".to_string(),
            variables_reference: globals_ref,
            expensive: false,
            source: None,
            line: None,
            column: None,
            end_line: None,
            end_column: None,
        });
    }
    if has_retain {
        let retain_ref = handles.alloc(VariableHandle::Retain);
        scopes.push(DebugScope {
            name: "Retain".to_string(),
            variables_reference: retain_ref,
            expensive: false,
            source: None,
            line: None,
            column: None,
            end_line: None,
            end_column: None,
        });
    }
    if has_io {
        let io_ref = handles.alloc(VariableHandle::IoRoot);
        scopes.push(DebugScope {
            name: "I/O".to_string(),
            variables_reference: io_ref,
            expensive: false,
            source: None,
            line: None,
            column: None,
            end_line: None,
            end_column: None,
        });
    }
    if has_instances {
        let instances_ref = handles.alloc(VariableHandle::Instances);
        scopes.push(DebugScope {
            name: "Instances".to_string(),
            variables_reference: instances_ref,
            expensive: false,
            source: None,
            line: None,
            column: None,
            end_line: None,
            end_column: None,
        });
    }

    debug!(
        "control debug.scopes result={:?}",
        scopes
            .iter()
            .map(|scope| scope.name.as_str())
            .collect::<Vec<_>>()
    );
    ControlResponse::ok(id, json!({ "scopes": scopes }))
}

fn handle_debug_variables(
    id: u64,
    params: Option<serde_json::Value>,
    state: &ControlState,
) -> ControlResponse {
    let params: DebugVariablesParams = match params {
        Some(value) => match serde_json::from_value(value) {
            Ok(parsed) => parsed,
            Err(err) => return ControlResponse::error(id, format!("invalid params: {err}")),
        },
        None => return ControlResponse::error(id, "missing params".into()),
    };
    debug!(
        "control debug.variables reference={}",
        params.variables_reference
    );
    let snapshot = match state.debug.snapshot() {
        Some(snapshot) => snapshot,
        None => return ControlResponse::error(id, "no snapshot available".into()),
    };
    let io_snapshot = state
        .io_snapshot
        .lock()
        .ok()
        .and_then(|guard| guard.clone());
    let mut handles = match state.debug_variables.lock() {
        Ok(guard) => guard,
        Err(_) => return ControlResponse::error(id, "debug variables unavailable".into()),
    };
    let Some(handle) = handles.get(params.variables_reference).cloned() else {
        return ControlResponse::ok(id, json!({ "variables": [] }));
    };
    debug!("control debug.variables handle={:?}", handle);
    let variables = match handle {
        VariableHandle::Locals(frame_id) => {
            let entries = snapshot
                .storage
                .frames()
                .iter()
                .find(|frame| frame.id == frame_id)
                .map(|frame| {
                    let mut entries = Vec::new();
                    if let Some(instance_id) = frame.instance_id {
                        if let Some(instance) = snapshot.storage.get_instance(instance_id) {
                            entries.extend(
                                instance
                                    .variables
                                    .iter()
                                    .map(|(name, value)| (name.to_string(), value.clone())),
                            );
                        }
                    }
                    entries.extend(
                        frame
                            .variables
                            .iter()
                            .map(|(name, value)| (name.to_string(), value.clone())),
                    );
                    entries
                })
                .unwrap_or_default();
            crate::debug::dap::variables_from_entries(&mut handles, entries)
        }
        VariableHandle::Globals => {
            let entries = snapshot
                .storage
                .globals()
                .iter()
                .map(|(name, value)| (name.to_string(), value.clone()))
                .collect::<Vec<_>>();
            crate::debug::dap::variables_from_entries(&mut handles, entries)
        }
        VariableHandle::Retain => {
            let entries = snapshot
                .storage
                .retain()
                .iter()
                .map(|(name, value)| (name.to_string(), value.clone()))
                .collect::<Vec<_>>();
            crate::debug::dap::variables_from_entries(&mut handles, entries)
        }
        VariableHandle::Instances => {
            let instances = snapshot
                .storage
                .instances()
                .iter()
                .map(|(id, data)| (*id, format!("{}#{}", data.type_name, id.0)))
                .collect::<Vec<_>>();
            crate::debug::dap::variables_from_instances(&mut handles, instances)
        }
        VariableHandle::Instance(instance_id) => {
            let entries = snapshot
                .storage
                .get_instance(instance_id)
                .map(|instance| {
                    let mut entries = instance
                        .variables
                        .iter()
                        .map(|(name, value)| (name.to_string(), value.clone()))
                        .collect::<Vec<_>>();
                    if let Some(parent) = instance.parent {
                        entries.push(("parent".to_string(), Value::Instance(parent)));
                    }
                    entries
                })
                .unwrap_or_default();
            crate::debug::dap::variables_from_entries(&mut handles, entries)
        }
        VariableHandle::Struct(value) => {
            crate::debug::dap::variables_from_struct(&mut handles, value)
        }
        VariableHandle::Array(value) => {
            crate::debug::dap::variables_from_array(&mut handles, value)
        }
        VariableHandle::Reference(value_ref) => {
            let value = snapshot.storage.read_by_ref(value_ref).cloned();
            value
                .map(|value| {
                    vec![crate::debug::dap::variable_from_value(
                        &mut handles,
                        "*".to_string(),
                        value,
                        None,
                    )]
                })
                .unwrap_or_default()
        }
        VariableHandle::IoRoot => {
            let inputs_ref = handles.alloc(VariableHandle::IoInputs);
            let outputs_ref = handles.alloc(VariableHandle::IoOutputs);
            let memory_ref = handles.alloc(VariableHandle::IoMemory);
            let state = io_snapshot.unwrap_or_default();
            vec![
                DebugVariable {
                    name: "Inputs".to_string(),
                    value: format!("{} items", state.inputs.len()),
                    r#type: None,
                    variables_reference: inputs_ref,
                    evaluate_name: None,
                },
                DebugVariable {
                    name: "Outputs".to_string(),
                    value: format!("{} items", state.outputs.len()),
                    r#type: None,
                    variables_reference: outputs_ref,
                    evaluate_name: None,
                },
                DebugVariable {
                    name: "Memory".to_string(),
                    value: format!("{} items", state.memory.len()),
                    r#type: None,
                    variables_reference: memory_ref,
                    evaluate_name: None,
                },
            ]
        }
        VariableHandle::IoInputs => {
            let state = io_snapshot.unwrap_or_default();
            crate::debug::dap::variables_from_io_entries(&state.inputs)
        }
        VariableHandle::IoOutputs => {
            let state = io_snapshot.unwrap_or_default();
            crate::debug::dap::variables_from_io_entries(&state.outputs)
        }
        VariableHandle::IoMemory => {
            let state = io_snapshot.unwrap_or_default();
            crate::debug::dap::variables_from_io_entries(&state.memory)
        }
    };
    debug!("control debug.variables result_count={}", variables.len());
    ControlResponse::ok(id, json!({ "variables": variables }))
}

fn handle_debug_evaluate(
    id: u64,
    params: Option<serde_json::Value>,
    state: &ControlState,
) -> ControlResponse {
    let params: DebugEvaluateParams = match params {
        Some(value) => match serde_json::from_value(value) {
            Ok(parsed) => parsed,
            Err(err) => return ControlResponse::error(id, format!("invalid params: {err}")),
        },
        None => return ControlResponse::error(id, "missing params".into()),
    };
    let snapshot = match state.debug.snapshot() {
        Some(snapshot) => snapshot,
        None => return ControlResponse::error(id, "no snapshot available".into()),
    };
    let frame_id = params.frame_id.map(crate::memory::FrameId);
    if let Some(frame_id) = frame_id {
        if !snapshot
            .storage
            .frames()
            .iter()
            .any(|frame| frame.id == frame_id)
        {
            return ControlResponse::error(id, "unknown frame id".into());
        }
    }
    let metadata = match state.metadata.lock() {
        Ok(guard) => guard,
        Err(_) => return ControlResponse::error(id, "metadata unavailable".into()),
    };
    let using = frame_id
        .and_then(|frame_id| metadata.using_for_frame(&snapshot.storage, frame_id))
        .unwrap_or_default();
    let mut registry = metadata.registry().clone();
    let expr = match crate::harness::parse_debug_expression(
        &params.expression,
        &mut registry,
        metadata.profile(),
        &using,
    ) {
        Ok(expr) => expr,
        Err(err) => return ControlResponse::error(id, err.to_string()),
    };
    let value = match evaluate_with_snapshot(&expr, &registry, frame_id, &snapshot, &using, state) {
        Ok(value) => value,
        Err(err) => return ControlResponse::error(id, err.to_string()),
    };
    let result = crate::debug::dap::format_value(&value);
    let type_name = crate::debug::dap::value_type_name(&value);
    ControlResponse::ok(
        id,
        json!({
            "result": result,
            "type": type_name,
            "variables_reference": 0,
        }),
    )
}

fn handle_debug_breakpoint_locations(
    id: u64,
    params: Option<serde_json::Value>,
    state: &ControlState,
) -> ControlResponse {
    let params: DebugBreakpointLocationsParams = match params {
        Some(value) => match serde_json::from_value(value) {
            Ok(parsed) => parsed,
            Err(err) => return ControlResponse::error(id, format!("invalid params: {err}")),
        },
        None => return ControlResponse::error(id, "missing params".into()),
    };
    let path = PathBuf::from(params.source);
    let file_id = match state.sources.file_id_for_path(&path) {
        Some(id) => id,
        None => return ControlResponse::error(id, "unknown source path".into()),
    };
    let source_text = match state.sources.source_text(file_id) {
        Some(text) => text,
        None => return ControlResponse::error(id, "source text not loaded".into()),
    };
    let metadata = match state.metadata.lock() {
        Ok(guard) => guard,
        Err(_) => return ControlResponse::error(id, "metadata unavailable".into()),
    };
    let mut breakpoints = Vec::new();
    if let Some(locations) = metadata.statement_locations(file_id) {
        let max_line = params.end_line.unwrap_or(params.line);
        for location in locations {
            let (loc_line, loc_col) = location_to_line_col(source_text, location);
            if loc_line < params.line || loc_line > max_line {
                continue;
            }
            if let Some(min_col) = params.column {
                if loc_line == params.line && loc_col < min_col {
                    continue;
                }
            }
            if let Some(max_col) = params.end_column {
                if loc_line == max_line && loc_col > max_col {
                    continue;
                }
            }
            breakpoints.push(json!({ "line": loc_line, "column": loc_col }));
        }
    }
    ControlResponse::ok(id, json!({ "breakpoints": breakpoints }))
}

fn debug_stop_to_json(
    stop: crate::debug::DebugStop,
    state: &ControlState,
) -> Option<serde_json::Value> {
    let reason = match stop.reason {
        crate::debug::DebugStopReason::Breakpoint => "breakpoint",
        crate::debug::DebugStopReason::Step => "step",
        crate::debug::DebugStopReason::Pause => "pause",
        crate::debug::DebugStopReason::Entry => "entry",
    };
    let mut payload = json!({
        "reason": reason,
        "thread_id": stop.thread_id,
        "breakpoint_generation": stop.breakpoint_generation,
    });
    if let Some(location) = stop.location {
        if let Some(text) = state.sources.source_text(location.file_id) {
            let (line, column) = location_to_line_col(text, &location);
            if let Some(source) = state
                .sources
                .files()
                .iter()
                .find(|file| file.id == location.file_id)
            {
                if let Some(obj) = payload.as_object_mut() {
                    obj.insert("file_id".to_string(), json!(location.file_id));
                    obj.insert("line".to_string(), json!(line));
                    obj.insert("column".to_string(), json!(column));
                    obj.insert(
                        "path".to_string(),
                        json!(source.path.to_string_lossy().to_string()),
                    );
                }
            }
        }
    }
    Some(payload)
}

fn location_to_source(
    location: &crate::debug::SourceLocation,
    state: &ControlState,
) -> Option<(DebugSource, u32, u32)> {
    let source_text = state.sources.source_text(location.file_id)?;
    let (line, column) = location_to_line_col(source_text, location);
    let source = state
        .sources
        .files()
        .iter()
        .find(|file| file.id == location.file_id)?;
    let name = source
        .path
        .file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.to_string());
    let path = Some(source.path.to_string_lossy().to_string());
    Some((DebugSource { name, path }, line, column))
}

fn location_to_stack_frame(
    frame_id: u32,
    frame_name: &str,
    location: &crate::debug::SourceLocation,
    state: &ControlState,
) -> Option<serde_json::Value> {
    let (source, line, column) = location_to_source(location, state)?;
    Some(json!({
        "id": frame_id,
        "name": frame_name,
        "source": source,
        "line": line,
        "column": column,
    }))
}

fn evaluate_with_snapshot(
    expr: &crate::eval::expr::Expr,
    registry: &trust_hir::types::TypeRegistry,
    frame_id: Option<crate::memory::FrameId>,
    snapshot: &crate::debug::DebugSnapshot,
    using: &[smol_str::SmolStr],
    state: &ControlState,
) -> Result<Value, RuntimeError> {
    let metadata = state
        .metadata
        .lock()
        .map_err(|_| RuntimeError::ControlError("metadata unavailable".into()))?;
    let profile = metadata.profile();
    let now = snapshot.now;
    let functions = metadata.functions();
    let stdlib = metadata.stdlib();
    let function_blocks = metadata.function_blocks();
    let classes = metadata.classes();
    let access = metadata.access_map();

    let mut storage = snapshot.storage.clone();
    let eval = |storage: &mut crate::memory::VariableStorage,
                instance_id: Option<crate::memory::InstanceId>|
     -> Result<Value, RuntimeError> {
        let mut ctx = crate::eval::EvalContext {
            storage,
            registry,
            profile,
            now,
            debug: None,
            call_depth: 0,
            functions: Some(functions),
            stdlib: Some(stdlib),
            function_blocks: Some(function_blocks),
            classes: Some(classes),
            using: if using.is_empty() { None } else { Some(using) },
            access: Some(access),
            current_instance: instance_id,
            return_name: None,
            loop_depth: 0,
            pause_requested: false,
            execution_deadline: None,
        };
        crate::eval::eval_expr(&mut ctx, expr)
    };

    if let Some(frame_id) = frame_id {
        storage
            .with_frame(frame_id, |storage| {
                let instance_id = storage.current_frame().and_then(|frame| frame.instance_id);
                eval(storage, instance_id)
            })
            .ok_or(RuntimeError::InvalidFrame(frame_id.0))?
    } else {
        eval(&mut storage, None)
    }
}

fn handle_breakpoints_set(
    id: u64,
    params: Option<serde_json::Value>,
    state: &ControlState,
) -> ControlResponse {
    if state.sources.is_empty() {
        return ControlResponse::error(id, "no sources registered".into());
    }
    let params: BreakpointsParams = match params {
        Some(value) => match serde_json::from_value(value) {
            Ok(parsed) => parsed,
            Err(err) => return ControlResponse::error(id, format!("invalid params: {err}")),
        },
        None => return ControlResponse::error(id, "missing params".into()),
    };
    let path = PathBuf::from(params.source);
    let file_id = match state.sources.file_id_for_path(&path) {
        Some(id) => id,
        None => return ControlResponse::error(id, "unknown source path".into()),
    };
    let source_text = match state.sources.source_text(file_id) {
        Some(text) => text,
        None => return ControlResponse::error(id, "source text not loaded".into()),
    };
    let metadata = match state.metadata.lock() {
        Ok(guard) => guard,
        Err(_) => return ControlResponse::error(id, "metadata unavailable".into()),
    };
    let mut breakpoints = Vec::new();
    let mut resolved = Vec::new();
    for line in params.lines {
        if let Some((location, resolved_line, resolved_col)) =
            metadata.resolve_breakpoint_position(source_text, file_id, line, 1)
        {
            breakpoints.push(DebugBreakpoint::new(location));
            resolved.push(json!({"line": resolved_line, "column": resolved_col}));
        }
    }
    state.debug.set_breakpoints_for_file(file_id, breakpoints);
    let generation = state.debug.breakpoint_generation(file_id);
    ControlResponse::ok(
        id,
        json!({
            "status": "ok",
            "file_id": file_id,
            "resolved": resolved,
            "generation": generation,
        }),
    )
}

fn handle_breakpoints_clear(
    id: u64,
    params: Option<serde_json::Value>,
    state: &ControlState,
) -> ControlResponse {
    let params: BreakpointsParams = match params {
        Some(value) => match serde_json::from_value(value) {
            Ok(parsed) => parsed,
            Err(err) => return ControlResponse::error(id, format!("invalid params: {err}")),
        },
        None => return ControlResponse::error(id, "missing params".into()),
    };
    let path = PathBuf::from(params.source);
    let file_id = match state.sources.file_id_for_path(&path) {
        Some(id) => id,
        None => return ControlResponse::error(id, "unknown source path".into()),
    };
    state.debug.set_breakpoints_for_file(file_id, Vec::new());
    ControlResponse::ok(id, json!({"status": "cleared"}))
}

fn handle_breakpoints_list(id: u64, state: &ControlState) -> ControlResponse {
    let breakpoints = state
        .debug
        .breakpoints()
        .into_iter()
        .map(|bp| {
            json!({
                "file_id": bp.location.file_id,
                "start": bp.location.start,
                "end": bp.location.end,
            })
        })
        .collect::<Vec<_>>();
    ControlResponse::ok(id, json!({ "breakpoints": breakpoints }))
}

fn handle_breakpoints_clear_all(id: u64, state: &ControlState) -> ControlResponse {
    state.debug.clear_breakpoints();
    ControlResponse::ok(id, json!({ "status": "cleared" }))
}

fn handle_breakpoints_clear_id(
    id: u64,
    params: Option<serde_json::Value>,
    state: &ControlState,
) -> ControlResponse {
    let params: BreakpointsClearIdParams = match params {
        Some(value) => match serde_json::from_value(value) {
            Ok(parsed) => parsed,
            Err(err) => return ControlResponse::error(id, format!("invalid params: {err}")),
        },
        None => return ControlResponse::error(id, "missing params".into()),
    };
    if state.sources.source_text(params.file_id).is_none() {
        return ControlResponse::error(id, "unknown file id".into());
    }
    state
        .debug
        .set_breakpoints_for_file(params.file_id, Vec::new());
    ControlResponse::ok(
        id,
        json!({ "status": "cleared", "file_id": params.file_id }),
    )
}

fn handle_io_read(id: u64, state: &ControlState) -> ControlResponse {
    let snapshot = state
        .io_snapshot
        .lock()
        .ok()
        .and_then(|guard| guard.clone());
    ControlResponse::ok(
        id,
        json!({
            "snapshot": snapshot.map(|snap| snap.into_json())
        }),
    )
}

fn handle_io_write(
    id: u64,
    params: Option<serde_json::Value>,
    state: &ControlState,
) -> ControlResponse {
    let params: IoWriteParams = match params {
        Some(value) => match serde_json::from_value(value) {
            Ok(parsed) => parsed,
            Err(err) => return ControlResponse::error(id, format!("invalid params: {err}")),
        },
        None => return ControlResponse::error(id, "missing params".into()),
    };
    let address = match IoAddress::parse(&params.address) {
        Ok(addr) => addr,
        Err(err) => return ControlResponse::error(id, err.to_string()),
    };
    let value = match parse_value(&params.value) {
        Ok(value) => value,
        Err(err) => return ControlResponse::error(id, err.to_string()),
    };
    state.debug.enqueue_io_write(address, value);
    ControlResponse::ok(id, json!({"status": "queued"}))
}

fn handle_io_force(
    id: u64,
    params: Option<serde_json::Value>,
    state: &ControlState,
) -> ControlResponse {
    let params: IoWriteParams = match params {
        Some(value) => match serde_json::from_value(value) {
            Ok(parsed) => parsed,
            Err(err) => return ControlResponse::error(id, format!("invalid params: {err}")),
        },
        None => return ControlResponse::error(id, "missing params".into()),
    };
    let address = match IoAddress::parse(&params.address) {
        Ok(addr) => addr,
        Err(err) => return ControlResponse::error(id, err.to_string()),
    };
    let value = match parse_value(&params.value) {
        Ok(value) => value,
        Err(err) => return ControlResponse::error(id, err.to_string()),
    };
    state.debug.force_io(address, value);
    ControlResponse::ok(id, json!({"status": "forced"}))
}

fn handle_io_unforce(
    id: u64,
    params: Option<serde_json::Value>,
    state: &ControlState,
) -> ControlResponse {
    let params: IoAddressParams = match params {
        Some(value) => match serde_json::from_value(value) {
            Ok(parsed) => parsed,
            Err(err) => return ControlResponse::error(id, format!("invalid params: {err}")),
        },
        None => return ControlResponse::error(id, "missing params".into()),
    };
    let address = match IoAddress::parse(&params.address) {
        Ok(addr) => addr,
        Err(err) => return ControlResponse::error(id, err.to_string()),
    };
    state.debug.release_io(&address);
    ControlResponse::ok(id, json!({"status": "released"}))
}

fn handle_eval(
    id: u64,
    params: Option<serde_json::Value>,
    state: &ControlState,
) -> ControlResponse {
    let params: EvalParams = match params {
        Some(value) => match serde_json::from_value(value) {
            Ok(parsed) => parsed,
            Err(err) => return ControlResponse::error(id, format!("invalid params: {err}")),
        },
        None => return ControlResponse::error(id, "missing params".into()),
    };
    let snapshot = match state.debug.snapshot() {
        Some(snapshot) => snapshot,
        None => return ControlResponse::error(id, "no snapshot available".into()),
    };
    let name = params.expr.trim();
    let value = snapshot
        .storage
        .get_global(name)
        .cloned()
        .or_else(|| snapshot.storage.get_retain(name).cloned());
    match value {
        Some(value) => ControlResponse::ok(id, json!({ "value": format!("{value:?}") })),
        None => ControlResponse::error(id, "unknown identifier".into()),
    }
}

fn handle_set(id: u64, params: Option<serde_json::Value>, state: &ControlState) -> ControlResponse {
    let params: SetParams = match params {
        Some(value) => match serde_json::from_value(value) {
            Ok(parsed) => parsed,
            Err(err) => return ControlResponse::error(id, format!("invalid params: {err}")),
        },
        None => return ControlResponse::error(id, "missing params".into()),
    };
    let value = match parse_value(&params.value) {
        Ok(value) => value,
        Err(err) => return ControlResponse::error(id, err.to_string()),
    };
    if let Some(name) = params.target.strip_prefix("global:") {
        state.debug.enqueue_global_write(name.trim(), value);
        return ControlResponse::ok(id, json!({"status": "queued"}));
    }
    if let Some(name) = params.target.strip_prefix("retain:") {
        state.debug.enqueue_retain_write(name.trim(), value);
        return ControlResponse::ok(id, json!({"status": "queued"}));
    }
    ControlResponse::error(id, "unsupported target".into())
}

fn parse_var_target(target: &str) -> Result<VarTarget, String> {
    if let Some(name) = target.strip_prefix("global:") {
        if name.trim().is_empty() {
            return Err("missing global name".into());
        }
        return Ok(VarTarget::Global(name.trim().to_string()));
    }
    if let Some(name) = target.strip_prefix("retain:") {
        if name.trim().is_empty() {
            return Err("missing retain name".into());
        }
        return Ok(VarTarget::Retain(name.trim().to_string()));
    }
    if let Some(rest) = target.strip_prefix("instance:") {
        let mut parts = rest.splitn(2, ':');
        let id = parts
            .next()
            .and_then(|value| value.parse::<u32>().ok())
            .ok_or_else(|| "invalid instance id".to_string())?;
        let name = parts.next().unwrap_or("").trim();
        if name.is_empty() {
            return Err("missing instance name".into());
        }
        return Ok(VarTarget::Instance(id, name.to_string()));
    }
    Err("unsupported target (use global:<name> or retain:<name>)".into())
}

fn handle_var_force(
    id: u64,
    params: Option<serde_json::Value>,
    state: &ControlState,
) -> ControlResponse {
    let params: VarForceParams = match params {
        Some(value) => match serde_json::from_value(value) {
            Ok(parsed) => parsed,
            Err(err) => return ControlResponse::error(id, format!("invalid params: {err}")),
        },
        None => return ControlResponse::error(id, "missing params".into()),
    };
    let target = match parse_var_target(&params.target) {
        Ok(target) => target,
        Err(err) => return ControlResponse::error(id, err),
    };
    let value = match parse_value(&params.value) {
        Ok(value) => value,
        Err(err) => return ControlResponse::error(id, err.to_string()),
    };
    match target {
        VarTarget::Global(name) => state.debug.force_global(name, value),
        VarTarget::Retain(name) => state.debug.force_retain(name, value),
        VarTarget::Instance(id, name) => {
            state
                .debug
                .force_instance(crate::memory::InstanceId(id), name, value)
        }
    }
    ControlResponse::ok(id, json!({ "status": "forced" }))
}

fn handle_var_unforce(
    id: u64,
    params: Option<serde_json::Value>,
    state: &ControlState,
) -> ControlResponse {
    let params: VarTargetParams = match params {
        Some(value) => match serde_json::from_value(value) {
            Ok(parsed) => parsed,
            Err(err) => return ControlResponse::error(id, format!("invalid params: {err}")),
        },
        None => return ControlResponse::error(id, "missing params".into()),
    };
    let target = match parse_var_target(&params.target) {
        Ok(target) => target,
        Err(err) => return ControlResponse::error(id, err),
    };
    match target {
        VarTarget::Global(name) => state.debug.release_global(&name),
        VarTarget::Retain(name) => state.debug.release_retain(&name),
        VarTarget::Instance(id, name) => state
            .debug
            .release_instance(crate::memory::InstanceId(id), &name),
    }
    ControlResponse::ok(id, json!({ "status": "released" }))
}

fn handle_var_forced(id: u64, state: &ControlState) -> ControlResponse {
    let snapshot = state.debug.forced_snapshot();
    let vars = snapshot
        .vars
        .into_iter()
        .map(|entry| {
            let target = match entry.target {
                crate::debug::ForcedVarTarget::Global(name) => {
                    format!("global:{name}")
                }
                crate::debug::ForcedVarTarget::Retain(name) => {
                    format!("retain:{name}")
                }
                crate::debug::ForcedVarTarget::Instance(id, name) => {
                    format!("instance:{}:{name}", id.0)
                }
            };
            json!({
                "target": target,
                "value": crate::debug::dap::format_value(&entry.value),
            })
        })
        .collect::<Vec<_>>();
    ControlResponse::ok(id, json!({ "vars": vars }))
}

fn handle_shutdown(id: u64, state: &ControlState) -> ControlResponse {
    state.resource.stop();
    ControlResponse::ok(id, json!({"status": "stopping"}))
}

fn handle_restart(
    id: u64,
    params: Option<serde_json::Value>,
    state: &ControlState,
) -> ControlResponse {
    let params: RestartParams = match params {
        Some(value) => match serde_json::from_value(value) {
            Ok(parsed) => parsed,
            Err(err) => return ControlResponse::error(id, format!("invalid params: {err}")),
        },
        None => return ControlResponse::error(id, "missing params".into()),
    };
    let mode = match params.mode.to_ascii_lowercase().as_str() {
        "cold" => RestartMode::Cold,
        "warm" => RestartMode::Warm,
        _ => return ControlResponse::error(id, "invalid restart mode".into()),
    };
    if let Ok(mut guard) = state.pending_restart.lock() {
        *guard = Some(mode);
    }
    ControlResponse::ok(id, json!({"status": "restart queued"}))
}

fn handle_bytecode_reload(
    id: u64,
    params: Option<serde_json::Value>,
    state: &ControlState,
) -> ControlResponse {
    let params: BytecodeReloadParams = match params {
        Some(value) => match serde_json::from_value(value) {
            Ok(parsed) => parsed,
            Err(err) => return ControlResponse::error(id, format!("invalid params: {err}")),
        },
        None => return ControlResponse::error(id, "missing params".into()),
    };
    let bytes = match BASE64_STANDARD.decode(params.bytes.as_bytes()) {
        Ok(bytes) => bytes,
        Err(err) => return ControlResponse::error(id, format!("invalid bytecode: {err}")),
    };
    let (tx, rx) = std::sync::mpsc::channel();
    if let Err(err) = state
        .resource
        .send_command(ResourceCommand::ReloadBytecode {
            bytes,
            respond_to: tx,
        })
    {
        return ControlResponse::error(id, err.to_string());
    }
    match rx.recv_timeout(std::time::Duration::from_secs(5)) {
        Ok(Ok(metadata)) => {
            if let Ok(mut guard) = state.metadata.lock() {
                *guard = metadata;
            }
            ControlResponse::ok(id, json!({ "status": "reloaded" }))
        }
        Ok(Err(err)) => ControlResponse::error(id, err.to_string()),
        Err(_) => ControlResponse::error(id, "reload timeout".into()),
    }
}

fn handle_pair_start(id: u64, state: &ControlState) -> ControlResponse {
    let Some(store) = state.pairing.as_ref() else {
        return ControlResponse::error(id, "pairing unavailable".into());
    };
    let code = store.start_pairing();
    ControlResponse::ok(
        id,
        json!({ "code": code.code, "expires_at": code.expires_at }),
    )
}

fn handle_pair_claim(
    id: u64,
    params: Option<serde_json::Value>,
    state: &ControlState,
) -> ControlResponse {
    let params: PairClaimParams = match params {
        Some(value) => match serde_json::from_value(value) {
            Ok(parsed) => parsed,
            Err(err) => return ControlResponse::error(id, format!("invalid params: {err}")),
        },
        None => return ControlResponse::error(id, "missing params".into()),
    };
    let Some(store) = state.pairing.as_ref() else {
        return ControlResponse::error(id, "pairing unavailable".into());
    };
    let requested_role = match params.role.as_deref() {
        Some(text) => match AccessRole::parse(text) {
            Some(role) => Some(role),
            None => return ControlResponse::error(id, "invalid role".into()),
        },
        None => None,
    };
    match store.claim(&params.code, requested_role) {
        Some(token) => ControlResponse::ok(id, json!({ "token": token })),
        None => ControlResponse::error(id, "invalid or expired code".into()),
    }
}

fn handle_pair_list(id: u64, state: &ControlState) -> ControlResponse {
    let Some(store) = state.pairing.as_ref() else {
        return ControlResponse::error(id, "pairing unavailable".into());
    };
    let tokens = store.list();
    ControlResponse::ok(id, json!({ "tokens": tokens }))
}

fn handle_pair_revoke(
    id: u64,
    params: Option<serde_json::Value>,
    state: &ControlState,
) -> ControlResponse {
    let params: PairRevokeParams = match params {
        Some(value) => match serde_json::from_value(value) {
            Ok(parsed) => parsed,
            Err(err) => return ControlResponse::error(id, format!("invalid params: {err}")),
        },
        None => return ControlResponse::error(id, "missing params".into()),
    };
    let Some(store) = state.pairing.as_ref() else {
        return ControlResponse::error(id, "pairing unavailable".into());
    };
    if params.id == "all" {
        let count = store.revoke_all();
        return ControlResponse::ok(id, json!({ "status": "revoked", "count": count }));
    }
    if store.revoke(&params.id) {
        ControlResponse::ok(id, json!({ "status": "revoked", "id": params.id }))
    } else {
        ControlResponse::error(id, "unknown token id".into())
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

fn parse_hmi_write_value(value: &serde_json::Value, template: &Value) -> Option<Value> {
    let parsed = match (value, template) {
        (serde_json::Value::Bool(value), Value::Bool(_)) => Some(Value::Bool(*value)),
        (serde_json::Value::Number(value), Value::SInt(_)) => {
            Some(Value::SInt(i8::try_from(value.as_i64()?).ok()?))
        }
        (serde_json::Value::Number(value), Value::Int(_)) => {
            Some(Value::Int(i16::try_from(value.as_i64()?).ok()?))
        }
        (serde_json::Value::Number(value), Value::DInt(_)) => {
            Some(Value::DInt(i32::try_from(value.as_i64()?).ok()?))
        }
        (serde_json::Value::Number(value), Value::LInt(_)) => Some(Value::LInt(value.as_i64()?)),
        (serde_json::Value::Number(value), Value::USInt(_)) => {
            Some(Value::USInt(u8::try_from(value.as_u64()?).ok()?))
        }
        (serde_json::Value::Number(value), Value::UInt(_)) => {
            Some(Value::UInt(u16::try_from(value.as_u64()?).ok()?))
        }
        (serde_json::Value::Number(value), Value::UDInt(_)) => {
            Some(Value::UDInt(u32::try_from(value.as_u64()?).ok()?))
        }
        (serde_json::Value::Number(value), Value::ULInt(_)) => Some(Value::ULInt(value.as_u64()?)),
        (serde_json::Value::Number(value), Value::Byte(_)) => {
            Some(Value::Byte(u8::try_from(value.as_u64()?).ok()?))
        }
        (serde_json::Value::Number(value), Value::Word(_)) => {
            Some(Value::Word(u16::try_from(value.as_u64()?).ok()?))
        }
        (serde_json::Value::Number(value), Value::DWord(_)) => {
            Some(Value::DWord(u32::try_from(value.as_u64()?).ok()?))
        }
        (serde_json::Value::Number(value), Value::LWord(_)) => Some(Value::LWord(value.as_u64()?)),
        (serde_json::Value::Number(value), Value::Real(_)) => {
            Some(Value::Real(value.as_f64()? as f32))
        }
        (serde_json::Value::Number(value), Value::LReal(_)) => Some(Value::LReal(value.as_f64()?)),
        (serde_json::Value::String(value), Value::String(_)) => {
            Some(Value::String(SmolStr::new(value)))
        }
        (serde_json::Value::String(value), Value::WString(_)) => {
            Some(Value::WString(value.clone()))
        }
        (serde_json::Value::String(value), Value::Char(_)) => {
            Some(Value::Char(single_u8_char(value)?))
        }
        (serde_json::Value::String(value), Value::WChar(_)) => Some(Value::WChar(
            u16::try_from(single_char(value)? as u32).ok()?,
        )),
        (serde_json::Value::String(text), _) => parse_hmi_write_from_text(text, template),
        _ => None,
    }?;
    Some(parsed)
}

fn parse_hmi_write_from_text(text: &str, template: &Value) -> Option<Value> {
    let trimmed = text.trim();
    match template {
        Value::Bool(_) => match trimmed.to_ascii_uppercase().as_str() {
            "TRUE" => Some(Value::Bool(true)),
            "FALSE" => Some(Value::Bool(false)),
            _ => None,
        },
        Value::SInt(_) => Some(Value::SInt(
            i8::try_from(trimmed.parse::<i64>().ok()?).ok()?,
        )),
        Value::Int(_) => Some(Value::Int(
            i16::try_from(trimmed.parse::<i64>().ok()?).ok()?,
        )),
        Value::DInt(_) => Some(Value::DInt(
            i32::try_from(trimmed.parse::<i64>().ok()?).ok()?,
        )),
        Value::LInt(_) => Some(Value::LInt(trimmed.parse::<i64>().ok()?)),
        Value::USInt(_) => Some(Value::USInt(
            u8::try_from(trimmed.parse::<u64>().ok()?).ok()?,
        )),
        Value::UInt(_) => Some(Value::UInt(
            u16::try_from(trimmed.parse::<u64>().ok()?).ok()?,
        )),
        Value::UDInt(_) => Some(Value::UDInt(
            u32::try_from(trimmed.parse::<u64>().ok()?).ok()?,
        )),
        Value::ULInt(_) => Some(Value::ULInt(trimmed.parse::<u64>().ok()?)),
        Value::Byte(_) => Some(Value::Byte(
            u8::try_from(trimmed.parse::<u64>().ok()?).ok()?,
        )),
        Value::Word(_) => Some(Value::Word(
            u16::try_from(trimmed.parse::<u64>().ok()?).ok()?,
        )),
        Value::DWord(_) => Some(Value::DWord(
            u32::try_from(trimmed.parse::<u64>().ok()?).ok()?,
        )),
        Value::LWord(_) => Some(Value::LWord(trimmed.parse::<u64>().ok()?)),
        Value::Real(_) => Some(Value::Real(trimmed.parse::<f32>().ok()?)),
        Value::LReal(_) => Some(Value::LReal(trimmed.parse::<f64>().ok()?)),
        Value::String(_) => Some(Value::String(SmolStr::new(trimmed))),
        Value::WString(_) => Some(Value::WString(trimmed.to_string())),
        Value::Char(_) => Some(Value::Char(single_u8_char(trimmed)?)),
        Value::WChar(_) => Some(Value::WChar(
            u16::try_from(single_char(trimmed)? as u32).ok()?,
        )),
        _ => None,
    }
}

fn single_char(value: &str) -> Option<char> {
    let mut chars = value.chars();
    let first = chars.next()?;
    if chars.next().is_some() {
        return None;
    }
    Some(first)
}

fn single_u8_char(value: &str) -> Option<u8> {
    let ch = single_char(value)?;
    u8::try_from(ch as u32).ok()
}

#[derive(Debug, Deserialize)]
struct ControlRequest {
    id: u64,
    #[serde(rename = "type")]
    r#type: String,
    params: Option<serde_json::Value>,
    auth: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct ControlResponse {
    id: u64,
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

impl ControlResponse {
    fn ok(id: u64, result: serde_json::Value) -> Self {
        Self {
            id,
            ok: true,
            result: Some(result),
            error: None,
        }
    }

    fn error(id: u64, error: String) -> Self {
        Self {
            id,
            ok: false,
            result: None,
            error: Some(error),
        }
    }
}

#[derive(Debug, Deserialize)]
struct BreakpointsParams {
    source: String,
    lines: Vec<u32>,
}

#[derive(Debug, Deserialize)]
struct BreakpointsClearIdParams {
    file_id: u32,
}

#[derive(Debug, Deserialize)]
struct DebugScopesParams {
    frame_id: u32,
}

#[derive(Debug, Deserialize)]
struct DebugVariablesParams {
    variables_reference: u32,
}

#[derive(Debug, Deserialize)]
struct DebugEvaluateParams {
    expression: String,
    frame_id: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct DebugBreakpointLocationsParams {
    source: String,
    line: u32,
    end_line: Option<u32>,
    column: Option<u32>,
    end_column: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct HmiValuesParams {
    ids: Option<Vec<String>>,
}

#[derive(Debug, Default, Deserialize)]
struct HmiTrendsParams {
    ids: Option<Vec<String>>,
    duration_ms: Option<u64>,
    buckets: Option<usize>,
}

#[derive(Debug, Default, Deserialize)]
struct HmiAlarmsParams {
    limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct HmiAlarmAckParams {
    id: String,
}

#[derive(Debug, Deserialize)]
struct HmiWriteParams {
    #[serde(alias = "path", alias = "target")]
    id: String,
    value: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct HmiDescriptorUpdateParams {
    descriptor: crate::hmi::HmiDirDescriptor,
}

#[derive(Debug, Default, Deserialize)]
struct HmiScaffoldResetParams {
    mode: Option<String>,
    style: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct HistorianQueryParams {
    variable: Option<String>,
    since_ms: Option<u128>,
    limit: Option<usize>,
}

#[derive(Debug, Default, Deserialize)]
struct HistorianAlertsParams {
    limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct IoWriteParams {
    address: String,
    value: String,
}

#[derive(Debug, Deserialize)]
struct IoAddressParams {
    address: String,
}

#[derive(Debug, Deserialize)]
struct RestartParams {
    mode: String,
}

#[derive(Debug, Deserialize)]
struct BytecodeReloadParams {
    bytes: String,
}

#[derive(Debug, Deserialize)]
struct EvalParams {
    expr: String,
}

#[derive(Debug, Deserialize)]
struct SetParams {
    target: String,
    value: String,
}

enum VarTarget {
    Global(String),
    Retain(String),
    Instance(u32, String),
}

#[derive(Deserialize)]
struct VarForceParams {
    target: String,
    value: String,
}

#[derive(Deserialize)]
struct VarTargetParams {
    target: String,
}

#[derive(Debug, Deserialize)]
struct PairClaimParams {
    code: String,
    role: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PairRevokeParams {
    id: String,
}

trait IoSnapshotJson {
    fn into_json(self) -> serde_json::Value;
}

impl IoSnapshotJson for IoSnapshot {
    fn into_json(self) -> serde_json::Value {
        json!({
            "inputs": self.inputs.iter().map(entry_to_json).collect::<Vec<_>>(),
            "outputs": self.outputs.iter().map(entry_to_json).collect::<Vec<_>>(),
            "memory": self.memory.iter().map(entry_to_json).collect::<Vec<_>>(),
        })
    }
}

fn entry_to_json(entry: &crate::io::IoSnapshotEntry) -> serde_json::Value {
    json!({
        "name": entry.name.as_ref().map(|name| name.as_str()),
        "address": format_address(&entry.address),
        "value": format_snapshot_value(&entry.value),
    })
}

fn format_snapshot_value(value: &crate::io::IoSnapshotValue) -> serde_json::Value {
    match value {
        crate::io::IoSnapshotValue::Value(value) => json!(format!("{value:?}")),
        crate::io::IoSnapshotValue::Error(err) => json!({ "error": err }),
        crate::io::IoSnapshotValue::Unresolved => json!("unresolved"),
    }
}

fn format_address(address: &IoAddress) -> String {
    let area = match address.area {
        crate::memory::IoArea::Input => "I",
        crate::memory::IoArea::Output => "Q",
        crate::memory::IoArea::Memory => "M",
    };
    let size = match address.size {
        crate::io::IoSize::Bit => "X",
        crate::io::IoSize::Byte => "B",
        crate::io::IoSize::Word => "W",
        crate::io::IoSize::DWord => "D",
        crate::io::IoSize::LWord => "L",
    };
    if address.wildcard {
        return format!("%{area}{size}*");
    }
    if address.size == crate::io::IoSize::Bit {
        format!("%{area}{size}{}.{}", address.byte, address.bit)
    } else {
        format!("%{area}{size}{}", address.byte)
    }
}

fn runtime_event_to_json(event: crate::debug::RuntimeEvent) -> serde_json::Value {
    match event {
        crate::debug::RuntimeEvent::CycleStart { cycle, time } => json!({
            "type": "cycle_start",
            "cycle": cycle,
            "time_ns": time.as_nanos(),
        }),
        crate::debug::RuntimeEvent::CycleEnd { cycle, time } => json!({
            "type": "cycle_end",
            "cycle": cycle,
            "time_ns": time.as_nanos(),
        }),
        crate::debug::RuntimeEvent::TaskStart {
            name,
            priority,
            time,
        } => json!({
            "type": "task_start",
            "name": name.as_str(),
            "priority": priority,
            "time_ns": time.as_nanos(),
        }),
        crate::debug::RuntimeEvent::TaskEnd {
            name,
            priority,
            time,
        } => json!({
            "type": "task_end",
            "name": name.as_str(),
            "priority": priority,
            "time_ns": time.as_nanos(),
        }),
        crate::debug::RuntimeEvent::TaskOverrun { name, missed, time } => json!({
            "type": "task_overrun",
            "name": name.as_str(),
            "missed": missed,
            "time_ns": time.as_nanos(),
        }),
        crate::debug::RuntimeEvent::Fault { error, time } => json!({
            "type": "fault",
            "error": error,
            "time_ns": time.as_nanos(),
        }),
    }
}

fn io_health_to_json(entry: &IoDriverStatus) -> serde_json::Value {
    match &entry.health {
        IoDriverHealth::Ok => json!({
            "name": entry.name.as_str(),
            "status": "ok",
        }),
        IoDriverHealth::Degraded { error } => json!({
            "name": entry.name.as_str(),
            "status": "degraded",
            "error": error.as_str(),
        }),
        IoDriverHealth::Faulted { error } => json!({
            "name": entry.name.as_str(),
            "status": "faulted",
            "error": error.as_str(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use std::fs;
    use std::path::Path;
    use std::path::PathBuf;
    use std::sync::atomic::AtomicBool;
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

    use indexmap::IndexMap;
    use serde_json::json;

    use crate::debug::{DebugVariableHandles, PendingVarTarget};
    use crate::error::RuntimeError;
    use crate::harness::TestHarness;
    use crate::historian::{AlertRule, HistorianConfig, HistorianService, RecordingMode};
    use crate::metrics::RuntimeMetrics;
    use crate::scheduler::{ResourceCommand, ResourceControl, StdClock};
    use crate::settings::{
        BaseSettings, DiscoverySettings, MeshSettings, RuntimeSettings, SimulationSettings,
        WebSettings,
    };
    use crate::watchdog::{FaultPolicy, RetainMode, WatchdogPolicy};
    use crate::web::pairing::PairingStore;

    fn temp_history_path(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        std::env::temp_dir().join(format!("trust-control-{name}-{stamp}.jsonl"))
    }

    fn temp_dir(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("trust-control-{name}-{stamp}"));
        fs::create_dir_all(&path).expect("create temp dir");
        path
    }

    fn write_file(path: &Path, content: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent");
        }
        fs::write(path, content).expect("write file");
    }

    fn runtime_settings() -> RuntimeSettings {
        RuntimeSettings::new(
            BaseSettings {
                log_level: SmolStr::new("info"),
                watchdog: WatchdogPolicy::default(),
                fault_policy: FaultPolicy::SafeHalt,
                retain_mode: RetainMode::None,
                retain_save_interval: None,
            },
            WebSettings {
                enabled: false,
                listen: SmolStr::new("127.0.0.1:0"),
                auth: SmolStr::new("local"),
                tls: false,
            },
            DiscoverySettings {
                enabled: false,
                service_name: SmolStr::new("truST"),
                advertise: false,
                interfaces: Vec::new(),
            },
            MeshSettings {
                enabled: false,
                listen: SmolStr::new("127.0.0.1:0"),
                tls: false,
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

    fn hmi_test_state(source: &str) -> ControlState {
        let mut harness = TestHarness::from_source(source).expect("build harness");
        let debug = harness.runtime_mut().enable_debug();
        harness.cycle();
        let snapshot = crate::debug::DebugSnapshot {
            storage: harness.runtime().storage().clone(),
            now: harness.runtime().current_time(),
        };

        let (resource, cmd_rx) = ResourceControl::stub(StdClock::new());
        std::thread::spawn(move || {
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
                    ResourceCommand::MeshApply { .. }
                    | ResourceCommand::Pause
                    | ResourceCommand::Resume
                    | ResourceCommand::UpdateWatchdog(_)
                    | ResourceCommand::UpdateFaultPolicy(_)
                    | ResourceCommand::UpdateRetainSaveInterval(_)
                    | ResourceCommand::UpdateIoSafeState(_) => {}
                }
            }
        });
        let sources = SourceRegistry::new(vec![SourceFile {
            id: 1,
            path: std::path::PathBuf::from("main.st"),
            text: source.to_string(),
        }]);
        let hmi_descriptor = Arc::new(Mutex::new(HmiRuntimeDescriptor::from_sources(
            None, &sources,
        )));
        ControlState {
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
            project_root: None,
            resource_name: SmolStr::new("RESOURCE"),
            io_health: Arc::new(Mutex::new(Vec::new())),
            debug_enabled: Arc::new(AtomicBool::new(true)),
            debug_variables: Arc::new(Mutex::new(DebugVariableHandles::new())),
            hmi_live: Arc::new(Mutex::new(crate::hmi::HmiLiveState::default())),
            hmi_descriptor,
            historian: None,
            pairing: None,
        }
    }

    fn set_hmi_project_root(state: &mut ControlState, root: &Path) {
        state.project_root = Some(root.to_path_buf());
        state.hmi_descriptor = Arc::new(Mutex::new(HmiRuntimeDescriptor::from_sources(
            state.project_root.as_deref(),
            &state.sources,
        )));
    }

    fn hmi_schema_result(state: &ControlState) -> serde_json::Value {
        let response =
            handle_request_value(json!({"id": 999, "type": "hmi.schema.get"}), state, None);
        assert!(response.ok, "schema response failed: {:?}", response.error);
        response.result.expect("schema result")
    }

    fn hmi_schema_revision_and_speed_label(state: &ControlState) -> (u64, String) {
        let result = hmi_schema_result(state);
        let revision = result
            .get("schema_revision")
            .and_then(serde_json::Value::as_u64)
            .expect("schema revision");
        let label = result
            .get("widgets")
            .and_then(serde_json::Value::as_array)
            .and_then(|widgets| {
                widgets.iter().find_map(|widget| {
                    if widget.get("path").and_then(serde_json::Value::as_str) == Some("Main.speed")
                    {
                        widget
                            .get("label")
                            .and_then(serde_json::Value::as_str)
                            .map(ToOwned::to_owned)
                    } else {
                        None
                    }
                })
            })
            .expect("speed label");
        (revision, label)
    }

    fn wait_for_schema_revision(
        state: &ControlState,
        min_revision: u64,
        timeout: Duration,
    ) -> (u64, String) {
        let deadline = Instant::now() + timeout;
        loop {
            let current = hmi_schema_revision_and_speed_label(state);
            if current.0 >= min_revision {
                return current;
            }
            if Instant::now() >= deadline {
                panic!(
                    "schema revision did not reach {min_revision}; last seen {:?}",
                    current
                );
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    fn pairing_file(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        std::env::temp_dir().join(format!("trust-pairing-control-{name}-{stamp}.json"))
    }

    #[test]
    fn hmi_schema_contract_includes_required_mapping() {
        let source = r#"
TYPE MODE : (OFF, AUTO); END_TYPE
TYPE POINT :
STRUCT
    X : INT;
    Y : INT;
END_STRUCT
END_TYPE

PROGRAM Main
VAR
    run : BOOL := TRUE;
    speed : REAL := 42.5;
    mode : MODE := MODE#AUTO;
    name : STRING := 'pump';
    nums : ARRAY[1..3] OF INT;
    point : POINT;
END_VAR
nums[1] := 1;
nums[2] := 2;
nums[3] := 3;
point.X := 11;
point.Y := 12;
END_PROGRAM
"#;
        let state = hmi_test_state(source);
        let response =
            handle_request_value(json!({"id": 1, "type": "hmi.schema.get"}), &state, None);
        assert!(
            response.ok,
            "schema response should be ok: {:?}",
            response.error
        );
        let result = response.result.expect("schema result");
        assert_eq!(
            result.get("mode").and_then(serde_json::Value::as_str),
            Some("read_only")
        );
        assert_eq!(
            result
                .get("schema_revision")
                .and_then(serde_json::Value::as_u64),
            Some(0)
        );
        assert_eq!(
            result.get("read_only").and_then(serde_json::Value::as_bool),
            Some(true)
        );
        assert_eq!(
            result
                .get("theme")
                .and_then(|theme| theme.get("style"))
                .and_then(serde_json::Value::as_str),
            Some("classic")
        );
        assert!(result
            .get("pages")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|pages| !pages.is_empty()));

        let widgets = result
            .get("widgets")
            .and_then(serde_json::Value::as_array)
            .expect("widgets array");
        let mut by_path = IndexMap::new();
        for widget in widgets {
            let path = widget
                .get("path")
                .and_then(serde_json::Value::as_str)
                .expect("widget path");
            let kind = widget
                .get("widget")
                .and_then(serde_json::Value::as_str)
                .expect("widget kind");
            by_path.insert(path.to_string(), kind.to_string());
        }

        assert_eq!(
            by_path.get("Main.run").map(String::as_str),
            Some("indicator")
        );
        assert_eq!(by_path.get("Main.speed").map(String::as_str), Some("value"));
        assert_eq!(
            by_path.get("Main.mode").map(String::as_str),
            Some("readout")
        );
        assert_eq!(by_path.get("Main.name").map(String::as_str), Some("text"));
        assert_eq!(by_path.get("Main.nums").map(String::as_str), Some("table"));
        assert_eq!(by_path.get("Main.point").map(String::as_str), Some("tree"));
        let run_widget = widgets
            .iter()
            .find(|widget| {
                widget
                    .get("path")
                    .and_then(serde_json::Value::as_str)
                    .map(|path| path == "Main.run")
                    .unwrap_or(false)
            })
            .expect("run widget");
        assert_eq!(
            run_widget.get("id").and_then(serde_json::Value::as_str),
            Some("resource/RESOURCE/program/Main/field/run")
        );
    }

    #[test]
    fn hmi_values_contract_returns_timestamp_quality_and_typed_values() {
        let source = r#"
TYPE POINT :
STRUCT
    X : INT;
END_STRUCT
END_TYPE

PROGRAM Main
VAR
    run : BOOL := TRUE;
    speed : REAL := 42.5;
    name : STRING := 'pump';
    nums : ARRAY[1..3] OF INT;
    point : POINT;
END_VAR
nums[1] := 1;
nums[2] := 2;
nums[3] := 3;
point.X := 11;
END_PROGRAM
"#;
        let state = hmi_test_state(source);
        let ids = vec![
            "resource/RESOURCE/program/Main/field/run",
            "resource/RESOURCE/program/Main/field/speed",
            "resource/RESOURCE/program/Main/field/name",
            "resource/RESOURCE/program/Main/field/nums",
            "resource/RESOURCE/program/Main/field/point",
        ];
        let response = handle_request_value(
            json!({
                "id": 2,
                "type": "hmi.values.get",
                "params": { "ids": ids }
            }),
            &state,
            None,
        );
        assert!(
            response.ok,
            "values response should be ok: {:?}",
            response.error
        );
        let result = response.result.expect("values result");
        assert_eq!(
            result.get("connected").and_then(serde_json::Value::as_bool),
            Some(true)
        );
        assert!(result
            .get("timestamp_ms")
            .and_then(serde_json::Value::as_u64)
            .is_some());

        let values = result
            .get("values")
            .and_then(serde_json::Value::as_object)
            .expect("values object");
        let run = values
            .get("resource/RESOURCE/program/Main/field/run")
            .expect("run value");
        assert_eq!(
            run.get("q").and_then(serde_json::Value::as_str),
            Some("good")
        );
        assert_eq!(
            run.get("v").and_then(serde_json::Value::as_bool),
            Some(true)
        );

        let speed = values
            .get("resource/RESOURCE/program/Main/field/speed")
            .expect("speed value");
        assert!(speed.get("v").and_then(serde_json::Value::as_f64).is_some());

        let name = values
            .get("resource/RESOURCE/program/Main/field/name")
            .expect("name value");
        assert_eq!(
            name.get("v").and_then(serde_json::Value::as_str),
            Some("pump")
        );

        let nums = values
            .get("resource/RESOURCE/program/Main/field/nums")
            .expect("nums value");
        assert_eq!(
            nums.get("v")
                .and_then(serde_json::Value::as_array)
                .map(|values| values.len()),
            Some(3)
        );

        let point = values
            .get("resource/RESOURCE/program/Main/field/point")
            .expect("point value");
        assert!(point
            .get("v")
            .and_then(serde_json::Value::as_object)
            .is_some());
    }

    #[test]
    fn hmi_write_is_disabled_in_read_only_mode() {
        let source = r#"
PROGRAM Main
VAR
    run : BOOL := TRUE;
END_VAR
END_PROGRAM
"#;
        let state = hmi_test_state(source);
        let response = handle_request_value(
            json!({
                "id": 3,
                "type": "hmi.write",
                "params": { "id": "resource/RESOURCE/program/Main/field/run", "value": false }
            }),
            &state,
            None,
        );
        assert!(!response.ok);
        assert_eq!(
            response.error.as_deref(),
            Some("hmi.write disabled in read-only mode")
        );
    }

    #[test]
    fn hmi_write_queues_allowlisted_program_variable_write() {
        let source = r#"
PROGRAM Main
VAR
    run : BOOL := TRUE;
END_VAR
END_PROGRAM
"#;
        let root = temp_dir("hmi-write-program");
        write_file(
            &root.join("hmi.toml"),
            r#"
[write]
enabled = true
allow = ["resource/RESOURCE/program/Main/field/run"]
"#,
        );

        let mut state = hmi_test_state(source);
        set_hmi_project_root(&mut state, &root);

        let response = handle_request_value(
            json!({
                "id": 4,
                "type": "hmi.write",
                "params": {
                    "id": "resource/RESOURCE/program/Main/field/run",
                    "value": false
                }
            }),
            &state,
            None,
        );
        assert!(response.ok, "hmi.write failed: {:?}", response.error);
        let result = response.result.expect("hmi.write result");
        assert_eq!(
            result.get("status").and_then(serde_json::Value::as_str),
            Some("queued")
        );
        assert_eq!(
            result.get("id").and_then(serde_json::Value::as_str),
            Some("resource/RESOURCE/program/Main/field/run")
        );

        let writes = state.debug.drain_var_writes();
        assert_eq!(writes.len(), 1);
        assert_eq!(writes[0].value, Value::Bool(false));
        match &writes[0].target {
            PendingVarTarget::Instance(_, name) => assert_eq!(name.as_str(), "run"),
            other => panic!("expected instance write, got {other:?}"),
        }

        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn hmi_write_supports_path_allowlist_and_alias_param() {
        let source = r#"
PROGRAM Main
VAR
    run : BOOL := TRUE;
END_VAR
END_PROGRAM
"#;
        let root = temp_dir("hmi-write-path");
        write_file(
            &root.join("hmi.toml"),
            r#"
[write]
enabled = true
allow = ["Main.run"]
"#,
        );

        let mut state = hmi_test_state(source);
        set_hmi_project_root(&mut state, &root);

        let response = handle_request_value(
            json!({
                "id": 5,
                "type": "hmi.write",
                "params": {
                    "path": "Main.run",
                    "value": "FALSE"
                }
            }),
            &state,
            None,
        );
        assert!(response.ok, "hmi.write failed: {:?}", response.error);
        let writes = state.debug.drain_var_writes();
        assert_eq!(writes.len(), 1);
        assert_eq!(writes[0].value, Value::Bool(false));

        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn hmi_write_rejects_non_allowlisted_target() {
        let source = r#"
PROGRAM Main
VAR
    run : BOOL := TRUE;
END_VAR
END_PROGRAM
"#;
        let root = temp_dir("hmi-write-denied");
        write_file(
            &root.join("hmi.toml"),
            r#"
[write]
enabled = true
allow = ["resource/RESOURCE/program/Main/field/other"]
"#,
        );

        let mut state = hmi_test_state(source);
        set_hmi_project_root(&mut state, &root);
        let response = handle_request_value(
            json!({
                "id": 6,
                "type": "hmi.write",
                "params": {
                    "id": "resource/RESOURCE/program/Main/field/run",
                    "value": true
                }
            }),
            &state,
            None,
        );
        assert!(!response.ok);
        assert_eq!(
            response.error.as_deref(),
            Some("hmi.write target is not in allowlist")
        );
        assert!(state.debug.drain_var_writes().is_empty());
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn hmi_write_rejects_type_mismatch() {
        let source = r#"
PROGRAM Main
VAR
    run : BOOL := TRUE;
END_VAR
END_PROGRAM
"#;
        let root = temp_dir("hmi-write-type");
        write_file(
            &root.join("hmi.toml"),
            r#"
[write]
enabled = true
allow = ["resource/RESOURCE/program/Main/field/run"]
"#,
        );

        let mut state = hmi_test_state(source);
        set_hmi_project_root(&mut state, &root);
        let response = handle_request_value(
            json!({
                "id": 7,
                "type": "hmi.write",
                "params": {
                    "id": "resource/RESOURCE/program/Main/field/run",
                    "value": 1
                }
            }),
            &state,
            None,
        );
        assert!(!response.ok);
        assert_eq!(
            response.error.as_deref(),
            Some("invalid hmi.write value for target 'resource/RESOURCE/program/Main/field/run'")
        );
        assert!(state.debug.drain_var_writes().is_empty());
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn hmi_write_processing_stays_under_cycle_budget() {
        let source = r#"
PROGRAM Main
VAR
    run : BOOL := TRUE;
END_VAR
END_PROGRAM
"#;
        let root = temp_dir("hmi-write-budget");
        write_file(
            &root.join("hmi.toml"),
            r#"
[write]
enabled = true
allow = ["resource/RESOURCE/program/Main/field/run"]
"#,
        );

        let mut state = hmi_test_state(source);
        set_hmi_project_root(&mut state, &root);

        let writes: u32 = 300;
        let mut max = Duration::ZERO;
        let mut total = Duration::ZERO;
        for index in 0..writes {
            let started = Instant::now();
            let response = handle_request_value(
                json!({
                    "id": 70_u64 + u64::from(index),
                    "type": "hmi.write",
                    "params": {
                        "id": "resource/RESOURCE/program/Main/field/run",
                        "value": index % 2 == 0
                    }
                }),
                &state,
                None,
            );
            let elapsed = started.elapsed();
            assert!(response.ok, "hmi.write failed: {:?}", response.error);
            max = max.max(elapsed);
            total += elapsed;
        }

        let avg = total / writes;
        assert!(
            max < Duration::from_millis(100),
            "max hmi.write latency {:?} exceeded write cycle budget",
            max
        );
        assert!(
            avg < Duration::from_millis(25),
            "avg hmi.write latency {:?} exceeded expected write overhead",
            avg
        );

        let drained = state.debug.drain_var_writes();
        assert!(
            !drained.is_empty(),
            "expected queued writes after budget benchmark loop"
        );
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn hmi_trends_and_alarm_contracts_support_ack_flow() {
        let source = r#"
PROGRAM Main
VAR
    // @hmi(min=0, max=100)
    speed : REAL := 120.0;
END_VAR
END_PROGRAM
"#;
        let state = hmi_test_state(source);

        let trends = handle_request_value(
            json!({
                "id": 10,
                "type": "hmi.trends.get",
                "params": { "duration_ms": 60_000, "buckets": 24 }
            }),
            &state,
            None,
        );
        assert!(trends.ok, "hmi.trends.get failed: {:?}", trends.error);
        let trend_series = trends
            .result
            .as_ref()
            .and_then(|value| value.get("series"))
            .and_then(serde_json::Value::as_array)
            .expect("trend series");
        assert!(!trend_series.is_empty(), "expected trend series");

        let alarms = handle_request_value(
            json!({
                "id": 11,
                "type": "hmi.alarms.get",
                "params": { "limit": 10 }
            }),
            &state,
            None,
        );
        assert!(alarms.ok, "hmi.alarms.get failed: {:?}", alarms.error);
        let active = alarms
            .result
            .as_ref()
            .and_then(|value| value.get("active"))
            .and_then(serde_json::Value::as_array)
            .expect("active alarms");
        assert_eq!(active.len(), 1, "expected one raised alarm");
        let alarm_id = active[0]
            .get("id")
            .and_then(serde_json::Value::as_str)
            .expect("alarm id");

        let ack = handle_request_value(
            json!({
                "id": 12,
                "type": "hmi.alarm.ack",
                "params": { "id": alarm_id }
            }),
            &state,
            None,
        );
        assert!(ack.ok, "hmi.alarm.ack failed: {:?}", ack.error);
        let ack_active = ack
            .result
            .as_ref()
            .and_then(|value| value.get("active"))
            .and_then(serde_json::Value::as_array)
            .expect("ack active alarms");
        assert_eq!(ack_active.len(), 1);
        assert_eq!(
            ack_active[0]
                .get("state")
                .and_then(serde_json::Value::as_str),
            Some("acknowledged")
        );
    }

    #[test]
    fn hmi_descriptor_watcher_updates_schema_without_runtime_restart() {
        let source = r#"
PROGRAM Main
VAR
    speed : REAL := 42.0;
END_VAR
END_PROGRAM
"#;
        let root = temp_dir("hmi-live-refresh");
        write_file(
            &root.join("hmi/overview.toml"),
            r#"
title = "Overview"

[[section]]
title = "Drive"
span = 12

[[section.widget]]
type = "value"
bind = "Main.speed"
label = "Speed A"
"#,
        );

        let mut state = hmi_test_state(source);
        set_hmi_project_root(&mut state, &root);
        let state = Arc::new(state);
        spawn_hmi_descriptor_watcher(state.clone());

        let (initial_revision, initial_label) = hmi_schema_revision_and_speed_label(state.as_ref());
        assert_eq!(initial_revision, 0);
        assert_eq!(initial_label, "Speed A");

        write_file(
            &root.join("hmi/overview.toml"),
            r#"
title = "Overview"

[[section]]
title = "Drive"
span = 12

[[section.widget]]
type = "value"
bind = "Main.speed"
label = "Speed B"
"#,
        );

        let (revision, label) = wait_for_schema_revision(state.as_ref(), 1, Duration::from_secs(5));
        assert_eq!(revision, 1);
        assert_eq!(label, "Speed B");
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn hmi_descriptor_watcher_retains_last_good_schema_on_invalid_toml() {
        let source = r#"
PROGRAM Main
VAR
    speed : REAL := 42.0;
END_VAR
END_PROGRAM
"#;
        let root = temp_dir("hmi-live-invalid");
        write_file(
            &root.join("hmi/overview.toml"),
            r#"
title = "Overview"

[[section]]
title = "Drive"
span = 12

[[section.widget]]
type = "value"
bind = "Main.speed"
label = "Speed A"
"#,
        );

        let mut state = hmi_test_state(source);
        set_hmi_project_root(&mut state, &root);
        let state = Arc::new(state);
        spawn_hmi_descriptor_watcher(state.clone());

        let (initial_revision, initial_label) = hmi_schema_revision_and_speed_label(state.as_ref());
        assert_eq!(initial_revision, 0);
        assert_eq!(initial_label, "Speed A");

        write_file(
            &root.join("hmi/overview.toml"),
            r#"
title = "Overview"

[[section]]
title = "Drive"
span = "wide"
"#,
        );

        std::thread::sleep(Duration::from_millis(600));
        let (revision_after_invalid, label_after_invalid) =
            hmi_schema_revision_and_speed_label(state.as_ref());
        assert_eq!(revision_after_invalid, 0);
        assert_eq!(label_after_invalid, "Speed A");
        let invalid_schema = hmi_schema_result(state.as_ref());
        assert!(
            invalid_schema
                .get("descriptor_error")
                .and_then(serde_json::Value::as_str)
                .is_some(),
            "descriptor_error should be present after invalid descriptor update"
        );

        write_file(
            &root.join("hmi/overview.toml"),
            r#"
title = "Overview"

[[section]]
title = "Drive"
span = 12

[[section.widget]]
type = "value"
bind = "Main.speed"
label = "Speed C"
"#,
        );

        let (revision_after_fix, label_after_fix) =
            wait_for_schema_revision(state.as_ref(), 1, Duration::from_secs(5));
        assert_eq!(revision_after_fix, 1);
        assert_eq!(label_after_fix, "Speed C");
        let fixed_schema = hmi_schema_result(state.as_ref());
        assert!(
            fixed_schema.get("descriptor_error").is_none(),
            "descriptor_error should clear after descriptor recovers"
        );
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn hmi_descriptor_watcher_handles_rapid_file_changes_without_deadlock() {
        let source = r#"
PROGRAM Main
VAR
    speed : REAL := 42.0;
END_VAR
END_PROGRAM
"#;
        let root = temp_dir("hmi-live-rapid");
        write_file(
            &root.join("hmi/overview.toml"),
            r#"
title = "Overview"

[[section]]
title = "Drive"
span = 12

[[section.widget]]
type = "value"
bind = "Main.speed"
label = "Speed A"
"#,
        );

        let mut state = hmi_test_state(source);
        set_hmi_project_root(&mut state, &root);
        let state = Arc::new(state);
        spawn_hmi_descriptor_watcher(state.clone());

        let (initial_revision, initial_label) = hmi_schema_revision_and_speed_label(state.as_ref());
        assert_eq!(initial_revision, 0);
        assert_eq!(initial_label, "Speed A");

        for index in 0..24_u32 {
            if index % 5 == 0 {
                write_file(
                    &root.join("hmi/overview.toml"),
                    r#"
title = "Overview"

[[section]]
title = "Drive"
span = "wide"
"#,
                );
            } else {
                write_file(
                    &root.join("hmi/overview.toml"),
                    format!(
                        r#"
title = "Overview"

[[section]]
title = "Drive"
span = 12

[[section.widget]]
type = "value"
bind = "Main.speed"
label = "Speed {index}"
"#
                    )
                    .as_str(),
                );
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        write_file(
            &root.join("hmi/overview.toml"),
            r#"
title = "Overview"

[[section]]
title = "Drive"
span = 12

[[section.widget]]
type = "value"
bind = "Main.speed"
label = "Speed Final"
"#,
        );

        let (revision_after_churn, label_after_churn) =
            wait_for_schema_revision(state.as_ref(), 1, Duration::from_secs(5));
        assert!(revision_after_churn >= 1);
        assert_eq!(label_after_churn, "Speed Final");

        for id in 0..40_u64 {
            let response = handle_request_value(
                json!({"id": 9_000_u64 + id, "type": "hmi.schema.get"}),
                &state,
                None,
            );
            assert!(
                response.ok,
                "schema request failed during churn: {:?}",
                response.error
            );
        }

        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn hmi_descriptor_get_returns_inferred_layout_when_files_missing() {
        let source = r#"
PROGRAM Main
VAR
    run : BOOL := TRUE;
    speed : REAL := 42.0;
END_VAR
END_PROGRAM
"#;
        let state = hmi_test_state(source);
        let response = handle_request_value(
            json!({"id": 700, "type": "hmi.descriptor.get"}),
            &state,
            None,
        );
        assert!(
            response.ok,
            "hmi.descriptor.get failed: {:?}",
            response.error
        );
        let pages = response
            .result
            .as_ref()
            .and_then(|value| value.get("pages"))
            .and_then(serde_json::Value::as_array)
            .expect("descriptor pages");
        assert!(
            !pages.is_empty(),
            "inferred descriptor should include at least one page"
        );
    }

    #[test]
    fn hmi_descriptor_update_writes_files_and_bumps_schema_revision() {
        let source = r#"
PROGRAM Main
VAR
    speed : REAL := 42.0;
END_VAR
END_PROGRAM
"#;
        let root = temp_dir("hmi-descriptor-update");
        let mut state = hmi_test_state(source);
        set_hmi_project_root(&mut state, &root);

        let response = handle_request_value(
            json!({
                "id": 701,
                "type": "hmi.descriptor.update",
                "params": {
                    "descriptor": {
                        "config": {
                            "theme": { "style": "industrial", "accent": "#22d3ee" },
                            "layout": {},
                            "write": {},
                            "alarm": []
                        },
                        "pages": [
                            {
                                "id": "overview",
                                "title": "Overview",
                                "icon": "activity",
                                "order": 0,
                                "kind": "dashboard",
                                "duration_ms": null,
                                "svg": null,
                                "signals": [],
                                "sections": [
                                    {
                                        "title": "Drive",
                                        "span": 12,
                                        "widgets": [
                                            {
                                                "widget_type": "gauge",
                                                "bind": "Main.speed",
                                                "label": "Speed Updated",
                                                "unit": "rpm",
                                                "min": 0,
                                                "max": 100,
                                                "span": 6,
                                                "on_color": null,
                                                "off_color": null,
                                                "zones": []
                                            }
                                        ]
                                    }
                                ],
                                "bindings": []
                            }
                        ]
                    }
                }
            }),
            &state,
            None,
        );
        assert!(
            response.ok,
            "hmi.descriptor.update failed: {:?}",
            response.error
        );
        let revision = response
            .result
            .as_ref()
            .and_then(|value| value.get("schema_revision"))
            .and_then(serde_json::Value::as_u64)
            .expect("schema revision");
        assert!(revision >= 1, "schema revision should increment");
        assert!(root.join("hmi/_config.toml").is_file());
        assert!(root.join("hmi/overview.toml").is_file());
        let overview = fs::read_to_string(root.join("hmi/overview.toml")).expect("read overview");
        assert!(overview.contains("Speed Updated"));
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn hmi_scaffold_reset_regenerates_required_pages_and_revision() {
        let source = r#"
PROGRAM Main
VAR_INPUT
    start_cmd : BOOL := FALSE;
END_VAR
VAR_OUTPUT
    speed : REAL := 42.0;
END_VAR
END_PROGRAM
"#;
        let root = temp_dir("hmi-scaffold-reset-endpoint");
        write_file(
            &root.join("hmi/overview.toml"),
            r#"
title = "Overview"

[[section]]
title = "Custom"
span = 12
"#,
        );
        let mut state = hmi_test_state(source);
        set_hmi_project_root(&mut state, &root);

        let response = handle_request_value(
            json!({
                "id": 702,
                "type": "hmi.scaffold.reset",
                "params": { "mode": "reset", "style": "industrial" }
            }),
            &state,
            None,
        );
        assert!(
            response.ok,
            "hmi.scaffold.reset failed: {:?}",
            response.error
        );
        let revision = response
            .result
            .as_ref()
            .and_then(|value| value.get("schema_revision"))
            .and_then(serde_json::Value::as_u64)
            .expect("schema revision");
        assert!(revision >= 1, "schema revision should increment");
        assert!(root.join("hmi/overview.toml").is_file());
        assert!(root.join("hmi/process.toml").is_file());
        assert!(root.join("hmi/control.toml").is_file());
        assert!(root.join("hmi/trends.toml").is_file());
        assert!(root.join("hmi/alarms.toml").is_file());
        let config = fs::read_to_string(root.join("hmi/_config.toml")).expect("read config");
        assert!(config.contains("version = 1"));
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn request_routing_contract_dispatches_core_handler_modules() {
        let source = r#"
PROGRAM Main
VAR
    run : BOOL := TRUE;
END_VAR
END_PROGRAM
"#;
        let state = hmi_test_state(source);
        let requests = vec![
            json!({"id": 1, "type": "status"}),
            json!({"id": 2, "type": "io.list"}),
            json!({"id": 3, "type": "debug.state"}),
            json!({"id": 4, "type": "var.forced"}),
            json!({"id": 5, "type": "restart", "params": { "mode": "warm" }}),
        ];

        for request in requests {
            let response = handle_request_value(request.clone(), &state, None);
            assert_ne!(
                response.error.as_deref(),
                Some("unsupported request"),
                "request should be routed by module split: {request}"
            );
        }
    }

    #[test]
    fn debug_program_and_io_handlers_preserve_behavior() {
        let source = r#"
PROGRAM Main
VAR
    run : BOOL := TRUE;
END_VAR
END_PROGRAM
"#;
        let state = hmi_test_state(source);

        let pause = handle_request_value(json!({"id": 1, "type": "pause"}), &state, None);
        assert!(pause.ok, "pause should succeed: {:?}", pause.error);

        let debug_state =
            handle_request_value(json!({"id": 2, "type": "debug.state"}), &state, None);
        assert!(
            debug_state.ok,
            "debug.state should succeed: {:?}",
            debug_state.error
        );

        let restart = handle_request_value(
            json!({"id": 3, "type": "restart", "params": { "mode": "warm" }}),
            &state,
            None,
        );
        assert!(restart.ok, "restart should succeed: {:?}", restart.error);
        assert_eq!(
            state.pending_restart.lock().ok().and_then(|guard| *guard),
            Some(RestartMode::Warm)
        );

        let io_write = handle_request_value(
            json!({
                "id": 4,
                "type": "io.write",
                "params": { "address": "%QX0.0", "value": "true" }
            }),
            &state,
            None,
        );
        assert!(io_write.ok, "io.write should succeed: {:?}", io_write.error);
        assert_eq!(
            io_write
                .result
                .as_ref()
                .and_then(|result| result.get("status"))
                .and_then(serde_json::Value::as_str),
            Some("queued")
        );
    }

    #[test]
    fn config_set_reports_field_level_diagnostics_for_unknown_and_type_errors() {
        let source = r#"
PROGRAM Main
VAR
    run : BOOL := TRUE;
END_VAR
END_PROGRAM
"#;
        let state = hmi_test_state(source);

        let unknown = handle_request_value(
            json!({
                "id": 20,
                "type": "config.set",
                "params": { "unknown.key": true }
            }),
            &state,
            None,
        );
        assert!(!unknown.ok);
        assert_eq!(
            unknown.error.as_deref(),
            Some("unknown config key 'unknown.key'")
        );

        let invalid_type = handle_request_value(
            json!({
                "id": 21,
                "type": "config.set",
                "params": { "web.enabled": "yes" }
            }),
            &state,
            None,
        );
        assert!(!invalid_type.ok);
        assert!(invalid_type
            .error
            .as_deref()
            .unwrap_or_default()
            .contains("invalid config value for 'web.enabled': expected boolean"));
    }

    #[test]
    fn config_set_reports_cross_field_auth_diagnostic() {
        let source = r#"
PROGRAM Main
VAR
    run : BOOL := TRUE;
END_VAR
END_PROGRAM
"#;
        let state = hmi_test_state(source);
        let response = handle_request_value(
            json!({
                "id": 22,
                "type": "config.set",
                "params": { "web.auth": "token" }
            }),
            &state,
            None,
        );
        assert!(!response.ok);
        assert!(response.error.as_deref().unwrap_or_default().contains(
            "invalid config value for 'web.auth': token mode requires control.auth_token"
        ));
    }

    #[test]
    fn invalid_and_malformed_requests_return_negative_responses() {
        let source = r#"
PROGRAM Main
VAR
    run : BOOL := TRUE;
END_VAR
END_PROGRAM
"#;
        let state = hmi_test_state(source);

        let invalid_line = handle_request_line("{invalid-json", &state, None)
            .expect("invalid request should still return response line");
        let invalid_json: serde_json::Value =
            serde_json::from_str(&invalid_line).expect("parse invalid response");
        let invalid_error = invalid_json
            .get("error")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        assert!(invalid_error.starts_with("invalid request:"));

        let unsupported =
            handle_request_value(json!({"id": 10, "type": "does.not.exist"}), &state, None);
        assert!(!unsupported.ok);
        assert_eq!(unsupported.error.as_deref(), Some("unsupported request"));

        let malformed_io = handle_request_value(
            json!({"id": 11, "type": "io.write", "params": { "address": "%QX0.0" }}),
            &state,
            None,
        );
        assert!(!malformed_io.ok);
        assert!(malformed_io
            .error
            .as_deref()
            .unwrap_or_default()
            .contains("invalid params"));

        let invalid_restart = handle_request_value(
            json!({"id": 12, "type": "restart", "params": { "mode": "sideways" }}),
            &state,
            None,
        );
        assert!(!invalid_restart.ok);
        assert_eq!(
            invalid_restart.error.as_deref(),
            Some("invalid restart mode")
        );
    }

    #[test]
    fn rbac_authorization_matrix_enforces_sensitive_endpoint_roles() {
        let source = r#"
PROGRAM Main
VAR
    run : BOOL := TRUE;
END_VAR
END_PROGRAM
"#;
        let mut state = hmi_test_state(source);
        state.auth_token = Arc::new(Mutex::new(Some(SmolStr::new("admin-token"))));
        state.control_requires_auth = true;
        let pairing_path = pairing_file("matrix");
        let store = Arc::new(PairingStore::load(pairing_path.clone()));
        state.pairing = Some(store.clone());

        let viewer_code = store.start_pairing();
        let viewer_token = store
            .claim(&viewer_code.code, Some(AccessRole::Viewer))
            .expect("viewer token");
        let operator_code = store.start_pairing();
        let operator_token = store
            .claim(&operator_code.code, Some(AccessRole::Operator))
            .expect("operator token");
        let engineer_code = store.start_pairing();
        let engineer_token = store
            .claim(&engineer_code.code, Some(AccessRole::Engineer))
            .expect("engineer token");

        let viewer_status = handle_request_value(
            json!({"id": 50, "type": "status", "auth": viewer_token}),
            &state,
            None,
        );
        assert!(viewer_status.ok, "viewer should read status");

        let viewer_restart = handle_request_value(
            json!({"id": 51, "type": "restart", "auth": viewer_token, "params": {"mode": "warm"}}),
            &state,
            None,
        );
        assert!(!viewer_restart.ok, "viewer must not restart runtime");
        assert!(viewer_restart
            .error
            .as_deref()
            .is_some_and(|msg| msg.contains("requires role operator")));

        let operator_restart = handle_request_value(
            json!({"id": 52, "type": "restart", "auth": operator_token, "params": {"mode": "warm"}}),
            &state,
            None,
        );
        assert!(operator_restart.ok, "operator should restart runtime");

        let operator_config = handle_request_value(
            json!({"id": 53, "type": "config.set", "auth": operator_token, "params": {"log.level": "debug"}}),
            &state,
            None,
        );
        assert!(!operator_config.ok, "operator must not write config");
        assert!(operator_config
            .error
            .as_deref()
            .is_some_and(|msg| msg.contains("requires role engineer")));

        let operator_hmi_write = handle_request_value(
            json!({
                "id": 531,
                "type": "hmi.write",
                "auth": operator_token,
                "params": { "id": "resource/RESOURCE/program/Main/field/run", "value": false }
            }),
            &state,
            None,
        );
        assert!(
            !operator_hmi_write.ok,
            "operator must not write HMI targets"
        );
        assert!(operator_hmi_write
            .error
            .as_deref()
            .is_some_and(|msg| msg.contains("requires role engineer")));

        let engineer_write = handle_request_value(
            json!({
                "id": 54,
                "type": "io.write",
                "auth": engineer_token,
                "params": { "address": "%QX0.0", "value": "true" }
            }),
            &state,
            None,
        );
        assert!(engineer_write.ok, "engineer should write I/O");

        let engineer_hmi_write = handle_request_value(
            json!({
                "id": 541,
                "type": "hmi.write",
                "auth": engineer_token,
                "params": { "id": "resource/RESOURCE/program/Main/field/run", "value": false }
            }),
            &state,
            None,
        );
        assert!(
            !engineer_hmi_write.ok,
            "engineer write should still be gated by read-only defaults"
        );
        assert_eq!(
            engineer_hmi_write.error.as_deref(),
            Some("hmi.write disabled in read-only mode")
        );

        let engineer_pair_start = handle_request_value(
            json!({"id": 55, "type": "pair.start", "auth": engineer_token}),
            &state,
            None,
        );
        assert!(!engineer_pair_start.ok, "engineer must not start pairing");
        assert!(engineer_pair_start
            .error
            .as_deref()
            .is_some_and(|msg| msg.contains("requires role admin")));

        let admin_set_auth = handle_request_value(
            json!({
                "id": 56,
                "type": "config.set",
                "auth": "admin-token",
                "params": { "control.auth_token": "new-admin-token" }
            }),
            &state,
            None,
        );
        assert!(admin_set_auth.ok, "admin should update auth token");

        let unauthorized = handle_request_value(
            json!({"id": 57, "type": "status", "auth": "invalid-token"}),
            &state,
            None,
        );
        assert!(!unauthorized.ok);
        assert_eq!(unauthorized.error.as_deref(), Some("unauthorized"));

        let _ = std::fs::remove_file(pairing_path);
    }

    #[test]
    fn historian_query_and_alert_control_requests_return_contract_payloads() {
        let source = r#"
PROGRAM Main
VAR
    run : BOOL := TRUE;
END_VAR
END_PROGRAM
"#;
        let mut state = hmi_test_state(source);
        let history_path = temp_history_path("historian");
        let hook_path = temp_history_path("hook");
        let historian = HistorianService::new(
            HistorianConfig {
                enabled: true,
                sample_interval_ms: 1,
                mode: RecordingMode::All,
                include: Vec::new(),
                history_path: history_path.clone(),
                max_entries: 500,
                prometheus_enabled: true,
                prometheus_path: SmolStr::new("/metrics"),
                alerts: vec![AlertRule {
                    name: SmolStr::new("run_high"),
                    variable: SmolStr::new("Main.run"),
                    above: Some(0.5),
                    below: None,
                    debounce_samples: 1,
                    hook: Some(SmolStr::new(hook_path.to_string_lossy())),
                }],
            },
            None,
        )
        .expect("historian");
        let (snapshot_tx, snapshot_rx) = std::sync::mpsc::channel();
        state
            .resource
            .send_command(ResourceCommand::Snapshot {
                respond_to: snapshot_tx,
            })
            .expect("request runtime snapshot");
        let snapshot = snapshot_rx
            .recv_timeout(std::time::Duration::from_millis(250))
            .expect("snapshot");
        historian
            .capture_snapshot_at(&snapshot, 1_000)
            .expect("capture initial");
        state.historian = Some(historian);

        let query = handle_request_value(
            json!({ "id": 80, "type": "historian.query", "params": { "limit": 20 } }),
            &state,
            None,
        );
        assert!(
            query.ok,
            "historian.query should succeed: {:?}",
            query.error
        );
        let items = query
            .result
            .as_ref()
            .and_then(|value| value.get("items"))
            .and_then(serde_json::Value::as_array)
            .expect("items");
        assert!(!items.is_empty());

        let alerts = handle_request_value(
            json!({ "id": 81, "type": "historian.alerts", "params": { "limit": 20 } }),
            &state,
            None,
        );
        assert!(
            alerts.ok,
            "historian.alerts should succeed: {:?}",
            alerts.error
        );
        let alert_items = alerts
            .result
            .as_ref()
            .and_then(|value| value.get("items"))
            .and_then(serde_json::Value::as_array)
            .expect("alerts");
        assert!(!alert_items.is_empty());

        let _ = std::fs::remove_file(history_path);
        let _ = std::fs::remove_file(hook_path);
    }
}
