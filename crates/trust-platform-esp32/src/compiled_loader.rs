//! The C6's loader of compiled programs (specification 44 §8, §10; specification 34 STATE-08 and
//! §11.2): it builds the running program out of an admitted container whose program is a compiled
//! program image, for the boot and for a `restart` that loads a newer container in place, in the same
//! steps and the same order, so a program starts the same way after a reset and after a `restart`
//! ([`crate::loader`] holds what it needs of the chip and takes from a record):
//!
//! 1. the image is checked where it lies in the mapped slot, against the C6's CPU, its memory for
//!    programs and its task stack (specification 44 §10.2), and its module is put where the CPU runs
//!    it ([`ModuleMemory`]): the C6 copies it into its program code region, and runs a module larger
//!    than that region where it lies in the slot (§10.3);
//! 2. the program's areas are allocated, its Live Values bound to its process-image bindings, and the
//!    hardware watchdog set from the record's cycle watchdog;
//! 3. the record's GPIO lines are claimed by pin name, each output driven to its initial value before
//!    it becomes an output (STATE-01), and the program starts cold; a program whose record keeps
//!    retained values opens the checkpoint log, and a warm start restores the newest checkpoint over
//!    the cold start through the image's retain map (specification 44 §11).
//!
//! The lines are claimed last, so an image that is refused leaves the outputs as the stop of the
//! program before it left them.
//!
//! An online change (§12.3) writes no flash: the new image stays where its upload lies, in the
//! heap's program region, for as long as the changed program runs ([`ChangedImage`]). The change is
//! checked first, its image, its memory beside the running program, its bindings against the
//! running program's lines; then its module runs where it lies, beside the running program's, which
//! stays where it is, its cold-init entry runs and it takes the running program's values (§12.2). A
//! refusal drops the candidate and leaves the running program as it was.

use alloc::borrow::Cow;
use alloc::boxed::Box;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use trust_runtime_core::deployment::Configuration;
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::native::image::Image;
use trust_runtime_core::native::live::io_bindings;
use trust_runtime_core::native::program::{self as native, ModuleCode, NativeProgram};
// The kernel's functions with the C6's wall clock (specification 44 §8.2); a host build of this
// crate, for its tests, has the core's.
#[cfg(all(target_arch = "riscv32", target_os = "none"))]
use crate::hardware::KERNEL_FUNCTIONS;
#[cfg(not(all(target_arch = "riscv32", target_os = "none")))]
use trust_runtime_core::native::KERNEL_FUNCTIONS;
use trust_runtime_core::ports::{HardwareWatchdog, MonotonicClock};
use trust_runtime_core::retain::checkpoint::CheckpointFlash;
use trust_runtime_core::retain::RestartMode;
use trust_runtime_core::watchdog::RetainMode;
use trust_runtime_services::chip::platform::ProgramValues;
use trust_runtime_services::chip::process_io::record_lines;
use trust_runtime_services::chip::watchdog::{cycle_timeout_ms, CycleWatchdog};
use trust_runtime_services::installation::{Admitted, ProgramFormat};
use trust_runtime_services::loading::select::Booted;
use trust_runtime_services::ports::StatusPort;
use trust_runtime_services::runner::{
    address_text, check_image_retained_values, unwired_binding, Candidate, ChangeMemory,
    CompiledCycle, DeviceLiveValues, Loaded, ProgramLoader, ResourceCycle, RetainSource,
};
use trust_runtime_services::settings::RuntimeSettings;

use crate::loader::{
    event_capacity, program_memory_bytes, retain_config, settings, start_failure, ProgramHardware,
    ONLINE_CHANGE_WORKING_MEMORY_BYTES,
};
use crate::platform::Platform;
use crate::profile::NATIVE_TARGET;

mod changed_image;
pub use changed_image::ChangedImage;

/// The cycle of a compiled program whose module runs where `K` put it.
pub type CompiledProgramCycle<C, I, K> = CompiledCycle<'static, C, I, K>;

