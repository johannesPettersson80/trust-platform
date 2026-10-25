//! The runtime's control endpoint: one JSON object per line each way (runtime protocol annex,
//! E0.1). Inline values and Control Studio share this client.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
#[cfg(unix)]
use std::os::unix::net::UnixStream;
use std::time::Duration;

use serde::Deserialize;
use serde_json::Value;

mod long_reply;
mod serial_port;
mod target_status;
mod transport;

pub use serial_port::{SerialLine, SerialStream, SERIAL_FRAME_BYTES};
pub use target_status::{probe, ProbeTimeouts, TargetProbe};
pub use transport::ByteStream;

pub mod version;
pub use version::VersionAdvice;

#[cfg(test)]
mod client_tests;
#[cfg(test)]
mod fake_chip;
#[cfg(test)]
mod fake_runtime;
#[cfg(test)]
mod framed_link_tests;
#[cfg(test)]
mod serial_port_tests;

/// The longest response line, newline included, that the command line and the Devices panel
/// read: 1 MiB (specification 11, 6.8.1). A client that reads bulk results, such as a test run's
/// samples, sets its own limit or none.
pub const RESPONSE_LINE_LIMIT: usize = 1024 * 1024;

/// The most text that a client skips on a chip's port while it waits for a reply (specification
/// 34, PORT-05, "The client: one request in flight"): 64 KiB.
const SKIPPED_TEXT_LIMIT: usize = 64 * 1024;

/// The start of every reply line, which the runtime writes with its `id` first.
const REPLY_START: &[u8] = br#"{"id":"#;

/// Where a runtime listens for control requests: `tcp://host:port`, `unix:///absolute/path`, or,
/// for a chip on a cable, `serial:///dev/ttyACM0` (`serial://COM5` on Windows).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ControlEndpoint {
    /// A TCP address.
    Tcp(String),
    /// A Unix socket.
    #[cfg(unix)]
    Unix(std::path::PathBuf),
    /// A serial port by the name the operating system gives it: a chip's USB serial port
    /// (specification 34, PORT-05).
    Serial(String),
}

impl ControlEndpoint {
    /// Reads an endpoint; the scheme is exact and the address is not empty. A serial port is an
    /// absolute path on Unix, and any name that is not empty elsewhere (`COM5`).
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
        if let Some(rest) = text.strip_prefix("serial://") {
            let name = rest.trim();
            #[cfg(unix)]
            let valid = std::path::Path::new(name).is_absolute();
            #[cfg(not(unix))]
            let valid = !name.is_empty();
            return valid.then(|| Self::Serial(name.to_string()));
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
    reader: BufReader<Box<dyn ByteStream>>,
    auth: Option<String>,
    response_limit: Option<usize>,
}

impl ControlClient {
    /// Connects to `endpoint`; every read and write waits at most `timeout`. A blank `auth` is
    /// never sent as a credential.
    ///
    /// A serial endpoint opens the port without resetting the chip (specification 34, PORT-05,
    /// "The client: opening the port").
    ///
    /// # Errors
    /// The endpoint cannot be reached, or the connection failed.
    pub fn connect(
        endpoint: &ControlEndpoint,
        auth: Option<&str>,
        timeout: Duration,
    ) -> Result<Self, ControlError> {
        let transport = |error: std::io::Error| ControlError::Transport(error.to_string());
        let stream: Box<dyn ByteStream> = match endpoint {
            ControlEndpoint::Tcp(address) => {
                let socket = resolve_socket_addr(address).ok_or_else(|| {
                    ControlError::Transport(format!("{address} is not a socket address"))
                })?;
                let stream = TcpStream::connect_timeout(&socket, timeout).map_err(transport)?;
                stream.set_timeout(timeout).map_err(transport)?;
                Box::new(stream)
            }
            #[cfg(unix)]
            ControlEndpoint::Unix(path) => {
                let stream = UnixStream::connect(path).map_err(transport)?;
                stream.set_timeout(timeout).map_err(transport)?;
                Box::new(stream)
            }
            ControlEndpoint::Serial(name) => serial_port::open(name, timeout).map_err(|error| {
                ControlError::Transport(format!("cannot open the serial port {name}: {error}"))
            })?,
        };
        Ok(Self::with_stream(stream, auth))
    }

