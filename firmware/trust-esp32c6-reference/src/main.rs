//! The ESP32-C6 truST runtime: the shared single-thread resource loop over the installed
//! application, on the C6 adapter (design §1.1 and §1.2, specification 34).
//!
//! Boot order (design §1.1, R1): the hardware first (peripherals, clock, the reset reason and the
//! reset record read once, this boot counted); then the installed application (its header read,
//! only its total length copied into the heap, and `installation::admit` against the C6's
//! descriptor); then the record's GPIO lines claimed by pin name, each output driven to its
//! initial value before it becomes an output; then the program prepared, instantiated and started;
//! then the runner serves the control link over USB.
//!
//! With no valid application, or one the device refuses, the same loop runs Stopped and serves the
//! link with the refusal's text (design §1.1, R2). A panic drives every configured output to its
//! safe level, writes the cause and location into the reset record, and resets the chip; it never
//! writes to the link (R4).
#![no_std]
#![no_main]

extern crate alloc;

mod application;
mod memory;

use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt::Write;
use core::time::Duration as CoreDuration;

use smol_str::SmolStr;
use trust_platform_esp32::process_io::GpioIo;
use trust_platform_esp32::{
    emergency_halt, profile::STBC_BYTES, Board, Monotonic, Platform, UsbLink, Wait, Watchdog,
};
use trust_runtime_core::deployment::{Configuration, GpioInput, GpioOutput};
use trust_runtime_core::ports::{MonotonicClock, MonotonicInstant};
use trust_runtime_core::value::Duration;
use trust_runtime_core::vm::PreparedModule;
use trust_runtime_core::watchdog::RetainMode;
use trust_runtime_services::runner::engine::{
    device_limits, CycleSettings, DeviceMemory, DeviceServices, EngineCycle,
};
use trust_runtime_services::runner::Runner;
use trust_runtime_services::status::StatusSettings;

esp_bootloader_esp_idf::esp_app_desc!();

/// Bytes the reset record keeps for a panic's location.
const LOCATION_BYTES: usize = 32;

#[esp_hal::main]
fn main() -> ! {
    memory::initialize();
    let mut board = match Board::take() {
        Ok(board) => board,
        Err(_) => emergency_halt("board-init"),
    };
    // Hardware first (design §1.1, R1): the reset reason and the reset record, read once, and
    // this boot counted for the next record.
    let _reset_reason = board.reset.reason();
    let _previous_boot = board.reset.take_reset_record();
    let _boot = board.reset.note_boot();

    // The installed application, admitted before any state exists.
    let admitted = application::read_and_admit(&mut board);
    let wait_alarm = board.take_wait_alarm();
    let admitted = match admitted {
        Ok(admitted) => admitted,
        Err(reason) => stop(board, wait_alarm, reason),
    };
    let program = admitted.program;
    let config = admitted.record.configuration;

    // Retain mode `none` until the C6 checkpoint driver exists (design §1.2).
    if config.retain_mode != RetainMode::None {
        stop(
            board,
            wait_alarm,
            String::from(
                "the installed application requests a retain mode, which the C6 does not carry out yet",
            ),
        );
    }

    // The record's GPIO lines, claimed by pin name, each output driven to its initial value
    // before it becomes an output (design §1.1, R1).
    let (gpio_inputs, gpio_outputs) = match gpio_lines(&config) {
        Ok(lines) => lines,
        Err(reason) => stop(board, wait_alarm, reason),
    };
    let claimed = board.claim_lines(&gpio_inputs, &gpio_outputs);
    let lines = match claimed {
        Ok(lines) => lines,
        Err(error) => stop(
            board,
            wait_alarm,
            format!("the installed application's GPIO lines cannot be claimed: {error}"),
        ),
    };

    // The hardware watchdog from the record's cycle watchdog timeout (design §2.5).
    let watchdog_ok = board
        .watchdog
        .set_cycle_timeout_ms(cycle_timeout_ms(&config))
        .is_ok();
    if !watchdog_ok {
        stop(
            board,
            wait_alarm,
            String::from("the installed application's watchdog timeout is outside the C6's range"),
        );
    }

    // Prepare the program with the limits its region and the heap allow (design §1.1).
    let limits = device_limits(DeviceMemory {
        region_bytes: STBC_BYTES,
        heap_bytes: memory::HEAP.capacity(),
    });
    let prepared = match PreparedModule::from_bytes(program, limits) {
        Ok(prepared) => prepared,
        Err(error) => stop(
            board,
            wait_alarm,
            format!("the installed application cannot be prepared: {error}"),
        ),
    };
    let prepared: &'static PreparedModule = Box::leak(Box::new(prepared));

    // The loop owns the board's watchdog, clock and wait from here.
    let mut parts = Parts::take(board, wait_alarm);
    let clock = parts.clock;
    let services = Box::leak(Box::new(DeviceServices::new(clock)));
    let settings = CycleSettings {
        cycle_interval: Duration::from_nanos(
            i64::try_from(config.cycle_interval_nanos).unwrap_or(i64::MAX),
        ),
        watchdog: config.watchdog,
        fault_policy: config.fault_policy,
        safe_outputs: config
            .safe_state
            .iter()
            .map(|entry| (entry.address.clone(), entry.value.clone()))
            .collect(),
    };
    let io = GpioIo::new(lines.0, lines.1);
    let cycle = match EngineCycle::start(prepared, services, io, settings) {
        Ok(cycle) => cycle,
        Err(error) => serve_stopped(
            parts,
            format!("the installed application cannot be started: {error}"),
        ),
    };
    let Some(alarm) = parts.wait_alarm.take() else {
        emergency_halt("wait-alarm");
    };
    parts.watchdog.start();
    let platform = Platform::new(
        parts.watchdog,
        config.resource_name.clone(),
        status_settings(),
        config.control_debug_enabled,
    );
    let mut runner = Box::new(Runner::new(
        UsbLink::bind(),
        Wait::new(alarm),
        cycle,
        platform,
        clock,
    ));
    let _ = runner.run();
    emergency_halt("runner")
}

