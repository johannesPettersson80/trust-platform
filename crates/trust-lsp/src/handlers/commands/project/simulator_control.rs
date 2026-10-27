//! The control endpoint of the simulator that a launch starts (specification 25 §6.8).
//!
//! - `create`: the endpoint and token that the debug adapter serves the simulator's control API on.
//!   On Linux and macOS the socket `trust-debug-<h>.sock` in the system temporary folder, `<h>` the
//!   first 8 hexadecimal digits of the SHA-256 of the workspace folder's path (of `default` without
//!   one), and a new control token (specification 11 §6.8.0a). On Windows none: the adapter takes
//!   its default endpoint. The extension asks once per workspace folder and session, so the launch
//!   and Devices & Connections use the same endpoint and token.
//! - `check`: whether a launch may bind its endpoint. A TCP endpoint where a program accepts a
//!   connection within 300 ms is taken, and the launch is refused.

use std::net::TcpStream;
use std::path::PathBuf;
use std::time::Duration;

use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use trust_runtime::control::ControlEndpoint;

use super::decode;

/// How long `check` waits for a connection to a launch's TCP endpoint.
const PORT_PROBE: Duration = Duration::from_millis(300);
/// The refusal of a launch whose TCP endpoint a program already holds.
const PORT_IN_USE: &str = "The runtime port is already in use.";
/// Hexadecimal digits of the workspace hash in the socket's name.
const HASH_HEX_DIGITS: usize = 8;

#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "camelCase")]
enum Args {
    /// The endpoint and token of a new simulator of `workspace`.
    Create {
        #[serde(default)]
        workspace: Option<PathBuf>,
    },
    /// Whether a launch may bind `endpoint`.
    Check { endpoint: String },
}

/// `create`: `{ ok: true, endpoint, authToken }`, both `null` on Windows. `check`:
/// `{ ok: true, problem }`, `problem` the refusal of the launch or `null`.
pub(super) fn answer(argument: Value) -> Result<Value, String> {
    match decode(argument)? {
        Args::Create { workspace } => create(workspace),
        Args::Check { endpoint } => Ok(json!({ "ok": true, "problem": check(&endpoint) })),
    }
}

#[cfg(unix)]
fn create(workspace: Option<PathBuf>) -> Result<Value, String> {
    let socket = std::env::temp_dir().join(socket_name(workspace));
    let token = trust_runtime::control::endpoint::generate_control_token()
        .map_err(|error| format!("No control token for the simulator: {error}"))?;
    Ok(json!({
        "ok": true,
        "endpoint": format!("unix://{}", socket.display()),
        "authToken": token,
    }))
}

#[cfg(not(unix))]
fn create(_workspace: Option<PathBuf>) -> Result<Value, String> {
    Ok(json!({ "ok": true, "endpoint": null, "authToken": null }))
}

/// `trust-debug-<h>.sock` for `workspace`.
#[cfg_attr(not(unix), allow(dead_code))]
fn socket_name(workspace: Option<PathBuf>) -> String {
    let key = workspace.map_or_else(
        || "default".to_owned(),
        |folder| folder.to_string_lossy().into_owned(),
    );
    let digest = hex::encode(Sha256::digest(key.as_bytes()));
    format!("trust-debug-{}.sock", &digest[..HASH_HEX_DIGITS])
}

/// The refusal of a launch on `endpoint`: a TCP endpoint where a program accepts a connection is
/// taken. Any other endpoint, and one the adapter cannot read, is left to the adapter, which binds
/// it and names its own error.
fn check(endpoint: &str) -> Option<&'static str> {
    match ControlEndpoint::parse(endpoint.trim()) {
        Ok(ControlEndpoint::Tcp(address)) => TcpStream::connect_timeout(&address, PORT_PROBE)
            .is_ok()
            .then_some(PORT_IN_USE),
        _ => None,
    }
}

#[cfg(test)]
#[path = "simulator_control_tests.rs"]
mod tests;
