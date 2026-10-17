//! The FIFO servicing of the C6's USB-Serial/JTAG control link (design §2.2 and specification 34,
//! PORT-05). The pass a host can check runs over a fake peripheral FIFO; the line link is the
//! real shared one, so the bytes a pass moves are the bytes a client sees.

use core::{cell::Cell, time::Duration};

use trust_platform_esp32::usb_service::{drain_out, service, Pending, UsbFifo, FIFO_BYTES};
use trust_runtime_core::ports::{LinkFrame, MonotonicInstant, PeerClass, WakeSignal};
use trust_runtime_services::link::DeviceLineLink;

/// The tests' monotonic clock: the link's stall rule is time-based, so a fixed instant keeps every
/// test independent of the wall clock.
fn now() -> MonotonicInstant {
    MonotonicInstant::from_origin(Duration::ZERO)
}

/// The wake counter a pass advances, as the adapter's `wait.rs` supplies one through `WakeSignal`.
#[derive(Default)]
struct Wake(Cell<u32>);

impl Wake {
    /// The wakes counted so far.
    fn count(&self) -> u32 {
        self.0.get()
    }
}

impl WakeSignal for Wake {
    fn wake(&self) {
        self.0.set(self.0.get() + 1);
    }
}

/// A fake USB-Serial/JTAG peripheral: both FIFOs, the interrupt flags and the interrupt enables.
/// Every method only touches these fields, as the peripheral's registers are touched on the chip.
struct FakeFifo {
    /// The bytes the host sent, oldest first: the 64-byte OUT FIFO.
    out: Vec<u8>,
    /// The bytes written for the host, oldest first: the IN FIFO.
    in_fifo: Vec<u8>,
    /// How many bytes the IN FIFO takes before it is full; the host reads it in between.
    in_room: usize,
    /// The interrupt flags pending now.
    pending: Pending,
    /// The IN FIFO's empty interrupt is enabled.
    in_empty_enabled: bool,
    /// The OUT packet interrupt is enabled.
    out_packet_enabled: bool,
    /// How often a pass marked a packet done (`WR_DONE`).
    done: u32,
}

impl Default for FakeFifo {
    /// An idle peripheral: both FIFOs empty, the IN FIFO with the room of one FIFO (64 bytes,
    /// design §2.2), no interrupt pending, and the packet interrupt enabled as `UsbLink::bind`
    /// leaves it.
    fn default() -> Self {
        Self {
            out: Vec::new(),
            in_fifo: Vec::new(),
            in_room: 64,
            pending: Pending::default(),
            in_empty_enabled: false,
            out_packet_enabled: true,
            done: 0,
        }
    }
}

impl UsbFifo for FakeFifo {
    fn take_pending(&mut self) -> Pending {
        core::mem::take(&mut self.pending)
    }

    fn read_out(&mut self, out: &mut [u8]) -> usize {
        let read = out.len().min(self.out.len());
        for (slot, byte) in out.iter_mut().zip(self.out.drain(..read)) {
            *slot = byte;
        }
        read
    }

    fn write_in(&mut self, bytes: &[u8]) -> usize {
        // One FIFO of bytes at most, and only into the room the host's last read left.
        let room = self.in_room.saturating_sub(self.in_fifo.len());
        let written = bytes.len().min(room).min(64);
        self.in_fifo.extend_from_slice(&bytes[..written]);
        self.done += 1;
        written
    }

    fn set_in_empty_enabled(&mut self, enabled: bool) {
        self.in_empty_enabled = enabled;
    }

    fn set_out_packet_enabled(&mut self, enabled: bool) {
        self.out_packet_enabled = enabled;
    }
}

/// A packet the host sent reaches the link as one request frame, and the pass wakes the loop.
///
/// PORT-05: a request is the bytes before a line feed, with one carriage return removed; design
/// §2.2: each event advances the wake counter. The wrong behaviour: bytes left unread in the FIFO
/// (the request never arrives) and a pass that wakes nobody (the loop waits for its deadline).
#[test]
fn received_bytes_reach_the_link_and_wake_the_loop() {
    let mut fifo = FakeFifo {
        out: b"status\n".to_vec(),
        pending: Pending {
            out_packet: true,
            ..Pending::default()
        },
        ..FakeFifo::default()
    };
    let mut link = DeviceLineLink::new(PeerClass::Local);
    let wake = Wake::default();

    service(&mut fifo, &mut link, &wake);

    assert_eq!(wake.count(), 1);
    // Every byte the FIFO held is with the link now.
    assert_eq!(fifo.out.len(), 0);
    let mut buffer = [0u8; DeviceLineLink::MAX_FRAME_LEN];
    assert_eq!(
        link.poll_frame(&mut buffer, now()),
        Some(LinkFrame::Frame {
            len: 6,
            peer: PeerClass::Local
        })
    );
    assert_eq!(&buffer[..6], b"status");
}

