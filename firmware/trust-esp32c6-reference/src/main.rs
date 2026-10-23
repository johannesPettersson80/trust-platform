//! ESP32-C6-DevKitC-1 M2E reference composition: the shared fixture runner on the C6 adapter. It
//! mirrors the F401 composition step by step. The trace grammar, record order and oracle belong to
//! `trust_board_fixture`.
#![no_std]
#![no_main]

mod application;
mod board;
mod memory;

use crate::board::{target_memory, Clock, Io, Watchdog};
use core::fmt::Write;
use trust_board_fixture::{board::Board as Fixture, records, runner};
use trust_platform_esp32::{
    emergency_halt,
    profile::{
        boot_decision, numeric_profile_ok, BootDecision, CHIP_ID, CPU_HZ, FLASH_KIB, WATCHDOG_MS,
    },
    Board, ResetCookie, RUNNING_PHASE, WATCHDOG_PHASE,
};

esp_bootloader_esp_idf::esp_app_desc!();

#[esp_hal::main]
fn main() -> ! {
    memory::initialize();
    let mut board = match Board::take() {
        Ok(board) => board,
        Err(_) => emergency_halt("board-init"),
    };
    board.io.safe_off();
    if board.clock.enable_tick(true).is_err() {
        emergency_halt("tick-timer");
    }
    let identity = board.identity;
    let reason = board.reset.reason();
    if writeln!(
        board.console,
        "B1,IDENTITY,{:08x},{:08x},{:08x},{:08x},{},{},{:08x},{:08x}\r",
        identity.chip_id,
        identity.revision,
        u32::from(u16::from_be_bytes([identity.mac[0], identity.mac[1]])),
        u32::from_be_bytes([
            identity.mac[2],
            identity.mac[3],
            identity.mac[4],
            identity.mac[5],
        ]),
        identity.flash_kib,
        identity.cpu_hz,
        identity.misa,
        reason
    )
    .is_err()
    {
        emergency_halt("identity-uart");
    }
    match boot_decision(reason, board.reset.cookie()) {
        BootDecision::CompleteWatchdogRun => {
            if board.reset.clear_cookie().is_err() {
                emergency_halt("reset-cookie");
            }
            if records::io(
                &mut board.console,
                &Io(&mut board.io),
                "watchdog-BOOT",
                false,
            )
            .is_err()
                || writeln!(board.console, "B1,STATE,WATCHDOG_RESET\r\nB1,DONE\r").is_err()
            {
                emergency_halt("reset-uart");
            }
            idle(&mut board);
        }
        // A USB or power-on reset starts a new run. A leftover cookie is discarded, as D.2 requires.
        BootDecision::StartRun => {
            if board.reset.clear_cookie().is_err() {
                emergency_halt("reset-cookie");
            }
        }
        // Do not silently rerun after a crash or an unexpected reset. The evidence of a failed run
        // needs an explicit new boot, as on F401.
        BootDecision::UnexpectedReset => emergency_halt("unexpected-reset-cookie"),
    }
    if identity.chip_id != CHIP_ID
        || identity.flash_kib != FLASH_KIB
        || identity.cpu_hz != CPU_HZ
        || !numeric_profile_ok(identity.misa)
    {
        emergency_halt("hardware-profile");
    }
    if let Err(error) = run(&mut board) {
        board.io.safe_off();
        records::failure(&mut board.console, "runner", error);
        idle(&mut board);
    }
    // The only deliberate missed feed. The cookie written here and the MWDT0 reset cause must both
    // confirm it on the next boot before DONE is emitted.
    if board
        .reset
        .set_cookie(ResetCookie {
            phase: WATCHDOG_PHASE,
        })
        .is_err()
        || writeln!(board.console, "B1,STATE,WATCHDOG_ARM\r").is_err()
    {
        emergency_halt("watchdog-arm");
    }
    board.io.safe_off();
    loop {
        wait_for_interrupt();
    }
}

/// The C6 composition: the shared M2E runner on this board's adapter and memory map.
fn run(board: &mut Board) -> Result<(), &'static str> {
    // The partition read borrows the whole board, so it finishes before the fixture borrows the
    // board's parts. Its outcome goes to `load`, which reports it after the BOOT records.
    let bundle = application::read(board);
    let clock = Clock(&board.clock);
    let mut io = Io(&mut board.io);
    let mut watchdog = Watchdog(&mut board.watchdog);
    let target = target_memory();
    let mut fixture = Fixture {
        clock: &clock,
        io: &mut io,
        console: &mut board.console,
        watchdog: &mut watchdog,
        memory: &target,
    };
    runner::run(
        &mut fixture,
        || application::load(bundle),
        |dog| {
            board
                .reset
                .set_cookie(ResetCookie {
                    phase: RUNNING_PHASE,
                })
                .map_err(|_| "reset-cookie")?;
            dog.0.start_ms(WATCHDOG_MS).map_err(|_| "watchdog-start")
        },
    )
}

/// Completion and failure supervision. Outputs stay off, and MWDT0 is fed between interrupts so that
/// a stopped board does not enter a reset-and-retest loop. No scan is reported here.
fn idle(board: &mut Board) -> ! {
    board.io.safe_off();
    loop {
        board.watchdog.feed();
        wait_for_interrupt();
    }
}

/// Sleep until the next interrupt. The 1 kHz tick wakes the core at least every millisecond.
fn wait_for_interrupt() {
    // SAFETY: `wfi` only suspends the core until an interrupt is pending. It touches no memory and no
    // register, and it does not change the stack.
    unsafe { core::arch::asm!("wfi", options(nomem, nostack, preserves_flags)) };
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo<'_>) -> ! {
    // Includes the stable allocator's allocation-error panic. Never formats the panic payload, never
    // allocates, and never steals the live Board's HAL ownership. esp-hal's default trap handler also
    // panics, so an exception reaches this path too.
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
    emergency_halt(message)
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
