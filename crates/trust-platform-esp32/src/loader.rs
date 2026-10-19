//! The C6's program loader (specification 34 STATE-08 and §11.2): the one place that builds a running
//! program out of an admitted container, for the boot and for a `restart` that loads a newer
//! container in place. The boot and the load take the same steps in the same order, so a program
//! starts the same way after a reset and after a `restart`.
//!
//! The order keeps the chip's memory at one program and one container, and keeps its outputs as they
//! are until a program is ready to take them:
//!
//! 1. the program is prepared from the container's bytes, with the limits the application region and
//!    the heap allow, and the container's bytes are freed at once;
//! 2. its Live Values are bound, and the hardware watchdog is set from the record's cycle watchdog;
//! 3. the checkpoint flash is taken when the record keeps retained values;
//! 4. the record's GPIO lines are claimed by pin name, each output driven to its initial value before
//!    it becomes an output (STATE-01), and the program is constructed over the lines, the module and
//!    the services; the cycle owns the module and the services and frees them when it is dropped
//!    ([`ProgramMemory`]);
//! 5. a `warm` start restores the retained values of the newest checkpoint (specification 11 §6.7).
//!
//! The lines are claimed last: a program that cannot be prepared leaves the outputs as the stop of the
//! program before it left them, at their safe values. The loader feeds the hardware watchdog between
//! the steps (TIME-04) and writes the values of the record on the platform only when the program has
//! started.

use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use trust_runtime_core::deployment::Configuration;
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::ports::{CheckpointStore, HardwareWatchdog, MonotonicClock};
use trust_runtime_core::retain::checkpoint::{CheckpointFlash, CheckpointIdentity};
use trust_runtime_core::retain::RestartMode;
use trust_runtime_core::value::Duration;
use trust_runtime_core::vm::PreparedModule;
use trust_runtime_core::watchdog::RetainMode;
use trust_runtime_services::chip::platform::ProgramValues;
use trust_runtime_services::loading::select::Booted;
use trust_runtime_services::runner::engine::{
    device_limits, CycleSettings, DeviceMemory, DeviceServices, ProcessIo,
};
use trust_runtime_services::runner::{
    EngineLiveValues, Loaded, ProgramLoader, ResourceCycle, RetainConfig, RetainSource,
};

use crate::platform::Platform;
use crate::profile::{CHECKPOINT_STEP_BYTES, DEVICE_DESCRIPTOR};
use crate::program_memory::{ProgramCycle, ProgramMemory};

/// What the loader needs of the chip's hardware: the lines of a record and the checkpoint flash.
pub trait ProgramHardware {
    /// The process I/O that the lines of a record make. Dropping it frees the pins of its lines, so the
    /// program loaded next claims them (specification 34 STATE-08).
    type Io: ProcessIo;

    /// Claims the lines of `config` by pin name, in `io.toml` order, drives each output to its initial
    /// value before it becomes an output (specification 34 STATE-01 and TARGET-01) and binds them to the
    /// process image.
    ///
    /// # Errors
    ///
    /// The reason a line cannot be claimed. The pins of the lines before it are free again.
    fn claim_lines(&mut self, config: &Configuration) -> Result<Self::Io, String>;

    /// The checkpoint flash for a program whose record keeps retained values: one writer at a time, free
    /// again when the program that held it is dropped.
    ///
    /// # Errors
    ///
    /// The reason the checkpoint flash is not available.
    fn checkpoint_flash(&mut self) -> Result<Box<dyn CheckpointFlash>, String>;
}

/// A hardware watchdog whose interval the loader sets from the cycle watchdog of a record.
pub trait CycleWatchdog {
    /// Sets the interval from the cycle watchdog timeout `cycle_timeout_ms` of a record: the larger of
    /// 4 s and the timeout plus 2 s (specification 34 TIME-04). Returns whether the interval is in the
    /// watchdog's range; when it is not, the interval did not change.
    fn apply_cycle_timeout_ms(&mut self, cycle_timeout_ms: u32) -> bool;
}

/// The loader of the C6: the hardware it claims from, the clock its programs run on, the memory its
/// programs are prepared in, and the room the heap has for a container.
pub struct Esp32Loader<H, C> {
    hardware: H,
    clock: C,
    memory: DeviceMemory,
    room: fn() -> usize,
}

impl<H, C> Esp32Loader<H, C> {
    /// A loader over `hardware` and `clock`. `memory` bounds the preparation of a program (the
    /// application region and the heap), and `room` says how many bytes of the heap a container can take
    /// now (specification 34 §5.1).
    #[must_use]
    pub fn new(hardware: H, clock: C, memory: DeviceMemory, room: fn() -> usize) -> Self {
        Self {
            hardware,
            clock,
            memory,
            room,
        }
    }
}

