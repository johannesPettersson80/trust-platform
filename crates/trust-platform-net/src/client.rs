//! The TCP connections that a chip's I/O drivers keep to servers (specification 11 §6.5:
//! `modbus-tcp` and `mqtt`; specification 34 TARGET-01). A driver asks the network for a
//! [`ClientLink`] to an address and port, and at its cycle writes the bytes it sends and reads
//! the bytes that came; the network opens the connection when the driver asks, moves the bytes
//! between the scans and reports what became of the connection. When to open again is the
//! driver's exchange logic: a link that closed stays closed until the driver opens it.

use alloc::collections::VecDeque;
use alloc::rc::Rc;
use alloc::vec;
use core::cell::RefCell;

use smoltcp::iface::{Context, SocketHandle, SocketSet};
use smoltcp::socket::tcp;
use smoltcp::time::Duration;
use smoltcp::wire::Ipv4Address;

/// Bytes of a link's socket buffers and of each of its queues: a Modbus TCP frame is at most 260
/// bytes and an MQTT packet of the process image a few hundred, so a link holds a few of each.
pub const LINK_BYTES: usize = 2_048;

/// How long a connection lives without a packet from its server before it is dropped.
const SILENCE: Duration = Duration::from_secs(60);

/// The first local port of the drivers' connections, the start of the dynamic ports (RFC 6335).
const FIRST_PORT: u16 = 49_152;

/// What became of a link's connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkStatus {
    /// The driver has not asked for the connection, or closed it.
    Idle,
    /// The connection is being made.
    Connecting,
    /// The connection is up: bytes go both ways.
    Open,
    /// The connection ended or could not be made, with the reason.
    Closed(&'static str),
}

/// The state a driver and the network share for one connection.
struct Shared {
    remote: (Ipv4Address, u16),
    /// The driver wants the connection open.
    wanted: bool,
    /// The driver asked for a new connection, which the next poll makes.
    connect_due: bool,
    status: LinkStatus,
    /// Bytes the driver queued that the socket has not taken yet.
    to_send: VecDeque<u8>,
    /// Bytes the socket received that the driver has not read yet.
    received: VecDeque<u8>,
}

/// A driver's connection to a server, which the network carries.
#[derive(Clone)]
pub struct ClientLink(Rc<RefCell<Shared>>);

impl ClientLink {
    /// The server this link connects to.
    #[must_use]
    pub fn remote(&self) -> (Ipv4Address, u16) {
        self.0.borrow().remote
    }

    /// Asks for the connection: a new one, with both queues emptied, made by the next poll.
    pub fn open(&self) {
        let mut shared = self.0.borrow_mut();
        shared.wanted = true;
        shared.connect_due = true;
        shared.status = LinkStatus::Connecting;
        shared.to_send.clear();
        shared.received.clear();
    }

    /// Ends the connection: the next poll closes it, and the queues are emptied.
    pub fn close(&self) {
        let mut shared = self.0.borrow_mut();
        shared.wanted = false;
        shared.connect_due = false;
        shared.status = LinkStatus::Idle;
        shared.to_send.clear();
        shared.received.clear();
    }

    /// What became of the connection.
    #[must_use]
    pub fn status(&self) -> LinkStatus {
        self.0.borrow().status
    }

    /// Queues `bytes` to send on the open connection, whole or not at all.
    ///
    /// # Errors
    ///
    /// [`LinkFull`] when the connection is not open, or the queue has no room for all of them.
    pub fn send(&self, bytes: &[u8]) -> Result<(), LinkFull> {
        let mut shared = self.0.borrow_mut();
        if shared.status != LinkStatus::Open || shared.to_send.len() + bytes.len() > LINK_BYTES {
            return Err(LinkFull);
        }
        shared.to_send.extend(bytes);
        Ok(())
    }

    /// Moves the bytes received since the last call into `into`.
    pub fn receive(&self, into: &mut alloc::vec::Vec<u8>) {
        into.extend(self.0.borrow_mut().received.drain(..));
    }
}

/// The refusal of bytes that a link cannot queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LinkFull;

/// The network's side of the links: each link's socket.
pub(crate) struct Clients {
    links: alloc::vec::Vec<(ClientLink, SocketHandle)>,
    next_port: u16,
}

