//! The STM32U5A9 truST runtime: the shared single-thread resource loop over the installed application,
//! on the U5 chip adapter (specification 34, Appendix D.5; design §6.2, S1b-9u).
//!
//! Boot order (the C6's, specification 34 R2): the clock first; then the monotonic clock and its wait;
//! the identity (DEV_ID, flash size and TZEN) and the reset record, read once, this boot counted; the
//! installed application admitted before any state exists; the watchdog timeout the record needs; the
//! record's pins claimed by name, each output at its safe level; the program prepared, instantiated and
//! started; then the runner serves the control link over USB. A refusal serves the link Stopped with
//! its reason (specification 34, STATE-08). A panic or a fault ends in `halt::emergency_halt`: the
//! outputs to their safe levels, the cause in the reset record, then a reset (R3). It never writes to
//! the link.
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
use alloc::string::String;
use alloc::vec::Vec;

use cortex_m_rt::{entry, exception};
use smol_str::SmolStr;
use stm32u5::stm32u5a9 as pac;
use trust_platform_stm32u5::clock::{self, ClockError};
use trust_platform_stm32u5::gpio::{DigitalIo, Ports};
use trust_platform_stm32u5::reset::ResetControl;
use trust_platform_stm32u5::usb_link;
use trust_platform_stm32u5::wait::{Monotonic, Wait};
use trust_runtime_core::deployment::{Configuration, ControlMode};
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::ports::{MonotonicClock, MonotonicInstant};
use trust_runtime_core::value::Duration;
use trust_runtime_core::vm::PreparedModule;
use trust_runtime_core::watchdog::RetainMode;
use trust_runtime_services::capabilities::LastReset;
use trust_runtime_services::chip::platform::status_settings;
use trust_runtime_services::runner::engine::{
    device_limits, CycleSettings, DeviceMemory, DeviceServices, EngineCycle,
};
use trust_runtime_services::runner::{EngineLiveValues, Runner};

use crate::platform::Platform;
use crate::process_io::{claim, GpioIo};
use crate::profile::{APPLICATION_BYTES, DEVICE_DESCRIPTOR};
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

/// What the boot has taken for the resource loop before the program is known.
struct Parts {
    clock: ChipClock,
    wait: Wait,
    watchdog: Watchdog,
    last_reset: LastReset,
}

/// The heap's count of its allocator operations, which the runtime reads around each scan
/// (specification 34, "The allocation count").
fn allocator_operations() -> u64 {
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

    // Prepare the program with the limits its region and the heap allow (design §1.1).
    let limits = device_limits(DeviceMemory {
        region_bytes: APPLICATION_BYTES,
        heap_bytes: memory::HEAP.capacity(),
    });
    let prepared = match PreparedModule::from_bytes(program, limits) {
        Ok(prepared) => prepared,
        Err(error) => serve_stopped(
            parts,
            format!("the installed application cannot be prepared: {error}"),
        ),
    };
    let prepared: &'static PreparedModule = Box::leak(Box::new(prepared));

    // The Live Values of the installed program (specification 11, "Live Values"; specification 34 §4.3.1).
    let live = match live_values(prepared) {
        Ok(live) => live,
        Err(error) => serve_stopped(
            parts,
            format!("the installed application's Live Values cannot be prepared: {error}"),
        ),
    };

    let services = Box::leak(Box::new(
        DeviceServices::new(parts.clock).with_allocator_operations(Some(allocator_operations)),
    ));
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
    let cycle = match EngineCycle::start(prepared, services, gpio, settings) {
        Ok(cycle) => cycle.with_debugger(),
        Err(error) => serve_stopped(
            parts,
            format!("the installed application cannot be started: {error}"),
        ),
    };

    if let Err(reason) = parts.watchdog.start(timeout) {
        halt::emergency_halt(reason);
    }
    let platform = Platform::new(
        parts.watchdog,
        config.resource_name.clone(),
        status_settings(),
        config.control_debug_enabled,
        config.control_mode,
        live,
        parts.last_reset,
        platform::device_runtime_id(),
    );
    let link = match usb_link::bind(parts.clock) {
        Ok(link) => link,
        Err(_) => halt::emergency_halt("usb-link"),
    };
    let mut runner = Box::new(Runner::new(
        link,
        parts.wait,
        cycle,
        platform,
        parts.clock,
        event_capacity(&config),
    ));
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
        EngineLiveValues::empty(),
        last_reset,
        platform::device_runtime_id(),
    );
    let link = match usb_link::bind(clock) {
        Ok(link) => link,
        Err(_) => halt::emergency_halt("usb-link"),
    };
    // A Stopped loop holds no cycle, so its type is named: the cycle a program would run.
    let mut runner = Box::new(
        Runner::<_, _, EngineCycle<'static, ChipClock, GpioIo>, _, _>::stopped(
            link,
            wait,
            platform,
            clock,
            SmolStr::from(reason),
            default_event_capacity(),
        ),
    );
    let _ = runner.run();
    halt::emergency_halt("runner-stopped")
}

/// The Live Values of the installed program `prepared`: its process-image bindings, each named by its
/// declared root and member path, and its retained declarations (specification 11, "Live Values";
/// specification 34 §4.3.1).
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
