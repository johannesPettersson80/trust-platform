//! The `mqtt` driver on the C6 (specification 11 §6.5; specification 32; specification 34
//! TARGET-01, §11.1 and Appendix D.2): the shared exchange of
//! `trust_runtime_services::io_drivers::mqtt` over a TCP connection of the chip's network
//! (`trust_platform_net::ClientLink`). The exchange decides every packet, check and error; this
//! module only carries its bytes.
//!
//! A computer's worker thread carries the session between the scans. On the C6 the network's idle
//! work steps it ([`IdleSession`]), so the keep alive goes out, and what the broker sends reaches
//! the exchange's queue, where the newest messages are kept, while no scan runs. The scan reads the
//! queue at the cycle start without waiting, and after the tasks hands the outputs off, waiting up
//! to the exchange's 40 ms while an earlier handoff is still on its way or the session is about to
//! start, as a computer's scan does.
//!
//! The C6's bounds (specification 34 Appendix D.2) are [`LIMITS`] and the link's queues of
//! [`QUEUE_BYTES`] each; the buffers are taken when the driver is claimed, and the received
//! payloads and the copies of the images are counted then too ([`ChipDriver::reserved_bytes`]).

use alloc::boxed::Box;
use alloc::format;
use alloc::rc::Rc;
use alloc::string::String;
use core::cell::RefCell;

use trust_platform_net::smoltcp::wire::Ipv4Address;
use trust_platform_net::{ClientLink, LinkStatus, Server};
use trust_runtime_core::error::RuntimeError;
use trust_runtime_services::chip::network::ChipNetwork;
use trust_runtime_services::chip::process_io::ChipDriver;
use trust_runtime_services::device_settings::ipv4;
use trust_runtime_services::io_drivers::mqtt::{
    MqttExchange, MqttLimits, MqttParams, MqttPoll, SCAN_WAIT_MS,
};
use trust_runtime_services::io_drivers::OnError;
use trust_runtime_services::status::IoDriverHealth;

use crate::hardware::calendar;
use crate::hardware::clock::now_us;
use crate::hardware::C6Net;
use crate::hardware::IdleSession;

/// The driver's name in `io.toml`.
pub(super) const NAME: &str = "mqtt";

/// The C6's bounds on the exchange (specification 34 Appendix D.2): packets of at most 1,024
/// bytes either way, 16 received messages and 1,024 payload bytes kept for the scan, and 2,048
/// bytes waiting to be sent.
const LIMITS: MqttLimits = MqttLimits {
    max_packet_bytes: 1_024,
    queue_messages: 16,
    queue_bytes: 1_024,
    outgoing_bytes: 2_048,
};

/// The bytes each of the link's queues holds.
const QUEUE_BYTES: usize = 1_024;

/// The client identifier of a session whose parameters name none, as trust-runtime's is
/// `trust-runtime-<process id>`: the C6 runs one runtime.
const DEFAULT_CLIENT_ID: &str = "trust-runtime-c6";

/// How long dropping the driver waits for the session's last packets to go out.
const STOP_WAIT_MS: u64 = 200;

/// The stream as the driver last told the exchange about it.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Stream {
    /// No stream, or one that closed and was reported.
    Closed,
    /// The link opens on the exchange's [`MqttPoll::Open`].
    Opening,
    /// The link is open and the exchange knows it.
    Open,
}

/// The exchange and the connection that carries it, which the driver and the network's idle work
/// share.
struct Session {
    exchange: MqttExchange,
    link: ClientLink,
    stream: Stream,
}

impl Session {
    /// Carries the exchange's steps over the link at `now`: what became of the connection, the
    /// bytes that came, the opens and closes the exchange asks for, and the bytes it sends.
    fn carry(&mut self, now: u64) {
        match (self.link.status(), self.stream) {
            (LinkStatus::Open, Stream::Opening) => {
                self.stream = Stream::Open;
                self.exchange.opened(now);
            }
            (LinkStatus::Closed(reason), Stream::Opening) => {
                self.stream = Stream::Closed;
                self.exchange.open_failed(now, reason);
            }
            (LinkStatus::Closed(reason), Stream::Open) => {
                self.stream = Stream::Closed;
                self.exchange.closed(now, reason);
            }
            _ => {}
        }
        let exchange = &mut self.exchange;
        self.link
            .take_received(|bytes| exchange.received(now, bytes));
        loop {
            match self.exchange.poll(now) {
                MqttPoll::Open => {
                    self.link.open();
                    self.stream = Stream::Opening;
                }
                MqttPoll::Close => {
                    self.link.close();
                    self.stream = Stream::Closed;
                }
                MqttPoll::Wait => break,
            }
        }
        if self.stream == Stream::Open {
            let taken = self.link.send_some(self.exchange.outgoing());
            self.exchange.sent(now, taken);
        }
        if let Some(utc_ms) = calendar::utc_ms() {
            self.exchange.set_clock(now, utc_ms);
        }
    }
}

impl IdleSession for Session {
    fn step(&mut self, now_ms: u64) {
        self.carry(now_ms);
    }

    fn due_ms(&self) -> Option<u64> {
        self.exchange.due_ms()
    }
}

/// An `mqtt` driver: the session, which the network's idle work steps too, and the network.
struct MqttDriver {
    session: Rc<RefCell<Session>>,
    network: Rc<RefCell<C6Net>>,
    on_error: OnError,
}

