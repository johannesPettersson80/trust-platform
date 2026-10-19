//! The STM32U5A9's reset flags (specification 34, D.4 and D.5; design §1.5): RCC_CSR's bits at the
//! positions of ST's `stm32u5a9xx.h`, which are not the F407's.

use trust_platform_stm32::ResetReason;
use trust_platform_stm32u5::reset::RESET_BITS;

#[test]
fn u5_reset_flags_decode_by_their_own_positions() {
    // Each row: RCC_CSR, then the flags read as watchdog, software, pin, power-on, brownout and
    // option-byte load, then whether the phase cookie may be trusted. The bits are ST's
    // `RCC_CSR_*_Pos`: IWDGRSTF 29, SFTRSTF 28, BORRSTF 27 (also the power-on flag), PINRSTF 26 and
    // OBLRSTF 25.
    let rows: [(u32, [bool; 6], bool); 4] = [
        // PIN and BOR: BORRSTF is a power-on, so the cookie of the previous boot is not trusted.
        (0x0C00_0000, [false, false, true, true, true, false], false),
        // IWDG and PIN: a watchdog reset, which trusts the cookie.
        (0x2400_0000, [true, false, true, false, false, false], true),
        // The option-byte load alone.
        (0x0200_0000, [false, false, false, false, false, true], true),
        // SFT and PIN: a software reset.
        (0x1400_0000, [false, true, true, false, false, false], true),
    ];
    for (raw, flags, trusted) in rows {
        let reason = ResetReason::new(raw, RESET_BITS);
        let decoded = [
            reason.independent_watchdog(),
            reason.software(),
            reason.pin(),
            reason.power_on(),
            reason.brownout(),
            reason.option_bytes(),
        ];
        assert_eq!(decoded, flags, "the flags of RCC_CSR {raw:#010x}");
        assert_eq!(
            reason.cookie_trusted(),
            trusted,
            "the cookie of RCC_CSR {raw:#010x}"
        );
    }
}
