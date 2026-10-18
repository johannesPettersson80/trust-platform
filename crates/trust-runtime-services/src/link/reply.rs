//! The reply a line link is sending (specification 34, PORT-05): one at a time, dropped when the
//! host stops reading, and the line feed that repairs a reply dropped part-way.

use core::time::Duration;

use trust_runtime_core::ports::{MonotonicInstant, TimeSpan};

/// How long a reply may make no progress before the link drops it.
pub const REPLY_STALL_LIMIT: TimeSpan = TimeSpan::new(Duration::from_secs(1));

/// Why a link did not take a reply.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplyRefused {
    /// Another reply is still being sent.
    Busy,
    /// The reply is longer than the link's frame.
    TooLong,
}

/// The reply a link is sending, in `N` bytes of fixed storage: frames of up to `N - 2` bytes,
/// with room for a repair line feed before the frame and the line feed after it.
///
/// The link takes one reply at a time. The platform's transmit interrupt writes [`Self::unsent`]
/// to the hardware and reports what the hardware took with [`Self::mark_sent`]. The loop applies
/// the stall rule whenever it polls the link ([`Self::poll`]): a reply that has made no progress
/// since the poll that last saw it progress, [`REPLY_STALL_LIMIT`] or longer ago, is dropped. The
/// hardware may already hold part of a dropped reply, which cannot be withdrawn, so the next reply
/// then starts with a line feed: the stale part ends as a line of its own that is not a reply.
#[derive(Debug, Clone)]
pub struct ReplyBuffer<const N: usize> {
    bytes: [u8; N],
    /// The reply's length with its line feeds; zero while no reply is in flight.
    len: usize,
    /// The bytes of it the hardware has taken.
    sent: usize,
    /// The reply in flight starts with a repair line feed.
    repairing: bool,
    /// The next reply starts with a repair line feed.
    repair_due: bool,
    /// The sent count at the poll that last saw progress, and when that poll was.
    seen: usize,
    progress_at: MonotonicInstant,
}

impl<const N: usize> ReplyBuffer<N> {
    /// The longest reply frame, in bytes without the line feeds.
    pub const MAX_FRAME_LEN: usize = N - 2;

    /// A buffer with no reply in flight.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            bytes: [0; N],
            len: 0,
            sent: 0,
            repairing: false,
            repair_due: false,
            seen: 0,
            progress_at: MonotonicInstant::from_origin(Duration::ZERO),
        }
    }

    /// Whether a reply is being sent.
    #[must_use]
    pub const fn in_flight(&self) -> bool {
        self.sent < self.len
    }

    /// Whether the next reply starts with a repair line feed.
    #[must_use]
    pub const fn repair_due(&self) -> bool {
        self.repair_due
    }

    /// Starts sending `frame` with its line feed, after a repair line feed when one is due; `now`
    /// is the platform's monotonic clock, from which the stall rule counts.
    ///
    /// # Errors
    ///
    /// [`ReplyRefused::Busy`] while another reply is in flight, and [`ReplyRefused::TooLong`] for
    /// a frame longer than [`Self::MAX_FRAME_LEN`]; the buffer is then unchanged.
    pub fn start(&mut self, frame: &[u8], now: MonotonicInstant) -> Result<(), ReplyRefused> {
        if self.in_flight() {
            return Err(ReplyRefused::Busy);
        }
        if frame.len() > Self::MAX_FRAME_LEN {
            return Err(ReplyRefused::TooLong);
        }
        let lead = usize::from(self.repair_due);
        let end = lead + frame.len();
        self.bytes[..lead].fill(b'\n');
        self.bytes[lead..end].copy_from_slice(frame);
        self.bytes[end] = b'\n';
        self.len = end + 1;
        self.sent = 0;
        self.repairing = self.repair_due;
        self.repair_due = false;
        self.seen = 0;
        self.progress_at = now;
        Ok(())
    }

    /// The bytes of the reply in flight that the hardware has not taken yet; empty while no reply
    /// is in flight.
    #[must_use]
    pub fn unsent(&self) -> &[u8] {
        &self.bytes[self.sent..self.len]
    }

    /// Records that the hardware took `count` more bytes of the reply in flight; true when that
    /// completed the reply.
    pub fn mark_sent(&mut self, count: usize) -> bool {
        if !self.in_flight() {
            return false;
        }
        self.sent = self.sent.saturating_add(count).min(self.len);
        if self.in_flight() {
            return false;
        }
        self.finish();
        true
    }

    /// The stall rule at a poll at `now`: true when it dropped the reply in flight.
    pub fn poll(&mut self, now: MonotonicInstant) -> bool {
        if !self.in_flight() {
            return false;
        }
        if self.sent != self.seen {
            self.seen = self.sent;
            self.progress_at = now;
            return false;
        }
        if now.saturating_since(self.progress_at) < REPLY_STALL_LIMIT {
            return false;
        }
        self.drop_reply();
        true
    }

    /// Drops the reply in flight, for a new session of the host; true when there was one.
    pub fn abandon(&mut self) -> bool {
        if !self.in_flight() {
            return false;
        }
        self.drop_reply();
        true
    }

    /// Drops the reply in flight. When the hardware may hold part of it, or the repair line feed
    /// it carried was not sent yet, the next reply starts with a repair line feed.
    fn drop_reply(&mut self) {
        self.repair_due = self.sent > 0 || self.repairing;
        self.finish();
    }

    fn finish(&mut self) {
        self.len = 0;
        self.sent = 0;
        self.repairing = false;
    }
}

impl<const N: usize> Default for ReplyBuffer<N> {
    fn default() -> Self {
        Self::new()
    }
}
