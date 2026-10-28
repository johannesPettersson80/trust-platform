//! The exchange with the window's loopback (specification 43, §3.1): one HTTP/1.1 POST per request,
//! on its own connection to 127.0.0.1, with the capability as the bearer token.
//!
//! The loopback has three routes: the tool list, a tool call and the cancellation of a call. Its
//! answer must have status 200 and a `Content-Length` body. The body is read whole, at most the
//! limit's raw bytes, and parsed once complete, so a character split across network reads arrives
//! intact (specification 43, Repairs — 7 October 2026, item 3).

use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::{Duration, Instant};

use serde_json::Value;

/// The route that answers the tools: `{ "tools": [...] }`.
pub(crate) const TOOLS: &str = "/mcp/tools";
/// The route that runs a tool: `{ "id", "name", "arguments" }`, answered with the tool's result.
pub(crate) const CALL: &str = "/mcp/call";
/// The route that cancels the call with an id: `{ "id" }`.
pub(crate) const CANCEL: &str = "/mcp/cancel";

/// How an exchange with the loopback failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExchangeError {
    /// The connection could not be made, or broke.
    Unreachable,
    /// The exchange took longer than its time.
    TimedOut,
    /// The loopback answered with a status other than 200.
    Status,
    /// The answer has more raw bytes than the limit.
    TooLarge,
    /// The answer is not an HTTP/1.x answer with a `Content-Length` JSON body.
    Malformed,
}

impl From<io::Error> for ExchangeError {
    fn from(error: io::Error) -> Self {
        match error.kind() {
            io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock => Self::TimedOut,
            _ => Self::Unreachable,
        }
    }
}

/// The loopback of one connection.
#[derive(Clone)]
pub(crate) struct Bridge {
    address: SocketAddr,
    /// `Bearer <capability>`.
    authorization: String,
    /// How long one exchange may take.
    time: Duration,
    /// The largest answer, in raw bytes.
    answer_bytes: usize,
}

impl Bridge {
    pub(crate) fn new(
        address: SocketAddr,
        capability: &str,
        time: Duration,
        answer_bytes: usize,
    ) -> Self {
        Self {
            address,
            authorization: format!("Bearer {capability}"),
            time,
            answer_bytes,
        }
    }

    /// Posts `body` to `route` and returns the loopback's JSON answer.
    pub(crate) fn post(&self, route: &str, body: &Value) -> Result<Value, ExchangeError> {
        let deadline = Instant::now() + self.time;
        let mut stream = TcpStream::connect_timeout(&self.address, remaining(deadline)?)?;
        let body = body.to_string();
        let mut request = format!(
            "POST {route} HTTP/1.1\r\n\
             Host: {address}\r\n\
             Authorization: {authorization}\r\n\
             Content-Type: application/json\r\n\
             Accept: application/json\r\n\
             Content-Length: {length}\r\n\
             Connection: close\r\n\r\n",
            address = self.address,
            authorization = self.authorization,
            length = body.len(),
        )
        .into_bytes();
        // One write for the whole request: a separate small write would wait for the loopback's
        // delayed acknowledgement.
        request.extend_from_slice(body.as_bytes());
        stream.set_write_timeout(Some(remaining(deadline)?))?;
        stream.write_all(&request)?;
        read_answer(&mut stream, deadline, self.answer_bytes)
    }
}

/// Reads one answer: its head, then exactly its `Content-Length` body, parsed once complete.
fn read_answer(
    stream: &mut TcpStream,
    deadline: Instant,
    limit: usize,
) -> Result<Value, ExchangeError> {
    let mut received = Vec::new();
    let mut buffer = vec![0_u8; 64 * 1024];
    let mut scanned = 0;
    let end_of_head = loop {
        if let Some(end) = head_end(&received, scanned) {
            break end;
        }
        scanned = received.len();
        if received.len() > limit {
            return Err(ExchangeError::TooLarge);
        }
        let read = read_some(stream, &mut buffer, deadline)?;
        if read == 0 {
            return Err(ExchangeError::Malformed);
        }
        received.extend_from_slice(&buffer[..read]);
    };
    let head =
        std::str::from_utf8(&received[..end_of_head]).map_err(|_| ExchangeError::Malformed)?;
    let length = body_length(head)?;
    if length > limit {
        return Err(ExchangeError::TooLarge);
    }
    let mut body = received.split_off(end_of_head);
    while body.len() < length {
        let read = read_some(stream, &mut buffer, deadline)?;
        if read == 0 {
            return Err(ExchangeError::Malformed);
        }
        body.extend_from_slice(&buffer[..read]);
    }
    if body.len() != length {
        return Err(ExchangeError::Malformed);
    }
    serde_json::from_slice(&body).map_err(|_| ExchangeError::Malformed)
}

/// The end of the head (after its empty line) in `received`, searching from `scanned` on.
fn head_end(received: &[u8], scanned: usize) -> Option<usize> {
    let from = scanned.saturating_sub(3);
    received
        .get(from..)?
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|at| from + at + 4)
}

/// The body length of an answer head whose status is 200.
fn body_length(head: &str) -> Result<usize, ExchangeError> {
    let mut lines = head.split("\r\n");
    let mut status_line = lines.next().unwrap_or_default().split(' ');
    let version = status_line.next().unwrap_or_default();
    let status = status_line.next().and_then(|code| code.parse::<u16>().ok());
    if !version.starts_with("HTTP/1.") {
        return Err(ExchangeError::Malformed);
    }
    match status {
        Some(200) => {}
        Some(_) => return Err(ExchangeError::Status),
        None => return Err(ExchangeError::Malformed),
    }
    let mut length = None;
    for line in lines {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        if name.eq_ignore_ascii_case("transfer-encoding") {
            return Err(ExchangeError::Malformed);
        }
        if name.eq_ignore_ascii_case("content-length") {
            let value = value
                .trim()
                .parse::<usize>()
                .map_err(|_| ExchangeError::Malformed)?;
            if length.is_some_and(|known| known != value) {
                return Err(ExchangeError::Malformed);
            }
            length = Some(value);
        }
    }
    length.ok_or(ExchangeError::Malformed)
}

/// Reads what has arrived, waiting at most until the deadline.
fn read_some(
    stream: &mut TcpStream,
    buffer: &mut [u8],
    deadline: Instant,
) -> Result<usize, ExchangeError> {
    loop {
        stream.set_read_timeout(Some(remaining(deadline)?))?;
        match stream.read(buffer) {
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            result => return Ok(result?),
        }
    }
}

/// The time left until the deadline; none left is a timeout.
fn remaining(deadline: Instant) -> Result<Duration, ExchangeError> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|left| !left.is_zero())
        .ok_or(ExchangeError::TimedOut)
}
