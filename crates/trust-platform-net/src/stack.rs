//! The chip's IPv4 interface: its address, its sockets and the bounded work of one poll.

use alloc::boxed::Box;
use alloc::vec::Vec;

use smoltcp::iface::{Config, Interface, PollIngressSingleResult, SocketHandle, SocketSet};
use smoltcp::phy::Device;
use smoltcp::socket::dhcpv4;
use smoltcp::time::Instant;
use smoltcp::wire::{DhcpOption, EthernetAddress, HardwareAddress, IpCidr, Ipv4Address, Ipv4Cidr};
use trust_runtime_core::ports::{LinkFrame, MonotonicInstant, WallClock};
use trust_runtime_services::link::ReplyRefused;

use crate::control::ControlServer;
use crate::discovery::Discovery;
use crate::settings::{Addressing, NetworkSettings};
use crate::time::{TimeResult, TimeService};

/// Frames one poll moves through the stack at most, so that a flood of frames cannot hold back a
/// due scan; the rest wait in the interface's receive queue for the next poll.
pub const FRAMES_PER_POLL: usize = 4;

/// The DHCP option that carries the device's name (RFC 2132, option 12).
const HOST_NAME_OPTION: u8 = 12;

/// A chip's network: the interface, its sockets, its address, the control connection, the
/// announcement that discovery finds and the time server.
pub struct Network {
    iface: Interface,
    sockets: SocketSet<'static>,
    addressing: Addressing,
    dhcp: Option<SocketHandle>,
    control: Option<ControlServer>,
    discovery: Option<Discovery>,
    time: Option<TimeService>,
    /// The time server of the settings, which [`Self::time_result`] reports.
    time_server: Option<alloc::string::String>,
    /// The chip's wall clock, which the time server sets.
    clock: Option<Box<dyn WallClock>>,
    link_up: bool,
    /// The name servers of the address in use.
    dns: Vec<Ipv4Address>,
}

