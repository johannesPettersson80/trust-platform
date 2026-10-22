//! Small platform contracts of the shared runtime (specification 34, §3.2 and §7).
//!
//! A platform adapter implements these for its operating system or chip: the monotonic and wall
//! clocks, the wait and wake of the resource loop (PORT-03), the hardware watchdog (TIME-04), the
//! control link, which moves whole request and reply frames (the management transport of §7), and
//! the checkpoint store a chip keeps its retained values in (§10), and the program store that holds
//! its installed program in two application slots (§5.1). The engine and the shared runtime
//! services use them and hold no platform state.
//!
//! Time on the monotonic clock has its own types (PORT-01): a
//! [`MonotonicInstant`](crate::ports::MonotonicInstant) is a reading of the clock and a
//! [`TimeSpan`](crate::ports::TimeSpan) a nonnegative length of time on it. Neither converts
//! implicitly to the program's logical time (`value::Duration`), and an IEC `TIME` value becomes a
//! span only through [`TimeSpan::from_time`](crate::ports::TimeSpan::from_time), which refuses a
//! negative one.

use alloc::boxed::Box;
use core::net::SocketAddr;
use core::time::Duration;

use crate::retain::checkpoint::{CheckpointFlash, FlashError, FlashGeometry, Slot};
use crate::value;

/// A reading of a platform's monotonic clock: the time since the clock's origin (PORT-01), in whole
/// nanoseconds up to about 584 years.
///
/// It is neither a logical (PLC) time nor a span. Readings of different clocks, or of one clock
/// across a device reset, are not comparable. A reading and a span are whole nanoseconds, not a
/// `Duration` of seconds and nanoseconds, so that a 32-bit chip compares and adds them as integers
/// and builds a reading from its timer's microseconds by one multiplication, where a `Duration`
/// takes two 64-bit divisions, which such a chip makes in software (specification 34 PERF-05).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MonotonicInstant(u64);

impl MonotonicInstant {
    /// The reading `since_origin` after the clock's origin, or the latest representable instant,
    /// about 584 years after the origin, for a longer time; a clock implementation builds its
    /// readings with it.
    #[must_use]
    pub const fn from_origin(since_origin: Duration) -> Self {
        Self(whole_nanos(since_origin))
    }

    /// The reading `micros` microseconds after the clock's origin, saturating as
    /// [`Self::from_origin`] does: a clock whose timer counts microseconds builds its readings with
    /// it without dividing.
    #[must_use]
    pub const fn from_micros(micros: u64) -> Self {
        Self(micros.saturating_mul(1_000))
    }

