//! The ESP32-C6's clock reading extended by the CPU cycle counter (checklist P1-6; specification 34
//! Appendix D.2).
//!
//! A SYSTIMER reading requests an update, waits for it and divides 64-bit ticks into microseconds,
//! several microseconds of a scan that checks its deadline at every standard call. The CPU cycle
//! counter is one instruction, and it counts the same crystal as SYSTIMER (the CPU at 160 MHz,
//! SYSTIMER at 16 MHz). So a reading while the CPU runs is the last SYSTIMER reading and the whole
//! microseconds of the cycles counted since. A wait stops the CPU, and the counter with it, so the
//! reading after a wait, and one a second or more after the last SYSTIMER reading, reads SYSTIMER
//! again. A reading is never less than the one before it.
//!
//! The base the readings extend lies in plain words ([`SharedBase`]) that one context reads and
//! writes, so that a reading takes no critical section.

use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use crate::profile::CPU_HZ;

const _: () = assert!(
    CPU_HZ == 160_000_000,
    "whole_micros divides by the C6's 160 cycles a microsecond"
);

/// 2^36 / 160 rounded up (`whole_micros`).
const MICROS_PER_CYCLE_Q36: u64 = 429_496_730;

/// The whole microseconds of `cycles` CPU cycles at 160 MHz, for fewer than 2^28 cycles (1.68 s):
/// `cycles / 160` by a multiplication, which the CPU makes in a cycle where its divider takes many
/// (checklist P1-6). The multiplier exceeds 2^36 / 160 by 0.4, so for fewer than 2^28 cycles the
/// product exceeds 2^36 times the quotient by less than 0.4 / 256 of a microsecond, under the
/// 1/160 that would carry it past the next whole microsecond.
fn whole_micros(cycles: u32) -> u64 {
    (u64::from(cycles) * MICROS_PER_CYCLE_Q36) >> 36
}

/// A SYSTIMER reading, the cycle count read with it, whether the counter has counted every cycle
/// since, and the last reading given.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CycleBase {
    micros: u64,
    cycles: u32,
    running: bool,
    last: u64,
}

impl CycleBase {
    /// The base before the first SYSTIMER reading: every reading reads SYSTIMER.
    pub(crate) const STALE: Self = Self {
        micros: 0,
        cycles: 0,
        running: false,
        last: 0,
    };

    /// The reading at cycle count `cycles`: the base's microseconds and the whole microseconds of
    /// the cycles counted since, at 160 a microsecond, when the counter has counted all of them and
    /// fewer than `rebase`, at most 2^28, have passed. `None` when SYSTIMER is read again.
    pub(crate) fn extend(self, cycles: u32, rebase: u32) -> Option<u64> {
        let counted = cycles.wrapping_sub(self.cycles);
        (self.running && counted < rebase).then(|| self.micros + whole_micros(counted))
    }

    /// The base after SYSTIMER read `micros` at cycle count `cycles`.
    pub(crate) fn rebased(self, micros: u64, cycles: u32) -> Self {
        Self {
            micros,
            cycles,
            running: true,
            last: self.last,
        }
    }

    /// The base after a wait that stopped the CPU: the next reading reads SYSTIMER.
    pub(crate) fn stopped(self) -> Self {
        Self {
            running: false,
            ..self
        }
    }

    /// `reading`, raised to the last reading given when it is less, and the base that keeps it as
    /// the last.
    pub(crate) fn give(self, reading: u64) -> (u64, Self) {
        let reading = reading.max(self.last);
        (
            reading,
            Self {
                last: reading,
                ..self
            },
        )
    }
}

/// A [`CycleBase`] in plain words, which one context reads and writes, so that a reading takes no
/// critical section (checklist P1-6). A context that read it while another wrote it could take the
/// words of two bases; on the C6 the main context is its one reader and writer, since no interrupt
/// handler reads the clock.
pub(crate) struct SharedBase {
    micros: [AtomicU32; 2],
    cycles: AtomicU32,
    running: AtomicBool,
    last: [AtomicU32; 2],
}

impl SharedBase {
    /// Words that hold `base`, such as [`CycleBase::STALE`] before the first SYSTIMER reading.
    pub(crate) const fn new(base: CycleBase) -> Self {
        Self {
            micros: words(base.micros),
            cycles: AtomicU32::new(base.cycles),
            running: AtomicBool::new(base.running),
            last: words(base.last),
        }
    }

    /// The base last stored.
    pub(crate) fn load(&self) -> CycleBase {
        CycleBase {
            micros: join(&self.micros),
            cycles: self.cycles.load(Ordering::Relaxed),
            running: self.running.load(Ordering::Relaxed),
            last: join(&self.last),
        }
    }

    /// Keeps `base` for the readings after it.
    pub(crate) fn store(&self, base: CycleBase) {
        split(&self.micros, base.micros);
        self.cycles.store(base.cycles, Ordering::Relaxed);
        self.running.store(base.running, Ordering::Relaxed);
        split(&self.last, base.last);
    }

    /// Marks a wait that stopped the CPU, as [`CycleBase::stopped`] does: the next reading reads
    /// SYSTIMER.
    pub(crate) fn stop(&self) {
        self.store(self.load().stopped());
    }
}

/// Two words that hold `value`, the low one first.
const fn words(value: u64) -> [AtomicU32; 2] {
    let [b0, b1, b2, b3, b4, b5, b6, b7] = value.to_le_bytes();
    [
        AtomicU32::new(u32::from_le_bytes([b0, b1, b2, b3])),
        AtomicU32::new(u32::from_le_bytes([b4, b5, b6, b7])),
    ]
}

