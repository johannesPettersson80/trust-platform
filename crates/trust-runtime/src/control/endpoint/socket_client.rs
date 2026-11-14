//! Requests to a runtime's socket on this computer, through the shared control client
//! (specification 11, 6.8.1). `trust-runtime ctl` and trust-dev's requests both go through here, so
//! that a client's waits, the limit of a response line and the checks of a response are the same
//! for every client of a runtime's socket.

use std::fmt;
use std::io;
use std::time::Duration;

use anyhow::Context;
use serde_json::Value;
use trust_runtime_client::ControlClient;

use super::ControlEndpoint;

/// How long a connection attempt to a runtime's TCP socket waits.
pub const CONNECT_WAIT: Duration = Duration::from_millis(500);

/// How long each read and write waits once the connection is made, for the reply to every request
/// but `bytecode.reload`.
pub const REPLY_WAIT: Duration = Duration::from_millis(750);

/// How long a client waits for the reply to `bytecode.reload`. The runtime answers it within 65
/// seconds, five to take the program and sixty to commit it (specification 11, 6.7.1), and a client
/// that gave up sooner would leave its user without the outcome of a change that may still apply.
pub const RELOAD_REPLY_WAIT: Duration = Duration::from_secs(70);

/// The longest response line, its newline included.
pub const MAX_RESPONSE_BYTES: usize = 1024 * 1024;

/// A connection to a runtime's socket that could not be made. It keeps the operating system's
/// error, so that the caller can tell a runtime that is not running from another failure
/// (specification 11, 6.8.0a).
#[derive(Debug)]
pub struct ConnectFailed {
    /// The endpoint, as `tcp://<address>` or `unix://<path>`.
    pub endpoint: String,
    /// Why the connection failed.
    pub error: io::Error,
}

impl fmt::Display for ConnectFailed {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "connect control endpoint {}: {}",
            self.endpoint, self.error
        )
    }
}

impl std::error::Error for ConnectFailed {}

/// How long a client waits for the reply to a request of operation `kind`.
#[must_use]
pub fn reply_wait(kind: &str) -> Duration {
    if kind == "bytecode.reload" {
        RELOAD_REPLY_WAIT
    } else {
        REPLY_WAIT
    }
}

/// The shared client over a new connection to a runtime's socket on this computer: a TCP
/// connection attempt of at most [`CONNECT_WAIT`], then reads and writes of at most [`REPLY_WAIT`]
/// each and response lines of at most [`MAX_RESPONSE_BYTES`]. The client connects itself, so that a
/// failed connection keeps the operating system's error ([`ConnectFailed`]).
///
/// # Errors
/// The connection failed, or the socket refused its timeout.
pub fn connect(endpoint: &ControlEndpoint, auth: Option<&str>) -> anyhow::Result<ControlClient> {
    let client = match endpoint {
        ControlEndpoint::Tcp(addr) => ControlClient::from_stream(
            std::net::TcpStream::connect_timeout(addr, CONNECT_WAIT).map_err(|error| {
                ConnectFailed {
                    endpoint: format!("tcp://{addr}"),
                    error,
                }
            })?,
            auth,
        ),
        #[cfg(unix)]
        ControlEndpoint::Unix(path) => ControlClient::from_stream(
            std::os::unix::net::UnixStream::connect(path).map_err(|error| ConnectFailed {
                endpoint: format!("unix://{}", path.display()),
                error,
            })?,
            auth,
        ),
    };
    client
        .set_timeout(REPLY_WAIT)
        .context("set the control endpoint's read and write timeout")?;
    Ok(client.with_response_limit(MAX_RESPONSE_BYTES))
}

/// Sends `request`, a request object with its `type` and, when it has them, `auth` and `params`,
/// to a runtime's socket on this computer over a new connection, waits for the reply as long as
/// [`reply_wait`] gives for its `type`, and returns the whole response object. The client numbers
/// the requests of its connection from 1 and writes each as one line.
///
/// # Errors
/// The request has no `type`, the connection failed ([`ConnectFailed`]), the exchange failed, or
/// the response does not carry the request's id.
pub fn request(endpoint: &ControlEndpoint, request: &Value) -> anyhow::Result<Value> {
    let kind = request
        .get("type")
        .and_then(Value::as_str)
        .context("control request missing string 'type'")?;
    let mut client = connect(endpoint, request.get("auth").and_then(Value::as_str))?;
    let wait = reply_wait(kind);
    if wait != REPLY_WAIT {
        client
            .set_timeout(wait)
            .context("set the control endpoint's read and write timeout")?;
    }
    let request_id = client.next_id();
    let response = client.exchange(kind, request.get("params").cloned())?;
    ensure_response_id(&response, request_id)?;
    Ok(response)
}

/// A response must carry the id of its request: a missing, noninteger or other id is a protocol
/// failure (specification 11, 6.8.1).
///
/// # Errors
/// The response's id is missing, not an integer, or another request's.
pub fn ensure_response_id(response: &Value, request_id: u64) -> anyhow::Result<()> {
    if response.get("id").and_then(Value::as_u64) != Some(request_id) {
        anyhow::bail!("control response id is missing or does not match request id {request_id}");
    }
    Ok(())
}
