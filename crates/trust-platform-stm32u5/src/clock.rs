//! The STM32U5A9 clock tree (specification 34, Appendix D.5, Clock). The arithmetic of PLL1 and the
//! clock report are pure and tested on the host. The register setup reaches 160 MHz from the board's
//! 16 MHz HSE in bypass mode; it is compiled for the chip only and is proved on the bench (S1b-9u).
//! Its order follows the board's project and OpenOCD's `stm32u5x.cfg`, which reach the same 160 MHz.

#[cfg(all(target_arch = "arm", target_os = "none"))]
use stm32u5::stm32u5a9 as pac;

/// The board's high-speed external clock, 16 MHz, from its CubeMX project (`RCC.HSE_VALUE`, D.5).
/// The dividers below are for this input.
pub const HSE_HZ: u32 = 16_000_000;
/// The internal 16 MHz oscillator, the PLL input when HSE is not ready (`RCC.HSI_VALUE`, D.5).
pub const HSI16_HZ: u32 = 16_000_000;
/// PLL1's divider M, 4: the 16 MHz input becomes 4 MHz, inside PLL1's 4 to 8 MHz input range.
pub const PLL1_M: u32 = 4;
/// PLL1's multiplier N, 80: the VCO runs at 320 MHz.
pub const PLL1_N: u32 = 80;
/// PLL1's divider R, 2: the R output is SYSCLK, 160 MHz.
pub const PLL1_R: u32 = 2;
/// PLL1's divider Q, 2: 160 MHz, which the USB clock does not take (see [`USB_KERNEL_HZ`]).
pub const PLL1_Q: u32 = 2;
/// The value of the PLL1DIVR.PLL1N field for N = 80: the field holds N - 1, as `stm32u5x.cfg`
/// writes 0x4F.
pub const PLL1N_FIELD: u16 = 0x4F;
/// SYSCLK and HCLK, 160 MHz, the top of voltage range 1 (D.5, Clock).
pub const SYSCLK_HZ: u32 = 160_000_000;
/// Flash wait states at 160 MHz in voltage range 1 (D.5, Clock; FLASH_ACR.LATENCY).
pub const FLASH_WAIT_STATES: u8 = 4;
/// The 48 MHz kernel clock of the USB device, from HSI48 (D.5, Clock).
pub const USB_KERNEL_HZ: u32 = 48_000_000;
/// Polls of a ready flag before its wait is given up. The count bounds a hang; it is not a timing
/// claim.
pub const READY_POLLS: u32 = 200_000;

/// The VCO frequency of PLL1 for an input clock: the input over M, times N.
#[must_use]
pub const fn pll1_vco_hz(input_hz: u32, m: u32, n: u32) -> u32 {
    input_hz / m * n
}

/// A PLL1 output: the VCO over one of its dividers.
#[must_use]
pub const fn pll1_output_hz(vco_hz: u32, divider: u32) -> u32 {
    vco_hz / divider
}

/// SYSCLK for an input clock through PLL1 with this tree's dividers (D.5, Clock).
#[must_use]
pub const fn sysclk_for(input_hz: u32) -> u32 {
    pll1_output_hz(pll1_vco_hz(input_hz, PLL1_M, PLL1_N), PLL1_R)
}

/// The clock the PLL runs from (D.5, Clock).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClockSource {
    /// The board's HSE in bypass mode.
    Hse,
    /// The internal 16 MHz oscillator, when HSE is not ready.
    Hsi16,
}

/// Why the PLL runs from this source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClockNote {
    /// HSE reported ready in bypass mode. Its figure is the board's, not a measurement.
    Bypass,
    /// HSE did not report ready within [`READY_POLLS`], so the clock runs from HSI16 (ARCH-13).
    HseNotReady,
}

/// The clock the chip started with, as the startup record reports it (the STM32F407's shape).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClockReport {
    /// The PLL input.
    pub source: ClockSource,
    /// Why this source.
    pub note: ClockNote,
    /// The HSE frequency as measured: 0, because the bypass clock is not measured (D.5, Clock).
    pub measured_hz: u32,
    /// The HSE frequency the PLL is configured for; 0 when PLL1 runs from HSI16.
    pub crystal_hz: u32,
    /// The system clock the clock tree reports.
    pub sysclk_hz: u32,
}

/// The clock that a start with HSE either ready or not reports. Both go through the same PLL1
/// dividers, so both reach 160 MHz (D.5, Clock; ARCH-13).
#[must_use]
pub const fn report(hse_ready: bool) -> ClockReport {
    if hse_ready {
        ClockReport {
            source: ClockSource::Hse,
            note: ClockNote::Bypass,
            measured_hz: 0,
            crystal_hz: HSE_HZ,
            sysclk_hz: sysclk_for(HSE_HZ),
        }
    } else {
        ClockReport {
            source: ClockSource::Hsi16,
            note: ClockNote::HseNotReady,
            measured_hz: 0,
            crystal_hz: 0,
            sysclk_hz: sysclk_for(HSI16_HZ),
        }
    }
}

