//! TIM2 polling clock and independent IRQ/cycle measurement.
use crate::{CounterExtension, SYSCLK_HZ};
use core::{
    cell::Cell,
    sync::atomic::{AtomicU32, Ordering},
};
use cortex_m::peripheral::{syst::SystClkSource, DWT, SYST};
use stm32f4::stm32f401 as pac;
use stm32f4xx_hal::{rcc::Rcc, timer::FTimer};

static IRQ_COUNT: AtomicU32 = AtomicU32::new(0);

/// Call from the firmware's `#[exception] fn SysTick()`. No allocation, GPIO,
/// formatting or runtime scheduling occurs in this instrumentation interrupt.
pub fn systick_interrupt() {
    IRQ_COUNT.fetch_add(1, Ordering::Relaxed);
}

/// Main-context clock owner. Do not sample this Cell-backed extension in an IRQ.
pub struct Monotonic {
    timer: pac::TIM2,
    extension: Cell<CounterExtension>,
    systick: SYST,
    _dwt: DWT,
}
impl Monotonic {
    pub(super) fn new(timer: pac::TIM2, systick: SYST, dwt: DWT, rcc: &mut Rcc) -> Self {
        // HAL configures the APB timer prescaler to exactly 1MHz. Set ARR directly:
        // Counter::start(u32::MAX) would instead create a 2^32-1 tick modulus.
        let timer = FTimer::<pac::TIM2, 1_000_000>::new(timer, rcc).release();
        timer.cr1().reset();
        timer.arr().write(|w| w.arr().set(u32::MAX));
        timer.cnt().write(|w| w.cnt().set(0));
        timer.egr().write(|w| w.ug().set_bit());
        timer.sr().reset();
        timer.cr1().write(|w| w.cen().set_bit());
        Self {
            timer,
            extension: Cell::new(CounterExtension::new()),
            systick,
            _dwt: dwt,
        }
    }

    /// Microseconds since TIM2 startup, including observed rollovers. Sample at
    /// least once per 2^32us; the fixture samples many times per 10ms cycle.
    pub fn micros(&self) -> u64 {
        let mut extension = self.extension.get();
        let now = extension.observe(self.timer.cnt().read().cnt().bits());
        self.extension.set(extension);
        now
    }

    /// Wait against hardware time, not a CPU-loop delay. This deliberately polls:
    /// no wake interrupt is required and no low-power/WFI claim is made.
    pub fn wait_until_us(&self, deadline: u64) {
        while self.micros() < deadline {
            cortex_m::asm::nop();
        }
    }

    /// Wrapping hardware cycle counter. Compare intervals with `wrapping_sub`;
    /// one interval must be shorter than 2^32 cycles (about 51 seconds at 84MHz).
    pub fn cycle_count(&self) -> u32 {
        DWT::cycle_count()
    }

    /// Enable/disable 1kHz SysTick instrumentation. Firmware must install the
    /// callback above before enabling it. Does not reset the cumulative IRQ count.
    pub fn enable_systick(&mut self, enable: bool) {
        self.systick.disable_interrupt();
        self.systick.disable_counter();
        if enable {
            self.systick.set_clock_source(SystClkSource::Core);
            self.systick.set_reload(SYSCLK_HZ / 1_000 - 1);
            self.systick.clear_current();
            self.systick.enable_interrupt();
            self.systick.enable_counter();
        }
    }

    /// Number of observed instrumentation interrupts, wrapping at u32::MAX.
    pub fn irq_count(&self) -> u32 {
        IRQ_COUNT.load(Ordering::Relaxed)
    }
}
