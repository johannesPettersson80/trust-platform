//! The crystal frequency, measured against the chip's internal clock (specification 34, TARGET-02,
//! Appendix D.4). The firmware does not assume the crystal of a board. Right after reset the chip
//! runs from its 16 MHz internal oscillator (HSI); the firmware starts the crystal oscillator
//! (HSE) and has TIM11 count HSI cycles between the edges of the crystal clock divided down by
//! RCC_CFGR.RTCPRE (the TIM11 option register selects HSE_RTC for input 1, TIM_TIM11_HSE in ST's
//! HAL). The arithmetic is here so that the host can test it; the hardware part is
//! `hardware_f407::crystal_timer`.
//!
//! The HSI is an RC oscillator: ST factory-trims it to 1 % at 25 degrees (DS8626, HSI
//! characteristics, wider over temperature), so the measurement is exact to about that much. The
//! result is rounded to a whole MHz, which is how crystals are sold, and the PLL is told that
//! figure. A crystal above the figure would run the chip above its 168 MHz, so a measurement
//! more than [`ABOVE_LIMIT_PERMILLE`] above a whole MHz is not accepted; a crystal below the
//! figure only slows the chip, so the limit below is wider. A measurement that does not round
//! like this, or lies outside the oscillator's 4 to 26 MHz, or a crystal that does not start,
//! sends the system clock to the PLL on the HSI instead, and says so in the startup record. A
//! crystal that is not a whole number of MHz is named in `device.toml` (TARGET-02).

/// Nominal frequency of the internal oscillator, the time base of the measurement.
pub const HSI_HZ: u32 = 16_000_000;
/// RCC_CFGR.RTCPRE while measuring: the crystal clock reaching TIM11 is the crystal divided by
/// this. 31 is the largest divider; it keeps the capture rate slow.
pub const RTC_DIVIDER: u8 = 31;
/// Edges of the divided crystal clock between two captures (TIM11 input capture prescaler).
pub const EVENTS_PER_CAPTURE: u8 = 8;
/// Capture intervals added up in one measurement.
pub const INTERVALS: u32 = 64;
/// Smallest crystal the oscillator supports (DS8626: 4 to 26 MHz).
pub const MIN_CRYSTAL_HZ: u32 = 4_000_000;
/// Largest crystal the oscillator supports.
pub const MAX_CRYSTAL_HZ: u32 = 26_000_000;
/// A measurement above a whole MHz names that crystal only when it exceeds it by at most this
/// many parts per thousand. A crystal above the figure the PLL is told runs the chip above its
/// limit, so this side is tight: the HSI's 1 % trim at 25 degrees (DS8626) with a margin. For a
/// crystal that is not a whole MHz and an HSI within that trim, the chip then runs at most
/// about 0.5 % above 168 MHz.
pub const ABOVE_LIMIT_PERMILLE: u32 = 12;
/// A measurement below a whole MHz names that crystal when it falls short by at most this many
/// parts per thousand. A crystal below the figure only slows the chip, and a fast HSI makes any
/// crystal look slow, so this side is wide.
pub const BELOW_LIMIT_PERMILLE: u32 = 50;
/// Polls of RCC_CR.HSERDY before the crystal counts as not started: about a tenth of a second
/// at the 16 MHz the chip runs from after reset.
pub const HSE_READY_POLLS: u32 = 200_000;
/// Polls of TIM11 for one capture before the measurement is given up: about a tenth of a second.
pub const CAPTURE_POLLS: u32 = 100_000;

/// What TIM11 counted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Capture {
    /// Capture intervals added up.
    pub intervals: u32,
    /// HSI cycles counted over those intervals.
    pub timer_counts: u32,
}

/// HSI cycles between two captures of the 16-bit TIM11 counter, across one wrap.
pub const fn elapsed(previous: u16, now: u16) -> u32 {
    now.wrapping_sub(previous) as u32
}

/// The crystal frequency a capture implies, from the HSI as a time base: the number of crystal
/// cycles in the intervals divided by the time they took.
pub fn measured_hz(capture: Capture) -> Option<u32> {
    if capture.timer_counts == 0 {
        return None;
    }
    let crystal_cycles = u64::from(capture.intervals)
        .checked_mul(u64::from(RTC_DIVIDER))?
        .checked_mul(u64::from(EVENTS_PER_CAPTURE))?;
    let numerator = crystal_cycles.checked_mul(u64::from(HSI_HZ))?;
    let counts = u64::from(capture.timer_counts);
    let hz = numerator.checked_add(counts / 2)? / counts;
    u32::try_from(hz).ok()
}

