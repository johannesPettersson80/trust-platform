//! The chip's time from the time server of its settings (specification 34 TARGET-06; the
//! checklist's T6): SNTP (RFC 4330) once the chip has an address, then every hour, and after a
//! failure again after a minute. A server named by a host name is looked up first, at the name
//! servers of the address in use. The messages are [`crate::sntp`]'s; this module keeps the sockets
//! and the schedule, and sets the chip's wall clock.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;

use smoltcp::iface::{Interface, SocketHandle, SocketSet};
use smoltcp::socket::{dns, udp};
use smoltcp::wire::{DnsQueryType, IpAddress, IpEndpoint, Ipv4Address};
use trust_runtime_core::ports::WallClock;

use crate::sntp::{self, NtpTimestamp, NTP_PORT};

/// How long the chip waits for a name's address or a server's reply, in microseconds.
const ANSWER_US: u64 = 5_000_000;
/// The wait after a time was taken, in microseconds: an hour.
const PERIOD_US: u64 = 3_600_000_000;
/// The wait after a failure, in microseconds: a minute.
const RETRY_US: u64 = 60_000_000;
/// The local UDP port of the requests.
const LOCAL_PORT: u16 = 50_123;
/// The earliest time a server's reply is resolved near while the chip's clock is unset, 1 January
/// 2026: no chip runs this firmware before then (`NtpTimestamp::to_unix_ms`).
const FLOOR_UNIX_MS: u64 = 1_767_225_600_000;

/// Where the time service stands.
enum Phase {
    /// The next request is due at `due_us`.
    Idle { due_us: u64 },
    /// The server's name is being looked up.
    Looking {
        query: dns::QueryHandle,
        deadline_us: u64,
    },
    /// A request was sent to `server`; its transmit timestamp is `sent`.
    Asking {
        server: Ipv4Address,
        sent: NtpTimestamp,
        deadline_us: u64,
    },
}

/// The time server of the settings.
enum Server {
    Address(Ipv4Address),
    Name(String),
}

/// What the last request gave: the time taken, with the clock's offset in milliseconds, or the
/// reason none was taken.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TimeResult {
    /// The clock was set.
    Taken {
        /// How far behind the server the clock was, in milliseconds; negative when it was ahead.
        offset_ms: i64,
    },
    /// No time was taken, for this reason.
    Failed(String),
}

/// The SNTP client.
pub(crate) struct TimeService {
    server: Server,
    socket: SocketHandle,
    dns: Option<SocketHandle>,
    phase: Phase,
    /// What the last request gave, none before the first.
    pub(crate) last: Option<TimeResult>,
}

impl TimeService {
    /// The client of `server`, a host name or a dotted IPv4 address.
    pub(crate) fn new(sockets: &mut SocketSet<'static>, server: &str) -> Self {
        let socket = sockets.add(udp::Socket::new(
            udp::PacketBuffer::new(vec![udp::PacketMetadata::EMPTY; 1], vec![0; 96]),
            udp::PacketBuffer::new(vec![udp::PacketMetadata::EMPTY; 1], vec![0; 48]),
        ));
        let (server, dns) = match trust_runtime_services::device_settings::ipv4(server) {
            Some(octets) => (Server::Address(Ipv4Address::from(octets)), None),
            None => (
                Server::Name(server.into()),
                Some(sockets.add(dns::Socket::new(&[], vec![None]))),
            ),
        };
        Self {
            server,
            socket,
            dns,
            phase: Phase::Idle { due_us: 0 },
            last: None,
        }
    }

    /// When the service's next step is due, in microseconds on the chip's clock.
    pub(crate) fn due_us(&self) -> u64 {
        match self.phase {
            Phase::Idle { due_us } => due_us,
            Phase::Looking { deadline_us, .. } | Phase::Asking { deadline_us, .. } => deadline_us,
        }
    }

    /// Asks the server when it is due and takes its reply. `dns_servers` are the name servers of
    /// the address in use; `addressed` says whether the interface has an address.
    pub(crate) fn poll(
        &mut self,
        iface: &mut Interface,
        sockets: &mut SocketSet<'static>,
        dns_servers: &[Ipv4Address],
        addressed: bool,
        clock: &dyn WallClock,
        now_us: u64,
    ) {
        let socket = sockets.get_mut::<udp::Socket>(self.socket);
        if !socket.is_open() {
            let _ = socket.bind(LOCAL_PORT);
        }
        if !addressed {
            // The first request goes out as soon as the chip has an address again.
            self.phase = Phase::Idle { due_us: 0 };
            return;
        }
        self.phase = match core::mem::replace(&mut self.phase, Phase::Idle { due_us: 0 }) {
            Phase::Idle { due_us } if now_us >= due_us => {
                self.start(iface, sockets, dns_servers, clock, now_us)
            }
            Phase::Looking { query, deadline_us } => {
                self.look(sockets, query, deadline_us, clock, now_us)
            }
            Phase::Asking {
                server,
                sent,
                deadline_us,
            } => self.take(sockets, server, sent, deadline_us, clock, now_us),
            idle @ Phase::Idle { .. } => idle,
        };
    }

