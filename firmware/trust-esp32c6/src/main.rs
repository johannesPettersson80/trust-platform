//! The ESP32-C6 truST runtime: the shared single-thread resource loop over the installed
//! application, on the C6 adapter (design §1.1 and §1.2, specification 34).
//!
//! Boot order (design §1.1, R1): the hardware first (peripherals, clock, the reset reason and the
//! reset record read once, this boot counted); then the installed application (its header read,
//! only its total length copied into the heap, and `installation::admit` against the C6's
//! descriptor); then the record's GPIO lines claimed by pin name, each output driven to its
//! initial value before it becomes an output; then the program prepared, instantiated and started
//! (with `mode = "file"`, warm from the newest committed checkpoint); then the runner serves the
//! control link over USB.
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
use trust_platform_esp32::process_io::{bind_lines, GpioIo};
use trust_platform_esp32::{
    emergency_halt,
    profile::{DEVICE_DESCRIPTOR, STBC_BYTES},
    Board, Monotonic, Platform, UsbLink, Wait, Watchdog,
};
use trust_runtime_core::deployment::{Configuration, ControlMode};
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::ports::{CheckpointStorage, MonotonicClock, MonotonicInstant};
use trust_runtime_core::retain::checkpoint::CheckpointIdentity;
use trust_runtime_core::retain::RestartMode;
use trust_runtime_core::value::Duration;
use trust_runtime_core::vm::{PreparedModule, RetainApplyEvent};
use trust_runtime_core::watchdog::RetainMode;
use trust_runtime_services::capabilities::LastReset;
use trust_runtime_services::chip::platform::status_settings;
use trust_runtime_services::chip::process_io::record_lines;
use trust_runtime_services::runner::engine::{
    device_limits, CycleSettings, DeviceMemory, DeviceServices, EngineCycle,
};
use trust_runtime_services::runner::{
    EngineLiveValues, ResourceCycle, RetainConfig, RetainSource, Runner,
};

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
    let reset_reason = board.reset.reason();
    let previous_record = board.reset.take_reset_record();
    let boot_count = board.reset.note_boot();
    // The reset the previous boot left, kept for the platform's `capabilities` reply (specification
    // 11, the runtime device capabilities contract; specification 34, the fixed-size reset record):
    // its record, or its hardware reason when it left none, with this boot's counter.
    let last_reset =
        Platform::<Watchdog>::boot_last_reset(reset_reason, previous_record, boot_count);

    // The installed application, admitted before any state exists.
    let admitted = application::read_and_admit(&mut board);
    let wait_alarm = board.take_wait_alarm();
    let admitted = match admitted {
        Ok(admitted) => admitted,
        Err(reason) => stop(board, wait_alarm, last_reset, reason),
    };
    let program = admitted.program;
    // The identity every checkpoint of this program carries (specification 34 §10.1).
    let identity = admitted.record.checkpoint_identity();
    let config = admitted.record.configuration;

    // The record's GPIO lines, claimed by pin name, each output driven to its initial value
    // before it becomes an output (design §1.1, R1).
    let (gpio_inputs, gpio_outputs) = match record_lines(&config, "C6") {
        Ok(lines) => lines,
        Err(reason) => stop(board, wait_alarm, last_reset, reason),
    };
    let claimed = board.claim_lines(&gpio_inputs, &gpio_outputs);
    let lines = match claimed {
        Ok(lines) => lines,
        Err(error) => stop(
            board,
            wait_alarm,
            last_reset,
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
            last_reset,
            String::from("the installed application's watchdog timeout is outside the C6's range"),
        );
    }

    // The chip's checkpoint flash, which the platform hands to the installed program (specification
    // 34 §10.1). It is taken after the application is read, which uses the same flash driver.
    let checkpoint_flash = match board.take_checkpoint_flash() {
        Ok(checkpoint_flash) => checkpoint_flash,
        Err(error) => stop(
            board,
            wait_alarm,
            last_reset,
            format!("the chip's checkpoint flash is not available: {error:?}"),
        ),
    };

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
            last_reset,
            format!("the installed application cannot be prepared: {error}"),
        ),
    };
    let prepared: &'static PreparedModule = Box::leak(Box::new(prepared));

    // The Live Values of the installed program, for the `io.*`, `set` and `var.*` operations over
    // the link (specification 11, "Live Values"; specification 34 §4.3.1).
    let live = match live_values(prepared) {
        Ok(live) => live,
        Err(error) => stop(
            board,
            wait_alarm,
            last_reset,
            format!("the installed application's Live Values cannot be prepared: {error}"),
        ),
    };

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
    let io = bind_lines(lines.0, lines.1);
    let Some(alarm) = parts.wait_alarm.take() else {
        emergency_halt("wait-alarm");
    };
    // The watchdog runs from here: the platform takes it, and starting the program takes far less
    // than its 4 s interval.
    parts.watchdog.start();
    let mut platform = Platform::new(
        parts.watchdog,
        config.resource_name.clone(),
        status_settings(),
        config.control_debug_enabled,
        config.control_mode,
        live,
        last_reset.clone(),
        parts.runtime_id.clone(),
    )
    .with_checkpoint(config.retain_mode, checkpoint_flash);
    let (cycle, boot_events) = match start_cycle(
        &mut platform,
        prepared,
        services,
        io,
        settings,
        identity,
        config.retain_save_interval,
    ) {
        Ok(started) => started,
        // The platform keeps the watchdog, which the stopped loop takes back.
        Err(reason) => serve_stopped(
            Parts {
                watchdog: platform.into_watchdog(),
                clock,
                wait_alarm: Some(alarm),
                runtime_id: parts.runtime_id.clone(),
            },
            last_reset,
            reason,
        ),
    };
    let mut runner = Box::new(Runner::new(
        UsbLink::bind(),
        Wait::new(alarm),
        cycle,
        platform,
        clock,
        event_capacity(&config),
    ));
    // The boot's warm start dropped or migrated retained values: the runner records them as a
    // restart's events are recorded, at the program's time zero.
    runner.record_boot_retain_events(boot_events);
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
    /// The chip's stable runtime ID, from its base MAC address (specification 11, `status`).
    runtime_id: SmolStr,
}

