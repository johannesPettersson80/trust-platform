//! The TCP connections that a chip's I/O drivers keep to servers (specification 11 §6.5:
//! `modbus-tcp` and `mqtt`; specification 34 TARGET-01). A driver asks the network for a
//! [`ClientLink`] to an address or a name and a port, and at its cycle writes the bytes it sends
//! and reads the bytes that came; the network looks a name up with its name servers at each
//! opening, opens the connection when the driver asks, moves the bytes between the scans and
//! reports what became of the connection. When to open again is the driver's exchange logic: a
//! link that closed stays closed until the driver opens it.

use alloc::collections::VecDeque;
use alloc::rc::Rc;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use core::cell::RefCell;

use smoltcp::iface::{Context, SocketHandle, SocketSet};
use smoltcp::socket::{dns, tcp};
use smoltcp::time::Duration;
use smoltcp::wire::{DnsQueryType, IpAddress, Ipv4Address};

/// Bytes of each of a link's socket buffers: a Modbus TCP frame is at most 260 bytes and an MQTT
/// packet of the process image a few hundred, so a socket holds a few of each. The link's own two
/// queues hold what its driver asks for when it adds the link, taken with the link.
pub const LINK_BYTES: usize = 2_048;

/// How long a link that its driver closed and let go may take to send what it queued and its FIN
/// before its socket is reset and given back.
const CLOSE_US: u64 = 2_000_000;

/// How long a connection lives without a packet from its server before it is dropped.
const SILENCE: Duration = Duration::from_secs(60);

/// The first local port of the drivers' connections, the start of the dynamic ports (RFC 6335).
const FIRST_PORT: u16 = 49_152;

/// How long a name server has to answer a lookup, as the time server's lookup waits.
const ANSWER_US: u64 = 5_000_000;

/// The server a link connects to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Server {
    /// An IPv4 address.
    Address(Ipv4Address),
    /// A name that the network's name servers look up at each opening.
    Name(String),
}

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
    server: Server,
    port: u16,
    /// The lookup of the server's name that is under way, with when it is given up.
    lookup: Option<(dns::QueryHandle, u64)>,
    /// The driver wants the connection open.
    wanted: bool,
    /// The most bytes each queue holds.
    queue_bytes: usize,
    /// Since when a link that its driver closed and let go is closing.
    closing_since: Option<u64>,
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
    /// The server this link connects to, and its port.
    #[must_use]
    pub fn remote(&self) -> (Server, u16) {
        let shared = self.0.borrow();
        (shared.server.clone(), shared.port)
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

    /// Ends the connection in order: the polls send the bytes queued before, then the FIN, also
    /// when the driver lets the link go at once; bytes received and not read are dropped.
    pub fn close(&self) {
        let mut shared = self.0.borrow_mut();
        shared.wanted = false;
        shared.connect_due = false;
        shared.status = LinkStatus::Idle;
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
        if shared.status != LinkStatus::Open
            || shared.to_send.len().saturating_add(bytes.len()) > shared.queue_bytes
        {
            return Err(LinkFull);
        }
        shared.to_send.extend(bytes);
        Ok(())
    }

    /// Queues the first of `bytes` that the queue has room for on the open connection, and gives
    /// how many it took: none while the connection is not open.
    #[must_use]
    pub fn send_some(&self, bytes: &[u8]) -> usize {
        let mut shared = self.0.borrow_mut();
        if shared.status != LinkStatus::Open {
            return 0;
        }
        let taken = bytes
            .len()
            .min(shared.queue_bytes.saturating_sub(shared.to_send.len()));
        shared
            .to_send
            .extend(bytes.get(..taken).unwrap_or_default());
        taken
    }

    /// Hands the bytes received since the last call to `take`, oldest first, in one or two parts.
    pub fn take_received(&self, mut take: impl FnMut(&[u8])) {
        let mut shared = self.0.borrow_mut();
        let (front, back) = shared.received.as_slices();
        for part in [front, back] {
            if !part.is_empty() {
                take(part);
            }
        }
        shared.received.clear();
    }
}

