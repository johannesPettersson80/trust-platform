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
mod radio_memory;

use alloc::boxed::Box;
use alloc::format;
use alloc::rc::Rc;
use core::cell::RefCell;
use core::fmt::Write;

use smol_str::SmolStr;
use trust_platform_esp32::compiled_loader::{
    compiled_install_check, CompiledLoader, CompiledProgramCycle,
};
use trust_platform_esp32::loader::{default_event_capacity, program_memory_bytes};
use trust_platform_esp32::process_io::GpioIo;
use trust_platform_esp32::profile::{runtime_id, transfer_session, PROGRAM_STEP_BYTES};
use trust_platform_esp32::{
    bring_up, emergency_halt, end_program_load, mark_program_load, pairing_random, start_scheduler,
    Board, C6Net, Monotonic, NetLink, RadioParts, UsbLink, Wait, Watchdog,
};
use trust_platform_esp32::{C6Code, Platform, ProgramCode};
use trust_runtime_core::ports::{HardwareWatchdog, MonotonicClock, MonotonicInstant, ProgramStore};
use trust_runtime_core::retain::RestartMode;
use trust_runtime_services::chip::network::{ChipNetwork, NetworkFacts};
use trust_runtime_services::chip::platform::status_settings;
use trust_runtime_services::firmware::{FirmwareFacts, FirmwareStore};
use trust_runtime_services::link::LinkPair;
use trust_runtime_services::runner::{unfinished_load_reason, DeviceLiveValues, Runner};

esp_bootloader_esp_idf::esp_app_desc!();

/// Bytes the reset record keeps for a panic's location.
const LOCATION_BYTES: usize = 32;

/// The C6's platform.
type ChipPlatform = Platform<Watchdog>;

/// The program the loader builds: a compiled program whose module runs from the program region, or
/// where its image lies when the region has no room for it.
type ChipCycle = CompiledProgramCycle<ChipClock, GpioIo, C6Code>;

/// The C6's two control links: its USB link and the control connection over its Wi-Fi network
/// (specification 34 ARCH-12).
type ChipLink = LinkPair<UsbLink, NetLink>;

/// The C6's resource loop: its links, the wait, the program the loader builds, the platform and the
/// clock.
type ChipRunner = Runner<ChipLink, Wait, ChipCycle, ChipPlatform, ChipClock>;

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
    // A reset that ended the last boot while it loaded the installed program left that load's mark:
    // this boot does not load the program again (specification 34 TARGET-08).
    let unfinished_load = board.reset.take_program_load();
    let Ok(boot_count) = board.count_boot() else {
        emergency_halt("boot-counter");
    };
    // The reset the previous boot left, kept for the platform's `capabilities` reply (specification
    // 11, the runtime device capabilities contract; specification 34, the fixed-size reset record):
    // its record, or its hardware reason when it left none, with this boot's counter.
    let last_reset = ChipPlatform::boot_last_reset(reset_reason, previous_record, boot_count);
    let unloaded = unfinished_load.then(|| unfinished_load_reason(Some(&last_reset)));

    // The program store: the flash of the two program slots, which the boot reads and an install
    // writes (specification 34 §5.1), and a second handle on the checkpoint slots for the install's
    // retained-value check. The loop is given both whether or not a program runs.
    let Ok(program_flash) = board.take_program_flash() else {
        emergency_halt("program-flash");
    };
    let Ok(checkpoint_reader) = board.checkpoint_reader() else {
        emergency_halt("checkpoint-reader");
    };
    // The firmware slots (specification 34 TARGET-04): the boot reads the choice the boot loader
    // left, whether this firmware runs on trial after an update, and the version of a newer firmware
    // that the boot loader rolled back; the loop writes an update into the slot that does not run.
    let mut firmware = board.firmware();
    let (trial, firmware_facts) = firmware.boot(env!("CARGO_PKG_VERSION"));
    let Ok(firmware_flash) = board.firmware_slots() else {
        emergency_halt("firmware-slots");
    };
    // The device settings, device.toml (specification 34 TARGET-02), in their two sectors, and the
    // pairings (specification 11 §6.9.6) in theirs.
    let settings_flash = board.settings_flash();
    let pairing_flash = board.pairing_flash();
    let firmware_store = FirmwareStore {
        slots: ProgramStore {
            flash: firmware_flash,
            program_budget: PROGRAM_STEP_BYTES,
        },
        device: Box::new(board.firmware()),
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

    // The platform starts with the values of a chip that has no application: it reports `production`
    // without a debug surface, the record's defaults (specification 11 §6.9). The loader sets those of
    // the installed record.
    let mut platform = ChipPlatform::new(
        parts.watchdog,
        SmolStr::new_static("Controller"),
        status_settings(),
        DeviceLiveValues::empty(),
        last_reset,
        runtime_id(parts.identity.mac),
    )
    .with_device_settings(settings_flash);
    platform.set_firmware(firmware_facts.clone());
    // The watchdog runs from here, through every step of the boot: a firmware on trial that hangs
    // before it confirms itself below is reset by it, and the boot loader then boots the firmware
    // before it (specification 34 TARGET-04). The boot's steps feed it: the network's start once
    // it is done, and the program's load between its steps.
    platform.watchdog_mut().start();
    // The network (specification 34 ARCH-12, checklist L3): the Wi-Fi network the device settings
    // name, joined before the program starts, so that its blocks are taken from the heap before the
    // memory for programs is measured. A radio that does not start leaves the chip on its USB link
    // and is reported in `capabilities`.
    let (platform, net) = start_network(parts.radio.take(), platform);
    // The pairings (specification 11 §6.9.6): codes and tokens come from the C6's generator, which
    // is random only while the radio runs, so the chip pairs only while it serves a network.
    let mut platform =
        platform.with_pairings(pairing_flash, net.as_ref().map(|_| pairing_random()));
    HardwareWatchdog::feed(&mut platform);
    // The loader builds the program of the installed container at the boot and again at every
    // `restart` that finds a newer one (specification 34 STATE-08): the same steps in the same order.
    // It also builds the candidate of an online change (specification 34 §11.3).
    let largest_payload =
        usize::try_from(checkpoint_reader.geometry().max_payload_len()).unwrap_or(usize::MAX);
    // An install checks that the newest checkpoint restores into the new image through its retain
    // map (specification 44 §11); the store's own check then admits the image as the kernel does.
    let (loader, retained_check) = (
        CompiledLoader::new(parts.hardware, ProgramCode, clock, heap_room)
            .with_allocator_operations(allocator_operations)
            .with_checkpoint_payload(largest_payload),
        compiled_install_check(checkpoint_reader),
    );

    // The loop is built before any program (specification 34 STATE-08, Appendix D.2): its receive
    // buffer, its event store and its boxes are taken from the heap first, so that no program's blocks
    // leave it without one. The reason it is built with is replaced when it loads the program.
    let link = LinkPair::new(UsbLink::bind(), NetLink::new(net));
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
        .with_firmware(firmware_store)
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
    // A load the reset of the last boot ended is not made again; this one is marked where a reset
    // keeps the mark until it is done (specification 34 TARGET-08).
    match &unloaded {
        Some(reason) => runner.boot_unloaded(reason),
        None => {
            mark_program_load();
            runner.boot(RestartMode::Warm);
            end_program_load();
        }
    }
    // The boot is done: a firmware on trial after an update confirms itself now, so the boot loader
    // keeps booting it (specification 34 TARGET-04). One that panicked before this point reset the
    // chip, and one that hung was reset by the watchdog, and the boot loader then booted the
    // firmware before it.
    #[cfg(feature = "bench-trial-reset")]
    if !firmware_facts.confirmed {
        esp_hal::system::software_reset();
    }
    // The bench's firmware that hangs on trial: the watchdog must reset it (checklist T4).
    #[cfg(feature = "bench-trial-hang")]
    if !firmware_facts.confirmed {
        #[allow(clippy::empty_loop)]
        loop {}
    }
    if !firmware_facts.confirmed && firmware.confirm(&trial).is_ok() {
        let _ = runner.with_platform(|platform| {
            platform.set_firmware(FirmwareFacts {
                confirmed: true,
                ..firmware_facts
            });
        });
    }

    // The loop feeds the watchdog in every pass.
    let _ = runner.run();
    emergency_halt("runner")
}