/// The value of two words, the low one first.
fn join(words: &[AtomicU32; 2]) -> u64 {
    u64::from(words[0].load(Ordering::Relaxed))
        | (u64::from(words[1].load(Ordering::Relaxed)) << 32)
}

/// Stores `value` in two words, the low one first.
fn split(words: &[AtomicU32; 2], value: u64) {
    let [low, high] = [value & u64::from(u32::MAX), value >> 32]
        .map(|word| u32::try_from(word).unwrap_or(u32::MAX));
    words[0].store(low, Ordering::Relaxed);
    words[1].store(high, Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The C6's one-second rebase.
    const REBASE: u32 = 160_000_000;

    /// Worked by hand: after SYSTIMER read 1,000 µs at cycle 0, cycle 1,600 is 10 µs later and
    /// cycle 1,759 still 10 whole microseconds later; a count that wrapped past 2^32 counts on
    /// (0xFFFF_FF00 to 0x100 is 512 cycles, 3 µs).
    /// Catches: a reading that rounds a partial microsecond up (a later SYSTIMER reading could then
    /// be less), and a wrapped count read as a huge one.
    #[test]
    fn a_reading_adds_the_whole_microseconds_counted_since_systimer() {
        let base = CycleBase::STALE.rebased(1_000, 0);
        assert_eq!(base.extend(0, REBASE), Some(1_000));
        assert_eq!(base.extend(1_600, REBASE), Some(1_010));
        assert_eq!(base.extend(1_759, REBASE), Some(1_010));
        let wrapped = CycleBase::STALE.rebased(5_000, 0xFFFF_FF00);
        assert_eq!(wrapped.extend(0x100, REBASE), Some(5_003));
    }

    /// Before the first SYSTIMER reading, after a wait and a second after the last SYSTIMER
    /// reading, a reading reads SYSTIMER again.
    /// Catches: a reading after a wait, during which the counter may not count, that misses the
    /// wait's time, and a stretch long enough for the 32-bit count to wrap twice.
    #[test]
    fn systimer_is_read_again_before_the_first_reading_after_a_wait_and_after_a_second() {
        assert_eq!(CycleBase::STALE.extend(1_600, REBASE), None);
        let base = CycleBase::STALE.rebased(1_000, 0);
        assert_eq!(base.stopped().extend(1_600, REBASE), None);
        // 159,999,999 cycles are 999,999 whole microseconds.
        assert_eq!(base.extend(REBASE - 1, REBASE), Some(1_000_999));
        assert_eq!(base.extend(REBASE, REBASE), None);
    }

    /// The whole microseconds of a cycle count are its quotient by 160 for every count below 2^28,
    /// which holds the one-second rebase: checked at both ends, `160 q` and `160 q + 159`, of every
    /// microsecond that ends below 2^28 (q to 1,677,720; 2^28 is 268,435,456 cycles), and at
    /// 268,435,360 and 2^28 - 1, both in microsecond 1,677,721. The multiplication's result never
    /// falls as the count grows, so a microsecond right at both its ends is right throughout.
    /// Catches a multiplier or shift that rounds a count just below a microsecond up to it, or one
    /// just above down, which would move a deadline check by a microsecond.
    #[test]
    fn a_cycle_count_gives_its_whole_microseconds_below_two_to_the_28() {
        assert_eq!(whole_micros(0), 0);
        assert_eq!(whole_micros(159), 0);
        assert_eq!(whole_micros(160), 1);
        assert_eq!(whole_micros(REBASE - 1), 999_999);
        for micro in 0..1_677_721_u32 {
            assert_eq!(whole_micros(micro * 160), u64::from(micro));
            assert_eq!(whole_micros(micro * 160 + 159), u64::from(micro));
        }
        assert_eq!(whole_micros(268_435_360), 1_677_721);
        assert_eq!(whole_micros((1 << 28) - 1), 1_677_721);
    }

    /// A reading less than the last one given, as SYSTIMER read just after a cycle-counted reading
    /// that had already passed it can be, is raised to the last one.
    /// Catches: a monotonic clock that goes back across a rebase.
    #[test]
    fn a_reading_is_never_less_than_the_one_before() {
        let (first, base) = CycleBase::STALE.rebased(1_000, 0).give(1_010);
        assert_eq!(first, 1_010);
        let (second, base) = base.rebased(1_009, 2_000).give(1_009);
        assert_eq!(second, 1_010);
        let (third, _) = base.give(1_011);
        assert_eq!(third, 1_011);
    }

    /// A base kept in words is the base stored, its microseconds and last reading in two words each:
    /// SYSTIMER read 0x1_0000_0002 µs (about 4,295 s after boot) at cycle 7 and 0x1_0000_0005 µs
    /// was given; a wait then stops the counter, so the next reading reads SYSTIMER.
    /// Catches: words joined in the wrong order or a high word dropped (a reading 4,295 s off), and
    /// a stop that leaves the counter counting (a reading after a wait that misses the wait).
    #[test]
    fn a_base_kept_in_words_is_the_base_stored() {
        let shared = SharedBase::new(CycleBase::STALE);
        assert_eq!(shared.load(), CycleBase::STALE);
        let (_, base) = CycleBase::STALE
            .rebased(0x1_0000_0002, 7)
            .give(0x1_0000_0005);
        shared.store(base);
        assert_eq!(shared.load(), base);
        assert_eq!(shared.load().extend(7 + 160, REBASE), Some(0x1_0000_0003));
        shared.stop();
        assert_eq!(shared.load(), base.stopped());
        assert_eq!(shared.load().extend(7 + 160, REBASE), None);
    }
}
