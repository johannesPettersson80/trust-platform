//! The ESP32-C6 truST runtime: the shared single-thread resource loop over the installed
//! application, on the C6 adapter (design §1.1 and §1.2, specification 34).
//!
//! Boot order (design §1.1, R1): the hardware first (peripherals, clock, the reset reason and the
//! reset record read once, this boot counted in the flash); then the resource loop, built Stopped with
//! the transfer session of the boot, the program store and the C6's loader before any program, so that
//! its own blocks (the receive buffer, the event store, the boxes) are taken from the heap first
//! (specification 34 STATE-08, Appendix D.2); then the memory the C6 has for programs, measured with
//! the loop built; then the installed application, which the loop loads as a `restart` that finds a
//! newer container does: the header of each of the two program slots read, the committed container
//! with the highest install sequence read once and admitted against the C6's descriptor, its compiled
//! program image checked and its module copied into the program region (specification 44 §10), the
//! record's GPIO lines claimed by pin name, each output driven to its initial value before it becomes
//! an output, and the program started (with `mode = "file"`, warm from the newest committed
//! checkpoint); then the runner serves the control link over USB. The same loader builds the candidate
//! of an online change (`bytecode.reload`, specification 34 §11.3, specification 44 §12): its image is
//! checked and its areas fit in the heap's room less a reserve while the running program stays
//! installed, and the commit hands it the running program's lines.
//!
//! With no valid application, or one that cannot start, the same loop stays Stopped and serves the
//! link with the reason (design §1.1, R2), so that a program can be installed over the link: the
//! install commits in steps in the Stopped state, and a `restart` loads it in place (specification 34
//! §11.2, STATE-08); the next boot selects it too. A panic drives every configured output to its
//! safe level, writes the cause and location into the reset record, and resets the chip; it never
//! writes to the link (R4).
#![no_std]
#![no_main]

extern crate alloc;

mod memcmp;
mod memcpy;
mod memory;
mod memset;

use alloc::boxed::Box;
use core::fmt::Write;

use smol_str::SmolStr;
use trust_platform_esp32::compiled_loader::{
    compiled_install_check, CompiledLoader, CompiledProgramCycle,
};
use trust_platform_esp32::loader::{default_event_capacity, program_memory_bytes};
use trust_platform_esp32::process_io::GpioIo;
use trust_platform_esp32::profile::{runtime_id, transfer_session, PROGRAM_STEP_BYTES};
use trust_platform_esp32::{emergency_halt, Board, Monotonic, UsbLink, Wait, Watchdog};
use trust_platform_esp32::{C6Code, Platform, ProgramCode};
use trust_runtime_core::deployment::ControlMode;
use trust_runtime_core::ports::{MonotonicClock, MonotonicInstant, ProgramStore};
use trust_runtime_core::retain::RestartMode;
use trust_runtime_services::chip::platform::status_settings;
use trust_runtime_services::runner::{DeviceLiveValues, Runner};

esp_bootloader_esp_idf::esp_app_desc!();

/// Bytes the reset record keeps for a panic's location.
const LOCATION_BYTES: usize = 32;

/// The C6's platform.
type ChipPlatform = Platform<Watchdog>;

/// The program the loader builds: a compiled program whose module runs from the program region, or
/// where its image lies when the region has no room for it.
type ChipCycle = CompiledProgramCycle<ChipClock, GpioIo, C6Code>;

/// The C6's resource loop: the USB link, the wait, the program the loader builds, the platform and
/// the clock.
type ChipRunner = Runner<UsbLink, Wait, ChipCycle, ChipPlatform, ChipClock>;

/// The bytes of the heap that a container can take now (specification 34 §5.1).
fn heap_room() -> usize {
    memory::HEAP.remaining()
}