/// The C6's check of an install of a compiled program (specification 34 §11.2, decision G13;
/// specification 44 §11): the retained values of the newest checkpoint restore into the new image,
/// as its warm start will restore them. `checkpoints` is only read: the writer belongs to the
/// program that runs (specification 34 §10.1). The firmware gives the loop this check with its
/// program store.
pub fn compiled_install_check(
    mut checkpoints: Box<dyn CheckpointFlash>,
) -> impl FnMut(&Admitted<'_>) -> Result<(), RuntimeError> {
    move |admitted| check_image_retained_values(&mut *checkpoints, admitted)
}

/// Where a device puts a compiled program's module so that its CPU runs it (specification 44 §10.3).
pub trait ModuleMemory {
    /// The module as the kernel calls it.
    type Code: ModuleCode;

    /// Puts `module`, the code and read-only data of an admitted image as they lie in the mapped
    /// slot, where the CPU runs them. The memory it takes is free again when the code is dropped.
    fn place(&mut self, module: &'static [u8]) -> Self::Code;

    /// Puts the module of an online change, which lies in the heap `image`, where the CPU runs it,
    /// beside the running program's module, which stays where it is (specification 44 §12.3, step
    /// 4). The code owns `image`, so the image lives as long as the program.
    ///
    /// # Errors
    ///
    /// The refusal of a module the device has no room to run.
    fn place_change(
        &mut self,
        module: &'static [u8],
        image: ChangedImage,
    ) -> Result<Self::Code, String>;
}

/// The loader of compiled programs: the hardware it claims from, where it puts their modules, the
/// clock its programs run on and the room the heap has for a container.
pub struct CompiledLoader<H, M, C> {
    hardware: H,
    modules: M,
    clock: C,
    room: fn() -> usize,
    /// The configuration of the running program's record, which an online change keeps, since it
    /// brings no record (specification 34 §11.3).
    running: Option<Configuration>,
    /// The heap's count of its allocator operations, which each program's cycle reads around its
    /// scans (specification 34, "The allocation count").
    allocator_operations: Option<fn() -> u32>,
    /// The checkpoint store's largest payload, which the memory for programs keeps beside a program
    /// ([`program_memory_bytes`]).
    largest_payload: usize,
}

impl<H, M, C> CompiledLoader<H, M, C> {
    /// A loader over `hardware` and `clock`, which puts each module into `modules`; `room` says how
    /// many bytes of the heap a container can take now (specification 34 §5.1).
    #[must_use]
    pub fn new(hardware: H, modules: M, clock: C, room: fn() -> usize) -> Self {
        Self {
            hardware,
            modules,
            clock,
            room,
            running: None,
            allocator_operations: None,
            largest_payload: 0,
        }
    }

    /// This loader, which keeps `bytes`, the checkpoint store's largest payload, beside each program
    /// when it measures the memory for programs at a load, as the boot does (specification 34 MEM-12).
    #[must_use]
    pub fn with_checkpoint_payload(mut self, bytes: usize) -> Self {
        self.largest_payload = bytes;
        self
    }

