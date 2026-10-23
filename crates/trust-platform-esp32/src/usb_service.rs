//! The FIFO servicing of the C6's USB-Serial/JTAG control link (design §2.2 and specification 34,
//! PORT-05): one pass that moves the bytes of the peripheral's 64-byte FIFOs between them and the
//! shared line link, that answers the peripheral's interrupt flags, and the bind-time sequence
//! that clears both FIFOs of the session a reset ended.
//!
//! The pass and the bind-time sequence are functions over [`UsbFifo`], the operations they need
//! from the peripheral, so the adapter's interrupt glue in `hardware/usb_link.rs` drives the
//! peripheral's registers through them and a host test drives a fake FIFO with literal bytes.
//! Every pass continues the reply in flight whenever the FIFO's free state shows it can take
//! bytes, so a reply never waits for the IN FIFO's empty event: the peripheral can leave that
//! event unraised with the FIFO already empty and free, and the interrupt only makes the reply
//! sooner. Nothing here blocks, spins or allocates.

use trust_runtime_core::ports::WakeSignal;
use trust_runtime_services::link::DeviceLineLink;

/// The bytes of one USB-Serial/JTAG FIFO: the most one pass reads or writes (design §2.2).
pub const FIFO_BYTES: usize = 64;

/// The interrupt flags of one servicing pass: the events the peripheral has to report.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Pending {
    /// The host sent a packet; its bytes wait in the OUT FIFO.
    pub out_packet: bool,
    /// The IN FIFO is empty; the reply in flight can be written into it. The event only makes the
    /// reply sooner: a servicing pass continues the reply whether or not this is set.
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

    /// Reads and drops every byte the OUT FIFO holds now. A reset the peripheral survived, such
    /// as a watchdog core reset, may leave the tail of a request here; it is discarded before the
    /// link binds, so it never joins the next request.
    fn discard_out(&mut self);

    /// Whether the IN FIFO can take bytes now: the peripheral's IN-endpoint data-free flag. The
    /// reply in flight is written whenever this is set, not only when the empty event arrives,
    /// because the peripheral can leave that event unraised with the FIFO empty and free.
    fn in_fifo_free(&mut self) -> bool;

    /// Writes at most [`FIFO_BYTES`] bytes of `bytes` into the IN FIFO, marks the packet done
    /// (`WR_DONE`) so the host can take it, and returns how many bytes it took.
    fn write_in(&mut self, bytes: &[u8]) -> usize;

    /// Empties the IN FIFO of a session a reset ended: marks whatever it holds done (`WR_DONE`)
    /// so the FIFO frees, and an earlier reply never prepends the next one.
    fn flush_in(&mut self);

    /// Enables or disables the IN FIFO's empty interrupt: enabled while a reply is unsent,
    /// disabled while there is nothing to send.
    fn set_in_empty_enabled(&mut self, enabled: bool);

    /// Enables or disables the OUT packet interrupt: disabled while the link cannot take the bytes
    /// the OUT FIFO holds, so the handler is not entered again for bytes it must leave.
    fn set_out_packet_enabled(&mut self, enabled: bool);
}

/// Brings the peripheral's FIFOs to a known state when the link binds (specification 34,
/// PORT-05): the OUT FIFO's stale bytes are discarded and the IN FIFO is emptied, before any
/// interrupt can move them into the link.
///
/// A watchdog core reset does not reset the USB-Serial/JTAG peripheral, so the session it ended
/// may still hold a half-received request in the OUT FIFO and a half-sent reply in the IN FIFO.
/// Discarding the first keeps a fragment the host sent before the reset out of the request it
/// sends after, and flushing the second frees the FIFO for the next reply.
pub fn bring_up<F>(fifo: &mut F)
where
    F: UsbFifo,
{
    fifo.discard_out();
    fifo.flush_in();
}

/// Serves one interrupt: the events `fifo` reports, and the bytes they carry (design §2.2).
///
/// A packet the host sent is moved into `link` without taking more than the link takes; a bus
/// reset drops the reply in flight and any partly received request. Whatever the events, the
/// reply in flight is continued into the IN FIFO by [`resume_reply`], so its progress never
/// depends on the IN-empty event. Every event advances `wake`, so the resource loop's wait ends at
/// once (design §2.5).
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
        wake.wake();
    }
    // The reply in flight is continued whatever the events: `resume_reply` writes into the IN FIFO
    // whenever its free state shows room, so a reply never waits for an empty event the peripheral
    // failed to raise (specification 34, PORT-05).
    resume_reply(fifo, link);
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

/// Continues the reply in flight into the IN FIFO whenever the FIFO can take bytes: at most one
/// FIFO, into what [`UsbFifo::in_fifo_free`] shows is free, marking the packet done (`WR_DONE`),
/// and the IN FIFO's empty interrupt is disabled once nothing is unsent (design §2.2). The last
/// packet of a reply is never a full FIFO, so it ends the host's transfer (specification 34,
/// Appendix D.2).
///
/// A watchdog core reset does not reset the USB-Serial/JTAG peripheral, and after it the
/// peripheral can report the IN FIFO free while its empty event never arrives (specification 34,
/// PORT-05). Every servicing pass calls this, so the reply in flight never waits for that event,
/// and the interrupt only makes it sooner.
pub fn resume_reply<F>(fifo: &mut F, link: &mut DeviceLineLink)
where
    F: UsbFifo,
{
    let unsent = link.unsent();
    if unsent.is_empty() || !fifo.in_fifo_free() {
        return;
    }
    // A bulk transfer ends at a packet shorter than the endpoint's 64 bytes, and the host's CDC-ACM
    // driver passes on what it has read only when a transfer ends or its two-packet buffer fills:
    // a reply whose last packet were a full 64 bytes would wait in the host until the next packet.
    // So the last 64 bytes go out as 63 and then 1 (specification 34, Appendix D.2).
    let take = if unsent.len() == FIFO_BYTES {
        FIFO_BYTES - 1
    } else {
        unsent.len()
    };
    let written = fifo.write_in(&unsent[..take]);
    link.mark_sent(written);
    if link.unsent().is_empty() {
        // Nothing is unsent: `send_frame` enables the interrupt again with the next reply.
        fifo.set_in_empty_enabled(false);
    }
}
