//! The control connection over TCP (specification 34 ARCH-12 and PORT-05): one client at a time on
//! the control port, its bytes framed by the line link every control link shares, and a second client
//! told at once that another holds the connection.

use alloc::boxed::Box;
use alloc::format;
use alloc::vec;
use alloc::vec::Vec;
use core::net::SocketAddr;

use serde::Deserialize;

use smoltcp::iface::{SocketHandle, SocketSet};
use smoltcp::socket::tcp;
use smoltcp::time::Duration;
use trust_runtime_core::ports::{LinkFrame, MonotonicInstant, PeerClass};
use trust_runtime_services::link::{DeviceLineLink, ReplyRefused};

/// The error a second client gets, as the reply to its first request, while another holds the
/// control connection; its connection is closed then.
pub const BUSY_ERROR: &str =
    "server busy: the device serves one control connection at a time, and another client holds it";

/// The `error_code` of the busy reply, which a client connects again for.
pub const BUSY_CODE: &str = "server_busy";

/// The most bytes of a second client's first request that are read for its `id`.
const BUSY_REQUEST_BYTES: usize = 512;

/// Bytes of the control socket's receive buffer, the window a client may send into: most of a
/// transfer part's line.
const RECEIVE_BYTES: usize = 4_096;
/// Bytes of the control socket's send buffer.
const SEND_BYTES: usize = 4_096;
/// How often an idle connection is probed, so that a client that vanished is noticed.
const KEEP_ALIVE: Duration = Duration::from_secs(10);
/// How long a connection lives without a packet from its client before it is dropped.
const SILENCE: Duration = Duration::from_secs(30);

/// The control port's sockets and the line link of its connection.
pub(crate) struct ControlServer {
    port: u16,
    /// The socket of the one connection.
    socket: SocketHandle,
    /// The socket that tells a second client that the device is busy.
    busy: SocketHandle,
    /// The framing of the connection's bytes; boxed, since it holds two frames.
    link: Box<DeviceLineLink>,
    /// The client of the connection, while one is connected.
    peer: Option<SocketAddr>,
    /// The client has closed its side and every byte it sent is in the link.
    client_done: bool,
    /// The link has served every request of a client that is done: the connection closes.
    close_due: bool,
    /// The reply the busy socket sends its client, once its first request has arrived.
    busy_reply: Option<Vec<u8>>,
    /// Bytes of the busy reply the busy socket has sent.
    busy_sent: usize,
}

impl ControlServer {
    /// The control connection on `port`, its sockets added to `sockets`.
    pub(crate) fn new(sockets: &mut SocketSet<'static>, port: u16) -> Self {
        let mut socket = tcp::Socket::new(
            tcp::SocketBuffer::new(vec![0; RECEIVE_BYTES]),
            tcp::SocketBuffer::new(vec![0; SEND_BYTES]),
        );
        // A reply goes out whole at once, and every segment is acknowledged at once: the loop may
        // wait longer than a delayed acknowledgement's timer.
        socket.set_nagle_enabled(false);
        socket.set_ack_delay(None);
        socket.set_keep_alive(Some(KEEP_ALIVE));
        socket.set_timeout(Some(SILENCE));
        let mut busy = tcp::Socket::new(
            tcp::SocketBuffer::new(vec![0; BUSY_REQUEST_BYTES]),
            tcp::SocketBuffer::new(vec![0; 256]),
        );
        busy.set_timeout(Some(SILENCE));
        Self {
            port,
            socket: sockets.add(socket),
            busy: sockets.add(busy),
            link: Box::new(DeviceLineLink::new(PeerClass::Local)),
            peer: None,
            client_done: false,
            close_due: false,
            busy_reply: None,
            busy_sent: 0,
        }
    }

    /// The TCP port of the control connection.
    pub(crate) fn port(&self) -> u16 {
        self.port
    }

