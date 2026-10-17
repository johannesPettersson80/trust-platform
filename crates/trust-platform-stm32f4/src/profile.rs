//! Chip clock profile and persistent reset-test marker encoding.

/// System frequency of the chip ([`crate::chip::CHIP`]): 168 MHz on the F407, from the crystal,
/// or from the HSI when there is none. The oscillator's tolerance remains the physical accuracy
/// of the clock.
pub const SYSCLK_HZ: u32 = crate::chip::CHIP.sysclk_hz;
/// Fixture sampling period; shared runtime scheduling remains engine-owned.
pub const SCAN_PERIOD_US: u64 = 10_000;

/// RCC_CSR reset flags captured before any clearing during board initialization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResetReason {
    /// Original register bits, including combinations of reset flags.
    pub raw: u32,
}
impl ResetReason {
    /// Independent-watchdog reset flag (RM0090 RCC_CSR IWDGRSTF).
    pub const fn independent_watchdog(self) -> bool {
        self.raw & (1 << 29) != 0
    }
    /// Power-on/power-down reset flag; a phase cookie is not trusted on this boot.
    pub const fn power_on(self) -> bool {
        self.raw & (1 << 27) != 0
    }
    /// Brownout reset flag; a phase cookie is not trusted on this boot.
    pub const fn brownout(self) -> bool {
        self.raw & (1 << 25) != 0
    }
}

/// Two-word backup-register cookie for the intentional reset phase only.
/// It is not a retain checkpoint or a promise of power-loss persistence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResetCookie {
    /// Application-owned nonzero phase identifier.
    pub phase: u16,
}
impl ResetCookie {
    /// Encode a phase with a magic prefix and complementary integrity word.
    pub const fn words(self) -> [u32; 2] {
        let word = 0x5452_0000 | self.phase as u32;
        [word, !word]
    }
    /// Reject erased, torn, zero-phase or unrecognized markers.
    pub const fn decode(words: [u32; 2]) -> Option<Self> {
        if words[0] ^ words[1] != u32::MAX
            || words[0] & 0xffff_0000 != 0x5452_0000
            || words[0] & 0xffff == 0
        {
            return None;
        }
        Some(Self {
            phase: words[0] as u16,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reset_marker_rejects_torn_and_uninitialized_pairs() {
        for phase in [1, 2, u16::MAX] {
            let cookie = ResetCookie { phase };
            assert_eq!(ResetCookie::decode(cookie.words()), Some(cookie));
            let mut torn = cookie.words();
            torn[1] ^= 1;
            assert_eq!(ResetCookie::decode(torn), None);
        }
        for words in [
            [0, 0],
            [u32::MAX, u32::MAX],
            [0, u32::MAX],
            ResetCookie { phase: 0 }.words(),
        ] {
            assert_eq!(ResetCookie::decode(words), None);
        }
        assert!(ResetReason { raw: 1 << 29 }.independent_watchdog());
        assert!(!ResetReason { raw: 1 << 27 }.independent_watchdog());
    }
}
