//! Small platform contracts of the shared runtime (specification 34, §3.2 and §7).
//!
//! A platform adapter implements these for its operating system or chip: the monotonic and wall
//! clocks, the wait and wake of the resource loop (PORT-03), the hardware watchdog (TIME-04) and
//! the control link, which moves whole request and reply frames (the management transport of §7).
//! The engine and the shared runtime services use them and hold no platform state.
//!
//! Time on the monotonic clock has its own types (PORT-01): a
//! [`MonotonicInstant`](crate::ports::MonotonicInstant) is a reading of the clock and a
//! [`TimeSpan`](crate::ports::TimeSpan) a nonnegative length of time on it. Neither converts
//! implicitly to the program's logical time (`value::Duration`), and an IEC `TIME` value becomes a
//! span only through [`TimeSpan::from_time`](crate::ports::TimeSpan::from_time), which refuses a
//! negative one.

use core::net::SocketAddr;
use core::time::Duration;

use crate::value;

/// A reading of a platform's monotonic clock: the time since the clock's origin (PORT-01).
///
/// It is neither a logical (PLC) time nor a span. Readings of different clocks, or of one clock
/// across a device reset, are not comparable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MonotonicInstant(Duration);

impl MonotonicInstant {
    /// The reading `since_origin` after the clock's origin; a clock implementation builds its
    /// readings with it.
    #[must_use]
    pub const fn from_origin(since_origin: Duration) -> Self {
        Self(since_origin)
    }

    /// The time since the clock's origin.
    #[must_use]
    pub const fn since_origin(self) -> Duration {
        self.0
    }

    /// The instant `span` later, or the latest representable instant, about 584 years after the
    /// origin, where that would overflow.
    #[must_use]
    pub fn saturating_add(self, span: TimeSpan) -> Self {
        Self(self.0.saturating_add(span.0))
    }

    /// The span from `earlier` to this instant, or zero when `earlier` is later: a deadline that
    /// has passed has no time left, and the arithmetic never goes negative.
    #[must_use]
    pub fn saturating_since(self, earlier: Self) -> TimeSpan {
        TimeSpan(self.0.saturating_sub(earlier.0))
    }
}

/// A nonnegative length of time on the monotonic clock: a budget, an interval or a measured
/// duration (PORT-01).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TimeSpan(Duration);

impl TimeSpan {
    /// No time.
    pub const ZERO: Self = Self(Duration::ZERO);

    /// The span `duration`.
    #[must_use]
    pub const fn new(duration: Duration) -> Self {
        Self(duration)
    }

    /// The span of IEC `TIME` value `time`, or `None` for a negative time, which is no span.
    #[must_use]
    pub fn from_time(time: value::Duration) -> Option<Self> {
        u64::try_from(time.as_nanos())
            .ok()
            .map(|nanos| Self(Duration::from_nanos(nanos)))
    }

    /// The span as a `Duration`.
    #[must_use]
    pub const fn as_duration(self) -> Duration {
        self.0
    }
}

/// The monotonic clock (specification 34, §7 and PORT-01).
///
/// - Origin and epoch: the platform's, for example the clock's creation or the device's reset;
///   a reset starts a new epoch.
/// - Resolution: the platform's timer resolution; a reading is truncated to it.
/// - Wrap: none. A platform extends its hardware counter to a 64-bit count, so a reading never
///   wraps within the device's operating life.
/// - Discontinuity: a reading never decreases.
pub trait MonotonicClock {
    /// The current reading.
    fn now(&self) -> MonotonicInstant;
}

impl<T: MonotonicClock + ?Sized> MonotonicClock for &T {
    fn now(&self) -> MonotonicInstant {
        (**self).now()
    }
}

/// Wall-clock (calendar) time.
pub trait WallClock {
    /// Milliseconds since the Unix epoch, or `None` while the time is not set: a device without a
    /// battery-held clock has no time until its time server sets it (TARGET-06).
    fn utc_ms(&self) -> Option<u64>;
}

/// A reading of the platform's wake counter (PORT-03).
///
/// Every wake advances the counter. A wait given an older reading returns at once, so a wake that
/// arrives between the reading and the wait is never lost. The counter wraps; a wait would miss a
/// wake only after exactly 2^32 wakes between the reading and the wait, which cannot happen within
/// one pass of the resource loop, and even then the wait ends at its deadline, so no pending work
/// is lost.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WakeToken(u32);

impl WakeToken {
    /// The token of wake counter value `count`.
    #[must_use]
    pub const fn new(count: u32) -> Self {
        Self(count)
    }

    /// The wake counter value.
    #[must_use]
    pub const fn count(self) -> u32 {
        self.0
    }
}

/// Why a wait ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WakeReason {
    /// The deadline was reached.
    Deadline,
    /// A wake arrived before the deadline, possibly before the wait began, or the wait ended
    /// early for a reason of the platform's own.
    Event,
}

/// A failure of a platform port (specification 34, §7.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PortError {
    /// The platform's code for the failure.
    pub code: u16,
    /// The failing device, numbered by the platform.
    pub device: u16,
}

/// The wait of the resource loop (PORT-03): it sleeps until a deadline or a wake.
pub trait RunnerWait {
    /// The current wake token. The loop reads it before it checks for pending work.
    fn wake_token(&self) -> WakeToken;

