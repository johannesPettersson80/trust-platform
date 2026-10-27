//! The reset record's encoding (specification 34, D.4 and D.5): the reset flags of RCC_CSR and the
//! two-word cookie that the firmware keeps in the backup registers for the intentional reset phase.
//! Shared by the STM32 adapters, so both chips store and read one record. The flags sit at different
//! bits on each chip, so a [`ResetReason`] carries the [`ResetBits`] of the chip that wrote it.

/// The bit positions of one chip's reset flags in RCC_CSR. The STM32F4 and the STM32U5 put the same
/// causes at different bits, so each adapter passes its own table to [`ResetReason::new`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResetBits {
    /// Independent-watchdog reset flag.
    pub watchdog: u8,
    /// Software reset flag.
    pub software: u8,
    /// Pin reset flag.
    pub pin: u8,
    /// Power-on reset flag. A chip with no power-on flag of its own sets its brownout flag here.
    pub power_on: u8,
    /// Brownout reset flag.
    pub brownout: u8,
    /// Option-byte load reset flag, when the chip has one.
    pub option_bytes: Option<u8>,
}

/// The STM32F4 family's RCC_CSR positions (RM0090), as the F407 adapter reads them. The F4 has no
/// option-byte load flag.
pub const F4_RESET_BITS: ResetBits = ResetBits {
    watchdog: 29,
    software: 28,
    pin: 26,
    power_on: 27,
    brownout: 25,
    option_bytes: None,
};

/// RCC_CSR reset flags captured before any clearing during board initialization, read at the
/// positions of the chip that wrote them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResetReason {
    /// Original register bits, including combinations of reset flags.
    pub raw: u32,
    /// The bit positions of the chip that wrote `raw`.
    pub bits: ResetBits,
}
impl ResetReason {
    /// The flags of `raw`, read at the positions in `bits`.
    pub const fn new(raw: u32, bits: ResetBits) -> Self {
        Self { raw, bits }
    }
    /// Whether the flag at bit `position` is set.
    const fn set(self, position: u8) -> bool {
        self.raw & (1 << position) != 0
    }
    /// Independent-watchdog reset.
    pub const fn independent_watchdog(self) -> bool {
        self.set(self.bits.watchdog)
    }
    /// Software reset.
    pub const fn software(self) -> bool {
        self.set(self.bits.software)
    }
    /// Pin reset.
    pub const fn pin(self) -> bool {
        self.set(self.bits.pin)
    }
    /// Power-on reset; a phase cookie is not trusted on this boot.
    pub const fn power_on(self) -> bool {
        self.set(self.bits.power_on)
    }
    /// Brownout reset; a phase cookie is not trusted on this boot.
    pub const fn brownout(self) -> bool {
        self.set(self.bits.brownout)
    }
    /// Option-byte load reset; false on a chip with no such flag.
    pub const fn option_bytes(self) -> bool {
        match self.bits.option_bytes {
            Some(position) => self.set(position),
            None => false,
        }
    }
    /// Whether the phase cookie of the previous boot may be trusted: not after a power-on or a
    /// brownout.
    pub const fn cookie_trusted(self) -> bool {
        !(self.power_on() || self.brownout())
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
    fn the_cookie_round_trips_and_refuses_torn_erased_and_zero_phase() {
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
    }

    /// The F407's present answers (RM0090 RCC_CSR): bit 29 is the independent watchdog, bit 27 the
    /// power-on reset and bit 25 the brownout. A power-on or a brownout does not trust the cookie.
    #[test]
    fn f407_reset_flags_still_decode_as_before() {
        // Each row: RCC_CSR, then the flags read as watchdog, power-on and brownout, then whether the
        // phase cookie may be trusted.
        let rows: [(u32, [bool; 3], bool); 4] = [
            (1 << 29, [true, false, false], true),
            (1 << 27, [false, true, false], false),
            (1 << 25, [false, false, true], false),
            (0, [false, false, false], true),
        ];
        for (raw, flags, trusted) in rows {
            let reason = ResetReason::new(raw, F4_RESET_BITS);
            let decoded = [
                reason.independent_watchdog(),
                reason.power_on(),
                reason.brownout(),
            ];
            assert_eq!(decoded, flags, "the flags of RCC_CSR {raw:#010x}");
            assert_eq!(
                reason.cookie_trusted(),
                trusted,
                "the cookie of RCC_CSR {raw:#010x}"
            );
        }

        // The F4 has no option-byte load flag, so no RCC_CSR value sets it.
        assert!(!ResetReason::new(u32::MAX, F4_RESET_BITS).option_bytes());
    }
}
