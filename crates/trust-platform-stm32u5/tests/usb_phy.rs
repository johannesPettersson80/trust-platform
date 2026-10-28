//! The on-chip high-speed PHY's rules of the STM32U5A9 (design §2.2; specification 34, PORT-05). The
//! sources are ST's, read on 18 October 2026: `stm32u5xx_hal.h` (the CLKSEL codes), `stm32u5a9xx.h`
//! (the register bits) and `stm32u5xx_ll_usb.c` (`USB_DevInit` and `USB_SetDevSpeed`). The expected
//! values are written here as literals and worked out by hand from those sources.

use trust_platform_stm32u5::usb::{
    dcfg_full_speed_on_hs_phy, device_mode_gccfg, phy_reference_select,
};

/// The reference clock codes are ST's `SYSCFG_OTG_HS_PHY_CLK_SELECT_1` to `_6` (`stm32u5xx_hal.h`,
/// lines 214 to 219). Each is the sum of the CLKSEL bits it names: CLKSEL_0 is 1, CLKSEL_1 is 2,
/// CLKSEL_2 is 4 and CLKSEL_3 is 8. So 16 MHz is 1 + 2 = 3, 19.2 MHz is 8, 20 MHz is 1 + 8 = 9,
/// 24 MHz is 2 + 8 = 10, 26 MHz is 2 + 4 + 8 = 14, and 32 MHz is 1 + 2 + 8 = 11. A frequency with no
/// code, such as 25 MHz, has none. The wrong behaviour: a code taken from the wrong bits, or a clock
/// given a neighbour's code.
#[test]
fn the_phy_reference_codes_are_st_s() {
    assert_eq!(phy_reference_select(16_000_000), Some(3));
    assert_eq!(phy_reference_select(19_200_000), Some(8));
    assert_eq!(phy_reference_select(20_000_000), Some(9));
    assert_eq!(phy_reference_select(24_000_000), Some(10));
    assert_eq!(phy_reference_select(26_000_000), Some(14));
    assert_eq!(phy_reference_select(32_000_000), Some(11));
    assert_eq!(phy_reference_select(25_000_000), None);
}

/// Device mode with VBUS sensing off is GCCFG 0x0180_0000: VBVALEXTOEN (bit 24) and VBVALOVAL (bit
/// 23) are set, and VBDEN (bit 21) and PULLDOWNEN (bit 25) are clear, as ST's `USB_DevInit` leaves
/// them (`stm32u5xx_ll_usb.c`, lines 313 to 329). The wrong behaviour: VBUS sensing left on (VBDEN
/// set), the pull-downs left on (PULLDOWNEN set), or the session override missing (bit 23 or 24
/// clear).
#[test]
fn device_mode_gccfg_overrides_vbus_and_keeps_sensing_and_pulldowns_off() {
    assert_eq!(device_mode_gccfg(), 0x0180_0000);
    assert_eq!(
        device_mode_gccfg() & 0x0020_0000,
        0,
        "VBDEN, bit 21, is clear"
    );
    assert_eq!(
        device_mode_gccfg() & 0x0200_0000,
        0,
        "PULLDOWNEN, bit 25, is clear"
    );
}

/// DCFG's speed (bits 1:0) becomes 0b01, full speed on the on-chip PHY (`USB_OTG_SPEED_HIGH_IN_FULL`,
/// `stm32u5xx_ll_usb.c`, line 365), and the other fields are kept. Worked by hand: 0x3 has bits 1:0
/// set, and replacing them gives 0x1; 0x17F3 is 0x17F0 with bits 1:0 set to 0b11, and replacing them
/// gives 0x17F1. The wrong behaviour: the speed left at 0b11, another field cleared, or the speed OR-ed
/// in, which keeps 0b11 and gives 0x3 and 0x17F3.
#[test]
fn full_speed_on_the_hs_phy_sets_dspd_to_one_and_keeps_the_other_fields() {
    assert_eq!(dcfg_full_speed_on_hs_phy(0x0000_0003), 0x0000_0001);
    assert_eq!(dcfg_full_speed_on_hs_phy(0x0000_17F3), 0x0000_17F1);
}
