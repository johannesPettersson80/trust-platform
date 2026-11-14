//! A managed runtime's status, read over its control endpoint, and the one control request that
//! the fleet commands send. A connection that nothing accepts is told apart from an exchange that
//! broke, so that a stop can tell a runtime that went away from a status that is unknown.

use std::fs;
use std::path::Path;

use anyhow::Context;
use serde::Serialize;
use trust_runtime_client::ControlError;

use crate::control::endpoint::socket_client::{self, ConnectFailed};
use crate::control::ControlEndpoint;

use super::{
    lifecycle_paths, read_control_auth_token, runtime_project_path, FleetManifestRuntime,
    FLEET_MANIFEST_SCHEMA_VERSION,
};

/// What the fleet commands report about a managed runtime: its status, and after a start or a
/// stop what the command reached.
#[derive(Debug, Clone, Serialize)]
pub struct FleetRuntimeLifecycleResponse {
    /// The schema version of the answer.
    pub schema_version: u32,
    /// The runtime's name.
    pub name: String,
    /// The runtime's project folder.
    pub path: String,
    /// The runtime's control endpoint.
    pub control_endpoint: String,
    /// `running`, `stopped`, `starting` or `stopping`.
    pub status: String,
    /// The process ID of the advisory PID file, when it holds one.
    pub pid: Option<u32>,
    /// The runtime's log file.
    pub log_path: String,
    /// What happened, in words.
    pub message: String,
    /// The runtime's answer to `status`, while it runs.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub control_status: Option<serde_json::Value>,
}

/// The status of `runtime`, registered under `fleet_root`: `running` when its control endpoint
/// answers `status`, and `stopped` when nothing accepts the connection.
///
/// # Errors
///
/// The runtime's `runtime.toml` cannot be read or is invalid, or a runtime accepted the
/// connection and its answer failed, was refused or is malformed: then the status is unknown,
/// never `stopped`.
pub fn managed_status(
    fleet_root: &Path,
    runtime: &FleetManifestRuntime,
) -> anyhow::Result<FleetRuntimeLifecycleResponse> {
    let project_path = runtime_project_path(fleet_root, runtime.path.as_str());
    let paths = lifecycle_paths(fleet_root, runtime.name.as_str());
    let pid = read_pid(&paths.pid).ok();
    let log_path = paths.log.display().to_string();
    let auth = read_control_auth_token(&project_path)?;
    match exchange_control(
        runtime.control_endpoint.as_str(),
        auth.as_deref(),
        "status",
        None,
    )? {
        ControlExchange::Response(result) => Ok(FleetRuntimeLifecycleResponse {
            schema_version: FLEET_MANIFEST_SCHEMA_VERSION,
            name: runtime.name.clone(),
            path: project_path.display().to_string(),
            control_endpoint: runtime.control_endpoint.clone(),
            status: "running".to_string(),
            pid,
            log_path,
            message: "Runtime is reachable.".to_string(),
            control_status: Some(result),
        }),
        ControlExchange::Unreachable(error) => Ok(FleetRuntimeLifecycleResponse {
            schema_version: FLEET_MANIFEST_SCHEMA_VERSION,
            name: runtime.name.clone(),
            path: project_path.display().to_string(),
            control_endpoint: runtime.control_endpoint.clone(),
            status: "stopped".to_string(),
            pid,
            log_path,
            message: format!("Runtime is not reachable: {error}"),
            control_status: None,
        }),
    }
}

fn read_pid(path: &Path) -> anyhow::Result<u32> {
    let text = fs::read_to_string(path)?;
    text.trim()
        .parse::<u32>()
        .with_context(|| format!("failed to parse {}", path.display()))
}

/// Whether a failed status read broke off in transport after the connection was accepted, so
/// that a stop may ask again instead of failing (specification 11 §6.8.3): a connection reset,
/// broken pipe, aborted connection, end of the connection before an answer, or a read or write
/// that waited out. Only a failure of the control exchange counts; an I/O error of the same kind
/// elsewhere, reading the configuration for example, does not.
#[must_use]
pub fn interrupted_stop_probe(error: &anyhow::Error) -> bool {
    error.chain().any(|cause| {
        cause
            .downcast_ref::<ControlError>()
            .and_then(ControlError::io_kind)
            .is_some_and(|kind| {
                matches!(
                    kind,
                    std::io::ErrorKind::ConnectionReset
                        | std::io::ErrorKind::ConnectionAborted
                        | std::io::ErrorKind::BrokenPipe
                        | std::io::ErrorKind::UnexpectedEof
                        | std::io::ErrorKind::TimedOut
                        | std::io::ErrorKind::WouldBlock
                )
            })
    })
}

enum ControlExchange {
    Response(serde_json::Value),
    Unreachable(String),
}

/// Sends one control request of type `request_type` and returns its result.
///
/// # Errors
///
/// Nothing accepts the connection, the exchange fails, or the runtime refuses the request or
/// answers with a malformed response.
pub fn request_control(
    endpoint: &str,
    auth_token: Option<&str>,
    request_type: &str,
) -> anyhow::Result<serde_json::Value> {
    request_control_with_params(endpoint, auth_token, request_type, None)
}

/// Sends one control request of type `request_type` with `params`, and returns its result.
///
/// # Errors
///
/// As [`request_control`].
pub fn request_control_with_params(
    endpoint: &str,
    auth_token: Option<&str>,
    request_type: &str,
    params: Option<serde_json::Value>,
) -> anyhow::Result<serde_json::Value> {
    match exchange_control(endpoint, auth_token, request_type, params)? {
        ControlExchange::Response(response) => Ok(response),
        ControlExchange::Unreachable(error) => anyhow::bail!("{error}"),
    }
}

/// Sends one request over a new connection of the shared control client of a runtime's socket,
/// with its waits, its response limit and its checks of a response (specification 11, 6.8.1). A
/// connection that nothing accepts is `Unreachable`, not an error.
fn exchange_control(
    endpoint: &str,
    auth_token: Option<&str>,
    request_type: &str,
    params: Option<serde_json::Value>,
) -> anyhow::Result<ControlExchange> {
    let endpoint = ControlEndpoint::parse(endpoint).map_err(anyhow::Error::msg)?;
    let mut client = match socket_client::connect(&endpoint, auth_token) {
        Ok(client) => client,
        Err(error) => {
            return match error.downcast_ref::<ConnectFailed>() {
                Some(failed) => Ok(ControlExchange::Unreachable(format!(
                    "failed to connect to {}: {}",
                    failed.endpoint, failed.error
                ))),
                None => Err(error),
            };
        }
    };
    let mut request = serde_json::json!({ "type": request_type });
    if let Some(params) = params {
        request["params"] = params;
    }
    let response = socket_client::exchange(&mut client, &request)?;
    if response
        .get("ok")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
    {
        return Ok(ControlExchange::Response(
            response
                .get("result")
                .cloned()
                .unwrap_or_else(|| serde_json::json!({})),
        ));
    }
    let message = response
        .get("error")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("control request failed");
    anyhow::bail!("{message}");
}

#[cfg(test)]
mod tests;
