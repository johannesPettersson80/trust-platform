//! The servicing pass of the control link over the CDC-ACM class (design §2.2 and §2.3; specification
//! 34, PORT-05). The pass runs over a fake class that holds literal packets. The line link is the
//! real shared one, so the bytes the pass moves are the bytes a client sees.

use core::cell::Cell;
use core::time::Duration;
use std::collections::VecDeque;

use trust_platform_stm32u5::usb_service::{CdcPort, LinkService};
use trust_runtime_core::ports::{LinkFrame, MonotonicInstant, PeerClass, WakeSignal};
use trust_runtime_services::link::DeviceLineLink;

/// The tests' monotonic clock. The link's stall rule reads time, so a fixed instant keeps every test
/// independent of the wall clock.
fn now() -> MonotonicInstant {
    MonotonicInstant::from_origin(Duration::ZERO)
}

/// Counts the wakes a pass raises: the wake counter the runner's wait reads (PORT-03).
#[derive(Default)]
struct Wakes(Cell<u32>);

impl Wakes {
    /// The wakes counted so far.
    fn count(&self) -> u32 {
        self.0.get()
    }
}

impl WakeSignal for Wakes {
    fn wake(&self) {
        self.0.set(self.0.get() + 1);
    }
}

/// A fake CDC-ACM class. `inbox` holds the packets the host sent, `outbox` the packets the endpoint
/// took, `accept` how many more packets the endpoint takes before it reports busy, and `dtr` and
/// `bus_reset` what usb-device reports.
struct FakeCdc {
    inbox: VecDeque<Vec<u8>>,
    outbox: Vec<Vec<u8>>,
    accept: usize,
    dtr: bool,
    bus_reset: bool,
}

impl Default for FakeCdc {
    /// A port with nothing sent, the endpoint free, DTR low and no bus reset.
    fn default() -> Self {
        Self {
            inbox: VecDeque::new(),
            outbox: Vec::new(),
            accept: usize::MAX,
            dtr: false,
            bus_reset: false,
        }
    }
}

impl CdcPort for FakeCdc {
    fn read_packet(&mut self, out: &mut [u8]) -> Option<usize> {
        let packet = self.inbox.pop_front()?;
        out[..packet.len()].copy_from_slice(&packet);
        Some(packet.len())
    }

    fn write_packet(&mut self, bytes: &[u8]) -> Option<usize> {
        if self.accept == 0 {
            return None;
        }
        self.accept -= 1;
        self.outbox.push(bytes.to_vec());
        Some(bytes.len())
    }

    fn dtr(&self) -> bool {
        self.dtr
    }

    fn take_bus_reset(&mut self) -> bool {
        std::mem::take(&mut self.bus_reset)
    }
}

/// The next request the loop polls, as its bytes. `None` when no request waits, or the frame is not
/// a local one.
fn next_request(link: &mut DeviceLineLink) -> Option<Vec<u8>> {
    let mut buffer = [0u8; DeviceLineLink::MAX_FRAME_LEN];
    match link.poll_frame(&mut buffer, now()) {
        Some(LinkFrame::Frame {
            len,
            peer: PeerClass::Local,
        }) => Some(buffer[..len].to_vec()),
        _ => None,
    }
}

/// A reply of 100 bytes: an 18-byte prefix, 80 digits and the closing `"}`. With its line feed it
/// takes two packets, of 64 and 37 bytes.
fn long_reply() -> Vec<u8> {
    let mut reply = b"{\"id\":7,\"result\":\"".to_vec();
    reply.extend(b"0123456789".repeat(8));
    reply.extend(b"\"}");
    assert_eq!(reply.len(), 100, "the fixture is 100 bytes");
    reply
}

/// A reply of 63 bytes, which with its line feed is exactly one 64-byte packet.
fn one_packet_reply() -> Vec<u8> {
    let mut reply = b"{\"id\":7,\"result\":\"".to_vec();
    reply.extend(std::iter::repeat_n(b'0', 43));
    reply.extend(b"\"}");
    assert_eq!(reply.len(), 63, "the fixture is 63 bytes");
    reply
}

/// The pass that starts the host session: it sees DTR rise, which empties the link of anything queued
/// before the session (S1b §2.3). The host opens the port before it sends its first request, so each
/// test starts the session before it queues a reply.
fn start_session(
    port: &mut FakeCdc,
    link: &mut DeviceLineLink,
    service: &mut LinkService,
    wakes: &Wakes,
) {
    service.pass(port, link, wakes);
}

/// A request that arrives in two packets reaches the link whole, and wakes the loop only once the
/// request is complete.
///
/// PORT-05: a request is the bytes before the line feed. The wrong behaviour: the first packet taken
/// as a request of its own, the second packet's bytes lost, or a wake before the line feed arrives.
#[test]
fn a_request_split_over_two_packets_arrives_whole() {
    let mut port = FakeCdc {
        dtr: true,
        ..FakeCdc::default()
    };
    let mut link = DeviceLineLink::new(PeerClass::Local);
    let wakes = Wakes::default();
    let mut service = LinkService::default();
    start_session(&mut port, &mut link, &mut service, &wakes);

    port.inbox.push_back(b"sta".to_vec());
    service.pass(&mut port, &mut link, &wakes);
    assert_eq!(wakes.count(), 0, "a partial request wakes nothing");
    assert_eq!(next_request(&mut link), None);

    port.inbox.push_back(b"tus\n".to_vec());
    service.pass(&mut port, &mut link, &wakes);
    assert_eq!(
        wakes.count(),
        1,
        "the completed request wakes the loop once"
    );
    assert_eq!(next_request(&mut link), Some(b"status".to_vec()));
}