impl Parts {
    /// Extracts the loop's parts from `board`, holding the clock for the rest of the boot.
    fn take(board: Board, wait_alarm: Option<esp_hal::timer::systimer::Alarm<'static>>) -> Self {
        Self {
            watchdog: board.watchdog,
            clock: ChipClock(Box::leak(Box::new(board.clock))),
            wait_alarm,
            runtime_id: trust_platform_esp32::profile::runtime_id(board.identity.mac),
        }
    }
}

/// The refusal's path: extract the loop's parts and serve the link Stopped with `reason`
/// (design §1.1, R2).
fn stop(
    board: Board,
    wait_alarm: Option<esp_hal::timer::systimer::Alarm<'static>>,
    last_reset: LastReset,
    reason: String,
) -> ! {
    let mut parts = Parts::take(board, wait_alarm);
    parts.watchdog.start();
    serve_stopped(parts, last_reset, reason)
}

/// Runs the resource loop without a program: the resource is Stopped, the loop serves the link
/// and feeds the hardware watchdog, and `restart` is refused with `reason` (design §1.1, R2). The
/// caller has started the watchdog.
fn serve_stopped(mut parts: Parts, last_reset: LastReset, reason: String) -> ! {
    let Some(alarm) = parts.wait_alarm.take() else {
        emergency_halt("wait-alarm");
    };
    let platform = Platform::new(
        parts.watchdog,
        SmolStr::new_static("Controller"),
        status_settings(),
        false,
        // A device that has not admitted an application has no record: it reports `production`,
        // the record's default (specification 11 §6.9).
        ControlMode::Production,
        EngineLiveValues::empty(),
        last_reset,
        parts.runtime_id.clone(),
    );
    // A Stopped loop holds no cycle, so its type is named: the cycle a program would run.
    let mut runner = Box::new(
        Runner::<_, _, EngineCycle<'static, ChipClock, GpioIo>, _, _>::stopped(
            UsbLink::bind(),
            Wait::new(alarm),
            platform,
            parts.clock,
            SmolStr::from(reason),
            default_event_capacity(),
        ),
    );
    let _ = runner.run();
    emergency_halt("runner-stopped")
}