/// The parts the resource loop owns, extracted from the board once the record's lines are claimed.
/// The board's console is dropped with the rest of the board, which does not stop the
/// USB-Serial/JTAG controller the link uses.
struct Parts {
    watchdog: Watchdog,
    clock: ChipClock,
    wait_alarm: Option<esp_hal::timer::systimer::Alarm<'static>>,
}

impl Parts {
    /// Extracts the loop's parts from `board`, holding the clock for the rest of the boot.
    fn take(board: Board, wait_alarm: Option<esp_hal::timer::systimer::Alarm<'static>>) -> Self {
        Self {
            watchdog: board.watchdog,
            clock: ChipClock(Box::leak(Box::new(board.clock))),
            wait_alarm,
        }
    }
}

/// The refusal's path: extract the loop's parts and serve the link Stopped with `reason`
/// (design §1.1, R2).
fn stop(
    board: Board,
    wait_alarm: Option<esp_hal::timer::systimer::Alarm<'static>>,
    reason: String,
) -> ! {
    serve_stopped(Parts::take(board, wait_alarm), reason)
}

/// Runs the resource loop without a program: the resource is Stopped, the loop serves the link
/// and feeds the hardware watchdog, and `restart` is refused with `reason` (design §1.1, R2).
fn serve_stopped(mut parts: Parts, reason: String) -> ! {
    let Some(alarm) = parts.wait_alarm.take() else {
        emergency_halt("wait-alarm");
    };
    parts.watchdog.start();
    let platform = Platform::new(
        parts.watchdog,
        SmolStr::new_static("Controller"),
        status_settings(),
        false,
    );
    // A Stopped loop holds no cycle, so its type is named: the cycle a program would run.
    let mut runner = Box::new(
        Runner::<_, _, EngineCycle<'static, ChipClock, GpioIo>, _, _>::stopped(
            UsbLink::bind(),
            Wait::new(alarm),
            platform,
            parts.clock,
            SmolStr::from(reason),
        ),
    );
    let _ = runner.run();
    emergency_halt("runner-stopped")
}

/// The resource loop's clock: the C6's SYSTIMER through the adapter's [`Monotonic`]. The adapter
/// type is not `Clone`, so the firmware holds it for the boot and copies this reference.
#[derive(Clone, Copy)]
struct ChipClock(&'static Monotonic);

impl MonotonicClock for ChipClock {
    fn now(&self) -> MonotonicInstant {
        MonotonicInstant::from_origin(CoreDuration::from_micros(self.0.micros()))
    }
}

/// The runtime settings `status` reports for a chip: the engine backend, production control and
/// no simulation.
fn status_settings() -> StatusSettings {
    StatusSettings {
        execution_backend: "vm",
        execution_backend_source: "default",
        simulation_enabled: false,
        simulation_time_scale: 1,
        simulation_mode: SmolStr::new_static("production"),
        simulation_warning: SmolStr::default(),
    }
}

/// The cycle watchdog timeout the record configures, in whole milliseconds; a disabled watchdog
/// is zero, which keeps the hardware watchdog at its 4 s minimum (design §2.5).
fn cycle_timeout_ms(config: &Configuration) -> u32 {
    u32::try_from(config.watchdog.timeout.as_millis().max(0)).unwrap_or(u32::MAX)
}

/// The record's `gpio` driver lines, in `io.toml` order. A driver other than `gpio` is refused
/// with its name: the C6 carries out only GPIO until slice T1 (specification 34, TARGET-01).
fn gpio_lines(config: &Configuration) -> Result<(Vec<GpioInput>, Vec<GpioOutput>), String> {
    let mut inputs = Vec::new();
    let mut outputs = Vec::new();
    for driver in &config.io_drivers {
        if !driver.enabled {
            continue;
        }
        if driver.name.as_str() != "gpio" {
            return Err(format!(
                "the installed application uses the {} driver, which the C6 does not carry out yet",
                driver.name
            ));
        }
        inputs.extend(driver.inputs.iter().cloned());
        outputs.extend(driver.outputs.iter().cloned());
    }
    Ok((inputs, outputs))
}

/// The terminal fault path (R4): format the panic's location into a fixed buffer and hand it to
/// the adapter, which drives every configured output to its safe level, records the cause and
/// resets the chip. The handler allocates nothing and never writes to the control link.
#[panic_handler]
fn panic(info: &core::panic::PanicInfo<'_>) -> ! {
    let mut record = LocationRecord {
        bytes: [0u8; LOCATION_BYTES],
        length: 0,
    };
    let written = match info.location() {
        Some(location) => write!(record, "{}:{}", location.file(), location.line()),
        None => write!(record, "panic"),
    };
    // A location that did not fit its buffer reads as `panic`, never as a partial path.
    let text = if written.is_ok() {
        core::str::from_utf8(&record.bytes[..record.length]).unwrap_or("panic")
    } else {
        "panic"
    };
    emergency_halt(text)
}

/// A fixed-size buffer the panic location is formatted into without allocating.
struct LocationRecord {
    bytes: [u8; LOCATION_BYTES],
    length: usize,
}

impl core::fmt::Write for LocationRecord {
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
