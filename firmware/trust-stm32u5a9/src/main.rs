//! The STM32U5A9 truST runtime: the shared single-thread resource loop over the installed application,
//! on the U5 chip adapter (specification 34, Appendix D.5; design §6.2, S1b-9u).
//!
//! Boot order (the C6's, specification 34 R2): the clock first; then the monotonic clock and its wait;
//! the identity (DEV_ID, flash size and TZEN) and the reset record, read once, this boot counted; the
//! installed application admitted before any state exists; the watchdog timeout the record needs; the
//! record's pins claimed by name, each output at its safe level; the compiled program image checked as
//! the kernel checks it, its module run where it lies in the internal flash (specification 44 §10.2,
//! §10.3), and the program started; then the runner, with the transfer session of the boot
//! (specification 34, PORT-05), serves the control link over USB. A refusal serves the link Stopped
//! with its reason (specification 34, STATE-08). A panic or a fault ends in `halt::emergency_halt`:
//! the outputs to their safe levels, the cause in the reset record, then a reset (R3). It never writes
//! to the link.
#![no_std]
#![no_main]

extern crate alloc;

mod application;
mod halt;
mod identity;
mod memory;
mod platform;
mod process_io;
mod profile;
mod watchdog;

use alloc::boxed::Box;
use alloc::format;
use alloc::string::{String, ToString};

use cortex_m_rt::{entry, exception};
use smol_str::SmolStr;
use stm32u5::stm32u5a9 as pac;
use trust_platform_stm32u5::clock::{self, ClockError};
use trust_platform_stm32u5::gpio::{DigitalIo, Ports};
use trust_platform_stm32u5::reset::ResetControl;
use trust_platform_stm32u5::usb_link;
use trust_platform_stm32u5::wait::{Monotonic, Wait};
use trust_plc_abi::kernel::InPlace;
use trust_runtime_core::deployment::{Configuration, ControlMode};
use trust_runtime_core::native::live::io_bindings;
use trust_runtime_core::native::program::{self as native, NativeProgram};
use trust_runtime_core::native::KERNEL_FUNCTIONS;
use trust_runtime_core::ports::{MonotonicClock, MonotonicInstant};
use trust_runtime_core::value::Duration;
use trust_runtime_core::watchdog::RetainMode;
use trust_runtime_services::capabilities::LastReset;
use trust_runtime_services::chip::platform::status_settings;
use trust_runtime_services::link::DeviceLineLink;
use trust_runtime_services::runner::{CompiledCycle, CycleSettings, DeviceLiveValues, Runner};

use crate::platform::Platform;
use crate::process_io::{claim, GpioIo};
use crate::profile::{
    transfer_session, DEVICE_DESCRIPTOR, NATIVE_TARGET, PROGRAM_MEMORY_RESERVE_BYTES,
};
use crate::watchdog::{timeout_for, Watchdog, STOPPED_TIMEOUT_MS};