/// A reply longer than one packet is written in order and whole, and its last packet is the short
/// one that ends it.
///
/// PORT-05: a reply is one JSON object followed by a line feed. The wrong behaviour: a packet longer
/// than the 64-byte endpoint, a reply without its line feed, a second packet that starts at the
/// wrong byte, or a reply that never completes.
#[test]
fn a_reply_longer_than_one_packet_is_written_in_order_and_whole() {
    let reply = long_reply();
    let mut port = FakeCdc {
        dtr: true,
        ..FakeCdc::default()
    };
    let mut link = DeviceLineLink::new(PeerClass::Local);
    let wakes = Wakes::default();
    let mut service = LinkService::default();
    start_session(&mut port, &mut link, &mut service, &wakes);
    assert_eq!(link.send_frame(&reply, now()), Ok(()));

    service.pass(&mut port, &mut link, &wakes);

    assert_eq!(port.outbox.len(), 2, "a 101-byte reply takes two packets");
    assert_eq!(port.outbox[0], reply[..64].to_vec());
    let mut last = reply[64..].to_vec();
    last.push(b'\n');
    assert_eq!(port.outbox[1], last, "the last 36 bytes and the line feed");
    assert_eq!(port.outbox[1].len(), 37);
    assert!(!link.reply_in_flight());
    assert_eq!(wakes.count(), 1, "a reply that moved wakes the loop");
}

/// A reply that is exactly one packet is followed by a zero-length packet, which ends the transfer.
///
/// USB 2.0 §5.8.3: a bulk transfer whose last packet is full ends with a short packet. The wrong
/// behaviour: no zero-length packet follows, so the host waits for bytes that never come.
#[test]
fn a_reply_of_exactly_one_packet_is_followed_by_a_zero_length_packet() {
    let reply = one_packet_reply();
    let mut port = FakeCdc {
        dtr: true,
        ..FakeCdc::default()
    };
    let mut link = DeviceLineLink::new(PeerClass::Local);
    let wakes = Wakes::default();
    let mut service = LinkService::default();
    start_session(&mut port, &mut link, &mut service, &wakes);
    assert_eq!(link.send_frame(&reply, now()), Ok(()));

    service.pass(&mut port, &mut link, &wakes);

    assert_eq!(port.outbox.len(), 2);
    assert_eq!(port.outbox[0].len(), 64, "the reply and its line feed");
    assert!(port.outbox[1].is_empty(), "then a zero-length packet");
}

/// A bus reset drops the reply in flight, and the next request of the new session is answered.
///
/// PORT-05: a new host session never receives an old reply. The wrong behaviour: the rest of the old
/// reply reaches the new session, or the new request waits behind the old reply.
#[test]
fn a_bus_reset_mid_reply_drops_it_and_the_next_request_is_answered() {
    let reply = long_reply();
    let answer = b"{\"id\":2,\"result\":\"OK\"}";
    assert_eq!(answer.len(), 22);
    let mut port = FakeCdc {
        dtr: true,
        // Only the first packet of the reply goes out before the endpoint reports busy.
        accept: 1,
        ..FakeCdc::default()
    };
    let mut link = DeviceLineLink::new(PeerClass::Local);
    let wakes = Wakes::default();
    let mut service = LinkService::default();
    start_session(&mut port, &mut link, &mut service, &wakes);
    assert_eq!(link.send_frame(&reply, now()), Ok(()));
    service.pass(&mut port, &mut link, &wakes);
    assert_eq!(port.outbox.len(), 1);
    assert!(link.reply_in_flight());

    // The bus reset clears DTR as usbd-serial's class does, and the host opens the port again.
    port.bus_reset = true;
    port.dtr = false;
    port.accept = usize::MAX;
    service.pass(&mut port, &mut link, &wakes);

    assert!(!link.reply_in_flight(), "the reply in flight is dropped");
    assert_eq!(link.counters().abandoned_replies, 1);
    // The host's opening the port is session 1 and the bus reset starts session 2.
    assert_eq!(link.counters().sessions, 2);
    assert_eq!(
        port.outbox.len(),
        1,
        "the rest of the old reply is never written"
    );

    port.dtr = true;
    start_session(&mut port, &mut link, &mut service, &wakes);
    port.inbox.push_back(b"status\n".to_vec());
    service.pass(&mut port, &mut link, &wakes);
    assert_eq!(next_request(&mut link), Some(b"status".to_vec()));
    assert_eq!(link.send_frame(answer, now()), Ok(()));
    service.pass(&mut port, &mut link, &wakes);

    // A reply dropped after one of its packets left the link starts the next reply with a repair
    // line feed, which ends the stale part as a line of its own (specification 34, PORT-05).
    let mut expected = vec![b'\n'];
    expected.extend_from_slice(answer);
    expected.push(b'\n');
    assert_eq!(port.outbox.len(), 2);
    assert_eq!(
        port.outbox[1], expected,
        "the new session gets the answer after the repair line feed"
    );
}