    /// Moves the bytes between the sockets and the line link: what the client sent into the link,
    /// and the reply in flight into the socket. A connection that ended ends its session, and the
    /// port listens again.
    pub(crate) fn pump(&mut self, sockets: &mut SocketSet<'static>) {
        self.serve_busy(sockets);
        let socket = sockets.get_mut::<tcp::Socket>(self.socket);
        if !socket.is_open() {
            if self.peer.take().is_some() {
                self.link.reset_session();
            }
            self.client_done = false;
            self.close_due = false;
            let _ = socket.listen(self.port);
            return;
        }
        if socket.is_listening() {
            return;
        }
        if self.peer.is_none() {
            // A new client: nothing of an earlier one's session reaches it.
            self.link.reset_session();
            self.peer = Some(
                socket
                    .remote_endpoint()
                    .map_or(SocketAddr::from(([0, 0, 0, 0], 0)), SocketAddr::from),
            );
        }
        let link = &mut self.link;
        while socket.can_recv() && link.receive_capacity() > 0 {
            let taken = socket
                .recv(|bytes| {
                    let taken = link.receive(bytes);
                    (taken, taken)
                })
                .unwrap_or(0);
            if taken == 0 {
                break;
            }
        }
        let unsent = link.unsent();
        if !unsent.is_empty()
            && socket.can_send()
            && let Ok(sent) = socket.send_slice(unsent)
        {
            link.mark_sent(sent);
        }
        if socket.state() == tcp::State::CloseWait && !socket.can_recv() {
            self.client_done = true;
        }
        if self.close_due {
            // The client has closed its side and has the reply to every request it sent.
            self.close_due = false;
            socket.close();
        }
    }

    /// The next request of the connection, with its frame at the start of `buffer`.
    pub(crate) fn poll_frame(
        &mut self,
        buffer: &mut [u8],
        now: MonotonicInstant,
    ) -> Option<LinkFrame> {
        let peer = PeerClass::Network(self.peer?);
        let Some(frame) = self.link.poll_frame(buffer, now) else {
            self.close_due = self.client_done && !self.link.reply_in_flight();
            return None;
        };
        Some(match frame {
            LinkFrame::Frame { len, .. } => LinkFrame::Frame { len, peer },
            LinkFrame::Oversized { .. } => LinkFrame::Oversized { peer },
        })
    }

    /// Starts sending `frame`, the reply to the request polled last.
    pub(crate) fn send_frame(
        &mut self,
        frame: &[u8],
        now: MonotonicInstant,
    ) -> Result<(), ReplyRefused> {
        self.link.send_frame(frame, now)
    }

    /// Whether a reply is still being sent.
    pub(crate) fn sending(&self) -> bool {
        self.link.reply_in_flight()
    }

    /// Answers a second client's first request with [`BUSY_ERROR`], then closes its connection.
    fn serve_busy(&mut self, sockets: &mut SocketSet<'static>) {
        let busy = sockets.get_mut::<tcp::Socket>(self.busy);
        if !busy.is_open() {
            self.busy_reply = None;
            self.busy_sent = 0;
            let _ = busy.listen(self.port);
            return;
        }
        if busy.is_listening() || busy.state() == tcp::State::SynReceived {
            // No client yet, or one whose connection is still being opened.
            return;
        }
        if self.busy_reply.is_none() {
            let mut request = [0u8; BUSY_REQUEST_BYTES];
            let read = busy.peek_slice(&mut request).unwrap_or(0);
            let line = request[..read].split(|&byte| byte == b'\n').next();
            let whole = read < BUSY_REQUEST_BYTES && line.is_some_and(|line| line.len() < read);
            if !whole && read < BUSY_REQUEST_BYTES && busy.may_recv() {
                // The first request has not arrived whole yet.
                return;
            }
            self.busy_reply = Some(busy_reply(line.unwrap_or_default()));
        }
        while busy.can_recv() {
            if busy.recv(|bytes| (bytes.len(), ())).is_err() {
                break;
            }
        }
        let reply = self.busy_reply.as_deref().unwrap_or_default();
        if self.busy_sent < reply.len()
            && let Ok(sent) = busy.send_slice(&reply[self.busy_sent..])
        {
            self.busy_sent += sent;
        }
        if self.busy_sent == reply.len() {
            busy.close();
        }
    }
}

/// The `id` of a control request.
#[derive(Deserialize)]
struct RequestId {
    id: Option<u64>,
}

/// The busy reply to the request `line`: with its `id`, so that a client takes it as the request's
/// reply, or without one when the line is not a request whose `id` reads.
fn busy_reply(line: &[u8]) -> Vec<u8> {
    let id = serde_json::from_slice::<RequestId>(line)
        .ok()
        .and_then(|request| request.id);
    let reply = match id {
        Some(id) => format!(
            "{{\"id\":{id},\"ok\":false,\"error\":\"{BUSY_ERROR}\",\"error_code\":\"{BUSY_CODE}\"}}\n"
        ),
        None => {
            format!("{{\"ok\":false,\"error\":\"{BUSY_ERROR}\",\"error_code\":\"{BUSY_CODE}\"}}\n")
        }
    };
    reply.into_bytes()
}
