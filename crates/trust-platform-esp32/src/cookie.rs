//! Two-word reset cookie codec, identical to the F401 profile so both boards store one format.

/// Phase written before the deliberately withheld MWDT0 feed (the F401 value).
pub const WATCHDOG_PHASE: u16 = 0x5744;
/// Phase written when a run starts (the F401 value).
pub const RUNNING_PHASE: u16 = 0x5255;

/// Two-word cookie for the intentional reset phase only, kept in RTC fast memory.
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
        for phase in [1, 2, WATCHDOG_PHASE, RUNNING_PHASE, u16::MAX] {
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
    }

    #[test]
    fn f401_phase_values_encode_with_the_shared_magic() {
        assert_eq!(
            ResetCookie {
                phase: WATCHDOG_PHASE
            }
            .words(),
            [0x5452_5744, !0x5452_5744]
        );
        assert_eq!(
            ResetCookie {
                phase: RUNNING_PHASE
            }
            .words(),
            [0x5452_5255, !0x5452_5255]
        );
    }
}
