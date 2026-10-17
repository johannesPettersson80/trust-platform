#![no_std]
#![no_main]

mod application;
mod board;
mod memory;

use crate::board::{Clock, Io, Watchdog, WIRING};
use core::fmt::Write;
use cortex_m_rt::{entry, exception};
use trust_board_fixture::{board::Board as Fixture, memory::TargetMemory, records, runner};
use trust_platform_stm32f4::{Board, ResetCookie, CHIP};

const WATCHDOG_PHASE: u16 = 0x5744;
const RUNNING_PHASE: u16 = 0x5255;

#[entry]
fn main() -> ! {
    memory::initialize();
    let mut board = match Board::take(&WIRING) {
        Ok(board) => board,
        Err(error) => trust_platform_stm32f4::emergency_halt(error.code()),
    };
    safe_off(&mut board);
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
    // The crystal as measured against the HSI, the one the PLL was configured for, and the clock
    // that results; when there is no usable crystal the source is HSI and the note says why.
    let clock = &identity.clock;
    if writeln!(
        board.console,
        "B1,CLOCK,{},{},{},{},{}\r",
        clock.source.code(),
        clock.measured_hz,
        clock.crystal_hz,
        clock.sysclk_hz,
        clock.note.code()
    )
    .is_err()
    {
        trust_platform_stm32f4::emergency_halt("clock-uart");
    }
    if let Some(cookie) = board.reset.cookie() {
        if cookie.phase == WATCHDOG_PHASE && reason.independent_watchdog() {
            board.reset.clear_cookie();
            if records::io(
                &mut board.console,
                &Io::new(&mut board.io, board.process),
                "watchdog-BOOT",
                false,
            )
            .is_err()
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
    if CHIP
        .verify_identity(identity.dbgmcu_idcode, identity.flash_kib)
        .is_err()
        || identity.sysclk_hz != CHIP.sysclk_hz
        || identity.fpscr & ((3 << 22) | (1 << 24)) != 0
    {
        trust_platform_stm32f4::emergency_halt("hardware-profile");
    }
    if let Err(error) = run(&mut board) {
        safe_off(&mut board);
        records::failure(&mut board.console, "runner", error);
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
    safe_off(&mut board);
    loop {
        cortex_m::asm::wfi();
    }
}

/// The F407 composition: the shared M2E runner on this chip's adapter and memory map.
fn run(board: &mut Board) -> Result<(), &'static str> {
    let clock = Clock(&board.clock);
    let mut io = Io::new(&mut board.io, board.process);
    let mut watchdog = Watchdog(&mut board.watchdog);
    let target = TargetMemory::new(&memory::HEAP, memory::STACK_WINDOW);
    let mut fixture = Fixture {
        clock: &clock,
        io: &mut io,
        console: &mut board.console,
        watchdog: &mut watchdog,
        memory: &target,
    };
    runner::run(&mut fixture, application::load, |dog| {
        board
            .reset
            .set_cookie(ResetCookie {
                phase: RUNNING_PHASE,
            })
            .map_err(|_| "reset-cookie")?;
        dog.0.start_ms(4000).map_err(|_| "watchdog-start")
    })
}

/// Drives the wired output to its safe level.
fn safe_off(board: &mut Board) {
    board.io.drive_safe(board.process.output);
}

fn idle(board: &mut Board) -> ! {
    safe_off(board);
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
    let (used, peak, allocations, failures) = memory::HEAP.emergency_metrics();
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
