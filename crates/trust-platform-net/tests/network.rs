//! A chip's network against a client on the other end of a cable (specification 34 ARCH-12 and
//! PORT-05): the control connection over TCP carries the same lines as the USB link, one client at a
//! time, and the address follows the link. The client is a second smoltcp interface; the cable two
//! queues of Ethernet frames.

mod support;

use std::net::SocketAddr;

use support::{cable, End};
use trust_platform_net::smoltcp::iface::{Config, Interface, SocketHandle, SocketSet};
use trust_platform_net::smoltcp::socket::tcp;
use trust_platform_net::smoltcp::time::Instant;
use trust_platform_net::smoltcp::wire::{
    EthernetAddress, HardwareAddress, IpAddress, IpCidr, Ipv4Address, Ipv4Cidr,
};
use trust_platform_net::{Addressing, Network, NetworkSettings, BUSY_ERROR};
use trust_runtime_core::ports::{LinkFrame, PeerClass};

const CHIP: Ipv4Address = Ipv4Address::new(10, 0, 0, 2);
const CLIENT: Ipv4Address = Ipv4Address::new(10, 0, 0, 1);
const PORT: u16 = 9901;

/// The chip's network and a client on the cable, with the clock both read.
struct Bench {
    chip: Network,
    chip_end: End,
    client: Interface,
    client_end: End,
    sockets: SocketSet<'static>,
    now_us: u64,
}

fn settings(control_port: Option<u16>) -> NetworkSettings {
    NetworkSettings {
        mac: [0x02, 0, 0, 0, 0, 0x02],
        addressing: Addressing::Fixed {
            address: Ipv4Cidr::new(CHIP, 24),
            gateway: None,
            dns: Vec::new(),
        },
        host_name: "bench".to_string(),
        control_port,
        identity: None,
        time_server: None,
    }
}

impl Bench {
    fn new(control_port: Option<u16>) -> Self {
        let (mut chip_end, mut client_end) = cable();
        let mut chip = Network::new(settings(control_port), &mut chip_end, 1, 0);
        chip.set_link(true);
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

    /// A client from `port` that connects to the control port.
    fn connect(&mut self, port: u16) -> SocketHandle {
        let socket = tcp::Socket::new(
            tcp::SocketBuffer::new(vec![0; 16_384]),
            tcp::SocketBuffer::new(vec![0; 16_384]),
        );
        let handle = self.sockets.add(socket);
        let context = self.client.context();
        self.sockets
            .get_mut::<tcp::Socket>(handle)
            .connect(context, (CHIP, PORT), port)
            .expect("the connect starts");
        self.run(20);
        handle
    }

    fn socket(&mut self, handle: SocketHandle) -> &mut tcp::Socket<'static> {
        self.sockets.get_mut::<tcp::Socket>(handle)
    }

    fn send(&mut self, handle: SocketHandle, bytes: &[u8]) {
        let mut sent = 0;
        while sent < bytes.len() {
            sent += self
                .socket(handle)
                .send_slice(&bytes[sent..])
                .expect("the client sends");
            self.run(5);
        }
        self.run(20);
    }

    fn received(&mut self, handle: SocketHandle) -> Vec<u8> {
        let socket = self.socket(handle);
        let mut bytes = Vec::new();
        while socket.can_recv() {
            socket
                .recv(|chunk| {
                    bytes.extend_from_slice(chunk);
                    (chunk.len(), ())
                })
                .expect("the client reads");
        }
        bytes
    }

    /// The next request the chip's loop would serve.
    fn frame(&mut self) -> Option<(Result<Vec<u8>, ()>, PeerClass)> {
        let mut buffer = vec![0; 8_192];
        Some(match self.chip.poll_frame(&mut buffer, self.now_us)? {
            LinkFrame::Frame { len, peer } => (Ok(buffer[..len].to_vec()), peer),
            LinkFrame::Oversized { peer } => (Err(()), peer),
        })
    }

    fn reply(&mut self, frame: &[u8]) {
        self.chip
            .send_frame(frame, self.now_us)
            .expect("the reply starts");
        self.run(20);
    }
}

fn client(port: u16) -> PeerClass {
    PeerClass::Network(SocketAddr::from(([10, 0, 0, 1], port)))
}

/// A request line over TCP is a frame from its client, and the reply goes back with its line feed.
/// Catches a request taken without its client's address, which access rights read, and a reply
/// that never leaves the chip.
#[test]
fn a_request_is_a_frame_from_its_client_and_the_reply_goes_back() {
    let mut bench = Bench::new(Some(PORT));
    let first = bench.connect(50_001);
    bench.send(first, b"{\"id\":1,\"type\":\"status\"}\n");
    let (request, peer) = bench.frame().expect("a request");
    assert_eq!(request, Ok(b"{\"id\":1,\"type\":\"status\"}".to_vec()));
    assert_eq!(peer, client(50_001));
    assert_eq!(bench.frame(), None, "one line, one frame");
    bench.reply(b"{\"id\":1,\"ok\":true}");
    assert_eq!(bench.received(first), b"{\"id\":1,\"ok\":true}\n".to_vec());
    assert!(!bench.chip.sending());
}

