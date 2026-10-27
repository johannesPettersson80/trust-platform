//! The time the C6 carries over a reset (specification 34 TARGET-06, Appendix D.2): the Unix time
//! at the loop's last copy and the LP timer's ticks then, in RTC fast memory, which every reset but
//! a loss of power keeps. A boot after a reset adds the ticks that the LP timer counted since that
//! copy, converted with its own calibration of the RC oscillator that drives the timer, so the
//! oscillator's error counts over those seconds only, never over the time since power-up.

/// The fraction bits of the RC oscillator's calibrated period, as esp-hal and ESP-IDF keep it in
/// `LP_AON_STORE1`: the microseconds of one tick times 2^19.
pub const PERIOD_FRACTION_BITS: u32 = 19;

/// The words of a carried time: a mark, the Unix time in microseconds and the ticks, each low word
/// first, and a check word.
pub const CARRY_WORDS: usize = 6;

/// The first word of a carried time: `CLK1`.
const MARK: u32 = 0x434C_4B31;

/// The Unix time at a copy and the LP timer's ticks then.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Carry {
    /// The Unix time in microseconds.
    pub unix_us: u64,
    /// The LP timer's ticks since power-up.
    pub ticks: u64,
}

impl Carry {
    /// The words that RTC fast memory keeps.
    #[must_use]
    pub const fn words(self) -> [u32; CARRY_WORDS] {
        let words = [
            MARK,
            self.unix_us as u32,
            (self.unix_us >> 32) as u32,
            self.ticks as u32,
            (self.ticks >> 32) as u32,
        ];
        [
            words[0],
            words[1],
            words[2],
            words[3],
            words[4],
            check(words),
        ]
    }

    /// The carry that `words` hold, or `None` for words that no copy wrote: the zeros after a
    /// power-on, a copy that a reset cut, or another layout.
    #[must_use]
    pub fn decode(words: [u32; CARRY_WORDS]) -> Option<Self> {
        let [mark, unix_low, unix_high, ticks_low, ticks_high, checked] = words;
        if mark != MARK || checked != check([mark, unix_low, unix_high, ticks_low, ticks_high]) {
            return None;
        }
        Some(Self {
            unix_us: u64::from(unix_low) | (u64::from(unix_high) << 32),
            ticks: u64::from(ticks_low) | (u64::from(ticks_high) << 32),
        })
    }

    /// The Unix time in microseconds when the same timer reads `ticks`: the ticks since the copy
    /// converted with `period`, the calibration of the boot that reads them. `None` when the timer
    /// reads fewer ticks than at the copy, which only a loss of power gives.
    #[must_use]
    pub fn unix_at(self, ticks: u64, period: u32) -> Option<u64> {
        let elapsed = ticks.checked_sub(self.ticks)?;
        Some(self.unix_us.saturating_add(micros_of(elapsed, period)))
    }
}

/// The check word of the other five: their exclusive or, inverted, so that zeros never check.
const fn check(words: [u32; 5]) -> u32 {
    !(words[0] ^ words[1] ^ words[2] ^ words[3] ^ words[4])
}

/// The ticks of `micros` microseconds with the calibrated period `period`; 0 without a period.
#[must_use]
pub fn ticks_of(micros: u64, period: u32) -> u64 {
    if period == 0 {
        return 0;
    }
    u64::try_from((u128::from(micros) << PERIOD_FRACTION_BITS) / u128::from(period))
        .unwrap_or(u64::MAX)
}

/// The microseconds of `ticks` ticks with the calibrated period `period`.
#[must_use]
pub fn micros_of(ticks: u64, period: u32) -> u64 {
    u64::try_from((u128::from(ticks) * u128::from(period)) >> PERIOD_FRACTION_BITS)
        .unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The C6's RC oscillator, about 136 kHz: 7.343 µs a tick, times 2^19.
    const PERIOD: u32 = 3_850_000;

    /// A carry reads back as it was written, and words that no copy wrote do not read: the zeros
    /// after a power-on, a word a reset cut, another mark. Catches a power-on read as a set time.
    #[test]
    fn a_carry_reads_back_and_other_words_do_not() {
        let carry = Carry {
            unix_us: 1_792_913_427_443_000,
            ticks: 35_272_000_000,
        };
        assert_eq!(Carry::decode(carry.words()), Some(carry));
        assert_eq!(Carry::decode([0; CARRY_WORDS]), None);
        for index in 0..CARRY_WORDS {
            let mut cut = carry.words();
            cut[index] ^= 1 << 7;
            assert_eq!(Carry::decode(cut), None, "word {index}");
        }
    }

    /// The bench (25 October 2026) found the time carried over a reset 623 s behind: esp-hal's
    /// boot time adds the timer's whole time since power-up, so a calibration 0.24 % apart after
    /// three days puts it about 622 s off. The carry adds only the ticks since the copy, with the
    /// new calibration: 4 s after the copy reads 4 s later, within one tick (7.4 µs). Catches the
    /// carry converting the ticks since power-up again.
    #[test]
    fn a_reset_after_days_carries_the_seconds_since_the_copy() {
        let unix_us = 1_792_913_427_443_000;
        let ticks = ticks_of(259_200_000_000, PERIOD);
        let carry = Carry { unix_us, ticks };
        let recalibrated = PERIOD + PERIOD / 1_000 * 24 / 10;
        let later = ticks + ticks_of(4_000_000, recalibrated);
        let carried = carry
            .unix_at(later, recalibrated)
            .expect("ticks after the copy");
        assert!(
            carried.abs_diff(unix_us + 4_000_000) <= 8,
            "carried {carried}, expected {}",
            unix_us + 4_000_000
        );
        // The time since power-up converted with each calibration, as the boot time does.
        let whole = micros_of(later, recalibrated) - micros_of(ticks, PERIOD);
        assert!(whole > 620_000_000, "the old way is {whole} µs");
    }

    /// A timer that reads fewer ticks than at the copy started again at a power-on: no time is
    /// carried. Catches a wrapped subtraction giving a time centuries away.
    #[test]
    fn fewer_ticks_than_at_the_copy_carry_no_time() {
        let carry = Carry {
            unix_us: 1_792_913_427_443_000,
            ticks: 1_000_000,
        };
        assert_eq!(carry.unix_at(999_999, PERIOD), None);
        assert_eq!(carry.unix_at(1_000_000, PERIOD), Some(carry.unix_us));
    }

    /// Ticks and microseconds convert with the 19 fraction bits of the period: 2^19 ticks of a
    /// period of 7 µs times 2^19 are 7 × 2^19 µs. Catches a fraction width other than esp-hal's.
    #[test]
    fn the_period_has_nineteen_fraction_bits() {
        let period = 7 << PERIOD_FRACTION_BITS;
        assert_eq!(micros_of(1 << 19, period), 7 << 19);
        assert_eq!(ticks_of(7 << 19, period), 1 << 19);
        assert_eq!(ticks_of(5, 0), 0);
    }
}