/// The heap's count of its allocator operations, which the runtime reads around each scan
/// (specification 34, "The allocation count").
fn allocator_operations() -> u32 {
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
    let last_reset = ChipPlatform::boot_last_reset(reset_reason, previous_record, boot_count);

    // The program store: the flash of the two program slots, which the boot reads and an install
    // writes (specification 34 §5.1), and a second handle on the checkpoint slots for the install's
    // retained-value check. The loop is given both whether or not a program runs.
    let Ok(program_flash) = board.take_program_flash() else {
        emergency_halt("program-flash");
    };
    let Ok(checkpoint_reader) = board.checkpoint_reader() else {
        emergency_halt("checkpoint-reader");
    };
    let mut parts = board.into_parts();
    let store = ProgramStore {
        flash: program_flash,
        program_budget: PROGRAM_STEP_BYTES,
    };
    let clock = ChipClock(Box::leak(Box::new(parts.clock)));
    let Some(alarm) = parts.wait_alarm.take() else {
        emergency_halt("wait-alarm");
    };

    // The platform starts with the values of a chip that has no application: it reports `production`,
    // the record's default (specification 11 §6.9). The loader sets those of the installed record.
    let platform = ChipPlatform::new(
        parts.watchdog,
        SmolStr::new_static("Controller"),
        status_settings(),
        false,
        ControlMode::Production,
        DeviceLiveValues::empty(),
        last_reset,
        runtime_id(parts.identity.mac),
    );
    // The loader builds the program of the installed container at the boot and again at every
    // `restart` that finds a newer one (specification 34 STATE-08): the same steps in the same order.
    // It also builds the candidate of an online change (specification 34 §11.3).
    let largest_payload =
        usize::try_from(checkpoint_reader.geometry().max_payload_len()).unwrap_or(usize::MAX);
    // An install checks that the newest checkpoint restores into the new image through its retain
    // map (specification 44 §11); the store's own check then admits the image as the kernel does.
    let (loader, retained_check) = (
        CompiledLoader::new(parts.hardware, ProgramCode, clock, heap_room)
            .with_allocator_operations(allocator_operations),
        compiled_install_check(checkpoint_reader),
    );

    // The loop is built before any program (specification 34 STATE-08, Appendix D.2): its receive
    // buffer, its event store and its boxes are taken from the heap first, so that no program's blocks
    // leave it without one. The reason it is built with is replaced when it loads the program.
    let link = UsbLink::bind();
    let wait = Wait::new(alarm);
    let mut runner: Box<ChipRunner> = Box::new(
        ChipRunner::stopped(
            link,
            wait,
            platform,
            clock,
            SmolStr::new_static("the installed program is being loaded"),
            default_event_capacity(),
        )
        .with_transfers(transfer_session(boot_count))
        .with_program_store(store, retained_check)
        .with_program_loader(loader, None),
    );
    // The memory the C6 has for a program (specification 34 MEM-12, Appendix D.2): the heap's free bytes
    // with the loop built and before any container is read, less the working memory of an online change
    // and the checkpoint store's buffer. The descriptor reports it, and a program whose memory figure is
    // more is refused at its install and at its load.
    let program_memory = program_memory_bytes(heap_room(), largest_payload);
    let _ = runner.with_platform(|platform| platform.set_program_memory(program_memory));
    // The installed application: the committed container with the highest install sequence, admitted,
    // loaded and started warm. One that cannot start leaves the loop Stopped with the reason.
    runner.boot(RestartMode::Warm);

    // The watchdog runs from here: building the program took far more time than any pass of the loop,
    // and the loop feeds it in every pass.
    let _ = runner.with_platform(|platform| platform.watchdog_mut().start());
    let _ = runner.run();
    emergency_halt("runner")
}

/// The resource loop's clock: the C6's SYSTIMER through the adapter's [`Monotonic`]. The adapter
/// type is not `Clone`, so the firmware holds it for the boot and copies this reference.
#[derive(Clone, Copy)]
struct ChipClock(&'static Monotonic);

impl MonotonicClock for ChipClock {
    fn now(&self) -> MonotonicInstant {
        MonotonicInstant::from_micros(self.0.micros())
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
