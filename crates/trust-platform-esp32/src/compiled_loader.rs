//! The C6's loader of compiled programs (specification 44 §8, §10): it builds the running program
//! out of an admitted container whose program is a compiled program image, for the boot and for a
//! `restart` that loads a newer container in place, in the steps and the order of the interpreter's
//! loader ([`crate::loader`]):
//!
//! 1. the image is checked where it lies in the mapped slot, against the C6's CPU, its memory for
//!    programs and its task stack (specification 44 §10.2), and its module is put where the CPU runs
//!    it ([`ModuleMemory`]): the C6 copies it into its program code region, and runs a module larger
//!    than that region where it lies in the slot (§10.3);
//! 2. the program's areas are allocated, its Live Values bound to its process-image bindings, and the
//!    hardware watchdog set from the record's cycle watchdog;
//! 3. the record's GPIO lines are claimed by pin name, each output driven to its initial value before
//!    it becomes an output (STATE-01), and the program starts cold.
//!
//! The lines are claimed last, so an image that is refused leaves the outputs as the stop of the
//! program before it left them. The retained values have no checkpoint until the retain map comes
//! (specification 44 §11, milestone M2), and an online change none until the copy plan comes (§12,
//! milestone M3): the loader refuses a change with that reason.

use alloc::borrow::Cow;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use trust_plc_abi::kernel::WITHOUT_DEBUGGER;
use trust_runtime_core::native::live::io_bindings;
use trust_runtime_core::native::program::{self as native, ModuleCode, NativeProgram};
use trust_runtime_core::ports::{HardwareWatchdog, MonotonicClock};
use trust_runtime_core::retain::RestartMode;
use trust_runtime_services::chip::platform::ProgramValues;
use trust_runtime_services::installation::ProgramFormat;
use trust_runtime_services::loading::select::Booted;
use trust_runtime_services::ports::StatusPort;
use trust_runtime_services::runner::{
    Candidate, CompiledCycle, EngineLiveValues, Loaded, ProgramLoader,
};

use crate::loader::{cycle_timeout_ms, event_capacity, settings, CycleWatchdog, ProgramHardware};
use crate::platform::NativePlatform;
use crate::profile::NATIVE_TARGET;

/// The cycle of a compiled program whose module runs where `K` put it.
pub type CompiledProgramCycle<C, I, K> = CompiledCycle<'static, C, I, K>;

/// Where a device puts a compiled program's module so that its CPU runs it (specification 44 §10.3).
pub trait ModuleMemory {
    /// The module as the kernel calls it.
    type Code: ModuleCode;

    /// Puts `module`, the code and read-only data of an admitted image as they lie in the mapped
    /// slot, where the CPU runs them. The memory it takes is free again when the code is dropped.
    fn place(&mut self, module: &'static [u8]) -> Self::Code;
}

/// The loader of compiled programs: the hardware it claims from, where it puts their modules, the
/// clock its programs run on and the room the heap has for a container.
pub struct CompiledLoader<H, M, C> {
    hardware: H,
    modules: M,
    clock: C,
    room: fn() -> usize,
    /// The heap's count of its allocator operations, which each program's cycle reads around its
    /// scans (specification 34, "The allocation count").
    allocator_operations: Option<fn() -> u32>,
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

impl<H, M, C, W> ProgramLoader<NativePlatform<W>, CompiledProgramCycle<C, H::Io, M::Code>>
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
        platform: &mut NativePlatform<W>,
        booted: Booted,
        _mode: RestartMode,
    ) -> Result<Loaded<CompiledProgramCycle<C, H::Io, M::Code>>, String> {
        let Booted {
            format,
            program,
            record,
            ..
        } = booted;
        let config = record.configuration;
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
        let device = NATIVE_TARGET.device(platform.device_descriptor().program_memory_bytes);
        let admitted = native::admit(image, &device)
            .map_err(|refusal| format!("the installed application cannot be loaded: {refusal}"))?;
        let (_, module) = admitted.module();
        let code = self.modules.place(module);
        let program = NativeProgram::new(admitted, code, &device, &WITHOUT_DEBUGGER)
            .map_err(|refusal| format!("the installed application cannot be loaded: {refusal}"))?;
        platform.feed();
        let (bindings, names) = io_bindings(program.image()).map_err(|error| {
            format!("the installed application's Live Values cannot be bound: {error}")
        })?;
        let live = EngineLiveValues::for_compiled(bindings, names).map_err(|error| {
            format!("the installed application's Live Values cannot be bound: {error}")
        })?;
        if !platform
            .watchdog_mut()
            .apply_cycle_timeout_ms(cycle_timeout_ms(&config))
        {
            return Err(String::from(
                "the installed application's watchdog timeout is outside the C6's range",
            ));
        }
        // The lines are claimed last, right before the program starts.
        let io = self.hardware.claim_lines(&config)?;
        let mut cycle = CompiledCycle::start(program, io, settings(&config), self.clock)
            .map_err(|error| format!("the installed application cannot be started: {error}"))?;
        if let Some(operations) = self.allocator_operations {
            cycle = cycle.with_allocator_operations(operations);
        }
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
            events: Vec::new(),
            event_capacity,
        })
    }

    fn prepare_change(
        &mut self,
        _platform: &mut NativePlatform<W>,
        _running: &mut CompiledProgramCycle<C, H::Io, M::Code>,
        _program: Vec<u8>,
    ) -> Result<Candidate<CompiledProgramCycle<C, H::Io, M::Code>>, String> {
        Err(String::from(
            "an online change of a compiled program comes with its copy plan (specification 44 §12)",
        ))
    }

    /// No change is ever prepared, so none is committed.
    fn commit_change(
        &mut self,
        _platform: &mut NativePlatform<W>,
        _running: &mut CompiledProgramCycle<C, H::Io, M::Code>,
        _candidate: &mut Candidate<CompiledProgramCycle<C, H::Io, M::Code>>,
    ) {
    }
}
