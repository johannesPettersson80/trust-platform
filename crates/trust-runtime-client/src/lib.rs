//! The runtime's control endpoint: one JSON object per line each way (runtime protocol annex,
//! E0.1). Inline values and Control Studio share this client.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
#[cfg(unix)]
use std::os::unix::net::UnixStream;
use std::time::Duration;

use serde::Deserialize;
use serde_json::Value;

mod target_status;

pub use target_status::{probe, ProbeTimeouts, TargetProbe};

#[cfg(test)]
mod client_tests;
#[cfg(test)]
mod fake_runtime;

/// The longest response line, newline included, that the command line and the Devices panel
/// read: 1 MiB (specification 11, 6.8.1). A client that reads bulk results, such as a test run's
/// samples, sets its own limit or none.
pub const RESPONSE_LINE_LIMIT: usize = 1024 * 1024;

/// Where a runtime listens for control requests: `tcp://host:port` or `unix:///absolute/path`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ControlEndpoint {
    /// A TCP address.
    Tcp(String),
    /// A Unix socket.
    #[cfg(unix)]
    Unix(std::path::PathBuf),
}

impl ControlEndpoint {
    /// Reads an endpoint; the scheme is exact and the address is not empty.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        if let Some(rest) = text.strip_prefix("tcp://") {
            let rest = rest.trim();
            return (!rest.is_empty()).then(|| Self::Tcp(rest.to_string()));
        }
        #[cfg(unix)]
        if let Some(rest) = text.strip_prefix("unix://") {
            let path = std::path::Path::new(rest.trim());
            return (!path.as_os_str().is_empty() && path.is_absolute())
                .then(|| Self::Unix(path.to_path_buf()));
        }
        None
    }
}

/// Why a control request has no result.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ControlError {
    /// The endpoint could not be reached, or the connection failed.
    #[error("{0}")]
    Transport(String),
    /// The runtime refused the request (annex E0.4).
    #[error("{error}")]
    Refused {
        /// The runtime's text, naming the failed check.
        error: String,
        /// The refusal code; absent for a refused `config.set`.
        code: Option<String>,
    },
    /// The answer is not a control response.
    #[error("the runtime's answer is not a control response: {0}")]
    Malformed(String),
}

impl ControlError {
    /// The credential failure that a refusal names by its code (specification 11, 6.9.6):
    /// `missing_auth_token` or `invalid_auth_token`. Every other refusal and every other error
    /// give `None`; the runtime's text is never read.
    #[must_use]
    pub fn auth_failure(&self) -> Option<AuthFailure> {
        let Self::Refused {
            code: Some(code), ..
        } = self
        else {
            return None;
        };
        match code.as_str() {
            "missing_auth_token" => Some(AuthFailure::Missing),
            "invalid_auth_token" => Some(AuthFailure::Rejected),
            _ => None,
        }
    }
}

/// How a runtime refused a request's credentials.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthFailure {
    /// The runtime requires a token, and the request carried none.
    Missing,
    /// The runtime does not accept the token that the request carried.
    Rejected,
}

/// A connection to a runtime's control endpoint.
#[derive(Debug)]
pub struct ControlClient {
    seq: u64,
    reader: BufReader<ControlStream>,
    auth: Option<String>,
    response_limit: Option<usize>,
}

impl ControlClient {
    /// Connects to `endpoint`; every read and write waits at most `timeout`. A blank `auth` is
    /// never sent as a credential.
    ///
    /// # Errors
    /// The endpoint cannot be reached.
    pub fn connect(
        endpoint: &ControlEndpoint,
        auth: Option<&str>,
        timeout: Duration,
    ) -> Result<Self, ControlError> {
        let transport = |error: std::io::Error| ControlError::Transport(error.to_string());
        let stream = match endpoint {
            ControlEndpoint::Tcp(address) => {
                let socket = resolve_socket_addr(address).ok_or_else(|| {
                    ControlError::Transport(format!("{address} is not a socket address"))
                })?;
                let stream = TcpStream::connect_timeout(&socket, timeout).map_err(transport)?;
                stream.set_read_timeout(Some(timeout)).map_err(transport)?;
                stream.set_write_timeout(Some(timeout)).map_err(transport)?;
                ControlStream::Tcp(stream)
            }
            #[cfg(unix)]
            ControlEndpoint::Unix(path) => {
                let stream = UnixStream::connect(path).map_err(transport)?;
                stream.set_read_timeout(Some(timeout)).map_err(transport)?;
                stream.set_write_timeout(Some(timeout)).map_err(transport)?;
                ControlStream::Unix(stream)
            }
        };
        Ok(Self {
            seq: 1,
            reader: BufReader::new(stream),
            auth: auth
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
            response_limit: None,
        })
    }

