//! Bare-metal ownership and initialization of the fixed board peripherals.
use crate::{ResetReason, SYSCLK_HZ};
use cortex_m::peripheral::Peripherals as CorePeripherals;
use stm32f4::stm32f401 as pac;
use stm32f4xx_hal::{prelude::*, rcc::Config, serial::Serial, signature::FlashSize};

mod clock;
mod console;
mod emergency;
mod io;
mod reset;
pub use clock::{systick_interrupt, Monotonic};
pub use console::Console;
pub use emergency::emergency_halt;
pub use io::DigitalIo;
pub use reset::{ResetControl, Watchdog};

/// Initialization or explicit peripheral-service failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoardError {
    /// Another owner already took the Cortex-M or device peripherals.
    PeripheralsTaken,
    /// The frozen HAL clock tree does not match the declared board profile.
    ClockProfile,
    /// This core does not expose the required hardware cycle counter.
    CycleCounterUnavailable,
    /// The fixed UART configuration was rejected by the HAL.
    SerialConfiguration,
    /// Watchdog interval must be between 1 and 32767 nominal milliseconds.
    WatchdogInterval,
    /// Phase zero is invalid or backup-register readback failed.
    ResetCookie,
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
    /// Nominal configured system clock, not a measurement of HSI tolerance.
    pub sysclk_hz: u32,
}

/// Separately owned peripherals allow the engine to borrow the clock while the
/// firmware samples/publishes process images and writes bounded trace records.
pub struct Board {
    /// Hardware monotonic time and optional SysTick interrupt instrumentation.
    pub clock: Monotonic,
    /// PC13 input and PA5 output.
    pub io: DigitalIo,
    /// ST-LINK virtual COM transmitter and reserved receive pin.
    pub console: Console,
    /// Independent hardware watchdog; initially not started by this adapter.
    pub watchdog: Watchdog,
    /// Boot reset flags and phase cookie, independent of PLC retention.
    pub reset: ResetControl,
    /// MCU and arithmetic configuration captured at initialization.
    pub identity: Identity,
}

impl Board {
    /// Take peripherals once and configure the stock NUCLEO-F401RE connection.
    /// GPIO is driven safe before UART, timer or user program initialization.
    #[inline(never)]
    pub fn take() -> Result<Self, BoardError> {
        let dp = pac::Peripherals::take().ok_or(BoardError::PeripheralsTaken)?;
        let mut cp = CorePeripherals::take().ok_or(BoardError::PeripheralsTaken)?;
        let reason = ResetReason {
            raw: dp.RCC.csr().read().bits(),
        };
        let mut rcc = dp.RCC.freeze(Config::hsi().sysclk(SYSCLK_HZ.Hz()));
        if rcc.clocks.sysclk().raw() != SYSCLK_HZ || rcc.clocks.hclk().raw() != SYSCLK_HZ {
            return Err(BoardError::ClockProfile);
        }
        let gpioa = dp.GPIOA.split(&mut rcc);
        let gpioc = dp.GPIOC.split(&mut rcc);
        let io = DigitalIo::new(
            gpioa.pa5.into_push_pull_output(),
            gpioc.pc13.into_pull_up_input(),
        );
        cp.SCB.enable_fpu();
        cortex_m::asm::dsb();
        cortex_m::asm::isb();
        cp.DCB.enable_trace();
        if !cortex_m::peripheral::DWT::has_cycle_counter() {
            return Err(BoardError::CycleCounterUnavailable);
        }
        cp.DWT.enable_cycle_counter();
        let clock = Monotonic::new(dp.TIM2, cp.SYST, cp.DWT, &mut rcc);
        let serial = Serial::<pac::USART2, u8>::new(
            dp.USART2,
            (gpioa.pa2, gpioa.pa3),
            230_400.bps(),
            &mut rcc,
        )
        .map_err(|_| BoardError::SerialConfiguration)?;
        let (tx, rx) = serial.split();
        let console = Console::new(tx, rx);
        emergency::console_ready();
        let mut watchdog = Watchdog::new(dp.IWDG);
        watchdog.continue_while_debug_halted(&dp.DBGMCU);
        let identity = Identity {
            dbgmcu_idcode: dp.DBGMCU.idcode().read().bits(),
            uid: read_uid(),
            flash_kib: FlashSize::get().kilo_bytes(),
            fpscr: cortex_m::register::fpscr::read().bits(),
            sysclk_hz: SYSCLK_HZ,
        };
        let reset = ResetControl::new(dp.RTC, dp.PWR, reason, &mut rcc);
        rcc.csr().modify(|_, w| w.rmvf().set_bit());
        Ok(Self {
            clock,
            io,
            console,
            watchdog,
            reset,
            identity,
        })
    }
}

fn read_uid() -> [u32; 3] {
    // SAFETY: RM0368 electronic signature defines three aligned read-only UID
    // words at 0x1FFF7A10..0x1FFF7A1B on STM32F401. This ARM-only crate fixes
    // that chip; no caller-provided address or aliasing mutable reference exists.
    unsafe {
        let uid = 0x1fff_7a10 as *const u32;
        [
            core::ptr::read_volatile(uid),
            core::ptr::read_volatile(uid.add(1)),
            core::ptr::read_volatile(uid.add(2)),
        ]
    }
}