    /// Waits until the monotonic clock, the [`MonotonicClock`] the loop reads, reaches
    /// `deadline`, or until a wake.
    ///
    /// The wait does not sleep when a wake arrived after `observed` was read or when the deadline
    /// has passed. It may also end earlier, as [`WakeReason::Event`]; the loop then runs another
    /// pass and waits again.
    ///
    /// # Errors
    ///
    /// The [`PortError`] of a wait mechanism that failed.
    fn wait_until(
        &mut self,
        deadline: MonotonicInstant,
        observed: WakeToken,
    ) -> Result<WakeReason, PortError>;
}

/// Wakes the resource loop's wait, for example from a link interrupt (PORT-03).
pub trait WakeSignal {
    /// Advances the wake counter, so a current or coming wait returns.
    fn wake(&self);
}

/// The platform's hardware watchdog (specification 34, TIME-04): it resets the device unless it
/// is fed in time.
///
/// The resource loop feeds it once in every pass, after the pass's work, in every resource state,
/// and never waits longer than [`Self::feed_interval`]; so the device is never reset while the
/// loop makes progress, also while the program is stopped, paused or faulted. A scan or a
/// callback that never returns stops the feeding, and the watchdog resets the device.
pub trait HardwareWatchdog {
    /// The longest wait between two feeds. The platform sets the watchdog's timeout longer than
    /// this interval plus the longest pass of the loop: one scan within its cycle watchdog and the
    /// requests the loop serves in one pass.
    fn feed_interval(&self) -> TimeSpan;

    /// Feeds the watchdog.
    fn feed(&mut self);
}

/// Who sent a frame on the control link; the access rights depend on it (specification 11,
/// §6.9.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeerClass {
    /// A client on the device itself or on a cable to it, such as USB, trusted like
    /// trust-runtime's local socket.
    Local,
    /// A client on a network, at this address.
    Network(SocketAddr),
}

/// What the control link received.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkFrame {
    /// A whole frame, now at the start of the buffer.
    Frame {
        /// Length of the frame in bytes.
        len: usize,
        /// Who sent it.
        peer: PeerClass,
    },
    /// A frame longer than the buffer arrived; the link dropped its bytes.
    Oversized {
        /// Who sent it.
        peer: PeerClass,
    },
}

/// The control link: the management transport that moves whole frames (specification 34, §7 and
/// ARCH-12).
///
/// A request frame is one JSON request and a reply frame one JSON reply, each without the line
/// end; a line-based link such as USB serial or TCP adds and removes the line end. The same
/// protocol runs on every link.
pub trait ControlLink {
    /// The longest frame the link moves in either direction, in bytes.
    fn max_frame_len(&self) -> usize;

    /// Moves the next whole frame that has arrived into `buffer`, which holds
    /// [`Self::max_frame_len`] bytes. It never waits.
    fn poll_frame(&mut self, buffer: &mut [u8]) -> Option<LinkFrame>;

    /// Sends `frame`, at most [`Self::max_frame_len`] bytes, to the sender of the frame polled
    /// last.
    ///
    /// # Errors
    ///
    /// The [`PortError`] of a reply the link could not deliver, for example to a peer that has
    /// gone.
    fn send_frame(&mut self, frame: &[u8]) -> Result<(), PortError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixed(Duration);

    impl MonotonicClock for Fixed {
        fn now(&self) -> MonotonicInstant {
            MonotonicInstant::from_origin(self.0)
        }
    }

    fn read<C: MonotonicClock>(clock: C) -> MonotonicInstant {
        clock.now()
    }

    #[test]
    fn a_borrowed_clock_reads_the_same_clock() {
        let clock = Fixed(Duration::from_millis(7));
        let borrowed: &Fixed = &clock;
        let expected = MonotonicInstant::from_origin(Duration::from_millis(7));
        assert_eq!(read(borrowed), expected);
        assert_eq!(read(clock), expected);
    }

    #[test]
    fn wake_tokens_compare_by_their_counter_value() {
        assert_eq!(WakeToken::new(3), WakeToken::new(3));
        assert_ne!(WakeToken::new(3), WakeToken::new(4));
        assert_eq!(WakeToken::new(u32::MAX).count(), u32::MAX);
    }

    #[test]
    fn instant_and_span_arithmetic_saturates_and_never_goes_negative() {
        let at = |ms| MonotonicInstant::from_origin(Duration::from_millis(ms));
        let span = |ms| TimeSpan::new(Duration::from_millis(ms));
        assert_eq!(at(10).saturating_add(span(5)), at(15));
        assert_eq!(at(15).saturating_since(at(10)), span(5));
        assert_eq!(
            at(10).saturating_since(at(15)),
            TimeSpan::ZERO,
            "a deadline that has passed has no time left"
        );
        let latest = MonotonicInstant::from_origin(Duration::MAX);
        assert_eq!(latest.saturating_add(span(1)), latest);
        assert_eq!(at(3).since_origin(), Duration::from_millis(3));
        assert_eq!(span(4).as_duration(), Duration::from_millis(4));
    }

    #[test]
    fn a_time_value_becomes_a_span_only_when_it_is_not_negative() {
        assert_eq!(
            TimeSpan::from_time(value::Duration::from_millis(20)),
            Some(TimeSpan::new(Duration::from_millis(20)))
        );
        assert_eq!(
            TimeSpan::from_time(value::Duration::ZERO),
            Some(TimeSpan::ZERO)
        );
        assert_eq!(TimeSpan::from_time(value::Duration::from_millis(-1)), None);
    }
}