/// The refusal of bytes that a link cannot queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LinkFull;

/// The network's side of the links: each link's socket, and the name lookup they share.
pub(crate) struct Clients {
    links: Vec<(ClientLink, SocketHandle)>,
    next_port: u16,
    /// The socket that looks the servers' names up, added with the first named link.
    resolver: Option<SocketHandle>,
}

impl Clients {
    /// No links yet; the local ports start at an offset from `random_seed`, so that a chip that
    /// resets does not reuse the ports of its connections before the reset at once.
    pub(crate) fn new(random_seed: u64) -> Self {
        let span = u64::from(u16::MAX - FIRST_PORT) + 1;
        Self {
            links: Vec::new(),
            next_port: FIRST_PORT + u16::try_from(random_seed % span).unwrap_or(0),
            resolver: None,
        }
    }

    /// A link to `server` on `port` whose queues hold `queue_bytes` each, its socket added to
    /// `sockets`; idle until its driver opens it.
    pub(crate) fn add(
        &mut self,
        sockets: &mut SocketSet<'static>,
        server: Server,
        port: u16,
        queue_bytes: usize,
    ) -> ClientLink {
        if matches!(server, Server::Name(_)) && self.resolver.is_none() {
            self.resolver = Some(sockets.add(dns::Socket::new(&[], vec![None])));
        }
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
            server,
            port,
            lookup: None,
            wanted: false,
            queue_bytes,
            closing_since: None,
            connect_due: false,
            status: LinkStatus::Idle,
            to_send: VecDeque::with_capacity(queue_bytes),
            received: VecDeque::with_capacity(queue_bytes),
        })));
        self.links.push((link.clone(), handle));
        link
    }

    /// Moves each link's bytes between its queues and its socket, looks the names up and makes the
    /// connections their drivers asked for, and closes the ones they let go. `addressed` says
    /// whether the chip has an address, without which no connection can be made; `name_servers`
    /// are the name servers of that address; `now_us` is the chip's clock. A link that no driver
    /// holds any more, whose program was unloaded, gives its socket and buffers back.
    pub(crate) fn pump(
        &mut self,
        context: &mut Context,
        sockets: &mut SocketSet<'static>,
        addressed: bool,
        name_servers: &[Ipv4Address],
        now_us: u64,
    ) {
        self.retire(sockets, now_us);
        let mut pass = Pass {
            context,
            sockets,
            resolver: self.resolver,
            next_port: &mut self.next_port,
            addressed,
            name_servers,
            now_us,
        };
        for (link, handle) in &self.links {
            let mut shared = link.0.borrow_mut();
            if !shared.wanted {
                close_in_order(pass.sockets.get_mut::<tcp::Socket>(*handle), &mut shared);
            } else if pass.connect(*handle, &mut shared) {
                carry(pass.sockets.get_mut::<tcp::Socket>(*handle), &mut shared);
            }
        }
    }

    /// Gives back the socket of each link that no driver holds any more once its connection has
    /// ended: one its driver closed first may send what it queued and its FIN for [`CLOSE_US`],
    /// and one let go without a close is reset at once.
    fn retire(&mut self, sockets: &mut SocketSet<'static>, now_us: u64) {
        self.links.retain(|(link, handle)| {
            if Rc::strong_count(&link.0) > 1 {
                return true;
            }
            let socket = sockets.get_mut::<tcp::Socket>(*handle);
            if socket.is_open() {
                let mut shared = link.0.borrow_mut();
                let since = *shared.closing_since.get_or_insert(now_us);
                if shared.wanted || now_us.saturating_sub(since) >= CLOSE_US {
                    // The reset goes out with the next egress, and the socket goes at the next
                    // pump.
                    socket.abort();
                }
                return true;
            }
            sockets.remove(*handle);
            false
        });
    }

    /// `now_us` when a link has work for the next pump: a connection or a lookup its driver asked
    /// for, or queued bytes its socket has room for; the end of a lookup's wait; `None` otherwise,
    /// as the sockets' own timers are the stack's.
    pub(crate) fn due_us(&self, sockets: &SocketSet<'static>, now_us: u64) -> Option<u64> {
        self.links
            .iter()
            .filter_map(|(link, handle)| {
                let shared = link.0.borrow();
                let ready = shared.connect_due
                    || (!shared.to_send.is_empty()
                        && sockets.get::<tcp::Socket>(*handle).can_send());
                if ready {
                    Some(now_us)
                } else {
                    shared.lookup.map(|(_, deadline_us)| deadline_us)
                }
            })
            .min()
    }
}