/// Why the clock could not be started.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClockError {
    /// The named step did not settle within [`READY_POLLS`] polls.
    Timeout(&'static str),
}

/// Starts the clock tree at 160 MHz (D.5, Clock). The order follows the board's project: the PWR
/// clock, the SMPS regulator with VDDIO2 valid, voltage range 1 with the EPOD booster, the flash
/// latency, HSE in bypass mode (HSI16 when it is not ready), PLL1, the buses undivided, HSI48 for
/// the USB clock, then SYSCLK from PLL1 and clock security when HSE runs.
#[cfg(all(target_arch = "arm", target_os = "none"))]
pub fn start(
    rcc: &pac::RCC,
    pwr: &pac::PWR,
    flash: &pac::FLASH,
) -> Result<ClockReport, ClockError> {
    rcc.ahb3enr().modify(|_, w| w.pwren().enabled());
    wait_for("PWR clock", || rcc.ahb3enr().read().pwren().bit())?;

    pwr.cr3().modify(|_, w| w.regsel().smps());
    pwr.svmcr().modify(|_, w| w.io2sv().present());
    pwr.vosr().modify(|_, w| w.vos().range1());
    wait_for("voltage range 1", || pwr.vosr().read().vosrdy().bit())?;
    pwr.vosr().modify(|_, w| w.boosten().enabled());
    wait_for("EPOD booster", || pwr.vosr().read().boostrdy().bit())?;

    // The latency is raised before the clock is, and checked by reading it back.
    // SAFETY: LATENCY takes the wait-state count; FLASH_WAIT_STATES (4) is the count for 160 MHz
    // in voltage range 1.
    flash
        .acr()
        .modify(|_, w| unsafe { w.latency().bits(FLASH_WAIT_STATES) });
    wait_for("flash latency", || {
        flash.acr().read().latency().bits() == FLASH_WAIT_STATES
    })?;

    // The bypass bit is written while HSE is off, then HSE is switched on.
    rcc.cr().modify(|_, w| w.hsebyp().bypassed());
    rcc.cr().modify(|_, w| w.hseon().enabled());
    let hse_ready = wait_for("HSE", || rcc.cr().read().hserdy().bit()).is_ok();
    if !hse_ready {
        rcc.cr().modify(|_, w| w.hseon().disabled());
        rcc.cr().modify(|_, w| w.hsebyp().not_bypassed());
        rcc.cr().modify(|_, w| w.hsion().enabled());
        wait_for("HSI16", || rcc.cr().read().hsirdy().bit())?;
    }

    // PLL1 from the chosen input: input range 4 to 8 MHz (RGE 0), M = 4, boost 1, R and Q on.
    rcc.pll1cfgr().modify(|_, w| {
        let w = if hse_ready {
            w.pll1src().hse()
        } else {
            w.pll1src().hsi16()
        };
        w.pll1rge()
            .range1()
            .pll1m()
            .div4()
            .pll1mboost()
            .div1()
            .pll1ren()
            .enabled()
            .pll1qen()
            .enabled()
    });
    // SAFETY: PLL1N is a 9-bit field and PLL1N_FIELD (0x4F) fits it; the register is written only
    // while PLL1 is off, which is its state after reset and before PLL1ON below.
    rcc.pll1divr().modify(|_, w| {
        let w = unsafe { w.pll1n().bits(PLL1N_FIELD) };
        w.pll1q().div2().pll1r().div2()
    });
    rcc.cr().modify(|_, w| w.pll1on().enabled());
    wait_for("PLL1", || rcc.cr().read().pll1rdy().bit())?;

    rcc.cfgr2()
        .modify(|_, w| w.hpre().div1().ppre1().div1().ppre2().div1());
    rcc.cfgr3().modify(|_, w| w.ppre3().div1());

    // The 48 MHz USB kernel clock is HSI48, and the OTG_HS PHY takes its reference from HSE.
    rcc.cr().modify(|_, w| w.hsi48on().enabled());
    wait_for("HSI48", || rcc.cr().read().hsi48rdy().bit())?;
    rcc.ccipr1().modify(|_, w| w.iclksel().hsi());
    rcc.ccipr2().modify(|_, w| w.otghssel().hse());

    rcc.cfgr1().modify(|_, w| w.sw().pll());
    wait_for("SYSCLK from PLL1", || rcc.cfgr1().read().sws().is_pll())?;
    if hse_ready {
        // Clock security: a failing HSE switches SYSCLK to HSI16 and raises the CSS flag.
        rcc.cr().modify(|_, w| w.csson().enabled());
    }
    Ok(report(hse_ready))
}

