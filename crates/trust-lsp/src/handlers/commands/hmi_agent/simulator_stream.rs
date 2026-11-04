//! The readback stream of a simulator journey (specification 43, AT-08): the page's `/ws/hmi`
//! stream, opened with the named session's cookie so that it also carries that session's command
//! outcomes. It holds no rule of the journey.
//!
//! The upgrade request goes out through `simulator_http`, which writes the cookie and the access
//! token without logging them; tungstenite only frames the messages after it. tungstenite's own
//! client handshake is not used, because it logs its whole request, cookie and token included, at
//! trace level (tungstenite 0.30.0, `src/handshake/client.rs`, line 197).

use std::io;
use std::time::{Duration, Instant};

use tungstenite::handshake::client::generate_key;
use tungstenite::handshake::derive_accept_key;
use tungstenite::protocol::{Role, WebSocketConfig};
use tungstenite::{Message, WebSocket};

use super::error::{AgentError, AgentResult, NOT_CONNECTED};
use super::simulator_connection::Connection;
use super::simulator_http::{SimulatorHmi, MAX_BODY};

/// What one read of the stream gave.
pub(super) enum Received {
    /// A message's text.
    Text(String),
    /// Nothing within the wait.
    Idle,
    /// The stream ended.
    Closed,
}

/// An open readback stream.
pub(super) struct Readback {
    socket: WebSocket<Box<dyn Connection>>,
}

impl Readback {
    /// Opens the stream at `path` within `timeout`.
    ///
    /// # Errors
    ///
    /// `NOT_CONNECTED` when the simulator does not open it.
    pub(super) fn open(hmi: &SimulatorHmi<'_>, path: &str, timeout: Duration) -> AgentResult<Self> {
        let key = generate_key();
        let upgrade = [
            ("Upgrade", "websocket"),
            ("Connection", "Upgrade"),
            ("Sec-WebSocket-Key", key.as_str()),
            ("Sec-WebSocket-Version", "13"),
        ];
        let deadline = Instant::now() + timeout;
        let (connection, head) = hmi.exchange("GET", path, "HTTP/1.1", &upgrade, "", deadline)?;
        let accept = derive_accept_key(key.as_bytes());
        if head.status != 101 || head.header("sec-websocket-accept") != Some(accept.as_str()) {
            let message = format!(
                "simulator readback stream unavailable: the simulator answered {}.",
                head.status
            );
            return Err(AgentError::new(NOT_CONNECTED, message));
        }
        let config = WebSocketConfig::default()
            .max_message_size(Some(MAX_BODY))
            .max_frame_size(Some(MAX_BODY));
        let socket =
            WebSocket::from_partially_read(connection, head.rest, Role::Client, Some(config));
        Ok(Self { socket })
    }

    /// The next message, waiting at most `wait` for it.
    pub(super) fn next(&mut self, wait: Duration) -> Received {
        if self.socket.get_ref().wait_at_most(wait).is_err() {
            return Received::Closed;
        }
        match self.socket.read() {
            Ok(Message::Text(text)) => Received::Text(text.as_str().to_owned()),
            Ok(Message::Binary(bytes)) => {
                Received::Text(String::from_utf8_lossy(&bytes).into_owned())
            }
            Ok(Message::Ping(_) | Message::Pong(_) | Message::Frame(_)) => Received::Idle,
            Ok(Message::Close(_)) => Received::Closed,
            Err(tungstenite::Error::Io(error))
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                ) =>
            {
                Received::Idle
            }
            Err(_) => Received::Closed,
        }
    }

    /// Closes the stream without waiting for the simulator's answer.
    pub(super) fn close(mut self) {
        let _ = self.socket.close(None);
        let _ = self.socket.flush();
    }
}
