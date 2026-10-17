//! Bare-metal ownership and initialization of the STM32F407 peripherals. The chip is the unit of
//! firmware (specification 34, ARCH-10): which pins a board wires comes in as a [`Wiring`], and
//! nothing in this adapter names a board. The timer, the watchdog and the reset marker are the
//! files the F401 composition uses; the clock tree, the pins, the console and the fault boundary
//! are the F407's own.
use crate::{
    chip::uid_words,
    crystal::ClockReport,
    pac,
    pins::{PinError, Wiring},
    ResetReason, SYSCLK_HZ,
};
use cortex_m::peripheral::Peripherals as CorePeripherals;
use stm32f4xx_hal::signature::{FlashSize, Uid};

#[path = "../hardware/clock.rs"]
mod clock;
mod clock_tree;
mod console;
mod crystal_timer;
mod emergency;
mod gpio;
#[path = "../hardware/reset.rs"]
mod reset;
pub use clock::{systick_interrupt, Monotonic};
pub use console::Console;
pub use emergency::emergency_halt;
pub use gpio::{DigitalIo, Input, Output};
pub use reset::{ResetControl, Watchdog};

/// Initialization or explicit peripheral-service failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoardError {
    /// Another owner already took the Cortex-M or device peripherals.
    PeripheralsTaken,
    /// The frozen HAL clock tree does not match the chip's system clock.
    ClockProfile,
    /// This core does not expose the required hardware cycle counter.
    CycleCounterUnavailable,
    /// The console speed does not fit the bus clock.
    SerialConfiguration,
    /// Watchdog interval must be between 1 and 32767 nominal milliseconds.
    WatchdogInterval,
    /// Phase zero is invalid or backup-register readback failed.
    ResetCookie,
    /// A line of the wiring does not name a usable pin of the chip.
    Pin(PinError),
}

impl BoardError {
    /// A short code for the failure record.
    pub const fn code(self) -> &'static str {
        match self {
            Self::PeripheralsTaken => "peripherals-taken",
            Self::ClockProfile => "clock-profile",
            Self::CycleCounterUnavailable => "cycle-counter",
            Self::SerialConfiguration => "serial-configuration",
            Self::WatchdogInterval => "watchdog-interval",
            Self::ResetCookie => "reset-cookie",
            Self::Pin(_) => "pin",
        }
    }
}

/// Electronic identity and configured execution premises, not PCB revision proof.
#[derive(Debug, Clone, Copy)]
pub struct Identity {
    /// DBGMCU device/revision identity register, unchanged raw bits.
    pub dbgmcu_idcode: u32,
    /// Full 96-bit factory UID in native little-endian words.
    pub uid: [u32; 3],
    /// Factory flash-density signature in KiB.
    pub flash_kib: u16,
    /// FPSCR sampled after the FPU is enabled; no rounding/FTZ override is applied.
    pub fpscr: u32,
    /// System clock the clock tree reports, not a measurement of the oscillator's tolerance.
    pub sysclk_hz: u32,
    /// Where the clock comes from: the crystal as measured against the HSI, or the HSI itself.
    pub clock: ClockReport,
}

/// The pins that carry the process image's one input bit and one output bit.
#[derive(Debug, Clone, Copy)]
pub struct Process {
    /// The input, for example a button.
    pub input: Input,
    /// The output, for example an LED.
    pub output: Output,
}

/// Separately owned peripherals allow the engine to borrow the clock while the
/// firmware samples/publishes process images and writes bounded trace records.
pub struct Board {
    /// Hardware monotonic time and optional SysTick interrupt instrumentation.
    pub clock: Monotonic,
    /// Every pin of the chip, named at run time.
    pub io: DigitalIo,
    /// The pins of the wiring that carry the process image.
    pub process: Process,
    /// Console transmitter on the chip's console pins.
    pub console: Console,
    /// Independent hardware watchdog; initially not started by this adapter.
    pub watchdog: Watchdog,
    /// Boot reset flags and phase cookie, independent of PLC retention.
    pub reset: ResetControl,
    /// MCU, clock and arithmetic configuration captured at initialization.
    pub identity: Identity,
}

impl Board {
    /// Take peripherals once and configure the chip for the wiring of the board it sits on.
    /// The output is driven to its safe level before the console, the timer or the user program
    /// start.
    #[inline(never)]
    pub fn take(wiring: &Wiring<'_>) -> Result<Self, BoardError> {
        let dp = pac::Peripherals::take().ok_or(BoardError::PeripheralsTaken)?;
        let mut cp = CorePeripherals::take().ok_or(BoardError::PeripheralsTaken)?;
        let reason = ResetReason {
            raw: dp.RCC.csr().read().bits(),
        };
        let (mut rcc, clock_report) = clock_tree::start(dp.RCC, &dp.TIM11);
        if rcc.clocks.sysclk().raw() != SYSCLK_HZ || rcc.clocks.hclk().raw() != SYSCLK_HZ {
            return Err(BoardError::ClockProfile);
        }
        let mut io = DigitalIo::new(
            gpio::Ports {
                a: dp.GPIOA,
                b: dp.GPIOB,
                c: dp.GPIOC,
                d: dp.GPIOD,
                e: dp.GPIOE,
                f: dp.GPIOF,
                g: dp.GPIOG,
                h: dp.GPIOH,
                i: dp.GPIOI,
            },
            &mut rcc,
        );
        let output = io
            .claim_output(wiring.output, wiring.output_safe_high)
            .map_err(BoardError::Pin)?;
        let input = io
            .claim_input(wiring.input, wiring.input_pull)
            .map_err(BoardError::Pin)?;
        cp.SCB.enable_fpu();
        cortex_m::asm::dsb();
        cortex_m::asm::isb();
        cp.DCB.enable_trace();
        if !cortex_m::peripheral::DWT::has_cycle_counter() {
            return Err(BoardError::CycleCounterUnavailable);
        }
        cp.DWT.enable_cycle_counter();
        let clock = Monotonic::new(dp.TIM2, cp.SYST, cp.DWT, &mut rcc);
        let console = Console::new(dp.USART2, &mut io, &mut rcc)?;
        emergency::console_ready();
        let mut watchdog = Watchdog::new(dp.IWDG);
        watchdog.continue_while_debug_halted(&dp.DBGMCU);
        let unique = Uid::get();
        let identity = Identity {
            dbgmcu_idcode: dp.DBGMCU.idcode().read().bits(),
            uid: uid_words(
                unique.x(),
                unique.y(),
                unique.waf_num(),
                unique.lot_num().as_bytes(),
            ),
            flash_kib: FlashSize::get().kilo_bytes(),
            fpscr: cortex_m::register::fpscr::read().bits(),
            sysclk_hz: SYSCLK_HZ,
            clock: clock_report,
        };
        let reset = ResetControl::new(dp.RTC, dp.PWR, reason, &mut rcc);
        rcc.csr().modify(|_, w| w.rmvf().set_bit());
        Ok(Self {
            clock,
            io,
            process: Process { input, output },
            console,
            watchdog,
            reset,
            identity,
        })
    }
}