impl Shared {
    /// Ends the link for `reason`: it stays closed until its driver opens it again.
    fn close_with(&mut self, reason: &'static str) {
        self.status = LinkStatus::Closed(reason);
        self.wanted = false;
    }
}

/// What one pump has of the network to make connections and look names up with.
struct Pass<'a> {
    context: &'a mut Context,
    sockets: &'a mut SocketSet<'static>,
    /// The socket that looks the servers' names up.
    resolver: Option<SocketHandle>,
    /// The local port of the next connection.
    next_port: &'a mut u16,
    /// Whether the chip has an address.
    addressed: bool,
    /// The name servers of the chip's address.
    name_servers: &'a [Ipv4Address],
    now_us: u64,
}

impl Pass<'_> {
    /// Makes the connection that `shared`'s driver asked for on the socket `handle`, starting with
    /// a lookup of a named server, or goes on with the lookup; whether the link may carry bytes in
    /// this pump.
    fn connect(&mut self, handle: SocketHandle, shared: &mut Shared) -> bool {
        if shared.connect_due {
            let socket = self.sockets.get_mut::<tcp::Socket>(handle);
            if socket.is_open() {
                // A new connection replaces the one before it at once.
                socket.abort();
                return false;
            }
            shared.connect_due = false;
            if !self.addressed {
                shared.close_with("the device has no network address");
                return false;
            }
            match shared.server.clone() {
                Server::Address(address) => self.connect_to(handle, shared, address),
                Server::Name(name) => self.start_lookup(&name, shared),
            }
        }
        if let Some((query, deadline_us)) = shared.lookup {
            match self.lookup_result(query, deadline_us) {
                Lookup::Pending => return false,
                Lookup::Failed(reason) => {
                    shared.lookup = None;
                    shared.close_with(reason);
                }
                Lookup::Found(address) => {
                    shared.lookup = None;
                    self.connect_to(handle, shared, address);
                }
            }
        }
        shared.wanted
    }

    /// Connects the socket `handle` to `address` and the link's port from the next local port; a
    /// connect that cannot start closes the link with the reason.
    fn connect_to(&mut self, handle: SocketHandle, shared: &mut Shared, address: Ipv4Address) {
        let port = *self.next_port;
        *self.next_port = port.checked_add(1).unwrap_or(FIRST_PORT);
        let socket = self.sockets.get_mut::<tcp::Socket>(handle);
        if socket
            .connect(&mut *self.context, (address, shared.port), port)
            .is_err()
        {
            shared.close_with("the server cannot be reached");
        }
    }

    /// Starts looking the server's `name` up with the name servers; a lookup that cannot start
    /// closes the link with the reason.
    fn start_lookup(&mut self, name: &str, shared: &mut Shared) {
        let Some(handle) = self.resolver else {
            return shared.close_with("the server's name cannot be looked up");
        };
        if self.name_servers.is_empty() {
            return shared.close_with("no name server to look the server's name up");
        }
        let servers: Vec<IpAddress> = self
            .name_servers
            .iter()
            .map(|server| IpAddress::Ipv4(*server))
            .collect();
        let resolver = self.sockets.get_mut::<dns::Socket>(handle);
        resolver.update_servers(&servers);
        match resolver.start_query(&mut *self.context, name, DnsQueryType::A) {
            Ok(query) => shared.lookup = Some((query, self.now_us.saturating_add(ANSWER_US))),
            Err(_) => shared.close_with("the server's name cannot be looked up"),
        }
    }

    /// The result of the lookup `query`, given up at `deadline_us`.
    fn lookup_result(&mut self, query: dns::QueryHandle, deadline_us: u64) -> Lookup {
        let Some(handle) = self.resolver else {
            return Lookup::Failed("the server's name cannot be looked up");
        };
        let resolver = self.sockets.get_mut::<dns::Socket>(handle);
        match resolver.get_query_result(query) {
            Ok(addresses) => addresses
                .iter()
                .find_map(|address| match address {
                    IpAddress::Ipv4(address) => Some(*address),
                    #[allow(
                        unreachable_patterns,
                        reason = "another crate of a build may enable smoltcp's IPv6"
                    )]
                    _ => None,
                })
                .map_or(
                    Lookup::Failed("the server's name was not found"),
                    Lookup::Found,
                ),
            Err(dns::GetQueryResultError::Pending) if self.now_us < deadline_us => Lookup::Pending,
            Err(dns::GetQueryResultError::Pending) => {
                resolver.cancel_query(query);
                Lookup::Failed("the name server did not answer for the server's name")
            }
            Err(_) => Lookup::Failed("the server's name was not found"),
        }
    }
}