    /// Refuses as malformed every following response line longer than `bytes`, newline
    /// included. Without it a response line has no length limit.
    #[must_use]
    pub fn with_response_limit(mut self, bytes: usize) -> Self {
        self.response_limit = Some(bytes);
        self
    }

    /// Waits at most `timeout` for each read and write of the following requests.
    ///
    /// # Errors
    /// The connection refuses the setting, for example for a zero `timeout`.
    pub fn set_timeout(&self, timeout: Duration) -> Result<(), ControlError> {
        let transport = |error: std::io::Error| ControlError::Transport(error.to_string());
        match self.reader.get_ref() {
            ControlStream::Tcp(stream) => {
                stream.set_read_timeout(Some(timeout)).map_err(transport)?;
                stream.set_write_timeout(Some(timeout)).map_err(transport)
            }
            #[cfg(unix)]
            ControlStream::Unix(stream) => {
                stream.set_read_timeout(Some(timeout)).map_err(transport)?;
                stream.set_write_timeout(Some(timeout)).map_err(transport)
            }
        }
    }

    /// Sends one request of operation `kind` and waits for its answer.
    ///
    /// # Errors
    /// The connection failed, the runtime refused, or the answer is malformed.
    pub fn request(&mut self, kind: &str, params: Option<Value>) -> Result<Value, ControlError> {
        let id = self.seq;
        self.seq = self.seq.saturating_add(1);
        let mut payload = serde_json::Map::new();
        payload.insert("id".to_string(), Value::from(id));
        payload.insert("type".to_string(), Value::from(kind));
        if let Some(params) = params {
            payload.insert("params".to_string(), params);
        }
        if let Some(auth) = &self.auth {
            payload.insert("auth".to_string(), Value::from(auth.clone()));
        }
        let mut line = Value::Object(payload).to_string();
        line.push('\n');
        let transport = |error: std::io::Error| ControlError::Transport(error.to_string());
        let stream = self.reader.get_mut();
        stream.write_all(line.as_bytes()).map_err(transport)?;
        stream.flush().map_err(transport)?;
        let mut answer = Vec::new();
        let read = match self.response_limit {
            // Reading one byte past the limit shows that the line is longer.
            Some(limit) => (&mut self.reader)
                .take(u64::try_from(limit).unwrap_or(u64::MAX).saturating_add(1))
                .read_until(b'\n', &mut answer),
            None => self.reader.read_until(b'\n', &mut answer),
        }
        .map_err(transport)?;
        if read == 0 {
            return Err(ControlError::Transport(
                "the runtime closed the connection".into(),
            ));
        }
        if let Some(limit) = self.response_limit
            && answer.len() > limit
        {
            return Err(ControlError::Malformed(format!(
                "the answer to {kind} is longer than {limit} bytes"
            )));
        }
        let response: ControlResponse = serde_json::from_slice(&answer)
            .map_err(|error| ControlError::Malformed(error.to_string()))?;
        // The runtime answers with the request's id (E0.1). An answer with another id belongs to
        // another request, such as one that timed out earlier on this connection. An answer
        // without an id is taken as this one's: a connection carries one request at a time.
        if let Some(answer_id) = response.id
            && answer_id != id
        {
            return Err(ControlError::Malformed(format!(
                "the answer to {kind} carries id {answer_id} instead of {id}"
            )));
        }
        if !response.ok {
            return Err(ControlError::Refused {
                error: response
                    .error
                    .unwrap_or_else(|| format!("the runtime refused {kind}")),
                code: response.error_code,
            });
        }
        // A successful answer carries a result (E0.1).
        response
            .result
            .ok_or_else(|| ControlError::Malformed(format!("the answer to {kind} has no result")))
    }
}