/// A full receive ring leaves the bytes with the hardware, which then stops accepting packets.
///
/// PORT-05: "Bytes that arrive while the receive buffer is full stay with the hardware, which
/// stops accepting more (USB flow control); no byte is lost". The wrong behaviour: reading a byte
/// out of the FIFO that the link cannot take (it would be lost), and asking for more packets while
/// they must be left (the handler would spin on them).
#[test]
fn a_full_ring_leaves_the_bytes_in_the_fifo() {
    let mut link = DeviceLineLink::new(PeerClass::Local);
    // A line longer than any frame fills the ring without a line feed: the frame is 8,192 bytes
    // and the ring holds one of them with its carriage return and line feed (design §2.1).
    let overlong = [b'A'; 8_194];
    assert_eq!(link.receive(&overlong), 8_194);
    assert_eq!(link.receive_capacity(), 0);
    let mut fifo = FakeFifo {
        out: b"status\n".to_vec(),
        pending: Pending {
            out_packet: true,
            ..Pending::default()
        },
        ..FakeFifo::default()
    };
    let wake = Wake::default();

    service(&mut fifo, &mut link, &wake);

    // The bytes stay in the OUT FIFO, oldest first.
    assert_eq!(fifo.out, b"status\n".to_vec());
    assert!(!fifo.out_packet_enabled);
    // The event still woke the loop, which will take the bytes once it has room.
    assert_eq!(wake.count(), 1);
}

/// The bytes a full ring left in the FIFO reach the link once the loop has taken requests.
///
/// PORT-05: "no byte is lost". The wrong behaviour: bytes left in the FIFO for flow control that
/// nothing ever reads again, so the request they carry waits forever.
#[test]
fn the_bytes_a_full_ring_left_in_the_fifo_reach_the_link() {
    let mut link = DeviceLineLink::new(PeerClass::Local);
    let overlong = [b'A'; 8_194];
    assert_eq!(link.receive(&overlong), 8_194);
    let mut fifo = FakeFifo {
        out: b"status\n".to_vec(),
        pending: Pending {
            out_packet: true,
            ..Pending::default()
        },
        ..FakeFifo::default()
    };
    let wake = Wake::default();
    service(&mut fifo, &mut link, &wake);
    assert_eq!(fifo.out, b"status\n".to_vec());

    // The loop polls: the full ring without a line feed is dropped, and the ring is free again.
    let mut buffer = [0u8; DeviceLineLink::MAX_FRAME_LEN];
    assert_eq!(link.poll_frame(&mut buffer, now()), None);
    // The loop takes the bytes the handler left, as `poll_frame` does before it polls.
    assert!(!drain_out(&mut fifo, &mut link));
    assert_eq!(fifo.out.len(), 0);
    // They are the tail of the overlong line: the link reports it once, as PORT-05 says.
    assert_eq!(
        link.poll_frame(&mut buffer, now()),
        Some(LinkFrame::Oversized {
            peer: PeerClass::Local
        })
    );
}