/// Polls `ready` up to [`READY_POLLS`] times. `step` names the wait that did not settle.
#[cfg(all(target_arch = "arm", target_os = "none"))]
fn wait_for(step: &'static str, ready: impl Fn() -> bool) -> Result<(), ClockError> {
    for _ in 0..READY_POLLS {
        if ready() {
            return Ok(());
        }
    }
    Err(ClockError::Timeout(step))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_board_hse_through_pll1_gives_a_160_mhz_system_clock() {
        // Specification 34, Appendix D.5 (Clock): HSE 16 MHz, M 4, N 80 and R 2 give a VCO of
        // 320 MHz and SYSCLK of 160 MHz. Catches a divider swapped for another, or N read as M.
        assert_eq!(pll1_vco_hz(16_000_000, 4, 80), 320_000_000);
        assert_eq!(pll1_output_hz(320_000_000, 2), 160_000_000);
        assert_eq!(sysclk_for(HSE_HZ), 160_000_000);
        assert_eq!(SYSCLK_HZ, 160_000_000);
    }

    #[test]
    fn the_r_and_q_outputs_divide_the_vco_by_two() {
        // Specification 34, Appendix D.5 (Clock): R and Q are both divide-by-2, field value 1, as in
        // stm32u5x.cfg (PLL1P = PLL1Q = PLL1R = /2). Catches a divider of 1 (320 MHz) or 4 (80 MHz).
        assert_eq!(PLL1_R, 2);
        assert_eq!(PLL1_Q, 2);
        assert_eq!(pll1_output_hz(320_000_000, PLL1_R), 160_000_000);
        assert_eq!(pll1_output_hz(320_000_000, PLL1_Q), 160_000_000);
    }

    #[test]
    fn the_n_field_holds_n_minus_one() {
        // Specification 34, Appendix D.5 (Clock): stm32u5x.cfg writes PLL1DIVR 0x0101024F, whose N
        // field is 0x4F and which the script comments as N = 80. Catches a field written as N itself,
        // which would make the VCO 316 MHz.
        assert_eq!(PLL1N_FIELD, 0x4F);
        assert_eq!(u32::from(PLL1N_FIELD) + 1, PLL1_N);
    }

    #[test]
    fn a_ready_hse_is_the_clock_with_the_board_figure_and_no_measurement() {
        // Specification 34, Appendix D.5 (Clock): the bypass clock is the board's 16 MHz and is not
        // measured. Catches a report that claims a measurement, or another crystal figure.
        assert_eq!(
            report(true),
            ClockReport {
                source: ClockSource::Hse,
                note: ClockNote::Bypass,
                measured_hz: 0,
                crystal_hz: 16_000_000,
                sysclk_hz: 160_000_000,
            }
        );
    }

    #[test]
    fn a_missing_hse_runs_the_same_pll1_from_hsi16_and_says_why() {
        // Specification 34, Appendix D.5 (Clock) and ARCH-13: without HSE the chip runs from the
        // 16 MHz HSI16 through the same PLL1, and the report names the reason. Catches a silent
        // fallback, a fallback with no reason, or another PLL input.
        assert_eq!(
            report(false),
            ClockReport {
                source: ClockSource::Hsi16,
                note: ClockNote::HseNotReady,
                measured_hz: 0,
                crystal_hz: 0,
                sysclk_hz: 160_000_000,
            }
        );
    }

    #[test]
    fn flash_has_four_wait_states_at_160_mhz_in_voltage_range_1() {
        // Specification 34, Appendix D.5 (Clock): 4 wait states at 160 MHz, the board's
        // FLASH_LATENCY_4 and stm32u5x.cfg's FLASH_ACR = 4. Catches 3 or 5 wait states.
        assert_eq!(FLASH_WAIT_STATES, 4);
    }

    #[test]
    fn the_usb_clock_is_48_mhz_from_hsi48_because_pll1_does_not_give_it() {
        // Specification 34, Appendix D.5 (Clock): the OTG_HS kernel clock is 48 MHz from HSI48 (ICLKSEL
        // HSI). PLL1's Q output is 160 MHz, so the USB clock cannot come from PLL1. Catches a USB
        // clock taken from PLL1.
        assert_eq!(USB_KERNEL_HZ, 48_000_000);
        assert_ne!(
            pll1_output_hz(pll1_vco_hz(HSE_HZ, PLL1_M, PLL1_N), PLL1_Q),
            USB_KERNEL_HZ
        );
    }
}
