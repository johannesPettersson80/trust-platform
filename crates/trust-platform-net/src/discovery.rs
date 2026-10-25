//! The chip's announcement on its network (specification 34 PORT-06; the checklist's S6;
//! specification 11, runtime discovery): an mDNS responder on UDP port 5353, so that every truST
//! runtime's browse of `_trust._plc._tcp.local.` finds the chip and can use it at once. The
//! messages are [`crate::mdns`]'s; this module keeps the socket, the timing rules of RFC 6762 that
//! need a clock, and the records' values.

use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;
use alloc::vec;

use smoltcp::iface::{Interface, SocketHandle, SocketSet};
use smoltcp::socket::udp;
use smoltcp::wire::{IpAddress, IpEndpoint, Ipv4Address, Ipv4Cidr};

use crate::mdns::{self, Announce, Announcement, Reply, MDNS_GROUP, MDNS_PORT};

/// The service name a computer's announcement starts its instance with (specification 11,
/// `runtime.discovery.service_name`'s default).
const SERVICE_NAME: &str = "truST";
/// The bytes of one mDNS message the chip sends or takes: one Ethernet frame's UDP payload.
const MESSAGE_BYTES: usize = 1_472;
/// Announcements sent when an address is taken, one second apart (RFC 6762 §8.3).
const ANNOUNCEMENTS: u8 = 2;
/// The gap between two announcements, and the least gap between two multicast replies (RFC 6762
/// §6, §8.3), in microseconds.
const SECOND_US: u64 = 1_000_000;

/// What the chip announces of itself (specification 11, runtime discovery).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identity {
    /// The stable runtime ID, the TXT property `id`.
    pub runtime_id: String,
    /// The truST version, `version`.
    pub version: &'static str,
    /// The chip, `platform`, such as `esp32c6`.
    pub platform: &'static str,
}

/// The responder.
pub(crate) struct Discovery {
    socket: SocketHandle,
    identity: Identity,
    /// The instance label, `truST-<name>`.
    instance: String,
    /// The host label, the device's name.
    host: String,
    /// The port the SRV record names: the control connection's.
    port: u16,
    /// The control endpoint the TXT record names, `tcp://<address>:<port>`, for the address in use.
    control: String,
    /// The address in use and its subnet, whose queries are answered (RFC 6762 §11).
    address: Option<Ipv4Cidr>,
    /// Announcements still to send for the address in use, and when the next is due.
    announcing: Option<(u8, u64)>,
    /// When the last multicast reply was sent.
    last_multicast_us: Option<u64>,
    /// The message being written.
    buffer: Box<[u8; MESSAGE_BYTES]>,
}

impl Discovery {
    /// The responder of the device `host`, which announces its control connection on `port`.
    pub(crate) fn new(
        sockets: &mut SocketSet<'static>,
        identity: Identity,
        host: &str,
        port: u16,
    ) -> Self {
        let mut socket = udp::Socket::new(
            udp::PacketBuffer::new(
                vec![udp::PacketMetadata::EMPTY; 4],
                vec![0; 2 * MESSAGE_BYTES],
            ),
            udp::PacketBuffer::new(vec![udp::PacketMetadata::EMPTY; 2], vec![0; MESSAGE_BYTES]),
        );
        // RFC 6762 §11: every mDNS message leaves with an IP time to live of 255.
        socket.set_hop_limit(Some(255));
        Self {
            socket: sockets.add(socket),
            identity,
            instance: format!("{SERVICE_NAME}-{host}"),
            host: host.into(),
            port,
            control: String::new(),
            address: None,
            announcing: None,
            last_multicast_us: None,
            buffer: Box::new([0; MESSAGE_BYTES]),
        }
    }

    /// When the next announcement is due, in microseconds on the chip's clock.
    pub(crate) fn due_us(&self) -> Option<u64> {
        self.announcing.map(|(_, due)| due)
    }

