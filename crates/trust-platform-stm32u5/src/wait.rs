//! The resource loop's clock, wait and wake on the STM32U5A9 (specification 34, PORT-01 and PORT-03;
//! design §2.5 and S1b §2.5). TIM2 counts microseconds, and its 32-bit count is extended to 64 bits.
//! The wait arms the counter's CC1 compare at its deadline and sleeps with WFI. The TIM2 interrupt
//! and the control link advance one wake counter, which the wait reads, so a wake that arrives before
//! the sleep is never lost.
//!
//! The compare arithmetic is compiled and host-tested everywhere. The timer, the interrupt and the
//! wait are compiled for the chip only.

use core::time::Duration;

use trust_runtime_core::ports::MonotonicInstant;

/// The longest compare the wait arms, 2^31 microseconds (about 35.8 minutes), and the window in which
/// [`compare_reached`] counts a compare as passed. It is half the counter's range, so the comparison
/// stays right across the counter's wrap.
pub const COMPARE_WINDOW_US: u32 = 1 << 31;

/// Whether the 32-bit counter has reached `compare`: it is at or past it, within [`COMPARE_WINDOW_US`]
/// (specification 34, PORT-03). A compare that passed before the wait armed it would fire only after
/// the counter wraps, so the wait checks this before it sleeps.
#[must_use]
pub const fn compare_reached(counter: u32, compare: u32) -> bool {
    counter.wrapping_sub(compare) < COMPARE_WINDOW_US
}

