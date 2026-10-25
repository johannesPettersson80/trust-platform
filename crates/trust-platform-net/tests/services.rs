//! A chip's discovery and time server against a client on the other end of a cable (specification
//! 34 PORT-06 and TARGET-06; specification 11, runtime discovery): the chip announces itself when it
//! takes its address and answers a browse with its control endpoint, and it sets its clock from the
//! time server of its settings.

mod support;

use std::cell::Cell;
use std::rc::Rc;

use support::{cable, End};
use trust_platform_net::smoltcp::iface::{Config, Interface, SocketHandle, SocketSet};
use trust_platform_net::smoltcp::socket::udp;
use trust_platform_net::smoltcp::time::Instant;
use trust_platform_net::smoltcp::wire::{
    EthernetAddress, HardwareAddress, IpAddress, IpCidr, IpEndpoint, Ipv4Address, Ipv4Cidr,
};
use trust_platform_net::sntp::NtpTimestamp;
use trust_platform_net::{Addressing, Identity, Network, NetworkSettings, TimeResult};
use trust_runtime_core::ports::WallClock;

const CHIP: Ipv4Address = Ipv4Address::new(10, 0, 0, 2);
const CLIENT: Ipv4Address = Ipv4Address::new(10, 0, 0, 1);
const GROUP: Ipv4Address = Ipv4Address::new(224, 0, 0, 251);

/// The chip's wall clock in a test: unset until a time source sets it.
#[derive(Clone, Default)]
struct TestClock(Rc<Cell<Option<u64>>>);

impl WallClock for TestClock {
    fn utc_ms(&self) -> Option<u64> {
        self.0.get()
    }

    fn set_utc_ms(&self, utc_ms: u64) -> bool {
        self.0.set(Some(utc_ms));
        true
    }
}

/// The chip, announced as `bench` with its control connection on 9901, and a client on the cable.
struct Bench {
    chip: Network,
    chip_end: End,
    client: Interface,
    client_end: End,
    sockets: SocketSet<'static>,
    now_us: u64,
}

fn settings(time_server: Option<&str>) -> NetworkSettings {
    NetworkSettings {
        mac: [0x02, 0, 0, 0, 0, 0x02],
        addressing: Addressing::Fixed {
            address: Ipv4Cidr::new(CHIP, 24),
            gateway: None,
            dns: Vec::new(),
        },
        host_name: "bench".to_string(),
        control_port: Some(9901),
        identity: Some(Identity {
            runtime_id: "esp32c6-206ef11704c0".to_string(),
            version: "0.24.75",
            platform: "esp32c6",
        }),
        time_server: time_server.map(ToString::to_string),
    }
}

impl Bench {
    fn new(time_server: Option<&str>, clock: &TestClock) -> Self {
        let (mut chip_end, mut client_end) = cable();
        let chip = Network::new(settings(time_server), &mut chip_end, 1, 0)
            .with_clock(Box::new(clock.clone()));
        let mut config = Config::new(HardwareAddress::Ethernet(EthernetAddress([
            0x02, 0, 0, 0, 0, 0x01,
        ])));
        config.random_seed = 2;
        let mut client = Interface::new(config, &mut client_end, Instant::from_micros(0));
        client.update_ip_addrs(|addresses| {
            addresses
                .push(IpCidr::new(IpAddress::Ipv4(CLIENT), 24))
                .expect("room for the address");
        });
        Self {
            chip,
            chip_end,
            client,
            client_end,
            sockets: SocketSet::new(Vec::new()),
            now_us: 0,
        }
    }

    /// Runs both ends for `steps` milliseconds.
    fn run(&mut self, steps: usize) {
        for _ in 0..steps {
            self.now_us += 1_000;
            self.chip.poll(self.now_us, &mut self.chip_end);
            let now = Instant::from_micros(i64::try_from(self.now_us).expect("small"));
            let _ = self
                .client
                .poll(now, &mut self.client_end, &mut self.sockets);
        }
    }

    /// A UDP socket of the client bound to `port`.
    fn udp(&mut self, port: u16) -> SocketHandle {
        let mut socket = udp::Socket::new(
            udp::PacketBuffer::new(vec![udp::PacketMetadata::EMPTY; 8], vec![0; 8_192]),
            udp::PacketBuffer::new(vec![udp::PacketMetadata::EMPTY; 8], vec![0; 8_192]),
        );
        socket.bind(port).expect("the port binds");
        self.sockets.add(socket)
    }

    /// The datagrams the client's socket `handle` has received, with their sources.
    fn datagrams(&mut self, handle: SocketHandle) -> Vec<(Vec<u8>, IpEndpoint)> {
        let socket = self.sockets.get_mut::<udp::Socket>(handle);
        let mut received = Vec::new();
        while let Ok((datagram, meta)) = socket.recv() {
            received.push((datagram.to_vec(), meta.endpoint));
        }
        received
    }
}

