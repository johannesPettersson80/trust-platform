//! Measures the crystal against the internal HSI with TIM11 (its option register connects
//! HSE_RTC, the crystal divided by RCC_CFGR.RTCPRE, to input 1 of the timer). The chip
//! runs from the HSI after reset, so the timer counts HSI cycles, and one capture is taken for
//! every eighth edge of the divided crystal clock. The arithmetic and the decision are in
//! `crate::crystal`, where the host tests them.
use crate::{
    crystal::{self, Capture, Probe},
    pac,
};

/// Starts the crystal oscillator and measures it. The crystal is left running when it started
/// (the clock tree switches it off again if it decides against it), and switched off when it did
/// not start. TIM11 and its clock are returned to their reset state.
pub(super) fn probe(rcc: &pac::RCC, timer: &pac::TIM11) -> Probe {
    // TIM11 sits on APB2, which runs from the HSI at reset.
    rcc.apb2enr().modify(|_, w| w.tim11en().set_bit());
    let _ = rcc.apb2enr().read();
    // The divider is written while the crystal oscillator is still off, as it is after reset.
    rcc.cfgr()
        .modify(|_, w| w.rtcpre().set(crystal::RTC_DIVIDER));
    rcc.cr().modify(|_, w| w.hseon().set_bit());
    let started = (0..crystal::HSE_READY_POLLS).any(|_| rcc.cr().read().hserdy().bit_is_set());
    let probe = if started {
        measure(timer).map_or(Probe::NoCapture, Probe::Captured)
    } else {
        rcc.cr().modify(|_, w| w.hseon().clear_bit());
        Probe::HseNotReady
    };
    release(rcc, timer);
    probe
}

/// Counts HSI cycles over `INTERVALS` captures of the divided crystal clock.
fn measure(timer: &pac::TIM11) -> Option<Capture> {
    timer.psc().write(|w| w.psc().set(0));
    timer.arr().write(|w| w.arr().set(u16::MAX));
    timer.sr().reset();
    // Channel 1 captures from TI1, once per eight edges, unfiltered.
    timer
        .ccmr1_input()
        .write(|w| w.cc1s().ti1().ic1psc().eight_events().ic1f().no_filter());
    // SAFETY: TIM11_OR bits 1:0 (TI1_RMP) select the source of input 1 and nothing else; 0b10
    // connects HSE_RTC, the value of TIM_TIM11_HSE in ST's HAL (TIM_OR_TI1_RMP_1, 0x2, in the
    // CMSIS device header). This is the owned TIM11, which the clock tree borrows before any
    // other code runs, and the register is reset again in `release`. The PAC offers no safe
    // writer for this field.
    timer.or().write(|w| unsafe { w.rmp().bits(0b10) });
    timer.ccer().write(|w| w.cc1e().enabled());
    timer.cr1().write(|w| w.cen().set_bit());
    // The first capture only starts the interval.
    let mut previous = next_capture(timer)?;
    let mut counts = 0_u32;
    for _ in 0..crystal::INTERVALS {
        let now = next_capture(timer)?;
        counts = counts.saturating_add(crystal::elapsed(previous, now));
        previous = now;
    }
    Some(Capture {
        intervals: crystal::INTERVALS,
        timer_counts: counts,
    })
}

/// Waits for the next capture. Reading CCR1 clears the capture flag; an overcapture flag means an
/// edge was missed, which makes the interval wrong, so the measurement is given up. Captures come
/// at most every 9.5 microseconds (a 26 MHz crystal): the release profile keeps up, an
/// unoptimized build may not for the fastest crystals, and then the clock runs from the HSI.
fn next_capture(timer: &pac::TIM11) -> Option<u16> {
    for _ in 0..crystal::CAPTURE_POLLS {
        let status = timer.sr().read();
        if status.cc1if().bit_is_set() {
            let value = timer.ccr1().read().ccr().bits();
            return if status.cc1of().bit_is_set() {
                None
            } else {
                Some(value)
            };
        }
    }
    None
}

/// Returns TIM11 and its clock to the reset state. The crystal divider stays at its value: it
/// affects only the HSE_RTC tap, which nothing else uses.
fn release(rcc: &pac::RCC, timer: &pac::TIM11) {
    timer.cr1().reset();
    timer.ccer().reset();
    timer.ccmr1_input().reset();
    timer.or().reset();
    timer.psc().reset();
    timer.arr().reset();
    timer.sr().reset();
    rcc.apb2enr().modify(|_, w| w.tim11en().clear_bit());
}