impl Network {
    /// The network of `settings` over `device`, its link down; `random_seed` seeds the stack's
    /// sequence numbers and ports, and `now_us` is the chip's clock.
    pub fn new(
        settings: NetworkSettings,
        device: &mut (impl Device + ?Sized),
        random_seed: u64,
        now_us: u64,
    ) -> Self {
        let mut config = Config::new(HardwareAddress::Ethernet(EthernetAddress(settings.mac)));
        config.random_seed = random_seed;
        let iface = Interface::new(config, device, instant(now_us));
        let mut sockets = SocketSet::new(Vec::new());
        let dhcp = matches!(settings.addressing, Addressing::Dhcp).then(|| {
            let mut socket = dhcpv4::Socket::new();
            // The option borrows its bytes for as long as the socket lives, the whole boot.
            let name: &'static [u8] =
                Box::leak(settings.host_name.clone().into_bytes().into_boxed_slice());
            let options: &'static [DhcpOption<'static>] = Box::leak(Box::new([DhcpOption {
                kind: HOST_NAME_OPTION,
                data: name,
            }]));
            socket.set_outgoing_options(options);
            sockets.add(socket)
        });
        let control = settings
            .control_port
            .map(|port| ControlServer::new(&mut sockets, port));
        // The chip is announced while it serves its control connection, which a runtime that finds
        // it can then use at once (ARCH-12).
        let discovery = settings
            .identity
            .zip(settings.control_port)
            .map(|(identity, port)| {
                Discovery::new(&mut sockets, identity, &settings.host_name, port)
            });
        let time = settings
            .time_server
            .as_deref()
            .map(|server| TimeService::new(&mut sockets, server));
        Self {
            iface,
            sockets,
            addressing: settings.addressing,
            dhcp,
            control,
            discovery,
            time,
            time_server: settings.time_server,
            clock: None,
            link_up: false,
            dns: Vec::new(),
        }
    }

    /// Sets `clock`, the chip's wall clock, from the time server of the settings (TARGET-06).
    #[must_use]
    pub fn with_clock(mut self, clock: Box<dyn WallClock>) -> Self {
        self.clock = Some(clock);
        self
    }

    /// The time server of the settings and what its last request gave, none before the first.
    #[must_use]
    pub fn time_result(&self) -> Option<(&str, Option<&TimeResult>)> {
        let server = self.time_server.as_deref()?;
        Some((
            server,
            self.time.as_ref().and_then(|time| time.last.as_ref()),
        ))
    }

    /// Withdraws the chip's announcement and sends it, before a reset the chip starts.
    pub fn leave(&mut self, now_us: u64, device: &mut (impl Device + ?Sized)) {
        if let Some(discovery) = &mut self.discovery {
            discovery.goodbye(&mut self.sockets);
        }
        let _ = self
            .iface
            .poll_egress(instant(now_us), device, &mut self.sockets);
    }

    /// The link went up or down: the chip joined its Wi-Fi network or left it, or its Ethernet
    /// cable was plugged or pulled. A fixed address holds from the link going up; DHCP asks again.
    pub fn set_link(&mut self, up: bool) {
        if up == self.link_up {
            return;
        }
        self.link_up = up;
        match self.addressing.clone() {
            Addressing::Dhcp => {
                if let Some(dhcp) = self.dhcp {
                    self.sockets.get_mut::<dhcpv4::Socket>(dhcp).reset();
                }
                self.set_address(None, None, Vec::new());
            }
            Addressing::Fixed {
                address,
                gateway,
                dns,
            } => {
                if up {
                    self.set_address(Some(address), gateway, dns);
                }
            }
        }
    }

    /// Whether the link is up.
    #[must_use]
    pub fn link_up(&self) -> bool {
        self.link_up
    }

    /// The interface's address, none until the link is up and, with DHCP, a server gave one.
    #[must_use]
    pub fn address(&self) -> Option<Ipv4Cidr> {
        self.iface.ip_addrs().iter().find_map(|cidr| match cidr {
            IpCidr::Ipv4(cidr) => Some(*cidr),
            #[allow(
                unreachable_patterns,
                reason = "another crate of a build may enable smoltcp's IPv6"
            )]
            _ => None,
        })
    }

    /// The TCP port of the control connection, none while the network serves none.
    #[must_use]
    pub fn control_port(&self) -> Option<u16> {
        self.control.as_ref().map(ControlServer::port)
    }

    /// The name servers of the address in use.
    #[must_use]
    pub fn dns(&self) -> &[Ipv4Address] {
        &self.dns
    }

    /// The network's bounded work: at most [`FRAMES_PER_POLL`] received frames through the stack,
    /// the address lease, the control connection's bytes and what the sockets queued. It never
    /// waits.
    pub fn poll(&mut self, now_us: u64, device: &mut (impl Device + ?Sized)) {
        let now = instant(now_us);
        for _ in 0..FRAMES_PER_POLL {
            if self
                .iface
                .poll_ingress_single(now, device, &mut self.sockets)
                == PollIngressSingleResult::None
            {
                break;
            }
        }
        self.poll_dhcp();
        let address = self.address();
        if let Some(discovery) = &mut self.discovery {
            discovery.set_address(&mut self.iface, address, now_us);
            discovery.poll(&mut self.sockets, now_us);
        }
        if let (Some(time), Some(clock)) = (&mut self.time, &self.clock) {
            time.poll(
                &mut self.iface,
                &mut self.sockets,
                &self.dns,
                address.is_some(),
                clock.as_ref(),
                now_us,
            );
        }
        if let Some(control) = &mut self.control {
            control.pump(&mut self.sockets);
        }
        let _ = self.iface.poll_egress(now, device, &mut self.sockets);
        if let Some(control) = &mut self.control {
            // What the egress made room for.
            control.pump(&mut self.sockets);
        }
    }

    /// When the stack's work is next due, a retransmission or a lease, in microseconds on the
    /// chip's clock.
    pub fn poll_due(&mut self, now_us: u64) -> Option<u64> {
        let stack = self
            .iface
            .poll_at(instant(now_us), &self.sockets)
            .map(|due| u64::try_from(due.total_micros()).unwrap_or(0));
        let announcing = self.discovery.as_ref().and_then(Discovery::due_us);
        let time = self
            .time
            .as_ref()
            .filter(|_| self.clock.is_some() && self.address().is_some())
            .map(TimeService::due_us);
        [stack, announcing, time].into_iter().flatten().min()
    }

    /// The next request of the control connection, with its frame at the start of `buffer`.
    pub fn poll_frame(&mut self, buffer: &mut [u8], now_us: u64) -> Option<LinkFrame> {
        let control = self.control.as_mut()?;
        let now = MonotonicInstant::from_micros(now_us);
        control.pump(&mut self.sockets);
        if let Some(frame) = control.poll_frame(buffer, now) {
            return Some(frame);
        }
        // A link full of the start of an overlong line has just been emptied: what waits in the
        // socket behind it may end that line.
        control.pump(&mut self.sockets);
        control.poll_frame(buffer, now)
    }

    /// Starts sending `frame` on the control connection, the reply to the request polled last; the
    /// next [`Self::poll`] sends it.
    ///
    /// # Errors
    ///
    /// The line link's [`ReplyRefused`]: another reply is in flight, or the frame is too long.
    /// [`ReplyRefused::Busy`] without a control connection.
    pub fn send_frame(&mut self, frame: &[u8], now_us: u64) -> Result<(), ReplyRefused> {
        self.control
            .as_mut()
            .ok_or(ReplyRefused::Busy)?
            .send_frame(frame, MonotonicInstant::from_micros(now_us))
    }

    /// Whether the control connection still holds reply bytes it has not sent.
    #[must_use]
    pub fn sending(&self) -> bool {
        self.control.as_ref().is_some_and(ControlServer::sending)
    }

    fn poll_dhcp(&mut self) {
        let Some(dhcp) = self.dhcp else {
            return;
        };
        let lease = match self.sockets.get_mut::<dhcpv4::Socket>(dhcp).poll() {
            None => return,
            Some(dhcpv4::Event::Configured(config)) => Some((
                config.address,
                config.router,
                config.dns_servers.iter().copied().collect(),
            )),
            Some(dhcpv4::Event::Deconfigured) => None,
        };
        match lease {
            Some((address, gateway, dns)) => self.set_address(Some(address), gateway, dns),
            None => self.set_address(None, None, Vec::new()),
        }
    }

    fn set_address(
        &mut self,
        address: Option<Ipv4Cidr>,
        gateway: Option<Ipv4Address>,
        dns: Vec<Ipv4Address>,
    ) {
        self.iface.update_ip_addrs(|addresses| {
            addresses.clear();
            if let Some(address) = address {
                let _ = addresses.push(IpCidr::Ipv4(address));
            }
        });
        match gateway {
            Some(gateway) => {
                let _ = self.iface.routes_mut().add_default_ipv4_route(gateway);
            }
            None => {
                let _ = self.iface.routes_mut().remove_default_ipv4_route();
            }
        }
        self.dns = dns;
    }
}

/// The stack's reading of the chip's clock.
fn instant(now_us: u64) -> Instant {
    Instant::from_micros(i64::try_from(now_us).unwrap_or(i64::MAX))
}
