//! The TCP connections of a chip's I/O drivers against a server on the other end of a cable
//! (specification 11 §6.5, `modbus-tcp` and `mqtt` on a chip): a link carries bytes both ways once
//! open, reports a server that closes or refuses, and stays closed until its driver opens it again.

mod support;

use support::{cable, End};
use trust_platform_net::smoltcp::iface::{Config, Interface, SocketHandle, SocketSet};
use trust_platform_net::smoltcp::socket::tcp;
use trust_platform_net::smoltcp::time::Instant;
use trust_platform_net::smoltcp::wire::{
    EthernetAddress, HardwareAddress, IpAddress, IpCidr, Ipv4Address, Ipv4Cidr,
};
use trust_platform_net::{Addressing, ClientLink, LinkStatus, Network, NetworkSettings};

const CHIP: Ipv4Address = Ipv4Address::new(10, 0, 0, 2);
const SERVER: Ipv4Address = Ipv4Address::new(10, 0, 0, 1);
const MODBUS: u16 = 502;

/// The chip's network and a server on the cable that listens on [`MODBUS`].
struct Bench {
    chip: Network,
    chip_end: End,
    server: Interface,
    server_end: End,
    sockets: SocketSet<'static>,
    listener: SocketHandle,
    now_us: u64,
}

impl Bench {
    fn new(addressing: Addressing) -> Self {
        let (mut chip_end, mut server_end) = cable();
        let settings = NetworkSettings {
            mac: [0x02, 0, 0, 0, 0, 0x02],
            addressing,
            host_name: "bench".to_string(),
            control_port: None,
            identity: None,
            time_server: None,
        };
        let mut chip = Network::new(settings, &mut chip_end, 7, 0);
        chip.set_link(true);
        let mut config = Config::new(HardwareAddress::Ethernet(EthernetAddress([
            0x02, 0, 0, 0, 0, 0x01,
        ])));
        config.random_seed = 2;
        let mut server = Interface::new(config, &mut server_end, Instant::from_micros(0));
        server.update_ip_addrs(|addresses| {
            addresses
                .push(IpCidr::new(IpAddress::Ipv4(SERVER), 24))
                .expect("room for the address");
        });
        let mut sockets = SocketSet::new(Vec::new());
        let mut socket = tcp::Socket::new(
            tcp::SocketBuffer::new(vec![0; 4_096]),
            tcp::SocketBuffer::new(vec![0; 4_096]),
        );
        socket.listen(MODBUS).expect("the server listens");
        let listener = sockets.add(socket);
        Self {
            chip,
            chip_end,
            server,
            server_end,
            sockets,
            listener,
            now_us: 0,
        }
    }

    fn fixed() -> Self {
        Self::new(Addressing::Fixed {
            address: Ipv4Cidr::new(CHIP, 24),
            gateway: None,
            dns: Vec::new(),
        })
    }

    /// Runs both ends for `steps` milliseconds.
    fn run(&mut self, steps: usize) {
        for _ in 0..steps {
            self.now_us += 1_000;
            self.chip.poll(self.now_us, &mut self.chip_end);
            let now = Instant::from_micros(i64::try_from(self.now_us).expect("small"));
            let _ = self
                .server
                .poll(now, &mut self.server_end, &mut self.sockets);
        }
    }

    fn server_socket(&mut self) -> &mut tcp::Socket<'static> {
        self.sockets.get_mut::<tcp::Socket>(self.listener)
    }
}

/// The bytes a link received so far.
fn received(link: &ClientLink) -> Vec<u8> {
    let mut bytes = Vec::new();
    link.receive(&mut bytes);
    bytes
}

/// A link is idle until its driver opens it, then open: what the driver sends reaches the server,
/// and what the server sends reaches the driver. When the server closes, the link reports
/// `the server closed the connection` and stays closed. Catches a link that connects by itself, one
/// that drops bytes either way, and a closed connection reported as open.
#[test]
fn a_link_carries_bytes_both_ways_until_the_server_closes() {
    let mut bench = Bench::fixed();
    let link = bench.chip.client((SERVER, MODBUS));
    bench.run(20);
    assert_eq!(link.status(), LinkStatus::Idle);
    assert!(!bench.server_socket().is_active(), "no connection yet");

    link.open();
    bench.run(20);
    assert_eq!(link.status(), LinkStatus::Open);
    link.send(b"request").expect("an open link takes bytes");
    bench.run(20);
    let mut buffer = [0u8; 64];
    let read = bench
        .server_socket()
        .recv_slice(&mut buffer)
        .expect("the server reads");
    assert_eq!(&buffer[..read], b"request");
    bench
        .server_socket()
        .send_slice(b"response")
        .expect("the server answers");
    bench.run(20);
    assert_eq!(received(&link), b"response");

    bench.server_socket().close();
    bench.run(50);
    assert_eq!(
        link.status(),
        LinkStatus::Closed("the server closed the connection")
    );
    assert_eq!(link.send(b"more"), Err(trust_platform_net::LinkFull));
}

/// A server that refuses the connection (no one listens on the port) leaves the link closed with
/// `the server refused the connection or did not answer`, and the link is not tried again until its
/// driver opens it; it then connects to a port that listens. Catches a link that retries at once,
/// which would flood a server that is down, and one that never connects again.
#[test]
fn a_refused_link_stays_closed_until_its_driver_opens_it_again() {
    let mut bench = Bench::fixed();
    let link = bench.chip.client((SERVER, MODBUS + 1));
    link.open();
    bench.run(50);
    assert_eq!(
        link.status(),
        LinkStatus::Closed("the server refused the connection or did not answer")
    );
    bench.run(200);
    assert_eq!(
        link.status(),
        LinkStatus::Closed("the server refused the connection or did not answer")
    );

    let good = bench.chip.client((SERVER, MODBUS));
    good.open();
    bench.run(20);
    assert_eq!(good.status(), LinkStatus::Open);
}

/// A chip without an address cannot make a connection, and says so. Catches a link that waits for
/// ever, which its driver would report as a server that does not answer.
#[test]
fn a_chip_without_an_address_reports_it() {
    let mut bench = Bench::new(Addressing::Dhcp);
    let link = bench.chip.client((SERVER, MODBUS));
    link.open();
    bench.run(5);
    assert_eq!(
        link.status(),
        LinkStatus::Closed("the device has no network address")
    );
}

/// A link whose driver let it go, as when its program is unloaded, gives its socket back: a new
/// link to the same server still connects, and the dropped link's connection is gone from the
/// server's side. Catches a chip that keeps a socket and its buffers for every program it ever
/// loaded.
#[test]
fn a_link_no_driver_holds_gives_its_socket_back() {
    let mut bench = Bench::fixed();
    let link = bench.chip.client((SERVER, MODBUS));
    link.open();
    bench.run(20);
    assert_eq!(link.status(), LinkStatus::Open);
    drop(link);
    bench.run(50);
    assert!(
        !bench.server_socket().may_recv(),
        "the server sees the connection end"
    );
}
