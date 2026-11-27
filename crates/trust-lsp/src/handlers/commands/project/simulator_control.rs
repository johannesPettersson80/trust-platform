//! The control endpoint of the simulator that a launch starts (specification 25 §6.8).
//!
//! - `create`: the endpoint and token that the debug adapter serves the simulator's control API on.
//!   On Linux and macOS the socket `trust-debug-<h>.sock` in the system temporary folder, on Windows
//!   a loopback port from 20000 to 39999, below the dynamic port range from 49152 that Windows
//!   hands out ports from: the first that binds from 20000 plus `<h>` modulo 20000 up, so that two
//!   workspaces whose hashes share a port both launch; `<h>` is the first 8 hexadecimal digits of
//!   the SHA-256 of the workspace folder's path (of `default` without one). On every platform a
//!   new control token
//!   (specification 11 §6.8.0a), which the simulator's control server requires on a TCP endpoint.
//!   The extension asks once per workspace folder and session, so the launch and Devices &
//!   Connections use the same endpoint and token. The server keeps them too, for the inline values
//!   of the folder's documents ([`crate::state::simulator_controls`]).
//! - `check`: whether a launch may bind its endpoint. A TCP endpoint where a program accepts a
//!   connection within 300 ms is taken, and the launch is refused.

use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use trust_runtime::control::ControlEndpoint;

use super::decode;
use crate::state::simulator_controls::SimulatorControl;
use crate::state::ServerState;

/// How long `check` waits for a connection to a launch's TCP endpoint.
const PORT_PROBE: Duration = Duration::from_millis(300);
/// The refusal of a launch whose TCP endpoint a program already holds.
const PORT_IN_USE: &str = "The runtime port is already in use.";
/// The first loopback port of a simulator on Windows.
const FIRST_PORT: u16 = 20_000;
/// How many loopback ports from [`FIRST_PORT`] simulators on Windows take, all below Windows'
/// dynamic port range from 49152.
const PORTS: u16 = 20_000;
/// How many ports from a workspace's own `create` tries on Windows before it names that one.
const PORT_TRIES: u16 = 64;

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

/// `create`: `{ ok: true, endpoint, authToken }`. `check`:
/// `{ ok: true, problem }`, `problem` the refusal of the launch or `null`.
pub(super) fn answer(state: &ServerState, argument: Value) -> Result<Value, String> {
    match decode(argument)? {
        Args::Create { workspace } => create(state, workspace),
        Args::Check { endpoint } => Ok(json!({ "ok": true, "problem": check(&endpoint) })),
    }
}

fn create(state: &ServerState, workspace: Option<PathBuf>) -> Result<Value, String> {
    let token = trust_runtime::control::endpoint::generate_control_token()
        .map_err(|error| format!("No control token for the simulator: {error}"))?;
    let hash = workspace_hash(workspace.as_deref());
    let control = state
        .simulator_controls()
        .name_and_record(workspace, |named_elsewhere| SimulatorControl {
            endpoint: endpoint(hash, named_elsewhere),
            auth_token: token,
        });
    Ok(json!({
        "ok": true,
        "endpoint": control.endpoint,
        "authToken": control.auth_token,
    }))
}

/// The socket `trust-debug-<h>.sock` in the system temporary folder, `<h>` the 8 hexadecimal
/// digits of `hash`.
#[cfg(unix)]
fn endpoint(hash: u32, _named_elsewhere: &dyn Fn(&str) -> bool) -> String {
    let socket = std::env::temp_dir().join(format!("trust-debug-{hash:08x}.sock"));
    format!("unix://{}", socket.display())
}

/// The first loopback port from [`workspace_port`] up that binds now and that this server has not
/// named for another workspace folder (`named_elsewhere`): a port that another workspace's
/// simulator, in this window or another, or another program holds is passed over.
#[cfg(not(unix))]
fn endpoint(hash: u32, named_elsewhere: &dyn Fn(&str) -> bool) -> String {
    let port = free_port_from(workspace_port(hash), |port| {
        !named_elsewhere(&format!("tcp://127.0.0.1:{port}"))
            && std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port)).is_ok()
    });
    format!("tcp://127.0.0.1:{port}")
}

/// The first 32 bits of the SHA-256 of `workspace`'s path (of `default` without one), whose
/// hexadecimal digits are the first 8 of the digest's.
fn workspace_hash(workspace: Option<&Path>) -> u32 {
    let key = workspace.map_or_else(
        || "default".to_owned(),
        |folder| folder.to_string_lossy().into_owned(),
    );
    let digest = Sha256::digest(key.as_bytes());
    u32::from_be_bytes([digest[0], digest[1], digest[2], digest[3]])
}

/// A Windows simulator's loopback port: [`FIRST_PORT`] plus `hash` modulo [`PORTS`]. Compiled
/// everywhere, so that every platform tests it.
#[cfg_attr(unix, allow(dead_code))]
fn workspace_port(hash: u32) -> u16 {
    // The remainder is below PORTS, so it fits a u16.
    FIRST_PORT + (hash % u32::from(PORTS)) as u16
}

/// The first of [`PORT_TRIES`] ports from `first` up that `is_free` takes, wrapping from the last
/// port of the range to [`FIRST_PORT`]; `first` when none is, so that the launch names the port it
/// cannot bind. Compiled everywhere, so that every platform tests it.
#[cfg_attr(unix, allow(dead_code))]
fn free_port_from(first: u16, is_free: impl Fn(u16) -> bool) -> u16 {
    (0..PORT_TRIES)
        .map(|step| FIRST_PORT + (first - FIRST_PORT + step) % PORTS)
        .find(|&port| is_free(port))
        .unwrap_or(first)
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