    /// The time since the clock's origin.
    #[must_use]
    pub const fn since_origin(self) -> Duration {
        Duration::from_nanos(self.0)
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
/// duration (PORT-01), in whole nanoseconds up to about 584 years, as a [`MonotonicInstant`] is.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TimeSpan(u64);

impl TimeSpan {
    /// No time.
    pub const ZERO: Self = Self(0);

    /// The span `duration`, or the longest representable span, about 584 years, for a longer one.
    #[must_use]
    pub const fn new(duration: Duration) -> Self {
        Self(whole_nanos(duration))
    }

    /// The span of IEC `TIME` value `time`, or `None` for a negative time, which is no span.
    #[must_use]
    pub fn from_time(time: value::Duration) -> Option<Self> {
        u64::try_from(time.as_nanos()).ok().map(Self)
    }

    /// The span in whole nanoseconds.
    #[must_use]
    pub const fn as_nanos(self) -> u64 {
        self.0
    }

    /// The span as a `Duration`. A span under a second, as a scan's or a task's is, needs no
    /// division into seconds, which a 32-bit chip makes in software (checklist P1-6).
    #[must_use]
    #[allow(clippy::cast_possible_truncation)] // a span under a second fits 32 bits
    pub const fn as_duration(self) -> Duration {
        if self.0 < 1_000_000_000 {
            return Duration::new(0, self.0 as u32);
        }
        Duration::from_nanos(self.0)
    }

    /// This span and `other` together, or the longest representable span, about 584 years, where
    /// that would overflow.
    #[must_use]
    pub fn saturating_add(self, other: Self) -> Self {
        Self(self.0.saturating_add(other.0))
    }

    /// This span less `other`, or zero when `other` is longer: a span never goes negative.
    #[must_use]
    pub fn saturating_sub(self, other: Self) -> Self {
        Self(self.0.saturating_sub(other.0))
    }
}

/// The whole nanoseconds of `duration`, or `u64::MAX`, about 584 years, for a longer one.
const fn whole_nanos(duration: Duration) -> u64 {
    match duration.as_secs().checked_mul(1_000_000_000) {
        Some(nanos) => nanos.saturating_add(duration.subsec_nanos() as u64),
        None => u64::MAX,
    }
}

/// The monotonic clock (specification 34, §7 and PORT-01).
///
/// - Origin and epoch: the platform's, for example the clock's creation or the device's reset;
///   a reset starts a new epoch.
/// - Resolution: the platform's timer resolution, at most a nanosecond; a reading is truncated to
///   it.
/// - Wrap: none. A platform extends its hardware counter to a 64-bit count, so a reading never
///   wraps within the device's operating life; a reading saturates about 584 years after the
///   origin.
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

    /// Whether the link still holds reply bytes it has not handed to its hardware.
    ///
    /// While this is true the resource loop services the link at least once a millisecond, so a
    /// reply in flight is driven to its end without depending on the link's interrupts
    /// (specification 34, PORT-05). A link that hands every reply to its hardware inside
    /// [`Self::send_frame`] keeps the default.
    fn sending(&self) -> bool {
        false
    }
}

/// The checkpoint store a platform supplies to the installed program's cycle (specification 34
/// §10): the flash of the device's two checkpoint slots and the program budget of one persister
/// step.
pub struct CheckpointStore {
    /// The flash of the device's two checkpoint slots, `checkpoint_a` and `checkpoint_b`.
    pub flash: Box<dyn CheckpointFlash>,
    /// The most bytes one persister step may program in the idle time before the next release, so
    /// that one program step fits between two scans (specification 34 §10).
    pub program_budget: usize,
}

/// A platform that keeps its retained values in the checkpoint flash of its chip file
/// (specification 34 §10).
///
/// The composition takes the store once, when it builds the installed program's cycle; the cycle's
/// persister holds the flash from then on. A warm start restores the newest committed checkpoint,
/// and the cycle saves the retained values into the log as the project's interval and a stop or
/// shutdown ask (specification 11 §6.7).
pub trait CheckpointStorage {
    /// Takes the platform's checkpoint store, or returns `None` when the device keeps no retained
    /// checkpoint, as a project with `[runtime.retain] mode = "none"` asks for.
    fn take_checkpoint_store(&mut self) -> Option<CheckpointStore>;
}

/// The flash of a platform's two application slots, as its adapter drives it (specification 34
/// §5.1, DEPLOY-03).
///
/// It has the shape of [`CheckpointFlash`]. Offsets count from the first byte of a slot. The
/// program store reads and programs only whole program units, at multiples of the unit and inside
/// the slot, and erases one erase unit at a time by its first byte. A power loss may stop a program
/// or an erase at any byte, and the store is built for that.
pub trait ProgramFlash {
    /// The flash's program and erase units and the slot size.
    fn geometry(&self) -> FlashGeometry;

    /// Read `bytes.len()` bytes from `offset` in `slot`.
    ///
    /// # Errors
    ///
    /// [`FlashError::Read`] when the flash cannot be read.
    fn read(&mut self, slot: Slot, offset: u32, bytes: &mut [u8]) -> Result<(), FlashError>;

    /// Program `bytes` at `offset` in `slot`: every bit that is 0 in `bytes` becomes 0. A program
    /// never sets a bit to 1.
    ///
    /// # Errors
    ///
    /// [`FlashError::Program`] when the flash reports the program as failed.
    fn program(&mut self, slot: Slot, offset: u32, bytes: &[u8]) -> Result<(), FlashError>;

    /// Erase the erase unit that starts at `offset` in `slot`: every byte becomes 0xFF.
    ///
    /// # Errors
    ///
    /// [`FlashError::Erase`] when the flash reports the erase as failed.
    fn erase(&mut self, slot: Slot, offset: u32) -> Result<(), FlashError>;

    /// The `len` bytes at `offset` in `slot` where they lie in the address space, when the platform
    /// reads its flash in place (specification 34 MEM-14, the program image in place); `None` when
    /// it does not, or when the bytes cannot be mapped, and the caller reads them instead.
    ///
    /// The bytes stay valid and unchanged while no install writes the slot and no later call maps
    /// another slot's bytes in their place: the program store writes no slot whose image the runtime
    /// reads (specification 34 §5.1), and the runtime maps a slot only after it has dropped the
    /// program that read the bytes an earlier call mapped.
    fn map(&mut self, slot: Slot, offset: u32, len: u32) -> Option<&'static [u8]> {
        let _ = (slot, offset, len);
        None
    }
}