    /// This loader, whose programs count their scans' allocator operations with `operations`, the
    /// heap's count, which must allocate nothing (specification 34, "The allocation count").
    #[must_use]
    pub fn with_allocator_operations(mut self, operations: fn() -> u32) -> Self {
        self.allocator_operations = Some(operations);
        self
    }
}

impl<H, M, C, W> ProgramLoader<Platform<W>, CompiledProgramCycle<C, H::Io, M::Code>>
    for CompiledLoader<H, M, C>
where
    H: ProgramHardware,
    M: ModuleMemory,
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
        room: usize,
    ) -> Result<Loaded<CompiledProgramCycle<C, H::Io, M::Code>>, String> {
        let Booted {
            format,
            program,
            record,
            ..
        } = booted;
        let identity = record.checkpoint_identity();
        let config = record.configuration;
        // A program that fails to load leaves no record of a running program.
        self.running = None;
        if format != ProgramFormat::Image {
            return Err(String::from(
                "the installed application is not a compiled program image: rebuild the project",
            ));
        }
        // The C6 maps its slot, so the image lies in flash, where its module runs (specification 44
        // §10.3).
        let Cow::Borrowed(image) = program else {
            return Err(String::from(
                "the installed application's image is not mapped where it lies",
            ));
        };
        // The memory for programs now, as the boot measures it (specification 34 MEM-12): in a
        // running loop what the services hold can leave less than at boot, and a program that does
        // not fit it is refused rather than loaded into a heap that runs out.
        let now = u64::try_from(program_memory_bytes(room, self.largest_payload)).ok();
        let memory = match (platform.device_descriptor().program_memory_bytes, now) {
            (Some(boot), Some(now)) => Some(boot.min(now)),
            (boot, now) => boot.or(now),
        };
        let device = NATIVE_TARGET.device(memory);
        let admitted = native::admit(image, &device)
            .map_err(|refusal| format!("the installed application cannot be loaded: {refusal}"))?;
        let (_, module) = admitted.module();
        let code = self.modules.place(module);
        let program = NativeProgram::new(admitted, code, &device, &KERNEL_FUNCTIONS)
            .map_err(|refusal| format!("the installed application cannot be loaded: {refusal}"))?;
        platform.feed();
        let (bindings, names) = io_bindings(&program.image()).map_err(|error| {
            format!("the installed application's Live Values cannot be bound: {error}")
        })?;
        let live = DeviceLiveValues::for_compiled(bindings, names).map_err(|error| {
            format!("the installed application's Live Values cannot be bound: {error}")
        })?;
        if !platform
            .watchdog_mut()
            .apply_cycle_timeout_ms(cycle_timeout_ms(&config.watchdog))
        {
            return Err(String::from(
                "the installed application's watchdog timeout is outside the C6's range",
            ));
        }
        let retains = config.retain_mode == RetainMode::File;
        let retain = if retains {
            Some(retain_config(&mut self.hardware, &config, identity)?)
        } else {
            None
        };
        // The lines are claimed last, right before the program starts.
        let io = self.hardware.claim_lines(&config)?;
        let mut cycle = CompiledCycle::start(program, io, settings(&config), self.clock)
            .map_err(|error| format!("the installed application cannot be started: {error}"))?;
        if let Some(retain) = retain {
            cycle = cycle
                .with_retain(retain)
                .map_err(|error| start_failure(retains, &error))?;
        }
        if let Some(operations) = self.allocator_operations {
            cycle = cycle.with_allocator_operations(operations);
        }
        // A new program starts cold; a warm start restores the newest checkpoint over it, as a
        // boot's warm start does. A program that keeps no retained values has nothing to restore.
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
        self.running = Some(config.clone());
        platform.set_program(ProgramValues {
            settings: RuntimeSettings::from_record(&config),
            resource_name: config.resource_name,
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
        running: &mut CompiledProgramCycle<C, H::Io, M::Code>,
        program: Vec<u8>,
    ) -> Result<Candidate<CompiledProgramCycle<C, H::Io, M::Code>>, String> {
        let config = self
            .running
            .clone()
            .ok_or_else(|| String::from("the running program's record is not known"))?;
        let budget = ChangeMemory::for_change(
            program.len(),
            (self.room)(),
            ONLINE_CHANGE_WORKING_MEMORY_BYTES,
        );
        let image = ChangedImage::new(program);
        // SAFETY: the bytes build the candidate's program alone, whose code takes `image`
        // (`place_change`), so they are freed with that program; on a refusal before that, `image`
        // is dropped after every value built from the bytes.
        let bytes = unsafe { image.bytes() };
        let device = NATIVE_TARGET.device(platform.device_descriptor().program_memory_bytes);
        let admitted = native::admit(bytes, &device)
            .map_err(|refusal| format!("the change cannot be loaded: {refusal}"))?;
        platform.feed();
        let figure = admitted.memory_figure();
        if let Some(refusal) = budget.held_figure_refusal(
            usize::try_from(figure.total.saturating_sub(figure.stack)).unwrap_or(usize::MAX),
        ) {
            return Err(refusal);
        }
        let (bindings, names) = io_bindings(&admitted)
            .map_err(|error| format!("the change's Live Values cannot be bound: {error}"))?;
        let (inputs, outputs) = record_lines(&config, "C6")?;
        if let Some(address) = unwired_binding(&bindings, &inputs, &outputs) {
            return Err(format!(
                "the candidate binds {}, which the running program's record gives no line",
                address_text(&address)
            ));
        }
        let live = DeviceLiveValues::for_compiled(bindings, names)
            .map_err(|error| format!("the change's Live Values cannot be bound: {error}"))?;
        // The candidate claims no line: the commit hands it the running program's.
        let io = self.hardware.claim_lines(&Configuration::default())?;
        let (_, module) = admitted.module();
        let code = self.modules.place_change(module, image)?;
        let (cycle, events) = build_candidate(admitted, code, io, running, &device, budget)?;
        platform.feed();
        // A client that names an instance by its identity after the change names the generation that
        // the change installed.
        live.set_generation(cycle.generation());
        Ok(Candidate {
            cycle,
            live,
            events,
        })
    }

    /// The lines and the checkpoint store change places with the candidate's empty ones: no pin is
    /// released or claimed, and no output is written (specification 34 §11.3).
    fn commit_change(
        &mut self,
        platform: &mut Platform<W>,
        running: &mut CompiledProgramCycle<C, H::Io, M::Code>,
        candidate: &mut Candidate<CompiledProgramCycle<C, H::Io, M::Code>>,
    ) {
        running.hand_over_to(&mut candidate.cycle);
        // The record stays the running one, with the settings `config.set` gave it, as a computer's
        // online change keeps them; only the Live Values are the changed program's.
        platform.set_live_values(candidate.live.clone());
    }
}

/// A changed program's cycle and the events of the values it did not keep as they were.
type Changed<C, I, K> = (
    CompiledProgramCycle<C, I, K>,
    Vec<trust_runtime_core::native::change::CarriedValueEvent>,
);

/// The candidate of an online change, once its module is where the CPU runs it: its program over the
/// process I/O `io` with the settings of `running`, checked against the running program's safe state,
/// with room in the checkpoint store for its capture, its cold-init entry run and the running
/// program's values taken (specification 44 §12.3, steps 3 and 4; specification 34 §11.3).
///
/// # Errors
///
/// The refusal of the change, which drops the candidate.
fn build_candidate<C, I, K>(
    admitted: Image<'static>,
    code: K,
    io: I,
    running: &mut CompiledProgramCycle<C, I, K>,
    device: &native::Device,
    budget: ChangeMemory,
) -> Result<Changed<C, I, K>, String>
where
    C: MonotonicClock + Copy + 'static,
    I: trust_runtime_services::runner::ProcessIo,
    K: ModuleCode,
{
    let program = NativeProgram::new(admitted, code, device, &KERNEL_FUNCTIONS)
        .map_err(|refusal| format!("the change cannot be loaded: {refusal}"))?;
    let mut cycle = CompiledCycle::candidate(program, io, running);
    cycle
        .check_safe_outputs()
        .map_err(|error| error.to_string())?;
    let payload = cycle
        .program()
        .retain_payload_bound()
        .map_err(|error| error.to_string())?;
    running
        .reserve_retain(payload)
        .map_err(|_| budget.needs_more())?;
    let events = cycle
        .take_values(running)
        .map_err(|error| format!("the change cannot start: {error}"))?;
    Ok((cycle, events))
}
