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

    /// The reading at cycle count `cycles`, `per_micro` cycles a microsecond: the base's
    /// microseconds and the whole microseconds of the cycles counted since, when the counter has
    /// counted all of them and fewer than `rebase` have passed. `None` when SYSTIMER is read again.
    pub(crate) fn extend(self, cycles: u32, per_micro: u32, rebase: u32) -> Option<u64> {
        let counted = cycles.wrapping_sub(self.cycles);
        (self.running && counted < rebase && per_micro != 0)
            .then(|| self.micros + u64::from(counted / per_micro))
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The C6's 160 cycles a microsecond and its one-second rebase.
    const PER_MICRO: u32 = 160;
    const REBASE: u32 = 160_000_000;

    /// Worked by hand: after SYSTIMER read 1,000 µs at cycle 0, cycle 1,600 is 10 µs later and
    /// cycle 1,759 still 10 whole microseconds later; a count that wrapped past 2^32 counts on
    /// (0xFFFF_FF00 to 0x100 is 512 cycles, 3 µs).
    /// Catches: a reading that rounds a partial microsecond up (a later SYSTIMER reading could then
    /// be less), and a wrapped count read as a huge one.
    #[test]
    fn a_reading_adds_the_whole_microseconds_counted_since_systimer() {
        let base = CycleBase::STALE.rebased(1_000, 0);
        assert_eq!(base.extend(0, PER_MICRO, REBASE), Some(1_000));
        assert_eq!(base.extend(1_600, PER_MICRO, REBASE), Some(1_010));
        assert_eq!(base.extend(1_759, PER_MICRO, REBASE), Some(1_010));
        let wrapped = CycleBase::STALE.rebased(5_000, 0xFFFF_FF00);
        assert_eq!(wrapped.extend(0x100, PER_MICRO, REBASE), Some(5_003));
    }

    /// Before the first SYSTIMER reading, after a wait and a second after the last SYSTIMER
    /// reading, a reading reads SYSTIMER again.
    /// Catches: a reading after a wait, during which the counter may not count, that misses the
    /// wait's time, and a stretch long enough for the 32-bit count to wrap twice.
    #[test]
    fn systimer_is_read_again_before_the_first_reading_after_a_wait_and_after_a_second() {
        assert_eq!(CycleBase::STALE.extend(1_600, PER_MICRO, REBASE), None);
        let base = CycleBase::STALE.rebased(1_000, 0);
        assert_eq!(base.stopped().extend(1_600, PER_MICRO, REBASE), None);
        // 159,999,999 cycles are 999,999 whole microseconds.
        assert_eq!(base.extend(REBASE - 1, PER_MICRO, REBASE), Some(1_000_999));
        assert_eq!(base.extend(REBASE, PER_MICRO, REBASE), None);
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
}