/// Whether `haystack` holds `needle`.
fn holds(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

/// A browse of `_trust._plc._tcp.local`: ID 0, one question, PTR IN (RFC 6762 §5.2).
const BROWSE: &[u8] = &[
    0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, //
    6, b'_', b't', b'r', b'u', b's', b't', 4, b'_', b'p', b'l', b'c', 4, b'_', b't', b'c', b'p', 5,
    b'l', b'o', b'c', b'a', b'l', 0, //
    0, 12, 0, 1,
];

/// When the chip takes its address it announces itself twice, a second apart, to the mDNS group,
/// with its instance `truST-bench` and its control endpoint; a browse a second later is answered to
/// the group. Catches a chip that a computer's browse does not find, and one whose announcement
/// names no endpoint that the computer can use at once.
#[test]
fn the_chip_announces_itself_and_answers_a_browse() {
    let clock = TestClock::default();
    let mut bench = Bench::new(None, &clock);
    bench
        .client
        .join_multicast_group(GROUP)
        .expect("the client joins the group");
    let listener = bench.udp(5353);
    bench.chip.set_link(true);
    bench.run(10);
    let first = bench.datagrams(listener);
    assert_eq!(first.len(), 1, "one announcement at once");
    let (announcement, from) = &first[0];
    assert_eq!(*from, IpEndpoint::new(IpAddress::Ipv4(CHIP), 5353));
    assert!(holds(announcement, b"\x0btruST-bench"));
    assert!(holds(announcement, b"control=tcp://10.0.0.2:9901"));
    assert!(holds(announcement, b"id=esp32c6-206ef11704c0"));
    assert!(holds(announcement, b"auth=token"));
    bench.run(1_000);
    assert_eq!(
        bench.datagrams(listener).len(),
        1,
        "the second announcement"
    );
    bench.run(1_100);
    assert!(
        bench.datagrams(listener).is_empty(),
        "two announcements only"
    );
    let target = IpEndpoint::new(IpAddress::Ipv4(GROUP), 5353);
    bench
        .sockets
        .get_mut::<udp::Socket>(listener)
        .send_slice(BROWSE, target)
        .expect("the browse is sent");
    bench.run(10);
    let answers = bench.datagrams(listener);
    let answer = answers
        .iter()
        .find(|(_, from)| from.addr == IpAddress::Ipv4(CHIP))
        .expect("the chip answers the browse");
    assert!(holds(&answer.0, b"control=tcp://10.0.0.2:9901"));
}

/// The chip asks the time server of its settings once it has an address and sets its clock to the
/// server's time. Catches a chip whose clock stays unset on a network with a time server, and one
/// set to the request's own time.
#[test]
fn the_chip_sets_its_clock_from_the_time_server() {
    let clock = TestClock::default();
    let mut bench = Bench::new(Some("10.0.0.1"), &clock);
    let server = bench.udp(123);
    bench.chip.set_link(true);
    bench.run(5);
    let requests = bench.datagrams(server);
    assert_eq!(requests.len(), 1, "one request");
    let (request, from) = &requests[0];
    assert_eq!(request.len(), 48);
    assert_eq!(request[0], 0x23, "version 4, a client");
    // The server's time: 1 November 2026, 12:00:00 UTC, when it received and sent the reply.
    let server_ms: u64 = 1_793_534_400_000;
    let mut reply = [0u8; 48];
    reply[0] = 0x24; // version 4, a server
    reply[1] = 2; // stratum 2
    reply[24..32].copy_from_slice(&request[40..48]);
    let stamp = NtpTimestamp::from_unix_ms(server_ms).0.to_be_bytes();
    reply[32..40].copy_from_slice(&stamp);
    reply[40..48].copy_from_slice(&stamp);
    bench
        .sockets
        .get_mut::<udp::Socket>(server)
        .send_slice(&reply, *from)
        .expect("the reply is sent");
    bench.run(5);
    let set = clock.0.get().expect("the clock is set");
    assert!(
        set.abs_diff(server_ms) < 50,
        "the clock reads the server's time: {set}"
    );
    assert!(matches!(
        bench.chip.time_result(),
        Some(("10.0.0.1", Some(TimeResult::Taken { .. })))
    ));
}

/// A time server named by a host name needs a name server: with a fixed address that names none,
/// the chip reports why it took no time. Catches a time service that fails silently.
#[test]
fn a_named_time_server_without_a_name_server_is_reported() {
    let clock = TestClock::default();
    let mut bench = Bench::new(Some("pool.ntp.org"), &clock);
    bench.chip.set_link(true);
    bench.run(5);
    assert_eq!(clock.0.get(), None);
    assert_eq!(
        bench.chip.time_result(),
        Some((
            "pool.ntp.org",
            Some(&TimeResult::Failed(
                "no name server to look the time server's name up".to_string()
            ))
        ))
    );
}