impl Clients {
    /// No links yet; the local ports start at an offset from `random_seed`, so that a chip that
    /// resets does not reuse the ports of its connections before the reset at once.
    pub(crate) fn new(random_seed: u64) -> Self {
        let span = u64::from(u16::MAX - FIRST_PORT) + 1;
        Self {
            links: alloc::vec::Vec::new(),
            next_port: FIRST_PORT + u16::try_from(random_seed % span).unwrap_or(0),
        }
    }

    /// A link to `remote`, its socket added to `sockets`; idle until its driver opens it.
    pub(crate) fn add(
        &mut self,
        sockets: &mut SocketSet<'static>,
        remote: (Ipv4Address, u16),
    ) -> ClientLink {
        let mut socket = tcp::Socket::new(
            tcp::SocketBuffer::new(vec![0; LINK_BYTES]),
            tcp::SocketBuffer::new(vec![0; LINK_BYTES]),
        );
        // A request goes out whole at once and every segment is acknowledged at once: the loop
        // may wait longer than a delayed acknowledgement's timer.
        socket.set_nagle_enabled(false);
        socket.set_ack_delay(None);
        socket.set_timeout(Some(SILENCE));
        let handle = sockets.add(socket);
        let link = ClientLink(Rc::new(RefCell::new(Shared {
            remote,
            wanted: false,
            connect_due: false,
            status: LinkStatus::Idle,
            to_send: VecDeque::new(),
            received: VecDeque::new(),
        })));
        self.links.push((link.clone(), handle));
        link
    }

    /// Moves each link's bytes between its queues and its socket, makes the connections their
    /// drivers asked for, and closes the ones they let go. `addressed` says whether the chip has
    /// an address: without one no connection can be made.
    pub(crate) fn pump(
        &mut self,
        context: &mut Context,
        sockets: &mut SocketSet<'static>,
        addressed: bool,
    ) {
        for (link, handle) in &self.links {
            let socket = sockets.get_mut::<tcp::Socket>(*handle);
            let mut shared = link.0.borrow_mut();
            if !shared.wanted {
                if socket.is_open() {
                    socket.close();
                }
                continue;
            }
            if shared.connect_due {
                if socket.is_open() {
                    // A new connection replaces the one before it at once.
                    socket.abort();
                    continue;
                }
                shared.connect_due = false;
                if !addressed {
                    shared.status = LinkStatus::Closed("the device has no network address");
                    shared.wanted = false;
                    continue;
                }
                let port = self.next_port;
                self.next_port = port.checked_add(1).unwrap_or(FIRST_PORT);
                let (address, server_port) = shared.remote;
                if socket
                    .connect(context, (address, server_port), port)
                    .is_err()
                {
                    shared.status = LinkStatus::Closed("the server cannot be reached");
                    shared.wanted = false;
                    continue;
                }
            }
            match shared.status {
                LinkStatus::Connecting if socket.may_send() => shared.status = LinkStatus::Open,
                LinkStatus::Connecting | LinkStatus::Open if !socket.is_active() => {
                    shared.status = LinkStatus::Closed(if shared.status == LinkStatus::Open {
                        "the server closed the connection"
                    } else {
                        "the server refused the connection or did not answer"
                    });
                    shared.wanted = false;
                    continue;
                }
                _ => {}
            }
            if shared.status != LinkStatus::Open {
                continue;
            }
            while socket.can_send() && !shared.to_send.is_empty() {
                let (front, _) = shared.to_send.as_slices();
                let Ok(sent) = socket.send_slice(front) else {
                    break;
                };
                shared.to_send.drain(..sent);
                if sent == 0 {
                    break;
                }
            }
            while socket.can_recv() && shared.received.len() < LINK_BYTES {
                let room = LINK_BYTES - shared.received.len();
                let taken = socket.recv(|bytes| {
                    let taken = bytes.len().min(room);
                    (taken, bytes[..taken].to_vec())
                });
                match taken {
                    Ok(bytes) if !bytes.is_empty() => shared.received.extend(bytes),
                    _ => break,
                }
            }
            if !socket.may_recv() && socket.may_send() {
                // The server sent its FIN: nothing more comes, so the connection is done.
                socket.close();
            }
        }
    }
}