/// The resource loop's clock: the chip's TIM2 microsecond counter through the adapter's [`Monotonic`].
/// The adapter's type is not `Clone`, so the boot holds it for the whole run and copies this reference.
#[derive(Clone, Copy)]
struct ChipClock(&'static Monotonic);

impl MonotonicClock for ChipClock {
    fn now(&self) -> MonotonicInstant {
        self.0.now()
    }
}

/// The STM32U5A9's resource loop: the USB link, the wait, the installed program, the platform and the
/// clock.
type ChipRunner = Runner<
    usb_link::UsbLink<ChipClock>,
    Wait,
    CompiledCycle<'static, ChipClock, GpioIo, InPlace>,
    Platform,
    ChipClock,
>;

/// What the boot has taken for the resource loop before the program is known.
struct Parts {
    clock: ChipClock,
    wait: Wait,
    watchdog: Watchdog,
    last_reset: LastReset,
    /// This boot's count, the boot number of the transfer ids (specification 34, PORT-05).
    boot_count: u32,
}

/// The heap's count of its allocator operations, which the runtime reads around each scan
/// (specification 34, "The allocation count").
fn allocator_operations() -> u32 {
    memory::HEAP.operations()
}

#[entry]
fn main() -> ! {
    memory::initialize();
    let Some(p) = pac::Peripherals::take() else {
        halt::emergency_halt("peripherals-taken")
    };
    // Hardware first (design §1.1, R1): the clock, then the monotonic clock and the wait it serves. The
    // clock report (HSE or HSI16) is not carried over the link yet.
    if let Err(ClockError::Timeout(step)) = clock::start(&p.RCC, &p.PWR, &p.FLASH) {
        halt::emergency_halt(step);
    }
    let chip_clock = ChipClock(Box::leak(Box::new(Monotonic::start())));
    let wait = Wait::start();
    let mut reset = ResetControl::take(&p.RCC, &p.PWR, p.TAMP);
    let watchdog_reset = reset.reason().independent_watchdog();
    let previous_record = reset.take_reset_record();
    let boot_count = reset.note_boot();
    let mut parts = Parts {
        clock: chip_clock,
        wait,
        watchdog: Watchdog::new(p.IWDG),
        last_reset: Platform::boot_last_reset(watchdog_reset, previous_record, boot_count),
        boot_count,
    };

    // The identity: a part that is not this chip, or that has TrustZone enabled, does not run (design
    // §1.6); the link reports why.
    if let Err(reason) = identity::check(identity::read(&p.DBGMCU, &p.FLASH)) {
        serve_stopped(parts, String::from(reason));
    }

    // The installed application, admitted before any state exists.
    let admitted = match application::read_and_admit() {
        Ok(admitted) => admitted,
        Err(reason) => serve_stopped(parts, reason),
    };
    let program = admitted.program;
    let config = admitted.record.configuration;

    // Retain mode `none` until the chip's checkpoint driver exists (design §4.4).
    if config.retain_mode != RetainMode::None {
        serve_stopped(
            parts,
            String::from(
                "the installed application requests a retain mode, which the STM32U5A9 does not carry out yet",
            ),
        );
    }

    // The hardware watchdog's timeout, refused before any pin is claimed (specification 34, TIME-04).
    let timeout = match timeout_for(cycle_timeout_ms(&config)) {
        Ok(timeout) => timeout,
        Err(reason) => serve_stopped(parts, String::from(reason)),
    };

    // The record's GPIO lines, claimed by pin name, each output latched to its safe level first.
    let ports = Ports {
        a: p.GPIOA,
        b: p.GPIOB,
        c: p.GPIOC,
        d: p.GPIOD,
        e: p.GPIOE,
        f: p.GPIOF,
        g: p.GPIOG,
        h: p.GPIOH,
        i: p.GPIOI,
        j: p.GPIOJ,
    };
    let gpio = match claim(DigitalIo::new(ports, &p.RCC), &config) {
        Ok(gpio) => gpio,
        Err(reason) => serve_stopped(parts, reason),
    };

    // The memory the STM32U5A9 has for a program (specification 34 MEM-12, Appendix D.5; specification
    // 44 §10.4): the heap's free bytes before the program is loaded, its container being in the flash,
    // less what the chip keeps beside any program: its resource loop, with its event store at its
    // maximum of events, and the reserve for its replies and the program's Live Values.
    let events = usize::try_from(DEVICE_DESCRIPTOR.event_store_max).unwrap_or(usize::MAX);
    let program_memory = memory::HEAP
        .remaining()
        .saturating_sub(ChipRunner::heap_bytes(
            DeviceLineLink::MAX_FRAME_LEN,
            events,
        ))
        .saturating_sub(PROGRAM_MEMORY_RESERVE_BYTES);

    // The image is checked where it lies, against the chip's CPU, its memory for programs and its
    // task stack (specification 44 §10.2), and its module runs there: the internal flash is executable
    // memory with no cache that the program shares (§10.3).
    let device = NATIVE_TARGET.device(u64::try_from(program_memory).ok());
    let admitted = match native::admit(program, &device) {
        Ok(admitted) => admitted,
        Err(refusal) => serve_stopped(
            parts,
            format!("the installed application cannot be loaded: {refusal}"),
        ),
    };
    let (_, module) = admitted.module();
    // SAFETY: `module` is the module of the image the kernel admitted, in the application region of
    // the internal flash, which the CPU executes; the firmware never writes or erases the region, so
    // it stays mapped and unchanged for the whole boot (`application.rs`).
    let code = unsafe { InPlace::new(module) };
    let program = match NativeProgram::new(admitted, code, &device, &KERNEL_FUNCTIONS) {
        Ok(program) => program,
        Err(refusal) => serve_stopped(
            parts,
            format!("the installed application cannot be loaded: {refusal}"),
        ),
    };

    // The Live Values of the installed program (specification 11, "Live Values"; specification 44
    // §13.3): its process-image bindings, each named by its variable.
    let live = match io_bindings(&program.image())
        .map_err(|error| error.to_string())
        .and_then(|(bindings, names)| {
            DeviceLiveValues::for_compiled(bindings, names).map_err(|error| error.to_string())
        }) {
        Ok(live) => live,
        Err(error) => serve_stopped(
            parts,
            format!("the installed application's Live Values cannot be bound: {error}"),
        ),
    };

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
    let cycle = match CompiledCycle::start(program, gpio, settings, parts.clock) {
        Ok(cycle) => cycle.with_allocator_operations(allocator_operations),
        Err(error) => serve_stopped(
            parts,
            format!("the installed application cannot be started: {error}"),
        ),
    };

    if let Err(reason) = parts.watchdog.start(timeout) {
        halt::emergency_halt(reason);
    }
    let mut platform = Platform::new(
        parts.watchdog,
        config.resource_name.clone(),
        status_settings(),
        config.control_debug_enabled,
        config.control_mode,
        live,
        parts.last_reset,
        platform::device_runtime_id(),
    );
    platform.set_program_memory(program_memory);
    let link = match usb_link::bind(parts.clock) {
        Ok(link) => link,
        Err(_) => halt::emergency_halt("usb-link"),
    };
    // The loop holds a reply longer than the link's frame in the transfer session of this boot, and the
    // client reads it in parts, as from trust-runtime on a computer (specification 34, PORT-05).
    let mut runner: Box<ChipRunner> = Box::new(
        Runner::new(
            link,
            parts.wait,
            cycle,
            platform,
            parts.clock,
            event_capacity(&config),
        )
        .with_transfers(transfer_session(parts.boot_count)),
    );
    let _ = runner.run();
    halt::emergency_halt("runner")
}

/// Serves the control link with no program: the resource is Stopped, every request answers as a stopped
/// resource does, the watchdog is fed, and `restart` is refused with `reason` (specification 34, STATE-08).
fn serve_stopped(parts: Parts, reason: String) -> ! {
    let Parts {
        clock,
        wait,
        mut watchdog,
        last_reset,
        boot_count,
    } = parts;
    if let Err(error) = watchdog.start(STOPPED_TIMEOUT_MS) {
        halt::emergency_halt(error);
    }
    let platform = Platform::new(
        watchdog,
        SmolStr::new_static("Controller"),
        status_settings(),
        false,
        // A device that has not admitted an application has no record: it reports `production`, the
        // record's default (specification 11 §6.9).
        ControlMode::Production,
        DeviceLiveValues::empty(),
        last_reset,
        platform::device_runtime_id(),
    );
    let link = match usb_link::bind(clock) {
        Ok(link) => link,
        Err(_) => halt::emergency_halt("usb-link"),
    };
    // A Stopped loop holds no cycle, so its type is named: the loop a program would run in. It holds a
    // long reply in the transfer session of this boot as the running loop does (PORT-05).
    let mut runner = Box::new(
        ChipRunner::stopped(
            link,
            wait,
            platform,
            clock,
            SmolStr::from(reason),
            default_event_capacity(),
        )
        .with_transfers(transfer_session(boot_count)),
    );
    let _ = runner.run();
    halt::emergency_halt("runner-stopped")
}

/// The cycle watchdog timeout the record configures, in whole milliseconds; a disabled watchdog is zero.
fn cycle_timeout_ms(config: &Configuration) -> u32 {
    u32::try_from(config.watchdog.timeout.as_millis().max(0)).unwrap_or(u32::MAX)
}

/// The newest events the event store keeps: the record's capacity, or the chip's default when the record
/// sets none. Admission has already refused a capacity above the chip's maximum.
fn event_capacity(config: &Configuration) -> usize {
    if config.event_capacity == 0 {
        default_event_capacity()
    } else {
        usize::try_from(config.event_capacity).unwrap_or(usize::MAX)
    }
}

/// The chip's default event-store capacity, from its descriptor (`[events]` of chips/stm32u5a9.toml).
fn default_event_capacity() -> usize {
    usize::try_from(DEVICE_DESCRIPTOR.event_store_default).unwrap_or(usize::MAX)
}

#[exception]
unsafe fn HardFault(_: &cortex_m_rt::ExceptionFrame) -> ! {
    halt::emergency_halt("hard-fault")
}
