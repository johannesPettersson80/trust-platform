//! The control link over a byte stream (specification 34, PORT-05), such as a chip's USB serial
//! port: the line framing, the frame bound, one reply in flight, the stall rule with its line-feed
//! repair, and the link's counters.
//!
//! A platform adapter keeps one [`LineLink`] for each such link behind its own critical section
//! and only moves bytes: its receive interrupt hands what arrived to [`LineLink::receive`], its
//! transmit interrupt writes [`LineLink::unsent`] to the hardware and reports what the hardware
//! took with [`LineLink::mark_sent`], and its
//! [`ControlLink`](trust_runtime_core::ports::ControlLink) answers the control loop with
//! [`LineLink::poll_frame`] and [`LineLink::send_frame`]. Everything here is fixed storage:
//! nothing allocates.

mod assembler;
mod line_link;
mod reply;
mod ring;

pub use assembler::{Line, LineAssembler};
pub use line_link::{LineLink, LinkCounters};
pub use reply::{ReplyBuffer, ReplyRefused, REPLY_STALL_LIMIT};
pub use ring::ByteRing;

/// The longest frame of a chip's USB link in either direction, in bytes without the line end
/// (PORT-05): the same on every chip, so every chip answers alike.
pub const DEVICE_FRAME_LEN: usize = 8_192;

/// The buffer size of a line link of [`DEVICE_FRAME_LEN`] frames: a line of the longest frame
/// with its carriage return and line feed, and a reply with a repair line feed and its line feed.
pub const DEVICE_LINK_BUFFER_LEN: usize = DEVICE_FRAME_LEN + 2;

/// The line link of a chip's USB serial port, for frames of up to [`DEVICE_FRAME_LEN`] bytes.
pub type DeviceLineLink = LineLink<DEVICE_LINK_BUFFER_LEN>;

#[cfg(test)]
mod framing_contract_tests;

#[cfg(test)]
mod reply_contract_tests;