/// The program store a platform supplies for its installed application (specification 34 §5.1):
/// the flash of its two application slots and the most bytes one install step may program.
pub struct ProgramStore {
    /// The flash of the two application slots, `Slot::A` and `Slot::B`.
    pub flash: Box<dyn ProgramFlash>,
    /// The most bytes one step of an install may program, so that one step fits between two loop
    /// passes, as the checkpoint store's budget does for a checkpoint.
    pub program_budget: usize,
}

/// A platform that keeps its installed program in the two application slots of its flash
/// (specification 34 §5.1).
///
/// The composition takes the store once, as it takes the checkpoint store, and the loop holds it
/// from then on. A device without application slots keeps no installed program and takes `None`.
pub trait ProgramStorage {
    /// Takes the platform's program store, or returns `None` when the device keeps no installed
    /// program in its flash.
    fn take_program_store(&mut self) -> Option<ProgramStore>;
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

    /// A reading and a span are whole nanoseconds up to `u64::MAX`, 18,446,744,073.709551615 s,
    /// about 584 years (specification 34 PERF-05): a reading of microseconds is a thousand times as
    /// many nanoseconds, a `Duration` keeps its nanoseconds, and anything longer is the latest
    /// instant or the longest span. Worked by hand: 1,500 µs is 1.5 ms; 18,446,744,073,709,551 µs
    /// is 18,446,744,073.709551 s, and one microsecond more passes `u64::MAX` nanoseconds.
    /// Catches: microseconds taken as milliseconds or nanoseconds, a `Duration`'s nanoseconds below
    /// the second lost, and a long time that wraps to a short one instead of saturating.
    #[test]
    fn readings_and_spans_are_whole_nanoseconds_that_saturate() {
        assert_eq!(
            MonotonicInstant::from_micros(1_500),
            MonotonicInstant::from_origin(Duration::from_micros(1_500))
        );
        assert_eq!(
            MonotonicInstant::from_micros(1_500).since_origin(),
            Duration::from_nanos(1_500_000)
        );
        let last_micro = 18_446_744_073_709_551;
        assert_eq!(
            MonotonicInstant::from_micros(last_micro).since_origin(),
            Duration::new(18_446_744_073, 709_551_000)
        );
        let latest = MonotonicInstant::from_origin(Duration::MAX);
        assert_eq!(MonotonicInstant::from_micros(last_micro + 1), latest);
        assert_eq!(
            latest.since_origin(),
            Duration::new(18_446_744_073, 709_551_615)
        );
        assert_eq!(
            TimeSpan::new(Duration::new(1, 999_999_999)).as_duration(),
            Duration::new(1, 999_999_999)
        );
        assert_eq!(
            TimeSpan::new(Duration::from_secs(18_446_744_074)),
            TimeSpan::new(Duration::MAX)
        );
        assert_eq!(
            TimeSpan::new(Duration::MAX).as_duration(),
            Duration::new(18_446_744_073, 709_551_615)
        );
    }

    /// A span converts to the same `Duration` on both sides of a second, where its conversion stops
    /// taking the short way (PORT-01): 999,999,999 ns is 0 s and 999,999,999 ns, 1,000,000,000 ns is
    /// 1 s, and 1,000,000,001 ns is 1 s and 1 ns. Catches a short way whose nanoseconds reach a whole
    /// second, or whose bound is off by one.
    #[test]
    fn a_span_converts_alike_on_both_sides_of_a_second() {
        let nanos = |n| TimeSpan::new(Duration::from_nanos(n)).as_duration();
        assert_eq!(nanos(999_999_999), Duration::new(0, 999_999_999));
        assert_eq!(nanos(1_000_000_000), Duration::new(1, 0));
        assert_eq!(nanos(1_000_000_001), Duration::new(1, 1));
    }

    /// Spans add and subtract without going past their range or below zero (PORT-01): 5 ms and
    /// 2 ms are 7 ms, 5 ms less 2 ms is 3 ms, 2 ms less 5 ms is no time, and the longest span with
    /// 1 ms more is still the longest. Catches a sum that wraps to a short span and a difference
    /// that wraps to a huge one, which would move a scan's deadline by centuries.
    #[test]
    fn spans_add_and_subtract_within_their_range() {
        let span = |ms| TimeSpan::new(Duration::from_millis(ms));
        assert_eq!(span(5).saturating_add(span(2)), span(7));
        assert_eq!(span(5).saturating_sub(span(2)), span(3));
        assert_eq!(span(2).saturating_sub(span(5)), TimeSpan::ZERO);
        let longest = TimeSpan::new(Duration::MAX);
        assert_eq!(longest.saturating_add(span(1)), longest);
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