#[derive(Debug, Deserialize)]
struct ControlResponse {
    #[serde(default)]
    id: Option<u64>,
    ok: bool,
    #[serde(default)]
    result: Option<Value>,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    error_code: Option<String>,
}

fn resolve_socket_addr(address: &str) -> Option<SocketAddr> {
    address
        .parse::<SocketAddr>()
        .ok()
        .or_else(|| address.to_socket_addrs().ok()?.next())
}

#[derive(Debug)]
enum ControlStream {
    Tcp(TcpStream),
    #[cfg(unix)]
    Unix(UnixStream),
}

impl Read for ControlStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            Self::Tcp(stream) => stream.read(buf),
            #[cfg(unix)]
            Self::Unix(stream) => stream.read(buf),
        }
    }
}

impl Write for ControlStream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            Self::Tcp(stream) => stream.write(buf),
            #[cfg(unix)]
            Self::Unix(stream) => stream.write(buf),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            Self::Tcp(stream) => stream.flush(),
            #[cfg(unix)]
            Self::Unix(stream) => stream.flush(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use std::thread;

    fn tcp_address(endpoint: ControlEndpoint) -> String {
        match endpoint {
            ControlEndpoint::Tcp(address) => address,
            #[cfg(unix)]
            ControlEndpoint::Unix(_) => panic!("expected TCP endpoint"),
        }
    }

    #[test]
    fn tcp_endpoint_requires_exact_scheme_and_nonempty_address() {
        assert_eq!(
            tcp_address(ControlEndpoint::parse("tcp://127.0.0.1:9000").unwrap()),
            "127.0.0.1:9000"
        );
        assert!(ControlEndpoint::parse("TCP://127.0.0.1:9000").is_none());
        assert!(ControlEndpoint::parse(" tcp://127.0.0.1:9000").is_none());
        assert!(ControlEndpoint::parse("tcp://").is_none());
    }

    #[cfg(unix)]
    #[test]
    fn unix_endpoint_requires_nonempty_absolute_path() {
        match ControlEndpoint::parse("unix:///tmp/trust-runtime.sock").unwrap() {
            ControlEndpoint::Unix(path) => {
                assert_eq!(path, std::path::PathBuf::from("/tmp/trust-runtime.sock"));
            }
            ControlEndpoint::Tcp(_) => panic!("expected Unix endpoint"),
        }
        assert!(ControlEndpoint::parse("unix://").is_none());
        assert!(ControlEndpoint::parse("unix://relative.sock").is_none());
    }

    #[test]
    fn numeric_socket_addresses_resolve_without_rewriting() {
        assert_eq!(
            resolve_socket_addr("127.0.0.1:9000"),
            Some("127.0.0.1:9000".parse().unwrap())
        );
        assert_eq!(
            resolve_socket_addr("[::1]:9000"),
            Some("[::1]:9000".parse().unwrap())
        );
        assert!(resolve_socket_addr("127.0.0.1").is_none());
    }

    #[test]
    fn a_refusal_keeps_the_runtimes_text_and_code() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let address = listener.local_addr().expect("address");
        let server = thread::spawn(move || {
            let (stream, _) = listener.accept().expect("accept");
            let mut reader = BufReader::new(stream.try_clone().expect("clone"));
            let mut request = String::new();
            reader.read_line(&mut request).expect("request");
            let mut stream = stream;
            writeln!(stream, r#"{{"id":1,"ok":false,"error":"Recording channels are limited to 32.","error_code":"recording_unavailable"}}"#)
                .expect("answer");
            serde_json::from_str::<Value>(&request).expect("json request")
        });
        let endpoint = ControlEndpoint::parse(&format!("tcp://{address}")).expect("endpoint");
        let mut client =
            ControlClient::connect(&endpoint, Some("  "), Duration::from_secs(2)).expect("connect");
        let refusal = client
            .request("control.recording.start", Some(serde_json::json!({})))
            .expect_err("refused");
        assert_eq!(
            refusal,
            ControlError::Refused {
                error: "Recording channels are limited to 32.".into(),
                code: Some("recording_unavailable".into())
            }
        );
        let request = server.join().expect("server");
        assert!(request.get("auth").is_none(), "a blank token is never sent");
        assert_eq!(request["type"], "control.recording.start");
    }
}