/// A reply longer than the socket's send buffer goes out whole as the client takes it. Catches a
/// reply cut at the send buffer's 4,096 bytes.
#[test]
fn a_reply_longer_than_the_send_buffer_goes_out_whole() {
    let mut bench = Bench::new(Some(PORT));
    let first = bench.connect(50_001);
    bench.send(first, b"{\"id\":2,\"type\":\"events\"}\n");
    assert!(bench.frame().is_some());
    let long: Vec<u8> = (b'a'..=b'z').cycle().take(8_000).collect();
    bench.reply(&long);
    bench.run(100);
    let mut expected = long.clone();
    expected.push(b'\n');
    assert_eq!(bench.received(first), expected);
}

/// While one client holds the control connection, a second gets the busy error as the reply to its
/// first request, with that request's `id`, and its connection is closed; the first is served on.
/// Catches a second client that hangs without an answer, a busy answer that a client takes for a
/// protocol fault, and a second client that takes the first client's replies.
#[test]
fn a_second_client_is_told_the_device_is_busy() {
    let mut bench = Bench::new(Some(PORT));
    let first = bench.connect(50_001);
    let second = bench.connect(50_002);
    bench.send(second, b"{\"auth\":\"x\",\"id\":7,\"type\":\"status\"}\n");
    assert_eq!(
        bench.received(second),
        format!("{{\"id\":7,\"ok\":false,\"error\":\"{BUSY_ERROR}\"}}\n").into_bytes()
    );
    assert!(!bench.socket(second).may_recv(), "the device closed it");
    bench.send(first, b"{\"id\":3,\"type\":\"status\"}\n");
    let (request, peer) = bench.frame().expect("the first client's request");
    assert_eq!(request, Ok(b"{\"id\":3,\"type\":\"status\"}".to_vec()));
    assert_eq!(peer, client(50_001));
}

/// A client that sends its request and closes its side still gets the reply; the device then
/// closes the connection, and the next client is served. Catches a connection closed before its
/// reply, and a port left busy by a client that has gone.
#[test]
fn a_client_that_closes_gets_its_reply_and_frees_the_port() {
    let mut bench = Bench::new(Some(PORT));
    let first = bench.connect(50_001);
    bench.send(first, b"{\"id\":4,\"type\":\"status\"}\n");
    bench.socket(first).close();
    bench.run(20);
    let (request, _) = bench.frame().expect("the request");
    assert_eq!(request, Ok(b"{\"id\":4,\"type\":\"status\"}".to_vec()));
    bench.reply(b"{\"id\":4,\"ok\":true}");
    assert_eq!(bench.received(first), b"{\"id\":4,\"ok\":true}\n".to_vec());
    assert_eq!(bench.frame(), None);
    bench.run(50);
    assert!(!bench.socket(first).is_open(), "the device closed its side");
    let next = bench.connect(50_003);
    bench.send(next, b"{\"id\":5,\"type\":\"status\"}\n");
    let (request, peer) = bench.frame().expect("the next client's request");
    assert_eq!(request, Ok(b"{\"id\":5,\"type\":\"status\"}".to_vec()));
    assert_eq!(peer, client(50_003));
}

/// A line longer than the frame is oversized, and the line after it is a frame again. Catches an
/// overlong line that ends the connection's framing.
#[test]
fn a_line_longer_than_the_frame_is_oversized() {
    let mut bench = Bench::new(Some(PORT));
    let first = bench.connect(50_001);
    let mut long = vec![b'x'; 9_000];
    long.push(b'\n');
    bench.send(first, &long);
    let (oversized, peer) = bench.frame().expect("the oversized line");
    assert_eq!(oversized, Err(()));
    assert_eq!(peer, client(50_001));
    bench.reply(b"{\"ok\":false}");
    bench.send(first, b"{\"id\":6}\n");
    let (request, _) = bench.frame().expect("the next line");
    assert_eq!(request, Ok(b"{\"id\":6}".to_vec()));
}

/// Without a control token the network serves no control connection: a client's connection is
/// refused. Catches a chip reachable on a network without a token.
#[test]
fn without_a_token_no_control_connection_is_served() {
    let mut bench = Bench::new(None);
    let first = bench.connect(50_001);
    bench.run(20);
    assert!(!bench.socket(first).is_open(), "the connection was refused");
    assert_eq!(bench.frame(), None);
}

/// A fixed address holds from the link going up and stays while the link is down. Catches a chip
/// that answers on its address before it has joined the network, and one that loses a fixed address
/// at a short drop of its Wi-Fi.
#[test]
fn a_fixed_address_holds_from_the_link_going_up() {
    let (mut chip_end, _) = cable();
    let mut chip = Network::new(settings(Some(PORT)), &mut chip_end, 1, 0);
    assert_eq!(chip.address(), None);
    chip.set_link(true);
    assert_eq!(chip.address(), Some(Ipv4Cidr::new(CHIP, 24)));
    chip.set_link(false);
    assert_eq!(chip.address(), Some(Ipv4Cidr::new(CHIP, 24)));
    let mut dhcp = settings(None);
    dhcp.addressing = Addressing::Dhcp;
    let mut leased = Network::new(dhcp, &mut chip_end, 1, 0);
    leased.set_link(true);
    assert_eq!(
        leased.address(),
        None,
        "no address until a server gives one"
    );
}
