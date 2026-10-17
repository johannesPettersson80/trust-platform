//! The FIFO servicing of the C6's USB-Serial/JTAG control link (design §2.2 and specification 34,
//! PORT-05): one pass that moves the bytes of the peripheral's 64-byte FIFOs between them and the
//! shared line link, and that answers the peripheral's interrupt flags.
//!
//! The pass is a function over [`UsbFifo`], the operations it needs from the peripheral, so the
//! adapter's interrupt glue in `hardware/usb_link.rs` drives the peripheral's registers through it
//! and a host test drives a fake FIFO with literal bytes.
//! Nothing here blocks, spins or allocates.

use trust_runtime_core::ports::WakeSignal;
use trust_runtime_services::link::DeviceLineLink;

/// The bytes of one USB-Serial/JTAG FIFO: the most one pass reads or writes (design §2.2).
pub const FIFO_BYTES: usize = 64;

/// The interrupt flags of one servicing pass: the events the peripheral has to report.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Pending {
    /// The host sent a packet; its bytes wait in the OUT FIFO.
    pub out_packet: bool,
    /// The IN FIFO is empty; the reply in flight can be written into it.
    pub in_empty: bool,
    /// The host reset the bus; this is a new host session.
    pub bus_reset: bool,
}

/// What one servicing pass needs from the USB-Serial/JTAG peripheral.
///
/// Every method is non-blocking: it touches the FIFOs and the interrupt registers and returns, so
/// the interrupt handler never waits for the host.
pub trait UsbFifo {
    /// The interrupt flags pending now, cleared. An event that arrives while the pass runs is
    /// latched again and the handler runs once more for it, so no event is lost to the clear.
    fn take_pending(&mut self) -> Pending;

    /// Reads and removes at most `out.len()` bytes from the OUT FIFO, oldest first, and returns
    /// how many it read.
    fn read_out(&mut self, out: &mut [u8]) -> usize;

    /// Writes at most [`FIFO_BYTES`] bytes of `bytes` into the IN FIFO, marks the packet done
    /// (`WR_DONE`) so the host can take it, and returns how many bytes it took.
    fn write_in(&mut self, bytes: &[u8]) -> usize;

    /// Enables or disables the IN FIFO's empty interrupt: enabled while a reply is unsent,
    /// disabled while there is nothing to send.
    fn set_in_empty_enabled(&mut self, enabled: bool);

    /// Enables or disables the OUT packet interrupt: disabled while the link cannot take the bytes
    /// the OUT FIFO holds, so the handler is not entered again for bytes it must leave.
    fn set_out_packet_enabled(&mut self, enabled: bool);
}

/// Serves one interrupt: the events `fifo` reports, and the bytes they carry (design §2.2).
///
/// A packet the host sent is moved into `link` without taking more than the link takes; a bus
/// reset drops the reply in flight and any partly received request; the reply in flight is written
/// into the IN FIFO 64 bytes at a time, and its interrupt is disabled once nothing is unsent.
/// Every event advances `wake`, so the resource loop's wait ends at once (design §2.5).
pub fn service<F, W>(fifo: &mut F, link: &mut DeviceLineLink, wake: &W)
where
    F: UsbFifo,
    W: WakeSignal,
{
    let pending = fifo.take_pending();
    if pending.bus_reset {
        // A new host session never receives an old reply (specification 34, PORT-05).
        link.reset_session();
        wake.wake();
    }
    if pending.out_packet {
        if drain_out(fifo, link) {
            // The bytes stay with the hardware, which stops accepting packets; the handler is not
            // entered again for them until the loop has taken requests and the link can take them.
            fifo.set_out_packet_enabled(false);
        }
        wake.wake();
    }
    if pending.in_empty {
        fill_in_fifo(fifo, link);
        wake.wake();
    }
}

/// Moves the bytes the OUT FIFO holds into `link`, oldest first, and no more than the link takes:
/// the rest stay in the FIFO, so the hardware stops accepting packets and no byte is lost
/// (specification 34, PORT-05).
///
/// Returns whether the OUT FIFO still holds bytes the link could not take, which is when the
/// caller disables the packet interrupt until the loop has taken requests again.
#[must_use]
pub fn drain_out<F>(fifo: &mut F, link: &mut DeviceLineLink) -> bool
where
    F: UsbFifo,
{
    // At most one FIFO of bytes and at most what the link takes now: a byte read out of the FIFO
    // but not taken by the link would be lost.
    let limit = link.receive_capacity().min(FIFO_BYTES);
    let mut bytes = [0u8; FIFO_BYTES];
    let read = fifo.read_out(&mut bytes[..limit]);
    if read > 0 {
        let taken = link.receive(&bytes[..read]);
        debug_assert_eq!(taken, read, "the link takes every byte within its capacity");
    }
    // The FIFO holds more than the link takes when the read filled the limit and the limit is
    // shorter than the FIFO, and when the link takes nothing at all.
    read == limit && limit < FIFO_BYTES
}

/// Writes the bytes of the reply in flight into the IN FIFO, at most [`FIFO_BYTES`] at a time, and
/// disables the IN FIFO's empty interrupt once nothing is unsent (design §2.2).
fn fill_in_fifo<F>(fifo: &mut F, link: &mut DeviceLineLink)
where
    F: UsbFifo,
{
    if !link.unsent().is_empty() {
        let written = fifo.write_in(link.unsent());
        link.mark_sent(written);
    }
    if link.unsent().is_empty() {
        // Nothing is unsent: `send_frame` enables the interrupt again with the next reply.
        fifo.set_in_empty_enabled(false);
    }
}
