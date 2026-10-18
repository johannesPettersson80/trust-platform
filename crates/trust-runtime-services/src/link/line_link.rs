//! One control link over a byte stream (specification 34, PORT-05): the request lines, the reply
//! in flight and the counters behind a platform's
//! [`ControlLink`](trust_runtime_core::ports::ControlLink).

use trust_runtime_core::ports::{LinkFrame, MonotonicInstant, PeerClass};

use super::assembler::{Line, LineAssembler};
use super::reply::{ReplyBuffer, ReplyRefused};

/// What a line link has done since it started. Every count saturates.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LinkCounters {
    /// Request frames handed to the loop, empty ones included.
    pub requests: u64,
    /// Requests longer than the frame, dropped and handed to the loop as oversized.
    pub oversized_requests: u64,
    /// Replies the hardware took whole.
    pub replies: u64,
    /// Replies dropped by the stall rule.
    pub stalled_replies: u64,
    /// Replies dropped by a new host session.
    pub abandoned_replies: u64,
    /// Replies the link refused: another was in flight, or the reply was longer than the frame.
    pub refused_replies: u64,
    /// Times the receive buffer was full and left bytes with the hardware.
    pub receive_full: u64,
    /// New host sessions.
    pub sessions: u64,
}

impl LinkCounters {
    /// No event counted.
    pub const ZERO: Self = Self {
        requests: 0,
        oversized_requests: 0,
        replies: 0,
        stalled_replies: 0,
        abandoned_replies: 0,
        refused_replies: 0,
        receive_full: 0,
        sessions: 0,
    };
}

/// A control link over a byte stream with buffers of `N` bytes, for frames of up to `N - 2` bytes
/// in both directions: the receive ring holds one line of the longest frame with its carriage
/// return and line feed, and the reply buffer one reply with a repair line feed before it and its
/// line feed after it.
///
/// The platform keeps it behind its own critical section and calls it from two sides:
/// - its interrupts hand over what the hardware received ([`Self::receive`]), send what is unsent
///   ([`Self::unsent`], [`Self::mark_sent`]) and report a new host session
///   ([`Self::reset_session`]);
/// - its [`ControlLink`](trust_runtime_core::ports::ControlLink) answers the loop with
///   [`Self::poll_frame`] and [`Self::send_frame`], with a reading of its monotonic clock.
///
/// It never allocates, and it keeps no platform state.
#[derive(Debug, Clone)]
pub struct LineLink<const N: usize> {
    assembler: LineAssembler<N>,
    reply: ReplyBuffer<N>,
    peer: PeerClass,
    counters: LinkCounters,
}

impl<const N: usize> LineLink<N> {
    /// A link to the client `peer`, [`PeerClass::Local`] for a USB cable, that has received and
    /// sent nothing.
    #[must_use]
    pub const fn new(peer: PeerClass) -> Self {
        Self {
            assembler: LineAssembler::new(),
            reply: ReplyBuffer::new(),
            peer,
            counters: LinkCounters::ZERO,
        }
    }

    /// The longest frame the link moves in either direction, in bytes without the line end.
    pub const MAX_FRAME_LEN: usize = LineAssembler::<N>::MAX_FRAME_LEN;

    /// How many bytes [`Self::receive`] takes now at the least. An interrupt that reads its
    /// hardware byte by byte reads no more than this, so that what it reads is never lost.
    #[must_use]
    pub const fn receive_capacity(&self) -> usize {
        self.assembler.receive_capacity()
    }

    /// Takes bytes the hardware received, oldest first, and returns how many it took. The rest
    /// stay with the hardware, which stops accepting more until the loop has taken requests.
    pub fn receive(&mut self, bytes: &[u8]) -> usize {
        let taken = self.assembler.receive(bytes);
        if taken < bytes.len() {
            bump(&mut self.counters.receive_full);
        }
        taken
    }

    /// The bytes of the reply in flight that the hardware has not taken yet; empty while no reply
    /// is in flight.
    #[must_use]
    pub fn unsent(&self) -> &[u8] {
        self.reply.unsent()
    }

    /// Records that the hardware took `count` more bytes of the reply in flight.
    pub fn mark_sent(&mut self, count: usize) {
        if self.reply.mark_sent(count) {
            bump(&mut self.counters.replies);
        }
    }

    /// A new host session, such as a USB bus reset: the reply in flight and any partly received
    /// request are dropped, so a new session never receives an old reply.
    pub fn reset_session(&mut self) {
        bump(&mut self.counters.sessions);
        if self.reply.abandon() {
            bump(&mut self.counters.abandoned_replies);
        }
        self.assembler.reset();
    }

    /// Whether a reply is being sent.
    #[must_use]
    pub const fn reply_in_flight(&self) -> bool {
        self.reply.in_flight()
    }

    /// The next request for the control loop, with its frame at the start of `buffer`; `now` is
    /// the platform's monotonic clock.
    ///
    /// The stall rule comes first: a reply that has made no progress for
    /// [`REPLY_STALL_LIMIT`](crate::link::REPLY_STALL_LIMIT) is dropped. While a reply is still in
    /// flight the link hands out no request, so the loop ends its pass and waits.
    pub fn poll_frame(&mut self, buffer: &mut [u8], now: MonotonicInstant) -> Option<LinkFrame> {
        if self.reply.poll(now) {
            bump(&mut self.counters.stalled_replies);
        }
        if self.reply.in_flight() {
            self.assembler.drop_overlong();
            return None;
        }
        let peer = self.peer;
        let frame = match self.assembler.next_line(buffer)? {
            Line::Frame(len) => {
                bump(&mut self.counters.requests);
                LinkFrame::Frame { len, peer }
            }
            Line::Oversized => {
                bump(&mut self.counters.oversized_requests);
                LinkFrame::Oversized { peer }
            }
        };
        Some(frame)
    }

    /// Starts sending `frame`, the reply to the request polled last, with its line feed; `now` is
    /// the platform's monotonic clock.
    ///
    /// # Errors
    ///
    /// [`ReplyRefused::Busy`] while another reply is in flight, and [`ReplyRefused::TooLong`] for
    /// a frame longer than [`Self::MAX_FRAME_LEN`]; the link keeps no part of it.
    pub fn send_frame(&mut self, frame: &[u8], now: MonotonicInstant) -> Result<(), ReplyRefused> {
        let started = self.reply.start(frame, now);
        if started.is_err() {
            bump(&mut self.counters.refused_replies);
        }
        started
    }

    /// What the link has done since it started.
    #[must_use]
    pub const fn counters(&self) -> LinkCounters {
        self.counters
    }
}

fn bump(counter: &mut u64) {
    *counter = counter.saturating_add(1);
}
