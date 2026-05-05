//! Runtime control server (JSON IPC).

#![allow(missing_docs)]

mod audit;
mod auth;
mod breakpoint_handlers;
mod config_handlers;
mod debug_handlers;
mod handlers;
mod hmi_handlers;
mod io_handlers;
mod policy;
mod program_handlers;
mod status_handlers;
mod transport;
mod types;
mod variable_handlers;

use std::collections::VecDeque;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::config::ControlMode;
use crate::debug::{DebugControl, DebugVariableHandles};
use crate::error::RuntimeError;
use crate::io::{IoDriverStatus, IoSnapshot};
use crate::linux_rt::LinuxRtRuntimeStatus;
use crate::metrics::RuntimeMetrics;
use crate::runtime::RuntimeMetadata;
use crate::scheduler::ResourceControl;
use crate::security::pairing::PairingStore;
use crate::settings::RuntimeSettings;
use crate::value::Value;
use crate::RestartMode;
use notify::{Event, RecursiveMode, Watcher};
use serde_json::json;
use smol_str::SmolStr;
use tracing::warn;

use self::audit::{record_audit, ControlAuditRecord};
use self::auth::resolve_request_role;
use self::policy::{is_debug_request, required_role_for_control_request};
pub(crate) use self::types::ControlResponse;
use self::types::*;

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
    pub realtime_status: Arc<Mutex<LinuxRtRuntimeStatus>>,
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
    pub event_id: SmolStr,
    pub timestamp_ms: u128,
    pub request_id: u64,
    pub request_type: SmolStr,
    pub correlation_id: Option<SmolStr>,
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

pub(crate) fn dispatch_web_control_request_port(
    mut payload: serde_json::Value,
    state: &ControlState,
    client: Option<&str>,
    request_token: Option<&str>,
) -> ControlResponse {
    if payload.get("auth").is_none() {
        if let Some(token) = request_token {
            payload["auth"] = serde_json::Value::String(token.to_string());
        }
    }
    handle_request_value(payload, state, client)
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
    Some(serde_json::to_string(&response).unwrap_or_else(|err| {
        json!({
            "id": 0_u64,
            "ok": false,
            "error": format!("response serialization failed: {err}"),
        })
        .to_string()
    }))
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
            let audit_id = record_audit(
                state,
                ControlAuditRecord {
                    request_id: 0,
                    request_type: SmolStr::new("invalid"),
                    correlation_id: None,
                    ok: false,
                    error: Some(SmolStr::new(format!("invalid request: {err}"))),
                    auth_present: false,
                    client,
                },
            );
            return response.with_audit_id(audit_id);
        }
    };
    let request_role = match resolve_request_role(&request, state, client) {
        Ok(role) => role,
        Err(error) => {
            let response = ControlResponse::error(request.id, error.to_string());
            let audit_id = record_audit(
                state,
                ControlAuditRecord {
                    request_id: request.id,
                    request_type: SmolStr::new(request.r#type.as_str()),
                    correlation_id: request.request_id.as_deref(),
                    ok: false,
                    error: Some(SmolStr::new(error)),
                    auth_present: request.auth.is_some(),
                    client,
                },
            );
            return response.with_audit_id(audit_id);
        }
    };
    let required_role =
        required_role_for_control_request(request.r#type.as_str(), request.params.as_ref());
    if !request_role.allows(required_role) {
        let error = format!("forbidden: requires role {}", required_role.as_str());
        let response = ControlResponse::error(request.id, error.clone());
        let audit_id = record_audit(
            state,
            ControlAuditRecord {
                request_id: request.id,
                request_type: SmolStr::new(request.r#type.as_str()),
                correlation_id: request.request_id.as_deref(),
                ok: false,
                error: Some(SmolStr::new(error)),
                auth_present: request.auth.is_some(),
                client,
            },
        );
        return response.with_audit_id(audit_id);
    }
    if !state.debug_enabled.load(Ordering::Relaxed) && is_debug_request(request.r#type.as_str()) {
        if let Ok(mut events) = state.events.lock() {
            events.push_back(crate::debug::RuntimeEvent::FeatureDisabled {
                feature: SmolStr::new("debug"),
                request_type: Some(SmolStr::new(request.r#type.as_str())),
                time: control_event_time_now(),
            });
        }
        let response = ControlResponse::error_with_code(
            request.id,
            "debug disabled".into(),
            "feature_disabled",
        );
        let audit_id = record_audit(
            state,
            ControlAuditRecord {
                request_id: request.id,
                request_type: SmolStr::new(request.r#type.as_str()),
                correlation_id: request.request_id.as_deref(),
                ok: false,
                error: Some(SmolStr::new("debug disabled")),
                auth_present: request.auth.is_some(),
                client,
            },
        );
        return response.with_audit_id(audit_id);
    }
    let mut response = handlers::dispatch(&request, state)
        .unwrap_or_else(|| ControlResponse::error(request.id, "unsupported request".into()));
    let audit_id = record_audit(
        state,
        ControlAuditRecord {
            request_id: request.id,
            request_type: SmolStr::new(request.r#type.as_str()),
            correlation_id: request.request_id.as_deref(),
            ok: response.ok,
            error: response.error.as_ref().map(SmolStr::new),
            auth_present: request.auth.is_some(),
            client,
        },
    );
    response = response.with_audit_id(audit_id);
    response
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
