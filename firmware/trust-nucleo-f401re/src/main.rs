#![no_std]
#![no_main]

extern crate alloc;

use trust_nucleo_f401re::application;
mod memory;
mod profile;
mod runner;
mod trace;

use core::fmt::Write;
use cortex_m_rt::{entry, exception};
use trust_platform_stm32f4::{Board, ResetCookie};

const WATCHDOG_PHASE: u16 = 0x5744;
const RUNNING_PHASE: u16 = 0x5255;

#[entry]
fn main() -> ! {
    memory::initialize();
    let mut board = match Board::take() {
        Ok(board) => board,
        Err(_) => trust_platform_stm32f4::emergency_halt("board-init"),
    };
    board.io.safe_off();
    board.clock.enable_systick(true);
    let identity = &board.identity;
    let reason = board.reset.reason();
    if writeln!(
        board.console,
        "B1,IDENTITY,{:08x},{:08x},{:08x},{:08x},{},{},{:08x},{:08x}\r",
        identity.dbgmcu_idcode,
        identity.uid[0],
        identity.uid[1],
        identity.uid[2],
        identity.flash_kib,
        identity.sysclk_hz,
        identity.fpscr,
        reason.raw
    )
    .is_err()
    {
        trust_platform_stm32f4::emergency_halt("identity-uart");
    }
    if let Some(cookie) = board.reset.cookie() {
        if cookie.phase == WATCHDOG_PHASE && reason.independent_watchdog() {
            board.reset.clear_cookie();
            if trace::io(&mut board.console, &board.io, "watchdog-BOOT", false).is_err()
                || writeln!(board.console, "B1,STATE,WATCHDOG_RESET\r\nB1,DONE\r").is_err()
            {
                trust_platform_stm32f4::emergency_halt("reset-uart");
            }
            idle(&mut board);
        }
        // Do not silently rerun after an OOM, crash or unexpected reset. The
        // preceding evidence is a failed run and requires an explicit new boot.
        trust_platform_stm32f4::emergency_halt("unexpected-reset-cookie");
    }
    if identity.flash_kib != 512
        || identity.sysclk_hz != 84_000_000
        || identity.fpscr & ((3 << 22) | (1 << 24)) != 0
    {
        trust_platform_stm32f4::emergency_halt("hardware-profile");
    }
    if let Err(error) = run(&mut board) {
        board.io.safe_off();
        trace::failure(&mut board.console, "runner", error);
        idle(&mut board);
    }
    // The only deliberate missed feed. Backup-register cookie and RCC reset
    // flags must both confirm this reset before the second boot emits DONE.
    if board
        .reset
        .set_cookie(ResetCookie {
            phase: WATCHDOG_PHASE,
        })
        .is_err()
        || writeln!(board.console, "B1,STATE,WATCHDOG_ARM\r").is_err()
    {
        trust_platform_stm32f4::emergency_halt("watchdog-arm");
    }
    board.io.safe_off();
    loop {
        cortex_m::asm::wfi();
    }
}

fn run(board: &mut Board) -> Result<(), &'static str> {
    trace::io(&mut board.console, &board.io, "BOOT", false)?;
    trace::memory(&mut board.console, "boot")?;
    let applications = application::load()?;
    board
        .reset
        .set_cookie(ResetCookie {
            phase: RUNNING_PHASE,
        })
        .map_err(|_| "reset-cookie")?;
    board
        .watchdog
        .start_ms(4000)
        .map_err(|_| "watchdog-start")?;
    runner::main_fixture(board, applications.main)?;
    // Each function drops its state and prepared module before the next one.
    trace::memory(&mut board.console, "main-dropped")?;
    runner::numeric_fixture(board, applications.numeric)?;
    trace::memory(&mut board.console, "numeric-dropped")?;
    runner::gpio_fixture(board, applications.gpio)?;
    trace::memory(&mut board.console, "gpio-dropped")?;
    writeln!(
        board.console,
        "B1,STACK_PEAK,{}\r",
        memory::boot_stack_peak()
    )
    .map_err(|_| "uart")?;
    Ok(())
}

fn idle(board: &mut Board) -> ! {
    board.io.safe_off();
    loop {
        // Completion/failure supervision only: the PLC is STOPPED, and this
        // prevents uncontrolled reset/retest loops. No scan is reported here.
        board.watchdog.feed();
        cortex_m::asm::wfi();
    }
}

#[exception]
fn SysTick() {
    trust_platform_stm32f4::systick_interrupt();
}

#[exception]
unsafe fn HardFault(_: &cortex_m_rt::ExceptionFrame) -> ! {
    trust_platform_stm32f4::emergency_halt("hard-fault")
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo<'_>) -> ! {
    // Includes the stable allocator's allocation-error panic. Never formats the
    // panic payload, allocates, or steals the live Board's HAL ownership.
    let (used, peak, allocations, failures) = memory::emergency_metrics();
    let mut record = FailureRecord {
        bytes: [0; 112],
        length: 0,
    };
    let _ = write!(
        record,
        "panic_used_{used}_peak_{peak}_allocations_{allocations}_failures_{failures}"
    );
    let message = core::str::from_utf8(&record.bytes[..record.length]).unwrap_or("panic-record");
    trust_platform_stm32f4::emergency_halt(message)
}

struct FailureRecord {
    bytes: [u8; 112],
    length: usize,
}
impl core::fmt::Write for FailureRecord {
    fn write_str(&mut self, text: &str) -> core::fmt::Result {
        let end = self
            .length
            .checked_add(text.len())
            .ok_or(core::fmt::Error)?;
        let target = self
            .bytes
            .get_mut(self.length..end)
            .ok_or(core::fmt::Error)?;
        target.copy_from_slice(text.as_bytes());
        self.length = end;
        Ok(())
    }
}