    /// A client over `stream`, any [`ByteStream`]: a socket that the caller connected, a serial
    /// port set up by [`SerialStream::new`], or a test's. A blank `auth` is never sent as a
    /// credential.
    #[must_use]
    pub fn from_stream(stream: impl ByteStream + 'static, auth: Option<&str>) -> Self {
        Self::with_stream(Box::new(stream), auth)
    }

    fn with_stream(stream: Box<dyn ByteStream>, auth: Option<&str>) -> Self {
        Self {
            seq: 1,
            reader: BufReader::new(stream),
            auth: auth
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
            response_limit: None,
        }
    }

    /// Refuses as malformed every following response line longer than `bytes`, newline included,
    /// and every reply that the runtime holds whole (specification 34, PORT-05) and that would be
    /// longer as a line. Without it a response line has no length limit.
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
        self.reader
            .get_ref()
            .set_timeout(timeout)
            .map_err(|error| ControlError::Transport(error.to_string()))
    }

    /// Sends one request of operation `kind` and waits for its answer.
    ///
    /// # Errors
    /// The connection failed, the runtime refused, or the answer is malformed.
    pub fn request(&mut self, kind: &str, params: Option<Value>) -> Result<Value, ControlError> {
        let response = self.exchange(kind, params)?;
        result_of(response, kind)
    }

    /// Sends one request of operation `kind` and returns the whole response object: `id`, `ok`
    /// and, as the runtime wrote them, `result`, `error`, `error_code` and `audit_id`. A refusal
    /// is a response here, not an error; [`Self::request`] turns it into one.
    ///
    /// A reply that the runtime holds whole because it is longer than the link's frame is read
    /// with `transfer.get`, checked against its SHA-256 and returned as if it had come in one line
    /// (specification 34, PORT-05, "The client: a reply longer than the frame").
    ///
    /// # Errors
    /// The connection failed, the request is longer than the link's frame, or the answer is not a
    /// response object of this request.
    pub fn exchange(&mut self, kind: &str, params: Option<Value>) -> Result<Value, ControlError> {
        let id = self.seq;
        let response = self.round_trip(kind, params)?;
        let Some(head) = long_reply::head_of(&response)? else {
            return Ok(response);
        };
        let reply = self.read_held(kind, &head)?;
        check_id(&reply, kind, id)?;
        Ok(reply)
    }

    /// Writes one request and reads the line that answers it, and checks that the answer is this
    /// request's. One request is in flight at a time.
    fn round_trip(&mut self, kind: &str, params: Option<Value>) -> Result<Value, ControlError> {
        let id = self.seq;
        let line = self.request_line(id, kind, params)?;
        self.seq = self.seq.saturating_add(1);
        let transport = |error: std::io::Error| ControlError::Transport(error.to_string());
        let stream = self.reader.get_mut();
        stream.write_all(line.as_bytes()).map_err(transport)?;
        stream.flush().map_err(transport)?;
        let response = self.read_response(kind)?;
        check_id(&response, kind, id)?;
        Ok(response)
    }

    /// The request as one line with its line end. A request longer than the link's frame is
    /// refused here, before any byte is written (specification 34, PORT-05).
    fn request_line(
        &self,
        id: u64,
        kind: &str,
        params: Option<Value>,
    ) -> Result<String, ControlError> {
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
        if let Some(frame) = self.reader.get_ref().frame_bytes()
            && line.len() > frame
        {
            return Err(ControlError::Refused {
                error: format!("invalid request: request longer than {frame} bytes"),
                code: None,
            });
        }
        line.push('\n');
        Ok(line)
    }

    /// Reads the response object. On a link with a frame, the first line that holds a response
    /// is it; the text before it on the port is skipped (specification 34, PORT-05, "The client:
    /// one request in flight").
    fn read_response(&mut self, kind: &str) -> Result<Value, ControlError> {
        let framed = self.reader.get_ref().frame_bytes().is_some();
        let mut skipped = 0_usize;
        loop {
            let line = self.read_line(kind)?;
            if !framed {
                let response: Value = serde_json::from_slice(&line)
                    .map_err(|error| ControlError::Malformed(error.to_string()))?;
                return if response.is_object() {
                    Ok(response)
                } else {
                    Err(ControlError::Malformed(format!(
                        "the answer to {kind} is not a JSON object"
                    )))
                };
            }
            if let Some(response) = response_in(&line) {
                return Ok(response);
            }
            skipped += line.len();
            if skipped >= SKIPPED_TEXT_LIMIT {
                return Err(ControlError::Malformed(format!(
                    "the port does not speak the control protocol: {SKIPPED_TEXT_LIMIT} bytes \
                     of text came before any answer to {kind}"
                )));
            }
        }
    }

