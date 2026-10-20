//! The ESP32-C6 truST runtime: the shared single-thread resource loop over the installed
//! application, on the C6 adapter (design §1.1 and §1.2, specification 34).
//!
//! Boot order (design §1.1, R1): the hardware first (peripherals, clock, the reset reason and the
//! reset record read once, this boot counted in the flash); then the program store and the installed
//! application (the header of each of the two program slots read, the committed container with the
//! highest install sequence read once, and admitted against the C6's descriptor); then the program
//! built by the C6's loader, which a `restart` uses again to load a newer container in place: the
//! program prepared, the record's GPIO lines claimed by pin name, each output driven to its initial
//! value before it becomes an output, and the program constructed and started (with `mode = "file"`,
//! warm from the newest committed checkpoint); then the runner serves the control link over USB. The
//! same loader builds the candidate of an online change (`bytecode.reload`, specification 34 §11.3):
//! it is prepared within the heap's room less a reserve while the running program stays installed,
//! and the commit hands it the running program's lines.
//!
//! With no valid application, or one the device refuses, the same loop runs Stopped and serves the
//! link with the refusal's text (design §1.1, R2). The loop is given the transfer session of the boot,
//! the program store and the loader in either case, so that a program can be installed over the link:
//! the install commits in steps in the Stopped state, and a `restart` loads it in place (specification
//! 34 §11.2, STATE-08); the next boot selects it too. A panic drives every configured output to its
//! safe level, writes the cause and location into the reset record, and resets the chip; it never
//! writes to the link (R4).
#![no_std]
#![no_main]

extern crate alloc;

mod application;
mod memory;

use alloc::boxed::Box;
use core::fmt::Write;
use core::time::Duration as CoreDuration;

use smol_str::SmolStr;
use trust_platform_esp32::loader::{default_event_capacity, install_check, Esp32Loader};
use trust_platform_esp32::process_io::GpioIo;
use trust_platform_esp32::profile::{runtime_id, transfer_session, PROGRAM_STEP_BYTES, STBC_BYTES};
use trust_platform_esp32::program_memory::ProgramCycle;
use trust_platform_esp32::{emergency_halt, Board, Monotonic, Platform, UsbLink, Wait, Watchdog};
use trust_runtime_core::deployment::ControlMode;
use trust_runtime_core::ports::{MonotonicClock, MonotonicInstant, ProgramStore};
use trust_runtime_core::retain::RestartMode;
use trust_runtime_services::chip::platform::status_settings;
use trust_runtime_services::runner::engine::DeviceMemory;
use trust_runtime_services::runner::{EngineLiveValues, Loaded, ProgramLoader, Runner};

use application::Refused;

esp_bootloader_esp_idf::esp_app_desc!();

/// Bytes the reset record keeps for a panic's location.
const LOCATION_BYTES: usize = 32;

/// The bytes of the heap that a container can take now (specification 34 §5.1).
fn heap_room() -> usize {
    memory::HEAP.remaining()
}

/// The heap's count of its allocator operations, which the runtime reads around each scan
/// (specification 34, "The allocation count").
fn allocator_operations() -> u64 {
    memory::HEAP.operations()
}

#[esp_hal::main]
fn main() -> ! {
    memory::initialize();
    let mut board = match Board::take() {
        Ok(board) => board,
        Err(_) => emergency_halt("board-init"),
    };
    // Hardware first (design §1.1, R1): the reset reason and the reset record, read once, and this
    // boot counted in the flash, where the count survives a loss of power (specification 34 PORT-05).
    let reset_reason = board.reset.reason();
    let previous_record = board.reset.take_reset_record();
    let Ok(boot_count) = board.count_boot() else {
        emergency_halt("boot-counter");
    };
    // The reset the previous boot left, kept for the platform's `capabilities` reply (specification
    // 11, the runtime device capabilities contract; specification 34, the fixed-size reset record):
    // its record, or its hardware reason when it left none, with this boot's counter.
    let last_reset =
        Platform::<Watchdog>::boot_last_reset(reset_reason, previous_record, boot_count);

    // The program store: the flash of the two program slots, which the boot reads and an install
    // writes (specification 34 §5.1), and a second handle on the checkpoint slots for the install's
    // retained-value check. The loop is given both whether or not a program runs.
    let Ok(program_flash) = board.take_program_flash() else {
        emergency_halt("program-flash");
    };
    let Ok(checkpoint_reader) = board.checkpoint_reader() else {
        emergency_halt("checkpoint-reader");
    };
    // A second handle that only reads, for the loader: an online change takes the newest checkpoint
    // through it while the running program holds the writer.
    let Ok(change_reader) = board.checkpoint_reader() else {
        emergency_halt("checkpoint-reader");
    };
    let mut parts = board.into_parts();
    let mut store = ProgramStore {
        flash: program_flash,
        program_budget: PROGRAM_STEP_BYTES,
    };
    let clock = ChipClock(Box::leak(Box::new(parts.clock)));
    let Some(alarm) = parts.wait_alarm.take() else {
        emergency_halt("wait-alarm");
    };

    // The installed application: the committed container with the highest install sequence,
    // admitted before any state exists.
    let installed = application::boot(&mut store);

    // The platform starts with the values of a chip that has no application: it reports `production`,
    // the record's default (specification 11 §6.9). The loader sets those of the installed record.
    let mut platform = Platform::new(
        parts.watchdog,
        SmolStr::new_static("Controller"),
        status_settings(),
        false,
        ControlMode::Production,
        EngineLiveValues::empty(),
        last_reset,
        runtime_id(parts.identity.mac),
    );
    // The loader builds the program of the installed container here and again at every `restart` that
    // finds a newer one (specification 34 STATE-08): the same steps in the same order. It also builds
    // the candidate of an online change (specification 34 §11.3).
    let device_memory = DeviceMemory {
        region_bytes: STBC_BYTES,
        heap_bytes: memory::HEAP.capacity(),
    };
    let mut loader = Esp32Loader::new(
        parts.hardware,
        clock,
        device_memory,
        heap_room,
        change_reader,
    )
    .with_allocator_operations(allocator_operations);
    let (program, loaded) = match installed {
        Ok(booted) => {
            let sequence = booted.sequence;
            (
                loader.load(&mut platform, booted, RestartMode::Warm),
                Some(sequence),
            )
        }
        Err(Refused { reason, sequence }) => (Err(reason), sequence),
    };

    // The watchdog runs from here: building the program took far more time than any pass of the loop,
    // and the loop feeds it in every pass.
    platform.watchdog_mut().start();
    let link = UsbLink::bind();
    let wait = Wait::new(alarm);
    let runner = match program {
        Ok(Loaded {
            cycle,
            events,
            event_capacity,
        }) => {
            let runner = Runner::new(link, wait, cycle, platform, clock, event_capacity);
            // The boot's warm start dropped or migrated retained values: the runner records them as a
            // restart's events are recorded, at the program's time zero.
            runner.record_boot_retain_events(events);
            runner
        }
        // A Stopped loop holds no cycle, so its type is named: the cycle a program would run.
        Err(reason) => Runner::<_, _, ProgramCycle<ChipClock, GpioIo>, _, _>::stopped(
            link,
            wait,
            platform,
            clock,
            SmolStr::from(reason),
            default_event_capacity(),
        ),
    };
    let mut runner = Box::new(
        runner
            .with_transfers(transfer_session(boot_count))
            .with_program_store(
                store,
                install_check(checkpoint_reader, device_memory, heap_room),
            )
            .with_program_loader(loader, loaded),
    );
    let _ = runner.run();
    emergency_halt("runner")
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