/// A reply is written into the IN FIFO 64 bytes at a time, and its interrupt is disabled once
/// nothing is unsent.
///
/// Design §2.2: "it refills the IN FIFO from the TX buffer (64 bytes and `WR_DONE`), and disables
/// that interrupt when the buffer is empty". The wrong behaviour: writing the whole reply at once
/// (more than the FIFO holds), marking the wrong number of bytes sent (the rest of the reply would
/// be lost), and leaving the interrupt enabled with nothing to send (the handler would spin).
#[test]
fn the_in_fifo_is_filled_64_bytes_at_a_time_then_its_interrupt_is_disabled() {
    // The design's FIFO is 64 bytes; the fake takes 64 as well, so this pins the constant.
    assert_eq!(FIFO_BYTES, 64);
    let reply: &[u8] = b"{\"id\":7,\"result\":\"01234567890123456789012345678901234567890123456789012345678901234567890123456789\"}";
    assert_eq!(reply.len(), 100);
    let mut link = DeviceLineLink::new(PeerClass::Local);
    assert_eq!(link.send_frame(reply, now()), Ok(()));
    // A reply frame ends with a line feed (PORT-05).
    assert_eq!(link.unsent().len(), 101);

    let mut fifo = FakeFifo {
        pending: Pending {
            in_empty: true,
            ..Pending::default()
        },
        // As `send_frame` leaves the interrupt with a reply in flight.
        in_empty_enabled: true,
        ..FakeFifo::default()
    };
    let wake = Wake::default();

    service(&mut fifo, &mut link, &wake);

    // One packet of 64 bytes, marked done, with the rest of the reply still unsent.
    assert_eq!(fifo.in_fifo, reply[..64].to_vec());
    assert_eq!(fifo.done, 1);
    assert_eq!(link.unsent().len(), 37);
    // The rest of the reply is still unsent, so the interrupt stays enabled for it.
    assert!(fifo.in_empty_enabled);

    // The host read the packet: the last 36 bytes and the line feed follow.
    fifo.in_fifo.clear();
    fifo.pending.in_empty = true;
    service(&mut fifo, &mut link, &wake);

    let mut expected = reply[64..].to_vec();
    expected.push(b'\n');
    assert_eq!(fifo.in_fifo, expected);
    assert_eq!(fifo.done, 2);
    assert_eq!(link.unsent().len(), 0);
    // Nothing is unsent: the interrupt is disabled until `send_frame` has another reply.
    assert!(!fifo.in_empty_enabled);
}

/// A bus reset starts a new host session: the reply in flight and any partly received request are
/// dropped.
///
/// PORT-05: "A new host session, such as a USB bus reset, drops the reply being sent and any
/// partly received request, so a new session never receives an old reply". The wrong behaviour:
/// keeping the reply in flight (the new session receives the old reply) and keeping the partial
/// request (its tail completes a request the old session never sent).
#[test]
fn a_bus_reset_starts_a_new_session() {
    let mut link = DeviceLineLink::new(PeerClass::Local);
    assert_eq!(link.send_frame(b"{\"id\":1}", now()), Ok(()));
    // A request line that has not ended yet.
    assert_eq!(link.receive(b"sta"), 3);
    let mut buffer = [0u8; DeviceLineLink::MAX_FRAME_LEN];
    assert_eq!(link.poll_frame(&mut buffer, now()), None);

    let mut fifo = FakeFifo {
        pending: Pending {
            bus_reset: true,
            ..Pending::default()
        },
        ..FakeFifo::default()
    };
    let wake = Wake::default();
    service(&mut fifo, &mut link, &wake);

    assert_eq!(wake.count(), 1);
    assert_eq!(link.counters().sessions, 1);
    assert_eq!(link.counters().abandoned_replies, 1);
    assert!(!link.reply_in_flight());
    assert_eq!(link.unsent().len(), 0);
    // The part of the request that arrived went with the session.
    assert_eq!(link.poll_frame(&mut buffer, now()), None);
}

/// One request line in and its reply out whole: the frame the loop polls, the reply the link
/// sends, and the bytes the IN FIFO receives.
///
/// PORT-05: the request is the bytes before the line feed with one carriage return removed, and a
/// reply is one JSON object followed by a line feed. The wrong behaviour: a frame that keeps the
/// carriage return, a reply without its line feed, and a reply split where the client cannot join
/// it.
#[test]
fn a_request_and_its_reply_cross_the_link_whole() {
    let mut fifo = FakeFifo {
        out: b"status\r\n".to_vec(),
        pending: Pending {
            out_packet: true,
            ..Pending::default()
        },
        ..FakeFifo::default()
    };
    let mut link = DeviceLineLink::new(PeerClass::Local);
    let wake = Wake::default();

    service(&mut fifo, &mut link, &wake);

    let mut buffer = [0u8; DeviceLineLink::MAX_FRAME_LEN];
    assert_eq!(
        link.poll_frame(&mut buffer, now()),
        Some(LinkFrame::Frame {
            len: 6,
            peer: PeerClass::Local
        })
    );
    assert_eq!(&buffer[..6], b"status");

    assert_eq!(
        link.send_frame(b"{\"id\":0,\"result\":\"STOPPED\"}", now()),
        Ok(())
    );
    fifo.pending.in_empty = true;
    service(&mut fifo, &mut link, &wake);

    assert_eq!(
        fifo.in_fifo,
        b"{\"id\":0,\"result\":\"STOPPED\"}\n".to_vec()
    );
    assert_eq!(link.unsent().len(), 0);
    // One wake for the packet and one for the reply, as design §2.2 counts every event.
    assert_eq!(wake.count(), 2);
}