/// The `mqtt` driver of `params`, its connection carried by `network`.
///
/// # Errors
///
/// `mqtt: <key>: <reason>` for parameters the shared rules refuse; for a TLS broker,
/// `mqtt: tls: ...`, as the C6 has no room for a TLS session beside a program; for `mappings`,
/// `mqtt: mappings: ...`, as the C6 does not yet bind tags to the program's variables; for an IPv6
/// broker, `mqtt: broker: ...`; without a network, `mqtt: the device serves no network: name one
/// in device.toml`; and `mqtt: insufficient memory: ...` without room for the session's buffers.
pub(super) fn claim(
    params: &str,
    network: Option<&Rc<RefCell<C6Net>>>,
) -> Result<Box<dyn ChipDriver>, String> {
    let params = MqttParams::from_json(params).map_err(|error| format!("{NAME}: {error}"))?;
    if params.tls.is_some() {
        return Err(format!(
            "{NAME}: tls: the C6 has no room for a TLS session beside a program: reach the broker \
             over a protected network without TLS, or run the driver on a computer or a board with \
             more memory"
        ));
    }
    if params.mappings.is_some() {
        return Err(format!(
            "{NAME}: mappings: the C6 does not yet bind tags to the program's variables: name the \
             typed points with input_points and output_points, or run the driver on a computer"
        ));
    }
    let network = network
        .ok_or_else(|| format!("{NAME}: the device serves no network: name one in device.toml"))?;
    let host = params.broker.host.as_str();
    let server = match ipv4(host) {
        Some([a, b, c, d]) => Server::Address(Ipv4Address::new(a, b, c, d)),
        None if host.contains(':') => {
            return Err(format!(
                "{NAME}: broker: the C6's network is IPv4, so a broker is named by an IPv4 address \
                 or a host name, not {host}"
            ));
        }
        None => Server::Name(String::from(host)),
    };
    let link = network
        .borrow_mut()
        .client(server, params.broker.port, QUEUE_BYTES);
    let on_error = params.on_error;
    let mut exchange = MqttExchange::new(params, DEFAULT_CLIENT_ID, now_ms()).with_limits(LIMITS);
    exchange
        .reserve_buffers(QUEUE_BYTES)
        .map_err(|_| format!("{NAME}: insufficient memory: no room for the session's buffers"))?;
    let session = Rc::new(RefCell::new(Session {
        exchange,
        link,
        stream: Stream::Closed,
    }));
    let stepped: Rc<RefCell<dyn IdleSession>> = session.clone();
    network.borrow_mut().add_session(Rc::downgrade(&stepped));
    Ok(Box::new(MqttDriver {
        session,
        network: Rc::clone(network),
        on_error,
    }))
}

impl MqttDriver {
    /// Carries the session at the chip's clock now, and gives what `then` makes of the exchange.
    fn carried<T>(&self, then: impl FnOnce(&mut MqttExchange, u64) -> T) -> T {
        let now = now_ms();
        let mut session = self.session.borrow_mut();
        session.carry(now);
        then(&mut session.exchange, now)
    }
}

impl ChipDriver for MqttDriver {
    fn name(&self) -> &str {
        NAME
    }

    /// Reads the messages that came since the last read into `image`, without waiting for the
    /// broker (specification 32 §8).
    fn read_inputs(&mut self, image: &mut [u8]) -> Result<(), RuntimeError> {
        self.carried(|exchange, now| exchange.read_inputs(now, image))
    }

    /// Hands the output image off after up to [`SCAN_WAIT_MS`] for an earlier handoff or a session
    /// about to start, as a computer's scan waits for its worker (specification 11, deadline
    /// integrity), and moves the first bytes of the handoff to the link.
    fn write_outputs(&mut self, image: &[u8]) -> Result<(), RuntimeError> {
        let deadline = now_ms().saturating_add(SCAN_WAIT_MS);
        while self.carried(|exchange, now| exchange.output_settling(now) && now < deadline) {
            self.network.borrow_mut().poll();
        }
        let result = self.carried(|exchange, now| exchange.write_outputs(now, image));
        self.session.borrow_mut().carry(now_ms());
        result
    }

    fn health(&self) -> IoDriverHealth {
        self.session.borrow().exchange.health()
    }

    fn output_health(&self) -> IoDriverHealth {
        self.session.borrow().exchange.output_health()
    }

    fn on_error(&self) -> OnError {
        self.on_error
    }

    /// The payloads of the received messages, the newest raw input payload, and the copy of the
    /// output image that waits for its handoff.
    fn reserved_bytes(&self, output_image_bytes: usize) -> usize {
        LIMITS
            .queue_bytes
            .saturating_add(LIMITS.max_packet_bytes)
            .saturating_add(output_image_bytes)
    }
}

impl Drop for MqttDriver {
    /// Ends the session in order, as a computer's driver does when it stops: with Sparkplug B its
    /// NDEATH, then DISCONNECT, for at most [`STOP_WAIT_MS`]; the link sends what it queued before
    /// its FIN.
    fn drop(&mut self) {
        let deadline = now_ms().saturating_add(STOP_WAIT_MS);
        if let Ok(mut session) = self.session.try_borrow_mut() {
            session.exchange.disconnect(now_ms());
        }
        loop {
            let now = now_ms();
            let open = match self.session.try_borrow_mut() {
                Ok(mut session) => {
                    session.carry(now);
                    session.stream != Stream::Closed
                }
                Err(_) => false,
            };
            if !open || now >= deadline {
                break;
            }
            if let Ok(mut network) = self.network.try_borrow_mut() {
                network.poll();
            }
        }
        if let Ok(session) = self.session.try_borrow() {
            session.link.close();
        }
    }
}

/// The chip's clock in milliseconds.
fn now_ms() -> u64 {
    now_us() / 1_000
}