    /// The interface took `address`, or lost its address: the chip joins the mDNS group and
    /// announces itself at the new address.
    pub(crate) fn set_address(
        &mut self,
        iface: &mut Interface,
        address: Option<Ipv4Cidr>,
        now_us: u64,
    ) {
        if address == self.address {
            return;
        }
        self.address = address;
        self.control = address.map_or_else(String::new, |cidr| {
            format!("tcp://{}:{}", cidr.address(), self.port)
        });
        self.announcing = address.map(|_| (ANNOUNCEMENTS, now_us));
        if address.is_some() {
            let _ = iface.join_multicast_group(Ipv4Address::from(MDNS_GROUP));
        }
    }

    /// Answers the queries that arrived and sends a due announcement.
    pub(crate) fn poll(&mut self, sockets: &mut SocketSet<'static>, now_us: u64) {
        let socket = sockets.get_mut::<udp::Socket>(self.socket);
        if !socket.is_open() {
            let _ = socket.bind(MDNS_PORT);
        }
        let Some(cidr) = self.address else {
            while socket.recv().is_ok() {}
            return;
        };
        let txt = properties(&self.identity, &self.host, &self.control);
        let announcement = Announcement {
            instance: &self.instance,
            host: &self.host,
            address: cidr.address().octets(),
            port: self.port,
            txt: &txt,
        };
        while let Ok((query, meta)) = socket.recv() {
            let on_link = match meta.endpoint.addr {
                IpAddress::Ipv4(source) => cidr.contains_addr(&source),
                #[allow(
                    unreachable_patterns,
                    reason = "another crate of a build may enable smoltcp's IPv6"
                )]
                _ => false,
            };
            if !on_link {
                // RFC 6762 §11: only queries from the local link are answered.
                continue;
            }
            let (len, to) = match mdns::answer(
                query,
                meta.endpoint.port,
                &announcement,
                &mut self.buffer[..],
            ) {
                None => continue,
                Some(Reply::Unicast(len)) => (len, meta.endpoint),
                Some(Reply::Multicast(len)) => {
                    if self
                        .last_multicast_us
                        .is_some_and(|last| now_us.saturating_sub(last) < SECOND_US)
                    {
                        // RFC 6762 §6: a record is multicast at most once a second.
                        continue;
                    }
                    self.last_multicast_us = Some(now_us);
                    (len, group())
                }
            };
            let _ = socket.send_slice(&self.buffer[..len], to);
        }
        if let Some((left, due)) = self.announcing
            && now_us >= due
        {
            if let Some(len) =
                mdns::announcement(&announcement, Announce::Hello, &mut self.buffer[..])
            {
                let _ = socket.send_slice(&self.buffer[..len], group());
                self.last_multicast_us = Some(now_us);
            }
            self.announcing = (left > 1).then(|| (left - 1, now_us.saturating_add(SECOND_US)));
        }
    }

    /// Withdraws the chip's records, before a reset the chip starts (RFC 6762 §10.1).
    pub(crate) fn goodbye(&mut self, sockets: &mut SocketSet<'static>) {
        let Some(cidr) = self.address else {
            return;
        };
        let txt = properties(&self.identity, &self.host, &self.control);
        let announcement = Announcement {
            instance: &self.instance,
            host: &self.host,
            address: cidr.address().octets(),
            port: self.port,
            txt: &txt,
        };
        if let Some(len) =
            mdns::announcement(&announcement, Announce::Goodbye, &mut self.buffer[..])
        {
            let _ = sockets
                .get_mut::<udp::Socket>(self.socket)
                .send_slice(&self.buffer[..len], group());
        }
    }
}

/// The TXT properties, in a computer's order (specification 11, runtime discovery): the chip's
/// control connection always takes a token.
fn properties<'a>(
    identity: &'a Identity,
    host: &'a str,
    control: &'a str,
) -> [(&'a str, &'a str); 6] {
    [
        ("id", identity.runtime_id.as_str()),
        ("name", host),
        ("version", identity.version),
        ("platform", identity.platform),
        ("auth", "token"),
        ("control", control),
    ]
}

/// The mDNS group's endpoint.
fn group() -> IpEndpoint {
    IpEndpoint::new(IpAddress::Ipv4(Ipv4Address::from(MDNS_GROUP)), MDNS_PORT)
}