/// Starts the Wi-Fi network that the stored device settings name: the radio's arena from the heap,
/// the radio's scheduler, the station and the first attempt to join. Without Wi-Fi settings the radio
/// stays off and nothing is taken.
fn start_network(
    radio: Option<RadioParts>,
    mut platform: ChipPlatform,
) -> (ChipPlatform, Option<Rc<RefCell<C6Net>>>) {
    let (Some(radio), Some(settings)) = (radio, platform.stored_settings()) else {
        return (platform, None);
    };
    let Some(wifi) = settings.network.wifi.clone() else {
        return (platform, None);
    };
    radio_memory::initialize();
    start_scheduler(radio.scheduler_alarm, radio.scheduler_interrupt);
    match bring_up(radio.wifi, &settings, &wifi, &radio_memory::RADIO_HEAP) {
        Ok(net) => {
            let network: Rc<RefCell<dyn ChipNetwork>> = net.clone();
            (platform.with_network(network), Some(net))
        }
        Err(failure) => {
            platform.set_network_failure(NetworkFacts {
                interface: "wifi",
                link_up: false,
                address: None,
                control_port: None,
                error: Some(format!("the Wi-Fi radio did not start: {failure:?}")),
                driver_memory: None,
                time: None,
            });
            (platform, None)
        }
    }
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
        cut: false,
    };
    let written = match info.location() {
        Some(location) => write!(record, "{}:{}", location.file(), location.line()),
        None => write!(record, "panic"),
    };
    // A location longer than the record keeps its end, the file and the line, after `...`.
    if record.cut {
        record.bytes[..3].copy_from_slice(b"...");
    }
    let text = if written.is_ok() {
        core::str::from_utf8(&record.bytes[..record.length]).unwrap_or("panic")
    } else {
        "panic"
    };
    emergency_halt(text)
}

/// A fixed-size buffer the panic location is formatted into without allocating; it keeps the last
/// [`LOCATION_BYTES`] bytes of a longer location.
struct LocationRecord {
    bytes: [u8; LOCATION_BYTES],
    length: usize,
    /// Whether bytes at the start were dropped.
    cut: bool,
}

impl core::fmt::Write for LocationRecord {
    fn write_str(&mut self, text: &str) -> core::fmt::Result {
        for &byte in text.as_bytes() {
            if self.length == LOCATION_BYTES {
                self.bytes.copy_within(1.., 0);
                self.length -= 1;
                self.cut = true;
            }
            self.bytes[self.length] = byte;
            self.length += 1;
        }
        Ok(())
    }
}