    /// Reads one line with its line end, no longer than the limits allow.
    fn read_line(&mut self, kind: &str) -> Result<Vec<u8>, ControlError> {
        let transport = |error: std::io::Error| ControlError::Transport(error.to_string());
        let limit = self.line_limit();
        let mut line = Vec::new();
        let read = match limit {
            // Reading one byte past the limit shows that the line is longer.
            Some(limit) => (&mut self.reader)
                .take(u64::try_from(limit).unwrap_or(u64::MAX).saturating_add(1))
                .read_until(b'\n', &mut line),
            None => self.reader.read_until(b'\n', &mut line),
        }
        .map_err(transport)?;
        if read == 0 {
            return Err(ControlError::Transport(
                "the runtime closed the connection".into(),
            ));
        }
        if let Some(limit) = limit
            && line.len() > limit
        {
            return Err(ControlError::Malformed(format!(
                "the answer to {kind} is longer than {limit} bytes"
            )));
        }
        Ok(line)
    }

    /// The longest line to read, newline included: the caller's limit, and a link's frame with its
    /// line end.
    fn line_limit(&self) -> Option<usize> {
        let frame = self
            .reader
            .get_ref()
            .frame_bytes()
            .map(|frame| frame.saturating_add(1));
        match (self.response_limit, frame) {
            (Some(limit), Some(frame)) => Some(limit.min(frame)),
            (limit, frame) => limit.or(frame),
        }
    }
}

/// The response object that `line` holds: the whole line, or, when text comes before it, the line
/// from its first `{"id":` (the boot loader's last line can lack its line end, and the reply
/// follows it). `None` for a line that holds none.
fn response_in(line: &[u8]) -> Option<Value> {
    let object = |bytes: &[u8]| {
        serde_json::from_slice::<Value>(bytes)
            .ok()
            .filter(Value::is_object)
    };
    object(line).or_else(|| {
        let start = line
            .windows(REPLY_START.len())
            .position(|window| window == REPLY_START)?;
        object(&line[start..])
    })
}

/// Checks that `response` answers request `id`. The runtime answers with the request's id (E0.1).
/// An answer with another id belongs to another request, such as one that timed out earlier on
/// this connection. An answer without an id is taken as this one's: a connection carries one
/// request at a time.
fn check_id(response: &Value, kind: &str, id: u64) -> Result<(), ControlError> {
    match response.get("id") {
        None | Some(Value::Null) => Ok(()),
        Some(Value::Number(number)) if number.as_u64() == Some(id) => Ok(()),
        Some(other) => Err(ControlError::Malformed(format!(
            "the answer to {kind} carries id {other} instead of {id}"
        ))),
    }
}

/// The result of `response`, a response object: a refusal is an error, and a successful response
/// has a result (E0.1).
fn result_of(response: Value, kind: &str) -> Result<Value, ControlError> {
    let response: ControlResponse = serde_json::from_value(response)
        .map_err(|error| ControlError::Malformed(error.to_string()))?;
    if !response.ok {
        return Err(ControlError::Refused {
            error: response
                .error
                .unwrap_or_else(|| format!("the runtime refused {kind}")),
            code: response.error_code,
        });
    }
    response
        .result
        .ok_or_else(|| ControlError::Malformed(format!("the answer to {kind} has no result")))
}

#[derive(Debug, Deserialize)]
struct ControlResponse {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use std::thread;

    fn tcp_address(endpoint: ControlEndpoint) -> String {
        let ControlEndpoint::Tcp(address) = endpoint else {
            panic!("expected TCP endpoint");
        };
        address
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
            ControlEndpoint::Tcp(_) | ControlEndpoint::Serial(_) => {
                panic!("expected Unix endpoint")
            }
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