/// Starts the installed program on the engine (specification 34 §10 and STATE-08). A platform with a
/// checkpoint store saves the retained values into it: the program starts cold, and a warm start then
/// restores the newest committed checkpoint, as a power-on or a reset does. Without a store the
/// program starts cold, with its retained values at their declared initial values. The debugger may
/// hold the program's scans at its statement starts (specification 13, "Safe Points"; specification
/// 34, STATE-07): the prepared module and the services are `'static`, as the debugger needs.
///
/// # Errors
///
/// The program's cycle, and the retain events of its warm start for the runner to record.
///
/// # Errors
///
/// The refusal's text: the program cannot start, the checkpoint log cannot be read, or the newest
/// committed checkpoint is damaged, which fails the warm start as a damaged retain file does
/// (specification 11 §6.7).
fn start_cycle(
    platform: &mut Platform<Watchdog>,
    prepared: &'static PreparedModule,
    services: &'static DeviceServices<ChipClock>,
    io: GpioIo,
    settings: CycleSettings,
    identity: CheckpointIdentity,
    save_interval: Duration,
) -> Result<
    (
        EngineCycle<'static, ChipClock, GpioIo>,
        Vec<RetainApplyEvent>,
    ),
    String,
> {
    let Some(store) = platform.take_checkpoint_store() else {
        let cycle = EngineCycle::start(prepared, services, io, settings)
            .map_err(|error| format!("the installed application cannot be started: {error}"))?
            .with_debugger();
        return Ok((cycle, Vec::new()));
    };
    // The largest payload one slot takes: a retained image is saved whole or not at all.
    let image_bytes = usize::try_from(store.flash.geometry().max_payload_len()).map_err(|_| {
        String::from("the checkpoint's largest payload exceeds the platform's address range")
    })?;
    let retain = RetainConfig {
        mode: RetainMode::File,
        store,
        identity,
        save_interval,
        image_bytes,
    };
    let mut cycle = EngineCycle::with_retain(prepared, services, io, settings, retain)
        .map_err(|error| {
            format!("the installed application's checkpoint cannot be opened: {error}")
        })?
        .with_debugger();
    let events = cycle
        .restart(RestartMode::Warm, RetainSource::Checkpoint)
        .map_err(|error| {
            format!("the retained values cannot be restored from the checkpoint: {error}")
        })?;
    Ok((cycle, events))
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

/// The Live Values of the installed program `prepared` (specification 11, "Live Values";
/// specification 34 §4.3.1): its process-image bindings, each named by its declared root/member
/// path as trust-runtime names it, and its retained declarations.
///
/// # Errors
///
/// The [`RuntimeError`] of an admitted binding whose declared name cannot be resolved, and of two
/// retained declarations that project to one label.
fn live_values(prepared: &PreparedModule) -> Result<EngineLiveValues, RuntimeError> {
    let names = prepared
        .io_bindings()
        .iter()
        .map(|binding| {
            prepared
                .persistent_reference_name(binding.ref_idx)
                .map(Some)
        })
        .collect::<Result<Vec<_>, _>>()?;
    EngineLiveValues::for_prepared(prepared, names)
}

/// The cycle watchdog timeout the record configures, in whole milliseconds; a disabled watchdog
/// is zero, which keeps the hardware watchdog at its 4 s minimum (design §2.5).
fn cycle_timeout_ms(config: &Configuration) -> u32 {
    u32::try_from(config.watchdog.timeout.as_millis().max(0)).unwrap_or(u32::MAX)
}

/// The newest events the event store keeps: the record's section-7 capacity, or the C6's default
/// when the record sets none (0). Admission has already refused a capacity above the chip's
/// maximum (specification 34 §5.1.1 step 7; specification 11, the event store).
fn event_capacity(config: &Configuration) -> usize {
    if config.event_capacity == 0 {
        default_event_capacity()
    } else {
        usize::try_from(config.event_capacity).unwrap_or(usize::MAX)
    }
}

/// The C6's default event-store capacity, from its descriptor (`chips/esp32c6.toml`).
fn default_event_capacity() -> usize {
    usize::try_from(DEVICE_DESCRIPTOR.event_store_default).unwrap_or(usize::MAX)
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
