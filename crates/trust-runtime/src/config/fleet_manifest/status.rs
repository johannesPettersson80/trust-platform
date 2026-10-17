//! A managed runtime's status, read over its control endpoint, and the one control request that
//! the fleet commands send. A connection that nothing accepts is told apart from an exchange that
//! broke, so that a stop can tell a runtime that went away from a status that is unknown.

use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::Path;
use std::time::Duration;

use anyhow::Context;
use serde::Serialize;

use crate::control::ControlEndpoint;

use super::{
    lifecycle_paths, read_control_auth_token, runtime_project_path, FleetManifestRuntime,
    FLEET_MANIFEST_SCHEMA_VERSION,
};

const MAX_CONTROL_RESPONSE_BYTES: usize = 1024 * 1024;

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
    match exchange_control(runtime.control_endpoint.as_str(), auth.as_deref(), "status")? {
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

// A typed boundary prevents configuration I/O errors from becoming retryable probes.
#[derive(Debug)]
struct InterruptedControlTransport(std::io::Error);

impl std::fmt::Display for InterruptedControlTransport {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.0, formatter)
    }
}

impl std::error::Error for InterruptedControlTransport {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}

fn control_transport_error(error: std::io::Error) -> anyhow::Error {
    if matches!(
        error.kind(),
        std::io::ErrorKind::ConnectionReset
            | std::io::ErrorKind::ConnectionAborted
            | std::io::ErrorKind::BrokenPipe
            | std::io::ErrorKind::UnexpectedEof
            | std::io::ErrorKind::TimedOut
            | std::io::ErrorKind::WouldBlock
    ) {
        InterruptedControlTransport(error).into()
    } else {
        error.into()
    }
}

/// Whether a failed status read broke off in transport after the connection was accepted, so
/// that a stop may ask again instead of failing.
#[must_use]
pub fn interrupted_stop_probe(error: &anyhow::Error) -> bool {
    error
        .downcast_ref::<InterruptedControlTransport>()
        .is_some()
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
    match exchange_control(endpoint, auth_token, request_type)? {
        ControlExchange::Response(response) => Ok(response),
        ControlExchange::Unreachable(error) => anyhow::bail!("{error}"),
    }
}

fn exchange_control(
    endpoint: &str,
    auth_token: Option<&str>,
    request_type: &str,
) -> anyhow::Result<ControlExchange> {
    let endpoint = ControlEndpoint::parse(endpoint).map_err(anyhow::Error::msg)?;
    let payload = serde_json::json!({
        "id": 1,
        "type": request_type,
        "auth": auth_token,
    });
    let response = match endpoint {
        ControlEndpoint::Tcp(addr) => {
            let mut stream = match TcpStream::connect_timeout(&addr, Duration::from_millis(500)) {
                Ok(stream) => stream,
                Err(error) => {
                    return Ok(ControlExchange::Unreachable(format!(
                        "failed to connect to tcp://{addr}: {error}"
                    )))
                }
            };
            stream
                .set_read_timeout(Some(Duration::from_millis(750)))
                .ok();
            stream
                .set_write_timeout(Some(Duration::from_millis(750)))
                .ok();
            writeln!(stream, "{payload}")
                .map_err(control_transport_error)
                .context("failed to send control request")?;
            let mut reader = BufReader::new(stream);
            read_control_response(&mut reader)?
        }
        #[cfg(unix)]
        ControlEndpoint::Unix(path) => {
            let mut stream = match std::os::unix::net::UnixStream::connect(&path) {
                Ok(stream) => stream,
                Err(error) => {
                    return Ok(ControlExchange::Unreachable(format!(
                        "failed to connect to unix://{}: {error}",
                        path.display()
                    )))
                }
            };
            stream
                .set_read_timeout(Some(Duration::from_millis(750)))
                .ok();
            stream
                .set_write_timeout(Some(Duration::from_millis(750)))
                .ok();
            writeln!(stream, "{payload}")
                .map_err(control_transport_error)
                .context("failed to send control request")?;
            let mut reader = BufReader::new(stream);
            read_control_response(&mut reader)?
        }
    };
    let response: serde_json::Value =
        serde_json::from_str(response.trim()).context("control response was not JSON")?;
    if response.get("id").and_then(serde_json::Value::as_u64) != Some(1) {
        anyhow::bail!("control response id is missing or does not match request id 1");
    }
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

fn read_control_response(reader: &mut impl BufRead) -> anyhow::Result<String> {
    let mut line = String::new();
    let bytes = reader
        .take((MAX_CONTROL_RESPONSE_BYTES + 1) as u64)
        .read_line(&mut line)
        .map_err(control_transport_error)
        .context("failed to read control response")?;
    if bytes == 0 {
        return Err(control_transport_error(std::io::Error::new(
            std::io::ErrorKind::UnexpectedEof,
            "control response was empty",
        )));
    }
    if bytes > MAX_CONTROL_RESPONSE_BYTES {
        anyhow::bail!(
            "control response exceeded {MAX_CONTROL_RESPONSE_BYTES} bytes without a complete line"
        );
    }
    Ok(line)
}

#[cfg(test)]
mod tests;
