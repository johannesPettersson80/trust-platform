//! The HTTP transport of a simulator journey (specification 43, AT-08): one request to the
//! simulator's HMI web server and its answer, over the connection of `simulator_connection`. It
//! holds no rule of the journey.
//!
//! Each request carries the runtime's access token (`X-Trust-Token`) and the named session's cookie
//! in its headers only: nothing here logs a request, a header or an answer's headers, and no error
//! quotes a header's value. A request is HTTP/1.0 with `Connection: close`, so its answer is never
//! chunked and ends with the connection. A redirect is reported, never followed. Every read is
//! bounded: the answer's status line and headers to 16 KiB, its body to 8 MiB, and the whole
//! exchange to the request's timeout (15 seconds, the extension's).

use std::io::{self, Write};
use std::time::{Duration, Instant};

use serde_json::Value;
use tower_lsp::lsp_types::Url;

use super::error::{AgentError, AgentResult, NOT_CONNECTED};
use super::simulator_connection::{connect, time_left, Connection};

/// How long one exchange may take: connecting, sending and reading the whole answer.
pub(super) const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);
/// The longest status line and headers of an answer.
const MAX_HEAD: usize = 16 * 1024;
/// The longest body of an answer, and the longest message of the readback stream.
pub(super) const MAX_BODY: usize = 8 * 1024 * 1024;
/// The most bytes one read takes.
const CHUNK: usize = 16 * 1024;

/// An answer's status, its headers (names in lower case) and the bytes read after them: the start of
/// the body, or of the readback stream's frames.
pub(super) struct Head {
    pub(super) status: u16,
    headers: Vec<(String, String)>,
    pub(super) rest: Vec<u8>,
}

impl Head {
    /// The value of the header `name`, given in lower case.
    pub(super) fn header(&self, name: &str) -> Option<&str> {
        let found = self.headers.iter().find(|(key, _)| key == name);
        found.map(|(_, value)| value.as_str())
    }
}

/// The simulator's HMI web server as one journey reaches it: its web address (such as
/// `http://127.0.0.1:8080`), the runtime's access token, the named session's cookie
/// (`trust_hmi_session=…`) and how long one exchange may take.
pub(super) struct SimulatorHmi<'a> {
    pub(super) base: &'a str,
    pub(super) token: Option<&'a str>,
    pub(super) cookie: &'a str,
    pub(super) timeout: Duration,
}