/// DTR falling drops the reply in flight, and no part of it is written after the host closes the
/// port or reopens it.
///
/// S1b §2.3: DTR falling drops the reply in flight. The wrong behaviour: the rest of the reply goes
/// out while the port is closed, or it goes out when the host reopens the port.
#[test]
fn dtr_falling_drops_the_reply_in_flight_and_none_of_it_follows() {
    let reply = long_reply();
    let mut port = FakeCdc {
        dtr: true,
        accept: 1,
        ..FakeCdc::default()
    };
    let mut link = DeviceLineLink::new(PeerClass::Local);
    let wakes = Wakes::default();
    let mut service = LinkService::default();
    start_session(&mut port, &mut link, &mut service, &wakes);
    assert_eq!(link.send_frame(&reply, now()), Ok(()));
    service.pass(&mut port, &mut link, &wakes);
    assert_eq!(port.outbox.len(), 1);

    port.dtr = false;
    port.accept = usize::MAX;
    service.pass(&mut port, &mut link, &wakes);
    assert!(!link.reply_in_flight(), "closing the port drops the reply");
    assert_eq!(link.counters().abandoned_replies, 1);

    port.dtr = true;
    service.pass(&mut port, &mut link, &wakes);
    assert_eq!(
        port.outbox.len(),
        1,
        "reopening the port sends none of the old reply"
    );
    assert_eq!(link.unsent().len(), 0);
}

/// While DTR is low nothing is written. A reply waiting then is dropped when the host opens the
/// port, and the next request is answered in the new session.
///
/// S1b §2.3: DTR rising empties the TX buffer, so a new session never receives an old reply. The
/// wrong behaviour: a reply written to a port the host has not opened, or a reply queued while the
/// port was closed reaching the host when it opens.
#[test]
fn nothing_is_written_while_dtr_is_low_and_the_waiting_reply_is_dropped_when_it_rises() {
    let reply = b"{\"id\":3}";
    let answer = b"{\"id\":4}";
    let mut port = FakeCdc::default();
    let mut link = DeviceLineLink::new(PeerClass::Local);
    let wakes = Wakes::default();
    let mut service = LinkService::default();
    assert_eq!(link.send_frame(reply, now()), Ok(()));

    service.pass(&mut port, &mut link, &wakes);
    assert!(port.outbox.is_empty(), "DTR is low: nothing is written");
    assert!(
        link.reply_in_flight(),
        "the reply waits while the port is closed"
    );

    port.dtr = true;
    service.pass(&mut port, &mut link, &wakes);
    assert!(
        port.outbox.is_empty(),
        "the waiting reply never reaches the new session"
    );
    assert!(!link.reply_in_flight());
    assert_eq!(link.counters().abandoned_replies, 1);

    port.inbox.push_back(b"status\n".to_vec());
    service.pass(&mut port, &mut link, &wakes);
    assert_eq!(next_request(&mut link), Some(b"status".to_vec()));
    assert_eq!(link.send_frame(answer, now()), Ok(()));
    service.pass(&mut port, &mut link, &wakes);
    assert_eq!(port.outbox, vec![b"{\"id\":4}\n".to_vec()]);
}

/// A packet the link has no room for stays with the port, and the pass reports that the OUT
/// interrupt must stay off until the loop takes a request.
///
/// PORT-05: bytes that arrive while the receive buffer is full stay with the hardware, which stops
/// accepting more; no byte is lost. The wrong behaviour: a packet read and then dropped, or the pass
/// reporting room the link does not have.
#[test]
fn a_full_link_leaves_the_packet_with_the_port() {
    let long = [b'A'; 8192];
    let mut link = DeviceLineLink::new(PeerClass::Local);
    let mut port = FakeCdc {
        dtr: true,
        ..FakeCdc::default()
    };
    let wakes = Wakes::default();
    let mut service = LinkService::default();
    start_session(&mut port, &mut link, &mut service, &wakes);
    assert_eq!(link.receive(&long), 8192);
    assert_eq!(link.receive(b"\n"), 1);
    assert_eq!(link.receive_capacity(), 1, "the ring holds 8194 bytes");
    port.inbox.push_back(b"status\n".to_vec());

    assert!(
        !service.pass(&mut port, &mut link, &wakes),
        "the link has no room for a whole packet"
    );
    assert_eq!(port.inbox.len(), 1, "the packet waits with the port");
    assert_eq!(wakes.count(), 0);

    assert_eq!(next_request(&mut link), Some(long.to_vec()));
    assert!(service.pass(&mut port, &mut link, &wakes));
    assert!(port.inbox.is_empty());
    assert_eq!(next_request(&mut link), Some(b"status".to_vec()));
}