/// What became of a lookup.
enum Lookup {
    /// The name server has not answered yet.
    Pending,
    /// The name's IPv4 address.
    Found(Ipv4Address),
    /// The reason no address was found.
    Failed(&'static str),
}

/// A link its driver closed: what it queued before goes out, then the FIN.
fn close_in_order(socket: &mut tcp::Socket<'static>, shared: &mut Shared) {
    if socket.is_open() {
        send_queued(socket, &mut shared.to_send);
        if shared.to_send.is_empty() || !socket.may_send() {
            shared.to_send.clear();
            socket.close();
        }
    } else {
        shared.to_send.clear();
    }
}

/// Notes what became of an open or opening connection and moves its bytes both ways; a server's
/// FIN closes it, as nothing more comes.
fn carry(socket: &mut tcp::Socket<'static>, shared: &mut Shared) {
    match shared.status {
        LinkStatus::Connecting if socket.may_send() => shared.status = LinkStatus::Open,
        LinkStatus::Connecting | LinkStatus::Open if !socket.is_active() => {
            shared.close_with(if shared.status == LinkStatus::Open {
                "the server closed the connection"
            } else {
                "the server refused the connection or did not answer"
            });
            return;
        }
        _ => {}
    }
    if shared.status != LinkStatus::Open {
        return;
    }
    send_queued(socket, &mut shared.to_send);
    receive_queued(socket, &mut shared.received, shared.queue_bytes);
    if !socket.may_recv() && socket.may_send() {
        socket.close();
    }
}

/// Moves the bytes of `to_send` that `socket` has room for into it.
fn send_queued(socket: &mut tcp::Socket<'static>, to_send: &mut VecDeque<u8>) {
    while socket.can_send() && !to_send.is_empty() {
        let (front, _) = to_send.as_slices();
        let Ok(sent) = socket.send_slice(front) else {
            break;
        };
        to_send.drain(..sent);
        if sent == 0 {
            break;
        }
    }
}

/// Moves what `socket` received into `received` while it holds fewer than `limit` bytes, through a
/// buffer on the stack rather than a block of the heap for each piece.
fn receive_queued(socket: &mut tcp::Socket<'static>, received: &mut VecDeque<u8>, limit: usize) {
    let mut chunk = [0_u8; 256];
    while socket.can_recv() {
        let room = limit.saturating_sub(received.len()).min(chunk.len());
        let Some(part) = chunk.get_mut(..room).filter(|part| !part.is_empty()) else {
            break;
        };
        match socket.recv_slice(part) {
            Ok(count) if count > 0 => received.extend(part.iter().take(count)),
            _ => break,
        }
    }
}
