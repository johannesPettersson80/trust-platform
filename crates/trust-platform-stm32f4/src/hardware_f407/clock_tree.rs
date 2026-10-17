//! The F407 clock tree: the PLL feeds the system clock, from the crystal when there is one and
//! from the HSI when there is not. The crystal is measured against the HSI instead of assumed
//! (specification 34, TARGET-02): see `crate::crystal` and `crystal_timer`.
use super::crystal_timer;
use crate::{
    crystal::{decide, ClockReport, ClockSource},
    pac, SYSCLK_HZ,
};
use stm32f4xx_hal::{
    prelude::*,
    rcc::{Config, Rcc},
};

/// Brings the chip from its reset clock (the HSI) to the chip's system clock and says how.
/// Everything below the crystal's own measurement is the HAL's: PLL dividers, flash wait states,
/// bus prescalers (APB1 at most 42 MHz, APB2 at most 84 MHz on the F407).
pub(super) fn start(rcc: pac::RCC, timer: &pac::TIM11) -> (Rcc, ClockReport) {
    let decision = decide(crystal_timer::probe(&rcc, timer));
    let config = match decision.source {
        ClockSource::Hse => Config::hse(decision.crystal_hz.Hz()),
        ClockSource::Hsi => {
            // No usable crystal: stop the oscillator, and run the PLL from the internal one.
            rcc.cr().modify(|_, w| w.hseon().clear_bit());
            Config::hsi()
        }
    }
    .sysclk(SYSCLK_HZ.Hz());
    let rcc = rcc.freeze(config);
    let report = ClockReport {
        source: decision.source,
        note: decision.note,
        measured_hz: decision.measured_hz,
        crystal_hz: decision.crystal_hz,
        sysclk_hz: rcc.clocks.sysclk().raw(),
    };
    (rcc, report)
}