/// The CC1 value that fires the compare for `deadline`: the counter's low 32 bits at `now`, plus the
/// time left to the deadline, clamped to [`COMPARE_WINDOW_US`]. A deadline further away ends the wait
/// early, as an event, and the loop waits again (specification 34, PORT-03).
#[must_use]
#[allow(
    clippy::cast_possible_truncation,
    reason = "both values are cut to the counter's 32 bits on purpose: the low word of the clock, and a time within the window"
)]
pub fn compare_value(now: MonotonicInstant, deadline: MonotonicInstant) -> u32 {
    let window = Duration::from_micros(u64::from(COMPARE_WINDOW_US));
    let remaining = deadline.saturating_since(now).as_duration().min(window);
    (now.since_origin().as_micros() as u32).wrapping_add(remaining.as_micros() as u32)
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
pub use hardware::{advance_wake, Monotonic, Wait};

#[cfg(all(target_arch = "arm", target_os = "none"))]
mod hardware {
    use core::cell::Cell;
    use core::sync::atomic::{AtomicU32, Ordering};

    use cortex_m::peripheral::NVIC;
    use critical_section::Mutex;
    use stm32u5::stm32u5a9::{self as pac, interrupt};
    use trust_platform_stm32::time::CounterExtension;
    use trust_runtime_core::ports::{
        MonotonicClock, MonotonicInstant, PortError, RunnerWait, WakeReason, WakeSignal, WakeToken,
    };

    use super::{compare_reached, compare_value};

    /// TIM2's prescaler: PCLK1 is 160 MHz, undivided (`clock.rs`), and 160 MHz / (159 + 1) is 1 MHz.
    const PRESCALER: u16 = 159;

    /// The wakes counted since boot. The TIM2 interrupt, the control link's servicing pass and
    /// [`WakeSignal::wake`] advance it, and the wait ends when it moves (design §2.5).
    static WAKE_COUNT: AtomicU32 = AtomicU32::new(0);

    /// The 64-bit extension of TIM2's 32-bit count. Only the main context samples it.
    static EXTENSION: Mutex<Cell<CounterExtension>> =
        Mutex::new(Cell::new(CounterExtension::new()));

    /// The TIM2 register block, the chip's own at its address (PAC 0.16.0).
    fn timer() -> &'static pac::tim2::RegisterBlock {
        // SAFETY: TIM2 is the chip's own timer block at its address (PAC 0.16.0). This module is the
        // only user of its registers: `Monotonic::start` writes them before the wait starts, and the
        // wait and the interrupt use them inside the critical section.
        unsafe { &*pac::TIM2::ptr() }
    }

    /// The 32-bit count of TIM2 now.
    fn counter() -> u32 {
        timer().cnt().read().cnt().bits()
    }

    /// Advances the wake counter, so a current or coming wait returns (design §2.5).
    pub fn advance_wake() {
        WAKE_COUNT.fetch_add(1, Ordering::Release);
    }

    /// The clock's reading now: the 32-bit count extended to 64 bits, in microseconds since TIM2
    /// started. Each resource-loop pass samples it, and a pass waits at most one second (TIME-04), so
    /// the count never wraps unobserved (`CounterExtension`).
    fn now_instant() -> MonotonicInstant {
        let micros = critical_section::with(|cs| {
            let cell = EXTENSION.borrow(cs);
            let mut extension = cell.get();
            let micros = extension.observe(counter());
            cell.set(extension);
            micros
        });
        MonotonicInstant::from_micros(micros)
    }

    /// The chip's monotonic clock (specification 34, PORT-01): TIM2 at 1 MHz, extended to 64 bits.
    pub struct Monotonic(());

    impl Monotonic {
        /// Starts TIM2 as the free-running microsecond counter. Call it once, after the clock tree has
        /// started (`clock::start`), because the timer counts PCLK1.
        #[must_use]
        pub fn start() -> Self {
            // SAFETY: RCC is the chip's own register block (PAC 0.16.0). The one write sets TIM2EN in
            // RCC_APB1ENR1, the timer's clock.
            let rcc = unsafe { &*pac::RCC::ptr() };
            rcc.apb1enr1().modify(|_, w| w.tim2en().set_bit());
            // Reading the enable register back waits out the clock's enable delay, so the timer
            // writes below take effect.
            let _ = rcc.apb1enr1().read().tim2en().bit();
            let tim = timer();
            tim.psc().write(|w| w.psc().set(PRESCALER));
            tim.arr().write(|w| w.arr().set(u32::MAX));
            // The update event loads the prescaler that the counter counts with.
            tim.egr().write(|w| w.ug().set_bit());
            tim.sr().reset();
            tim.cr1().modify(|_, w| w.cen().set_bit());
            Self(())
        }
    }

    impl MonotonicClock for Monotonic {
        fn now(&self) -> MonotonicInstant {
            now_instant()
        }
    }

    /// The TIM2 interrupt: the CC1 compare has fired. It clears the flag, disables the compare and
    /// advances the wake counter, so the wait that armed the compare ends (design §2.5).
    #[allow(
        non_snake_case,
        reason = "the vector's symbol is the PAC's interrupt name"
    )]
    #[interrupt]
    fn TIM2() {
        let tim = timer();
        tim.sr().modify(|_, w| w.cc1if().clear_bit());
        tim.dier().modify(|_, w| w.cc1ie().clear_bit());
        advance_wake();
    }

    /// Sets CCR1 to `compare`, clears a stale CC1 flag, and enables the compare interrupt.
    fn arm(compare: u32) {
        let tim = timer();
        tim.ccr1().write(|w| w.ccr().set(compare));
        tim.sr().modify(|_, w| w.cc1if().clear_bit());
        tim.dier().modify(|_, w| w.cc1ie().set_bit());
    }

    /// The resource loop's wait on the chip (specification 34, PORT-03): the CC1 compare on TIM2 ends
    /// it at the deadline, and the wake counter ends it for a wake.
    pub struct Wait(());

    impl Wait {
        /// Unmasks the TIM2 interrupt, which ends a wait at its deadline. Call it once, after
        /// [`Monotonic::start`].
        #[must_use]
        pub fn start() -> Self {
            // SAFETY: NVIC::unmask enables the TIM2 interrupt, whose handler is the `TIM2` function of
            // this module, in the vector table. The handler touches only the timer and the wake counter.
            unsafe { NVIC::unmask(pac::Interrupt::TIM2) };
            Self(())
        }
    }

    impl RunnerWait for Wait {
        fn wake_token(&self) -> WakeToken {
            WakeToken::new(WAKE_COUNT.load(Ordering::Acquire))
        }

        fn wait_until(
            &mut self,
            deadline: MonotonicInstant,
            observed: WakeToken,
        ) -> Result<WakeReason, PortError> {
            critical_section::with(|_| {
                // A wake since the loop read its token ends the wait at once.
                if WAKE_COUNT.load(Ordering::Acquire) != observed.count() {
                    return Ok(WakeReason::Event);
                }
                // So does a deadline that has passed.
                let now = now_instant();
                if now >= deadline {
                    return Ok(WakeReason::Deadline);
                }
                let compare = compare_value(now, deadline);
                arm(compare);
                // A compare that has already passed would fire only after the counter wraps, so the
                // wait returns without sleeping.
                if compare_reached(counter(), compare) {
                    return Ok(WakeReason::Deadline);
                }
                // Interrupts are masked for the WFI. A pending interrupt ends WFI even while it is
                // masked, so a wake that arrives after the checks above is not lost.
                cortex_m::asm::wfi();
                if now_instant() >= deadline {
                    Ok(WakeReason::Deadline)
                } else {
                    Ok(WakeReason::Event)
                }
            })
        }
    }

    impl WakeSignal for Wait {
        fn wake(&self) {
            advance_wake();
        }
    }
}