/// Why a crystal could not be used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClockNote {
    /// A crystal was measured and is in use.
    Measured,
    /// The crystal oscillator did not start.
    HseNotReady,
    /// The crystal started but TIM11 did not capture its edges cleanly.
    NoCapture,
    /// The measurement is not between 4 and 26 MHz.
    OutOfRange,
    /// The measurement is not close enough to a whole MHz to name the crystal.
    Ambiguous,
}

impl ClockNote {
    /// Short code for the startup record.
    pub const fn code(self) -> &'static str {
        match self {
            Self::Measured => "measured",
            Self::HseNotReady => "hse-not-ready",
            Self::NoCapture => "no-capture",
            Self::OutOfRange => "out-of-range",
            Self::Ambiguous => "ambiguous",
        }
    }
}

/// The crystal frequency in Hz that a measurement names: the nearest whole MHz, when the
/// measurement is at most [`ABOVE_LIMIT_PERMILLE`] above it or [`BELOW_LIMIT_PERMILLE`] below
/// it, and the result is a crystal the oscillator supports.
pub fn crystal_hz(measured_hz: u32) -> Result<u32, ClockNote> {
    let nearest = (measured_hz.saturating_add(500_000) / 1_000_000).saturating_mul(1_000_000);
    let limit = |permille: u32| u64::from(nearest) * u64::from(permille) / 1000;
    let refused = if measured_hz >= nearest {
        u64::from(measured_hz - nearest) > limit(ABOVE_LIMIT_PERMILLE)
    } else {
        u64::from(nearest - measured_hz) > limit(BELOW_LIMIT_PERMILLE)
    };
    if refused {
        return Err(ClockNote::Ambiguous);
    }
    if !(MIN_CRYSTAL_HZ..=MAX_CRYSTAL_HZ).contains(&nearest) {
        return Err(ClockNote::OutOfRange);
    }
    Ok(nearest)
}

/// What the hardware probe found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Probe {
    /// The crystal oscillator did not report ready.
    HseNotReady,
    /// The oscillator started but no usable capture came.
    NoCapture,
    /// TIM11 counted this.
    Captured(Capture),
}

/// Where the system clock comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClockSource {
    /// The PLL runs from the crystal.
    Hse,
    /// The PLL runs from the internal oscillator.
    Hsi,
}

impl ClockSource {
    /// Short code for the startup record.
    pub const fn code(self) -> &'static str {
        match self {
            Self::Hse => "HSE",
            Self::Hsi => "HSI",
        }
    }
}

/// The clock source chosen from a probe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Decision {
    /// The PLL input.
    pub source: ClockSource,
    /// Why this source.
    pub note: ClockNote,
    /// The crystal frequency as measured, 0 when nothing was measured.
    pub measured_hz: u32,
    /// The crystal frequency the PLL is configured for, 0 when the PLL runs from the HSI.
    pub crystal_hz: u32,
}

fn fallback(note: ClockNote, measured_hz: u32) -> Decision {
    Decision {
        source: ClockSource::Hsi,
        note,
        measured_hz,
        crystal_hz: 0,
    }
}

/// Chooses the PLL input: the measured crystal, or the HSI when there is none.
pub fn decide(probe: Probe) -> Decision {
    match probe {
        Probe::HseNotReady => fallback(ClockNote::HseNotReady, 0),
        Probe::NoCapture => fallback(ClockNote::NoCapture, 0),
        Probe::Captured(capture) => match measured_hz(capture) {
            None => fallback(ClockNote::NoCapture, 0),
            Some(measured) => match crystal_hz(measured) {
                Ok(crystal) => Decision {
                    source: ClockSource::Hse,
                    note: ClockNote::Measured,
                    measured_hz: measured,
                    crystal_hz: crystal,
                },
                Err(note) => fallback(note, measured),
            },
        },
    }
}

