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
//!
//! The same loader builds the candidate of an online change (specification 34 §11.3), while the
//! running program stays installed and keeps its lines. The candidate is prepared with the limits that
//! the heap's room allows now, less the working memory of the change
//! ([`ONLINE_CHANGE_WORKING_MEMORY_BYTES`]), and checked against the lines of the running program's
//! record. It is then built from the running program's state by the engine's replacement, as a computer
//! builds the candidate of its online change: over the running program's settings and an empty process
//! I/O, so that it claims no pin, it continues the running program's installation generation, keeps
//! its logical time, process images and task schedule, and keeps the retained values the running
//! program holds, with the retained function-block instances and references live when it declares the
//! same retained storage. It reads no checkpoint. Last, it is checked against the running program's
//! safe state. The commit hands the running program's lines and checkpoint store to the candidate,
//! which the loop then makes its program. The shared order is in `trust_runtime_services::runner`; the
//! loader supplies the C6's steps.

use alloc::borrow::Cow;
use alloc::boxed::Box;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use trust_runtime_core::deployment::{admit_memory, Configuration};
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::ports::{CheckpointStore, HardwareWatchdog, MonotonicClock};
use trust_runtime_core::retain::checkpoint::{CheckpointFlash, CheckpointIdentity};
use trust_runtime_core::retain::RestartMode;
use trust_runtime_core::value::Duration;
use trust_runtime_core::vm::PreparedModule;
use trust_runtime_core::watchdog::RetainMode;
use trust_runtime_services::chip::platform::ProgramValues;
use trust_runtime_services::chip::process_io::record_lines;
use trust_runtime_services::installation::Admitted;
use trust_runtime_services::loading::select::Booted;
use trust_runtime_services::ports::StatusPort;
use trust_runtime_services::runner::engine::{
    device_limits, CycleSettings, DeviceMemory, DeviceServices, ProcessIo,
};
use trust_runtime_services::runner::{
    address_text, check_install, newest_checkpoint_payload_len, prepare_candidate, stack_shortage,
    too_deep, unwired_binding, Candidate, EngineLiveValues, Loaded, ProgramLoader, ResourceCycle,
    RetainConfig, RetainSource,
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

/// The bytes of the heap that an online change and the check of an install keep free for the working
/// memory of preparing and constructing a program, beyond the program itself (specification 34 §11.3,
/// UPDATE-06 and Appendix D.2). A change's program has the limits of the heap's room now less these
/// bytes; an install's check also keeps the payload of the newest checkpoint when the new record keeps
/// retained values ([`install_reserve`]). What either takes at its peak beyond the two programs stays
/// inside the heap.
///
/// The size comes from `tests/online_change_heap.rs`. With a counting allocator it changes the A4
/// program to itself and to three other programs of the repository, and the retained-blocks program
/// to itself, with a record that keeps no retained values and with one that does, and reads the most
/// the heap held at once beyond the running program and the candidate: at most 3,994 bytes on the
/// host (20 October 2026). Twice that, for the fragmentation of a first-fit heap that a host cannot
/// measure, is 7,988 bytes, rounded up to 8 KiB. The host's pointers are 64 bits wide and the C6's 32, so the host's
/// figures are the larger ones; the bench replaces them (specification 34, Appendix D.2).
pub const ONLINE_CHANGE_WORKING_MEMORY_BYTES: usize = 8 * 1024;

/// The memory the C6 has for a program (specification 34 MEM-12, Appendix D.2), which its descriptor
/// reports and its admission compares a program's memory figure with: the heap's `free` bytes once
/// the resource loop is built, with the program loader and the retained-value check the firmware
/// gives it, and before any container is read, less what the chip keeps beside any program besides
/// its loop. That is the working memory of an online change
/// ([`ONLINE_CHANGE_WORKING_MEMORY_BYTES`]), which also holds the program's Live Values and services,
/// and the checkpoint store's payload buffer, the `largest_payload` that a checkpoint slot takes,
/// which a warm start also reads the newest checkpoint into.
#[must_use]
pub fn program_memory_bytes(free: usize, largest_payload: usize) -> usize {
    free.saturating_sub(ONLINE_CHANGE_WORKING_MEMORY_BYTES.saturating_add(largest_payload))
}

/// The bytes of the heap that the check of an install keeps free (specification 34 §11.2, decision
/// G13): the working memory, [`ONLINE_CHANGE_WORKING_MEMORY_BYTES`], always, and, when the new
/// program's record keeps retained values (`retains`), the length of the payload of the newest
/// committed checkpoint, which the check's warm start reads whole into the heap beside the running
/// program and the new one. The length is read from the record header of the checkpoint in
/// `checkpoints`, the bytes the check will really read, and is 0 when the log holds no committed
/// checkpoint. When the header cannot say, the reserve holds the largest payload that a slot takes.
/// The checkpoint slots are only read.
#[must_use]
pub fn install_reserve<F: CheckpointFlash + ?Sized>(retains: bool, checkpoints: &mut F) -> usize {
    if !retains {
        return ONLINE_CHANGE_WORKING_MEMORY_BYTES;
    }
    let payload = newest_checkpoint_payload_len(checkpoints).unwrap_or_else(|_| {
        usize::try_from(checkpoints.geometry().max_payload_len()).unwrap_or(usize::MAX)
    });
    ONLINE_CHANGE_WORKING_MEMORY_BYTES.saturating_add(payload)
}

/// What the loader keeps of the program that runs: the configuration of its record. An online change
/// brings no record, so the candidate runs over this one (specification 34 §11.3).
struct Running {
    config: Configuration,
}

/// The loader of the C6: the hardware it claims from, the clock its programs run on, the memory its
/// programs are prepared in and the room the heap has for a container.
pub struct Esp32Loader<H, C> {
    hardware: H,
    clock: C,
    memory: DeviceMemory,
    room: fn() -> usize,
    running: Option<Running>,
    /// The heap's count of its allocator operations, in 32 bits that wrap, which each program's
    /// services read around a scan (specification 34, "The allocation count").
    allocator_operations: Option<fn() -> u32>,
}

impl<H, C> Esp32Loader<H, C> {
    /// A loader over `hardware` and `clock`. `memory` bounds the preparation of a program (the
    /// application region and the heap), and `room` says how many bytes of the heap a container can
    /// take now (specification 34 §5.1).
    #[must_use]
    pub fn new(hardware: H, clock: C, memory: DeviceMemory, room: fn() -> usize) -> Self {
        Self {
            hardware,
            clock,
            memory,
            room,
            running: None,
            allocator_operations: None,
        }
    }

    /// This loader, whose programs count their scans' allocator operations with `operations`, the
    /// heap's count, which must allocate nothing (specification 34, "The allocation count").
    #[must_use]
    pub fn with_allocator_operations(mut self, operations: fn() -> u32) -> Self {
        self.allocator_operations = Some(operations);
        self
    }
}

/// The C6's check of an install (specification 34 §11.2, decision G13): the new program is prepared
/// with the limits that the heap allows now, its room less the reserve of the check
/// ([`install_reserve`], for the new program's record), so that a program the heap cannot hold is
/// refused with `insufficient memory` and its numbers instead of resetting the chip, and then its warm
/// start is tried with the newest checkpoint, which `checkpoints` only reads: the writer belongs to the
/// program that runs (specification 34 §10.1). The firmware gives the loop this check with its program
/// store ([`Runner::with_program_store`](trust_runtime_services::runner::Runner::with_program_store)).
pub fn install_check(
    mut checkpoints: Box<dyn CheckpointFlash>,
    memory: DeviceMemory,
    room: fn() -> usize,
) -> impl FnMut(&Admitted<'_>) -> Result<(), RuntimeError> {
    move |admitted| {
        let retains = admitted.record.configuration.retain_mode == RetainMode::File;
        let reserve = install_reserve(retains, &mut *checkpoints);
        check_install(&mut *checkpoints, admitted, memory, room(), reserve)
    }
}

/// The checkpoint store of a program whose record keeps retained values, over the checkpoint flash of
/// `hardware`, with the identity that its checkpoints carry: an interpreted program's and a compiled
/// program's alike (specification 34 §10; specification 44 §11).
pub(crate) fn retain_config<H: ProgramHardware>(
    hardware: &mut H,
    config: &Configuration,
    identity: CheckpointIdentity,
) -> Result<RetainConfig, String> {
    let flash = hardware.checkpoint_flash()?;
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
        // A program that fails to load leaves no record of a running program.
        self.running = None;

        // The program is prepared first. The C6 maps its slot, so the program's bytes lie in flash,
        // where the prepared program reads its declarations and initializers (specification 34
        // MEM-14); bytes that a boot read into memory are decoded and freed as soon as it is.
        let bytes = program.len();
        let limits = device_limits(self.memory);
        let prepared = match program {
            Cow::Borrowed(mapped) => PreparedModule::from_mapped(mapped, limits),
            Cow::Owned(read) => PreparedModule::from_bytes(&read, limits),
        }
        .map_err(|error| format!("the installed application cannot be prepared: {error}"))?;
        platform.feed();
        // It calls no deeper than the C6's main stack holds, before anything of it is constructed
        // (specification 34 MEM-03).
        if let Some(depth) = too_deep(&prepared, self.memory.call_depth) {
            return Err(format!(
                "the installed application cannot be loaded: {}",
                stack_shortage("a program", bytes, depth, self.memory.call_depth)
            ));
        }
        // Its memory figure fits the memory the C6 has for programs, before anything of it is
        // constructed (specification 34 MEM-12).
        let retains = config.retain_mode == RetainMode::File;
        admit_memory(
            &prepared.memory_figure(),
            retains,
            &platform.device_descriptor(),
        )
        .map_err(|error| format!("the installed application cannot be loaded: {error}"))?;
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
        let retain = if retains {
            Some(retain_config(&mut self.hardware, &config, identity)?)
        } else {
            None
        };

        // The lines are claimed last, right before the program starts.
        let io = self.hardware.claim_lines(&config)?;
        let mut cycle = ProgramMemory::start(
            prepared,
            DeviceServices::new(self.clock).with_allocator_operations(self.allocator_operations),
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
        self.running = Some(Running {
            config: config.clone(),
        });
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

    fn prepare_change(
        &mut self,
        platform: &mut Platform<W>,
        running: &mut ProgramCycle<C, H::Io>,
        program: Vec<u8>,
    ) -> Result<Candidate<ProgramCycle<C, H::Io>>, String> {
        let record = self
            .running
            .as_ref()
            .ok_or_else(|| String::from("the running program's record is not known"))?;

        // The memory check (UPDATE-06): the limits are those of the heap's room now, less the working
        // memory of the change, and the upload is freed as soon as the module is prepared. The change
        // reads no checkpoint, so it keeps no room for one.
        let retains = record.config.retain_mode == RetainMode::File;
        let (prepared, budget) = prepare_candidate(
            program,
            self.memory,
            (self.room)(),
            ONLINE_CHANGE_WORKING_MEMORY_BYTES,
        )?;
        platform.feed();
        // Its memory figure fits the heap that remains for it beside the running program
        // (specification 34 MEM-12).
        if let Some(refusal) = budget.figure_refusal(prepared.memory_figure().total(retains)) {
            return Err(refusal);
        }

        // The candidate's bindings are checked against the lines of the running program's record.
        let (inputs, outputs) = record_lines(&record.config, "C6")?;
        if let Some(address) = unwired_binding(prepared.io_bindings(), &inputs, &outputs) {
            return Err(format!(
                "the candidate binds {}, which the running program's record gives no line",
                address_text(&address)
            ));
        }
        let live = live_values(&prepared).map_err(|error| budget.refusal(&error))?;

        // The candidate is built from the running program's state by the engine's replacement, the
        // body that a computer's online change runs: it continues the running program's generation,
        // keeps its logical time, process images and task schedule, and keeps the retained values the
        // running program holds, with the retained function-block instances and references live when
        // it declares the same retained storage; no checkpoint is applied (OC-1). It is constructed
        // with the running program's settings and an empty process I/O: an empty configuration claims
        // no pin, and the commit hands it the running program's lines. It lives in memory of its own,
        // so the running program's is freed with that program, whenever it is dropped.
        let io = self.hardware.claim_lines(&Configuration::default())?;
        let settings = running.settings().clone();
        let retained_payload = prepared
            .retained_payload_bytes()
            .map_err(|error| budget.refusal(&error))?;
        let (mut cycle, events) = ProgramMemory::replace(
            running,
            prepared,
            DeviceServices::new(self.clock).with_allocator_operations(self.allocator_operations),
            io,
            settings,
        )
        .map_err(|error| budget.refusal(&error))?;
        platform.feed();

        // The running program's safe state must be valid in the candidate, as a computer checks its
        // I/O policy against a replacement.
        cycle
            .check_safe_outputs()
            .map_err(|error| error.to_string())?;

        // The checkpoint store, which the commit hands the candidate, gets room for the candidate's
        // retained values now, so that the candidate's first capture grows nothing (specification 34
        // MEM-09).
        running
            .reserve_retain(retained_payload)
            .map_err(|_| budget.needs_more())?;

        // A client that names an instance by its identity after the change names the generation that
        // the change installed.
        live.set_generation(cycle.generation());
        Ok(Candidate {
            cycle,
            live,
            events,
        })
    }

    fn commit_change(
        &mut self,
        platform: &mut Platform<W>,
        running: &mut ProgramCycle<C, H::Io>,
        candidate: &mut Candidate<ProgramCycle<C, H::Io>>,
    ) {
        // The lines and the checkpoint store change places with the candidate's empty ones: no pin is
        // released or claimed, and no output is written.
        running.hand_over_to(&mut candidate.cycle);
        if let Some(record) = &self.running {
            platform.set_program(ProgramValues {
                resource_name: record.config.resource_name.clone(),
                debug_enabled: record.config.control_debug_enabled,
                control_mode: record.config.control_mode,
                live: candidate.live.clone(),
            });
        }
    }
}

/// The refusal of a program that cannot start: its checkpoint log cannot be opened when the record
/// keeps retained values, and otherwise the program itself cannot be constructed.
pub(crate) fn start_failure(retains: bool, error: &RuntimeError) -> String {
    if retains {
        format!("the installed application's checkpoint cannot be opened: {error}")
    } else {
        format!("the installed application cannot be started: {error}")
    }
}

/// What the deployment configures for the program besides its code (specification 34 §5.1 and
/// TARGET-08): the cycle interval, the watchdog and fault policies, and the safe value of each output.
pub(crate) fn settings(config: &Configuration) -> CycleSettings {
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
