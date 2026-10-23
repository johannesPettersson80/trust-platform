//! Fixed board profile and persistent reset-test marker encoding.

/// F401 HSI/PLL system frequency; HSI accuracy remains an oscillator tolerance.
pub const SYSCLK_HZ: u32 = 84_000_000;
/// Fixture sampling period; shared runtime scheduling remains engine-owned.
pub const SCAN_PERIOD_US: u64 = 10_000;

/// One physical PC13 observation, independent of any injected fixture input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InputSnapshot {
    /// Actual GPIO IDR level.
    pub pc13_high: bool,
    /// NUCLEO-F401RE B1 is active low: pressing connects PC13 to ground.
    pub button_pressed: bool,
}
impl InputSnapshot {
    /// Normalize the physical level; this does not debounce or invent a press.
    pub const fn from_pc13(high: bool) -> Self {
        Self {
            pc13_high: high,
            button_pressed: !high,
        }
    }
}

/// RCC_CSR reset flags captured before any clearing during board initialization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResetReason {
    /// Original register bits, including combinations of reset flags.
    pub raw: u32,
}
impl ResetReason {
    /// Independent-watchdog reset flag (RM0368 RCC_CSR IWDGRSTF).
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
    fn physical_button_polarity_is_not_a_synthetic_input() {
        assert_eq!(
            InputSnapshot::from_pc13(true),
            InputSnapshot {
                pc13_high: true,
                button_pressed: false
            }
        );
        assert_eq!(
            InputSnapshot::from_pc13(false),
            InputSnapshot {
                pc13_high: false,
                button_pressed: true
            }
        );
    }
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