impl SimulatorHmi<'_> {
    /// `GET path`, or `POST path` with a JSON body: the answer's status and its body as JSON, `null`
    /// when the body is not JSON.
    pub(super) fn send(&self, path: &str, body: Option<&Value>) -> AgentResult<(u16, Value)> {
        let deadline = Instant::now() + self.timeout;
        let body = body.map(Value::to_string);
        let length = body.as_ref().map(|body| body.len().to_string());
        let mut headers = vec![("Accept", "application/json"), ("Connection", "close")];
        if let Some(length) = &length {
            headers.push(("Content-Type", "application/json"));
            headers.push(("Content-Length", length));
        }
        let method = if body.is_some() { "POST" } else { "GET" };
        let content = body.as_deref().unwrap_or_default();
        let (mut connection, head) =
            self.exchange(method, path, "HTTP/1.0", &headers, content, deadline)?;
        if (300..400).contains(&head.status) {
            return Err(self.failure("answered with a redirect, which a journey never follows"));
        }
        let coding = head.header("transfer-encoding");
        if coding.is_some_and(|coding| !coding.eq_ignore_ascii_case("identity")) {
            return Err(self.failure("answered in a transfer encoding a journey does not read"));
        }
        let length = head.header("content-length").map(str::parse::<usize>);
        let length = length.transpose().map_err(|_| self.not_http())?;
        let too_long = "answered with a body longer than 8 MiB";
        if length.is_some_and(|length| length > MAX_BODY) {
            return Err(self.failure(too_long));
        }
        let (status, mut body) = (head.status, head.rest);
        let enough = |bytes: &[u8]| length.is_some_and(|length| bytes.len() >= length);
        let connection = connection.as_mut();
        let ended = self.read_until(connection, &mut body, MAX_BODY, too_long, deadline, enough)?;
        if let Some(length) = length {
            if !ended {
                return Err(self.failure("closed the connection before its answer ended"));
            }
            body.truncate(length);
        }
        Ok((status, serde_json::from_slice(&body).unwrap_or(Value::Null)))
    }

    /// Connects, sends one request with `headers`, the access token and the cookie, and reads the
    /// answer's status line and headers. The readback stream's upgrade uses it too.
    pub(super) fn exchange(
        &self,
        method: &str,
        path: &str,
        version: &str,
        headers: &[(&str, &str)],
        body: &str,
        deadline: Instant,
    ) -> AgentResult<(Box<dyn Connection>, Head)> {
        let url = Url::parse(&format!("{}{path}", self.base))
            .map_err(|_| self.failure("is not a web address"))?;
        let host = url.host_str().unwrap_or_default();
        let authority = match url.port() {
            Some(port) => format!("{host}:{port}"),
            None => host.to_owned(),
        };
        let target = match url.query() {
            Some(query) => format!("{}?{query}", url.path()),
            None => url.path().to_owned(),
        };
        let mut request = format!("{method} {target} {version}\r\nHost: {authority}\r\n");
        let credentials = [("X-Trust-Token", self.token), ("Cookie", Some(self.cookie))];
        let named = headers.iter().map(|&(name, value)| (name, Some(value)));
        for (name, value) in named.chain(credentials) {
            let Some(value) = value else { continue };
            let allowed = |byte: u8| byte == b'\t' || (b' '..=b'~').contains(&byte);
            if !value.bytes().all(allowed) {
                return Err(self.failure("would get a header value that HTTP does not allow"));
            }
            request.push_str(&format!("{name}: {value}\r\n"));
        }
        request.push_str("\r\n");
        request.push_str(body);
        let mut connection = connect(&url, deadline).map_err(|error| self.unreachable(&error))?;
        connection
            .write_all(request.as_bytes())
            .and_then(|()| connection.flush())
            .map_err(|error| self.unreachable(&error))?;
        let mut buffer = Vec::new();
        let ended = |bytes: &[u8]| bytes.windows(4).any(|window| window == b"\r\n\r\n");
        let too_long = "answered with headers longer than 16 KiB";
        let reading = connection.as_mut();
        if !self.read_until(reading, &mut buffer, MAX_HEAD, too_long, deadline, ended)? {
            return Err(self.failure("closed the connection before it answered"));
        }
        let end = buffer.windows(4).position(|window| window == b"\r\n\r\n");
        let end = end.unwrap_or(buffer.len());
        let rest = buffer.split_off((end + 4).min(buffer.len()));
        let text = std::str::from_utf8(&buffer[..end]).map_err(|_| self.not_http())?;
        let mut lines = text.split("\r\n");
        let status = lines
            .next()
            .and_then(|line| line.strip_prefix("HTTP/")?.split(' ').nth(1)?.parse().ok())
            .ok_or_else(|| self.not_http())?;
        let headers = lines
            .map(|line| {
                let (name, value) = line.split_once(':')?;
                Some((name.trim().to_ascii_lowercase(), value.trim().to_owned()))
            })
            .collect::<Option<_>>()
            .ok_or_else(|| self.not_http())?;
        let head = Head {
            status,
            headers,
            rest,
        };
        Ok((connection, head))
    }

    /// Reads onto `buffer` until `done` holds for it (`true`) or the server closes the connection
    /// (`false`); more than `limit` bytes are refused with `too_long`.
    fn read_until(
        &self,
        connection: &mut dyn Connection,
        buffer: &mut Vec<u8>,
        limit: usize,
        too_long: &str,
        deadline: Instant,
        done: impl Fn(&[u8]) -> bool,
    ) -> AgentResult<bool> {
        let mut chunk = [0_u8; CHUNK];
        while !done(buffer) {
            if buffer.len() > limit {
                return Err(self.failure(too_long));
            }
            let waited = time_left(deadline).and_then(|wait| connection.wait_at_most(wait));
            waited.map_err(|error| self.unreachable(&error))?;
            let room = (limit + 1 - buffer.len()).min(CHUNK);
            match connection.read(&mut chunk[..room]) {
                Ok(0) => return Ok(false),
                Ok(read) => buffer.extend_from_slice(&chunk[..read]),
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(error) => return Err(self.unreachable(&error)),
            }
        }
        Ok(true)
    }

    /// The refusal of a connection that failed or ran out of time.
    fn unreachable(&self, error: &io::Error) -> AgentError {
        match error.kind() {
            io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut => {
                self.failure(&format!("did not answer within {:?}", self.timeout))
            }
            _ => self.failure(&format!("could not be reached: {error}")),
        }
    }

    fn not_http(&self) -> AgentError {
        self.failure("answered with something that is not HTTP")
    }

    fn failure(&self, what: &str) -> AgentError {
        let message = format!("the simulator's HMI at {} {what}.", self.base);
        AgentError::new(NOT_CONNECTED, message)
    }
}

#[cfg(test)]
#[path = "simulator_http_tests.rs"]
mod tests;