impl<H: ProgramHardware, C> Esp32Loader<H, C> {
    /// The checkpoint store of a program whose record keeps retained values, with the identity that its
    /// checkpoints carry.
    fn retain_config(
        &mut self,
        config: &Configuration,
        identity: CheckpointIdentity,
    ) -> Result<RetainConfig, String> {
        let flash = self.hardware.checkpoint_flash()?;
        // The largest payload one slot takes: a retained image is saved whole or not at all.
        let image_bytes = usize::try_from(flash.geometry().max_payload_len()).map_err(|_| {
            String::from("the checkpoint's largest payload exceeds the platform's address range")
        })?;
        Ok(RetainConfig {
            mode: RetainMode::File,
            store: CheckpointStore {
                flash,
                program_budget: CHECKPOINT_STEP_BYTES,
            },
            identity,
            save_interval: config.retain_save_interval,
            image_bytes,
        })
    }
}

impl<H, C, W> ProgramLoader<Platform<W>, ProgramCycle<C, H::Io>> for Esp32Loader<H, C>
where
    H: ProgramHardware,
    C: MonotonicClock + Copy + 'static,
    W: CycleWatchdog + HardwareWatchdog,
{
    fn room(&self) -> usize {
        (self.room)()
    }

    fn load(
        &mut self,
        platform: &mut Platform<W>,
        booted: Booted,
        mode: RestartMode,
    ) -> Result<Loaded<ProgramCycle<C, H::Io>>, String> {
        let Booted {
            program, record, ..
        } = booted;
        let identity = record.checkpoint_identity();
        let config = record.configuration;

        // The program is prepared first, and the container's bytes are freed as soon as it is.
        let prepared = PreparedModule::from_bytes(&program, device_limits(self.memory))
            .map_err(|error| format!("the installed application cannot be prepared: {error}"))?;
        drop(program);
        platform.feed();
        let live = live_values(&prepared).map_err(|error| {
            format!("the installed application's Live Values cannot be prepared: {error}")
        })?;
        if !platform
            .watchdog_mut()
            .apply_cycle_timeout_ms(cycle_timeout_ms(&config))
        {
            return Err(String::from(
                "the installed application's watchdog timeout is outside the C6's range",
            ));
        }
        platform.feed();
        let retains = config.retain_mode == RetainMode::File;
        let retain = if retains {
            Some(self.retain_config(&config, identity)?)
        } else {
            None
        };

        // The lines are claimed last, right before the program starts.
        let io = self.hardware.claim_lines(&config)?;
        let mut cycle = ProgramMemory::start(
            prepared,
            DeviceServices::new(self.clock),
            io,
            settings(&config),
            retain,
        )
        .map_err(|error| start_failure(retains, &error))?;
        // A new program starts cold; a warm start restores the newest checkpoint over it, as a boot's
        // warm start does. A program that keeps no retained values has nothing to restore.
        let events = if retains && mode == RestartMode::Warm {
            cycle
                .restart(RestartMode::Warm, RetainSource::Checkpoint)
                .map_err(|error| {
                    format!("the retained values cannot be restored from the checkpoint: {error}")
                })?
        } else {
            Vec::new()
        };
        platform.feed();

        let event_capacity = event_capacity(&config);
        platform.set_program(ProgramValues {
            resource_name: config.resource_name,
            debug_enabled: config.control_debug_enabled,
            control_mode: config.control_mode,
            live,
        });
        Ok(Loaded {
            cycle,
            events,
            event_capacity,
        })
    }
}

/// The refusal of a program that cannot start: its checkpoint log cannot be opened when the record
/// keeps retained values, and otherwise the program itself cannot be constructed.
fn start_failure(retains: bool, error: &RuntimeError) -> String {
    if retains {
        format!("the installed application's checkpoint cannot be opened: {error}")
    } else {
        format!("the installed application cannot be started: {error}")
    }
}

/// What the deployment configures for the program besides its code (specification 34 §5.1 and
/// TARGET-08): the cycle interval, the watchdog and fault policies, and the safe value of each output.
fn settings(config: &Configuration) -> CycleSettings {
    CycleSettings {
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
    }
}

/// The Live Values of the program `prepared` (specification 11, "Live Values"; specification 34
/// §4.3.1): its process-image bindings, each named by its declared root/member path as trust-runtime
/// names it, and its retained declarations.
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

/// The cycle watchdog timeout the record configures, in whole milliseconds; a disabled watchdog is
/// zero, which keeps the hardware watchdog at its 4 s minimum (design §2.5).
#[must_use]
pub fn cycle_timeout_ms(config: &Configuration) -> u32 {
    u32::try_from(config.watchdog.timeout.as_millis().max(0)).unwrap_or(u32::MAX)
}

/// The newest events the event store keeps: the record's section-7 capacity, or the C6's default when
/// the record sets none (0). Admission has already refused a capacity above the chip's maximum
/// (specification 34 §5.1.1 step 7; specification 11, the event store).
#[must_use]
pub fn event_capacity(config: &Configuration) -> usize {
    if config.event_capacity == 0 {
        default_event_capacity()
    } else {
        usize::try_from(config.event_capacity).unwrap_or(usize::MAX)
    }
}

/// The C6's default event-store capacity, from its descriptor (`chips/esp32c6.toml`).
#[must_use]
pub fn default_event_capacity() -> usize {
    usize::try_from(DEVICE_DESCRIPTOR.event_store_default).unwrap_or(usize::MAX)
}