/// The clock the board started with, for the startup record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClockReport {
    /// The PLL input.
    pub source: ClockSource,
    /// Why this source.
    pub note: ClockNote,
    /// The crystal frequency as measured against the HSI, 0 when nothing was measured.
    pub measured_hz: u32,
    /// The crystal frequency the PLL is configured for, 0 when the PLL runs from the HSI.
    pub crystal_hz: u32,
    /// The system clock the clock tree reports.
    pub sysclk_hz: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The capture a crystal of `crystal_hz` gives when the HSI runs at `hsi_hz`.
    fn capture_of(crystal_hz: u32, hsi_hz: u32) -> Capture {
        let cycles = u64::from(INTERVALS) * u64::from(RTC_DIVIDER) * u64::from(EVENTS_PER_CAPTURE);
        let counts =
            (cycles * u64::from(hsi_hz) + u64::from(crystal_hz) / 2) / u64::from(crystal_hz);
        Capture {
            intervals: INTERVALS,
            timer_counts: u32::try_from(counts).unwrap(),
        }
    }

    const CRYSTALS: [u32; 11] = [
        4_000_000, 5_000_000, 6_000_000, 8_000_000, 10_000_000, 12_000_000, 16_000_000, 20_000_000,
        24_000_000, 25_000_000, 26_000_000,
    ];

    /// HSI frequencies within the 1 % factory trim at 25 degrees (DS8626), and the nominal one.
    const HSI_TRIM: [u32; 5] = [15_840_000, 15_920_000, HSI_HZ, 16_080_000, 16_160_000];

    #[test]
    fn the_capture_of_a_crystal_gives_its_frequency_back() {
        for crystal in CRYSTALS {
            let measured = measured_hz(capture_of(crystal, HSI_HZ)).unwrap();
            // The counts are whole numbers, so the result is exact to well under 0.1 %.
            assert!(
                measured.abs_diff(crystal) < crystal / 1000,
                "{crystal} measured as {measured}"
            );
            assert_eq!(crystal_hz(measured), Ok(crystal));
        }
        // The DISC1's 8 MHz crystal: 64 intervals of 31 * 8 cycles take 31744 HSI cycles.
        assert_eq!(capture_of(8_000_000, HSI_HZ).timer_counts, 31_744);
        assert_eq!(measured_hz(capture_of(8_000_000, HSI_HZ)), Some(8_000_000));
    }

    #[test]
    fn a_trimmed_hsi_still_names_the_crystal() {
        // DS8626: the factory-trimmed HSI is within 1 % at 25 degrees. A fast HSI counts more
        // cycles and makes the crystal look slow, a slow one the reverse.
        for crystal in CRYSTALS {
            for hsi in HSI_TRIM {
                let measured = measured_hz(capture_of(crystal, hsi)).unwrap();
                assert_eq!(crystal_hz(measured), Ok(crystal), "{crystal} at HSI {hsi}");
            }
        }
    }

    #[test]
    fn the_measurement_is_reported_as_measured_even_when_it_rounds() {
        let decision = decide(Probe::Captured(capture_of(8_000_000, 16_100_000)));
        assert_eq!(decision.source, ClockSource::Hse);
        assert_eq!(decision.note, ClockNote::Measured);
        assert_eq!(decision.crystal_hz, 8_000_000);
        // 8 MHz seen through a 0.6 % fast HSI.
        assert!(decision.measured_hz.abs_diff(7_950_000) < 10_000);
        assert_ne!(decision.measured_hz, decision.crystal_hz);
    }

    #[test]
    fn a_measurement_names_a_crystal_only_close_to_a_whole_mhz() {
        assert_eq!(crystal_hz(8_000_000), Ok(8_000_000));
        // Above a whole MHz the limit is tight, 1.2 % (96 kHz at 8 MHz): a crystal above the
        // figure the PLL is told would run the chip faster than 168 MHz.
        assert_eq!(crystal_hz(8_096_000), Ok(8_000_000));
        assert_eq!(crystal_hz(8_096_001), Err(ClockNote::Ambiguous));
        // Below it the limit is wide, 5 % (400 kHz at 8 MHz): a slow crystal only slows the
        // chip, and a fast HSI makes any crystal look slow.
        assert_eq!(crystal_hz(7_600_000), Ok(8_000_000));
        assert_eq!(crystal_hz(7_599_999), Err(ClockNote::Ambiguous));
        // Half way between two whole MHz nothing is named, apart from the safe side of 9 MHz.
        assert_eq!(crystal_hz(8_499_999), Err(ClockNote::Ambiguous));
        assert_eq!(crystal_hz(8_500_000), Err(ClockNote::Ambiguous));
        assert_eq!(crystal_hz(8_549_999), Err(ClockNote::Ambiguous));
        assert_eq!(crystal_hz(8_550_000), Ok(9_000_000));
        // Crystals that are not whole MHz. A little below a whole number they round to it and
        // run the chip slower than 168 MHz; a little above they are refused, and the startup
        // record shows the HSI. Both are named in device.toml (TARGET-02).
        assert_eq!(crystal_hz(14_745_600), Ok(15_000_000));
        assert_eq!(crystal_hz(22_579_200), Ok(23_000_000));
        assert_eq!(crystal_hz(8_192_000), Err(ClockNote::Ambiguous));
        assert_eq!(crystal_hz(12_288_000), Err(ClockNote::Ambiguous));
        assert_eq!(crystal_hz(4_194_304), Err(ClockNote::Ambiguous));
        // 11.0592 MHz is 0.5 % above 11 MHz, inside the limit: 168.8 MHz.
        assert_eq!(crystal_hz(11_059_200), Ok(11_000_000));
    }

    #[test]
    fn a_crystal_that_is_not_a_whole_mhz_never_pushes_the_chip_far_above_168_mhz() {
        // The PLL is told the whole MHz, so the chip runs at 168 MHz times the true crystal over
        // that figure. With the HSI inside its 1 % trim, such a crystal is either refused or at
        // most 1 % above its figure.
        let odd = [
            4_194_304, 4_433_619, 6_144_000, 7_372_800, 8_192_000, 11_059_200, 12_288_000,
            14_318_180, 14_745_600, 16_384_000, 18_432_000, 22_118_400, 24_576_000,
        ];
        let mut accepted = 0;
        for crystal in odd {
            for hsi in HSI_TRIM {
                let decision = decide(Probe::Captured(capture_of(crystal, hsi)));
                if decision.source == ClockSource::Hse {
                    accepted += 1;
                    assert!(
                        u64::from(crystal) * 100 <= u64::from(decision.crystal_hz) * 101,
                        "{crystal} at HSI {hsi} configured as {}",
                        decision.crystal_hz
                    );
                }
            }
        }
        // The check is not empty: some of these crystals do round (11.0592 to 11, 14.7456 to 15).
        assert!(accepted > 0);
    }

    #[test]
    fn crystals_outside_the_oscillator_range_are_refused() {
        assert_eq!(crystal_hz(4_000_000), Ok(4_000_000));
        assert_eq!(crystal_hz(3_900_000), Ok(4_000_000));
        assert_eq!(crystal_hz(3_000_000), Err(ClockNote::OutOfRange));
        assert_eq!(crystal_hz(1_000_000), Err(ClockNote::OutOfRange));
        assert_eq!(crystal_hz(0), Err(ClockNote::OutOfRange));
        assert_eq!(crystal_hz(26_000_000), Ok(26_000_000));
        assert_eq!(crystal_hz(26_200_000), Ok(26_000_000));
        assert_eq!(crystal_hz(27_000_000), Err(ClockNote::OutOfRange));
        assert_eq!(crystal_hz(50_000_000), Err(ClockNote::OutOfRange));
        assert_eq!(crystal_hz(4_000_000_000), Err(ClockNote::OutOfRange));
        // The edge of the number range gives an error, never a wrong crystal or an overflow.
        assert!(crystal_hz(u32::MAX).is_err());
    }

    #[test]
    fn a_probe_without_a_crystal_falls_back_to_the_hsi() {
        let none = Decision {
            source: ClockSource::Hsi,
            note: ClockNote::HseNotReady,
            measured_hz: 0,
            crystal_hz: 0,
        };
        assert_eq!(decide(Probe::HseNotReady), none);
        assert_eq!(
            decide(Probe::NoCapture),
            Decision {
                note: ClockNote::NoCapture,
                ..none
            }
        );
        // No counts, or counts the arithmetic cannot hold, are not a measurement.
        for counts in [0, 1] {
            let empty = Capture {
                intervals: INTERVALS,
                timer_counts: counts,
            };
            assert_eq!(
                decide(Probe::Captured(empty)),
                Decision {
                    note: ClockNote::NoCapture,
                    ..none
                }
            );
        }
        assert_eq!(
            decide(Probe::Captured(Capture {
                intervals: u32::MAX,
                timer_counts: 1
            }))
            .note,
            ClockNote::NoCapture
        );
    }

    #[test]
    fn a_measurement_that_names_no_crystal_keeps_the_figure_and_falls_back() {
        // 8.5 MHz sits between two crystals.
        let ambiguous = decide(Probe::Captured(capture_of(8_500_000, HSI_HZ)));
        assert_eq!(ambiguous.source, ClockSource::Hsi);
        assert_eq!(ambiguous.note, ClockNote::Ambiguous);
        assert_eq!(ambiguous.crystal_hz, 0);
        assert!(ambiguous.measured_hz.abs_diff(8_500_000) < 10_000);
        // 2 MHz is below the oscillator's range.
        let slow = decide(Probe::Captured(capture_of(2_000_000, HSI_HZ)));
        assert_eq!(
            (slow.source, slow.note),
            (ClockSource::Hsi, ClockNote::OutOfRange)
        );
        assert!(slow.measured_hz.abs_diff(2_000_000) < 10_000);
        // 40 MHz is above it.
        let fast = decide(Probe::Captured(capture_of(40_000_000, HSI_HZ)));
        assert_eq!(
            (fast.source, fast.note),
            (ClockSource::Hsi, ClockNote::OutOfRange)
        );
    }

    #[test]
    fn the_timer_counter_wraps_without_losing_an_interval() {
        assert_eq!(elapsed(100, 596), 496);
        assert_eq!(elapsed(0, 0), 0);
        assert_eq!(elapsed(65_500, 100), 136);
        assert_eq!(elapsed(65_535, 0), 1);
        assert_eq!(elapsed(1, 0), 65_535);
        // 64 intervals of an 8 MHz crystal add up across any number of wraps.
        let interval = capture_of(8_000_000, HSI_HZ).timer_counts / INTERVALS;
        assert_eq!(interval, 496);
        let mut previous = 60_000_u16;
        let mut total = 0_u32;
        for _ in 0..INTERVALS {
            let now = previous.wrapping_add(interval as u16);
            total += elapsed(previous, now);
            previous = now;
        }
        assert_eq!(total, capture_of(8_000_000, HSI_HZ).timer_counts);
    }

    #[test]
    fn every_supported_crystal_fits_the_16_bit_timer_and_the_polling_rate() {
        let events = u64::from(RTC_DIVIDER) * u64::from(EVENTS_PER_CAPTURE);
        let mut crystal = MIN_CRYSTAL_HZ;
        while crystal <= MAX_CRYSTAL_HZ {
            // HSI cycles between two captures: below the 16-bit counter's wrap, and at least 100,
            // which is 6 microseconds for the polling loop to read one capture and enough for
            // the interval to be measured to 1 %.
            let counts = u64::from(HSI_HZ) * events / u64::from(crystal);
            assert!(counts < 65_536, "{crystal}: {counts}");
            assert!(counts >= 100, "{crystal}: {counts}");
            crystal += 1_000_000;
        }
        // The sum of all intervals fits a u32 at the slowest crystal.
        let slowest = u64::from(HSI_HZ) * events * u64::from(INTERVALS) / u64::from(MIN_CRYSTAL_HZ);
        assert!(slowest < u64::from(u32::MAX));
    }

    #[test]
    fn codes_for_the_startup_record_are_stable() {
        assert_eq!(ClockSource::Hse.code(), "HSE");
        assert_eq!(ClockSource::Hsi.code(), "HSI");
        assert_eq!(ClockNote::Measured.code(), "measured");
        assert_eq!(ClockNote::HseNotReady.code(), "hse-not-ready");
        assert_eq!(ClockNote::NoCapture.code(), "no-capture");
        assert_eq!(ClockNote::OutOfRange.code(), "out-of-range");
        assert_eq!(ClockNote::Ambiguous.code(), "ambiguous");
    }
}