    /// Asks the server at its address, or starts looking its name up.
    fn start(
        &mut self,
        iface: &mut Interface,
        sockets: &mut SocketSet<'static>,
        dns_servers: &[Ipv4Address],
        clock: &dyn WallClock,
        now_us: u64,
    ) -> Phase {
        let name = match &self.server {
            Server::Address(address) => {
                let address = *address;
                return self.ask(sockets, address, clock, now_us);
            }
            Server::Name(name) => name.clone(),
        };
        let Some(handle) = self.dns else {
            return self.failed("the time server's name cannot be looked up".into(), now_us);
        };
        if dns_servers.is_empty() {
            return self.failed(
                "no name server to look the time server's name up".into(),
                now_us,
            );
        }
        let servers: Vec<IpAddress> = dns_servers
            .iter()
            .map(|server| IpAddress::Ipv4(*server))
            .collect();
        let resolver = sockets.get_mut::<dns::Socket>(handle);
        resolver.update_servers(&servers);
        match resolver.start_query(iface.context(), &name, DnsQueryType::A) {
            Ok(query) => Phase::Looking {
                query,
                deadline_us: now_us.saturating_add(ANSWER_US),
            },
            Err(error) => self.failed(
                format!("the time server's name cannot be looked up: {error}"),
                now_us,
            ),
        }
    }

    /// Takes the name's address once it is found, and asks the server there.
    fn look(
        &mut self,
        sockets: &mut SocketSet<'static>,
        query: dns::QueryHandle,
        deadline_us: u64,
        clock: &dyn WallClock,
        now_us: u64,
    ) -> Phase {
        let Some(handle) = self.dns else {
            return Phase::Idle { due_us: 0 };
        };
        let resolver = sockets.get_mut::<dns::Socket>(handle);
        let found = match resolver.get_query_result(query) {
            Ok(addresses) => addresses.iter().find_map(|address| match address {
                IpAddress::Ipv4(address) => Some(*address),
                #[allow(
                    unreachable_patterns,
                    reason = "another crate of a build may enable smoltcp's IPv6"
                )]
                _ => None,
            }),
            Err(dns::GetQueryResultError::Pending) if now_us < deadline_us => {
                return Phase::Looking { query, deadline_us };
            }
            Err(dns::GetQueryResultError::Pending) => {
                resolver.cancel_query(query);
                return self.failed(
                    "the name server did not answer for the time server's name".into(),
                    now_us,
                );
            }
            Err(_) => None,
        };
        match found {
            Some(server) => self.ask(sockets, server, clock, now_us),
            None => self.failed("the time server's name was not found".into(), now_us),
        }
    }

    /// Sends the request to `server`.
    fn ask(
        &mut self,
        sockets: &mut SocketSet<'static>,
        server: Ipv4Address,
        clock: &dyn WallClock,
        now_us: u64,
    ) -> Phase {
        let sent = NtpTimestamp::from_unix_ms(clock.utc_ms().unwrap_or(0));
        let target = IpEndpoint::new(IpAddress::Ipv4(server), NTP_PORT);
        match sockets
            .get_mut::<udp::Socket>(self.socket)
            .send_slice(&sntp::request(sent), target)
        {
            Ok(()) => Phase::Asking {
                server,
                sent,
                deadline_us: now_us.saturating_add(ANSWER_US),
            },
            Err(error) => self.failed(format!("the request was not sent: {error}"), now_us),
        }
    }

    /// Takes the server's reply, sets the clock, and schedules the next request.
    fn take(
        &mut self,
        sockets: &mut SocketSet<'static>,
        server: Ipv4Address,
        sent: NtpTimestamp,
        deadline_us: u64,
        clock: &dyn WallClock,
        now_us: u64,
    ) -> Phase {
        let from = IpEndpoint::new(IpAddress::Ipv4(server), NTP_PORT);
        let socket = sockets.get_mut::<udp::Socket>(self.socket);
        let mut reply = None;
        while let Ok((datagram, meta)) = socket.recv() {
            if meta.endpoint == from {
                let received = clock.utc_ms().unwrap_or(0);
                reply = Some(sntp::parse_reply(
                    datagram,
                    sent,
                    received,
                    received.max(FLOOR_UNIX_MS),
                ));
                break;
            }
        }
        match reply {
            Some(Ok(time)) if clock.set_utc_ms(time.unix_ms) => {
                self.last = Some(TimeResult::Taken {
                    offset_ms: time.offset_ms,
                });
                Phase::Idle {
                    due_us: now_us.saturating_add(PERIOD_US),
                }
            }
            Some(Ok(_)) => self.failed("the clock did not take the server's time".into(), now_us),
            Some(Err(error)) => self.failed(error.to_string(), now_us),
            None if now_us >= deadline_us => {
                self.failed("the time server did not answer".into(), now_us)
            }
            None => Phase::Asking {
                server,
                sent,
                deadline_us,
            },
        }
    }

    /// Records `reason` and schedules the next request a minute on.
    fn failed(&mut self, reason: String, now_us: u64) -> Phase {
        self.last = Some(TimeResult::Failed(reason));
        Phase::Idle {
            due_us: now_us.saturating_add(RETRY_US),
        }
    }
}
