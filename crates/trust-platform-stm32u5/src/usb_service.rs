//! The servicing pass of the STM32U5A9's control link over its USB CDC-ACM class (design §2.2 and
//! §2.3; specification 34, PORT-05). One pass moves the packets the host sent into the shared line
//! link, writes the reply in flight into the class as far as the endpoint takes it, and advances the
//! wake counter when a request completed or a reply moved. A bus reset, and DTR falling or rising,
//! end the host session: the reply in flight and any partly received request are dropped, so a new
//! session never receives an old byte.
//!
//! The pass runs over a [`CdcPort`], so the host tests drive it over a fake class with literal
//! packets, and the chip's glue implements the port over usb-device. Nothing here blocks or
//! allocates.

use trust_runtime_core::ports::WakeSignal;
use trust_runtime_services::link::DeviceLineLink;

/// The bytes of one full-speed bulk packet of the control class (design §2.2). A reply longer than
/// this takes several packets, and a reply that ends on a full packet is followed by a zero-length
/// packet, which the host needs to see the transfer end.
pub const PACKET_BYTES: usize = 64;

/// The packets of the control class, as a servicing pass moves them. Every method is non-blocking.
pub trait CdcPort {
    /// Reads the next packet the host sent into `out`, which holds at least [`PACKET_BYTES`] bytes,
    /// and returns its length; `None` when no packet waits.
    fn read_packet(&mut self, out: &mut [u8]) -> Option<usize>;

    /// Writes one packet of `bytes`, at most [`PACKET_BYTES`] bytes, and returns how many bytes the
    /// endpoint took; `None` while the endpoint still holds a packet. An empty `bytes` sends a
    /// zero-length packet.
    fn write_packet(&mut self, bytes: &[u8]) -> Option<usize>;

    /// Whether the host has the port open: DTR is high.
    fn dtr(&self) -> bool;

    /// Whether usb-device reported a bus reset since the last call; the call clears the report.
    fn take_bus_reset(&mut self) -> bool;
}

/// What the servicing pass keeps between two passes: the DTR level it last saw, and whether a
/// zero-length packet is still owed to the host.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct LinkService {
    dtr: bool,
    zero_length_due: bool,
}

impl LinkService {
    /// Runs one pass and returns whether the link can take a whole packet now. The caller keeps the
    /// OUT interrupt enabled only then: while the link has no room, the peripheral keeps the packet
    /// and must not raise the interrupt again and again (design §2.2).
    pub fn pass<P, W>(&mut self, port: &mut P, link: &mut DeviceLineLink, wake: &W) -> bool
    where
        P: CdcPort,
        W: WakeSignal,
    {
        if port.take_bus_reset() {
            // A bus reset closes the port: the class clears DTR with it, so the check below sees no
            // edge and the reset is counted as one session.
            self.dtr = false;
            self.end_session(link);
        }
        let dtr = port.dtr();
        if dtr != self.dtr {
            // DTR falling closes the session and DTR rising opens one; either way the transmit
            // side starts empty, so no reply of the old session reaches the new one (S1b §2.3).
            self.dtr = dtr;
            self.end_session(link);
        }
        let completed = receive(port, link);
        let moved = self.send(port, link);
        if completed || moved {
            wake.wake();
        }
        link.receive_capacity() >= PACKET_BYTES
    }

    /// A new host session: the reply in flight and any partly received request are dropped
    /// (specification 34, PORT-05).
    fn end_session(&mut self, link: &mut DeviceLineLink) {
        link.reset_session();
        self.zero_length_due = false;
    }

    /// Writes the reply in flight while the port is open, a packet at a time, and reports whether
    /// any byte moved. The owed zero-length packet goes first, so it never follows a later reply.
    fn send<P: CdcPort>(&mut self, port: &mut P, link: &mut DeviceLineLink) -> bool {
        if !self.dtr || (self.zero_length_due && port.write_packet(&[]).is_none()) {
            return false;
        }
        self.zero_length_due = false;
        let mut moved = false;
        loop {
            let unsent = link.unsent();
            if unsent.is_empty() {
                break;
            }
            let chunk = &unsent[..unsent.len().min(PACKET_BYTES)];
            let Some(taken) = port.write_packet(chunk) else {
                break;
            };
            link.mark_sent(taken);
            moved |= taken > 0;
            if taken == PACKET_BYTES && link.unsent().is_empty() {
                // A full packet that ends the reply: the host waits for a short packet to end the
                // transfer, so a zero-length one follows, now if the endpoint takes it, else later.
                self.zero_length_due = port.write_packet(&[]).is_none();
            }
        }
        moved
    }
}

/// Moves the packets the host sent into the link, while the link can take a whole packet, and
/// reports whether one of them completed a request line, that is carried the line feed. A packet
/// the link cannot take stays with the port: nothing is read and then lost.
fn receive<P: CdcPort>(port: &mut P, link: &mut DeviceLineLink) -> bool {
    let mut packet = [0u8; PACKET_BYTES];
    let mut completed = false;
    while link.receive_capacity() >= PACKET_BYTES {
        let Some(len) = port.read_packet(&mut packet) else {
            break;
        };
        let received = &packet[..len];
        let taken = link.receive(received);
        debug_assert_eq!(
            taken, len,
            "the link takes a whole packet within its capacity"
        );
        completed |= received.contains(&b'\n');
    }
    completed
}
