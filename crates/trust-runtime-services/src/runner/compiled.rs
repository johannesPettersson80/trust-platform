//! A compiled program as the resource loop drives it (specification 44 §8.7): the
//! [`ResourceCycle`] of a device that runs program images, over the kernel's [`NativeProgram`] with
//! the platform's process I/O.
//!
//! A scan keeps the engine cycle's order and output rules (specification 34 §8, CYCLE-01 to
//! CYCLE-06): the drivers read the inputs, the program samples them, runs its due tasks by priority
//! and then its programs without a task, and stages its outputs; the scan's deadline is checked; the
//! snapshot is published after the output sample; then the drivers drive the outputs. A fault of the
//! module ends the scan before any output is driven, and a latched fault refuses every scan until a
//! restart (STATE-04). The loop's observers see the cycle's start, the input sample, each task's
//! overruns, start and end, the output sample and the cycle's end, in an engine scan's order,
//! through [`CycleHooks::on_compiled_phase`].
//!
//! A stepped scan (`cycle.step`, specification 11 §6.8.1) runs the same scan without the drivers,
//! the observers and the capture of the retained values, the names found in the image's symbol
//! table as the test harness finds them.
//!
//! A cycle built with [`CompiledCycle::with_retain`] also holds the chip's checkpoint store, as an
//! engine cycle does (specification 34 §10; specification 44 §11): the retained values are captured
//! through the image's retain map after the observers saw the output sample, a pending checkpoint
//! advances between scans, a stop completes it, and a warm restart from the checkpoint restores it
//! by name. Without one, a stop saves nothing and a warm restart keeps the retained values the
//! program holds in its memory.

use alloc::string::ToString;
use alloc::vec::Vec;

use smol_str::SmolStr;
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::io_image::{write_flat_image, FlatImages};
use trust_runtime_core::memory::Generation;
use trust_runtime_core::native::image::BACKGROUND_PRIORITY;
use trust_runtime_core::native::program::{ModuleCode, NativeProgram, NoHooks, ScanHooks};
use trust_runtime_core::native::stop::Stop;
use trust_runtime_core::native::variables::CompiledAccessPoint;
use trust_runtime_core::ports::{MonotonicClock, MonotonicInstant, TimeSpan};
use trust_runtime_core::retain::{RestartMode, RetainSnapshot};
use trust_runtime_core::value::Duration;
use trust_runtime_core::vm::{CycleHooks, CyclePhase, MemoryFigure, RetainApplyEvent};
use trust_runtime_core::watchdog::{FaultPolicy, WatchdogPolicy};

use super::cycle::{CompiledView, ResourceCycle, RetainSource, TaskDescriptor};
use super::engine::{CycleSettings, ProcessIo};
use super::hold::HoldHandle;
use super::retain::{RetainConfig, RetainStore};
use crate::cycle_step::{StepOutcome, StepRefusal, StepRequest};
use crate::metrics::ProgramMemory;
use crate::status::IoDriverStatus;

/// The installed compiled program of a device: the kernel's program of an admitted image, with the
/// platform's process I/O and clock.
///
/// `'i` is the life of the image, which a chip maps for as long as the program runs. `C` is the
/// platform's monotonic clock, `I` its process I/O and `K` where the module's code is.
pub struct CompiledCycle<'i, C, I, K> {
    program: NativeProgram<'i, K>,
    io: I,
    settings: CycleSettings,
    clock: C,
    /// The configured tasks, in the image's order without the programs that have no task.
    tasks: Vec<TaskDescriptor>,
    /// For each task of the image, its index in `tasks`; `None` for the programs without a task.
    task_index: Vec<Option<usize>>,
    /// The process images as the last completed scan left them.
    images: FlatImages,
    generation: Generation,
    /// The fault latched since the last restart.
    fault: Option<RuntimeError>,
    /// The platform's count of its allocator operations, when it counts them.
    operations: Option<fn() -> u32>,
    /// The allocator operations the last scan's program made.
    last_allocations: Option<u64>,
    /// The checkpoint store of the retained values, when the program keeps them.
    retain: Option<RetainStore>,
    /// The logical time of the last scan, which a stop's checkpoint carries.
    now: Duration,
    /// The debugger: the loop's hold, the statement table's sources and the armed statements
    /// (specification 44 §13.2).
    debug: debug::CompiledDebug,
}

impl<'i, C: MonotonicClock, I: ProcessIo, K: ModuleCode> CompiledCycle<'i, C, I, K> {
    /// Starts `program` as power-on starts it, with a cold restart (specification 44 §6.7), over the
    /// process I/O `io` with `settings`. The resource loop then starts its task timing.
    ///
    /// # Errors
    ///
    /// The fault of the program's cold-start initialization.
    pub fn start(
        mut program: NativeProgram<'i, K>,
        io: I,
        settings: CycleSettings,
        clock: C,
    ) -> Result<Self, RuntimeError> {
        program.cold_start(Duration::ZERO)?;
        Ok(Self::built(program, io, settings, clock))
    }

    /// The cycle of `program` as its memory is, in the first generation: its tasks, its snapshot
    /// and its debugger.
    fn built(program: NativeProgram<'i, K>, io: I, settings: CycleSettings, clock: C) -> Self {
        let debug = debug::CompiledDebug::new(program.image());
        let mut tasks = Vec::new();
        let task_index = program
            .tasks()
            .map(|(name, priority)| {
                (priority != BACKGROUND_PRIORITY).then(|| {
                    tasks.push(TaskDescriptor {
                        name: SmolStr::new(name),
                        priority,
                    });
                    tasks.len() - 1
                })
            })
            .collect();
        let mut images = FlatImages::default();
        program.copy_images(&mut images);
        Self {
            program,
            io,
            settings,
            clock,
            tasks,
            task_index,
            images,
            generation: Generation::FIRST,
            fault: None,
            operations: None,
            last_allocations: None,
            retain: None,
            now: Duration::ZERO,
            debug,
        }
    }

    /// This cycle with the checkpoint store of `retain` (specification 34 §10; specification 44
    /// §11): the persister opens the log over the platform's flash, and the capture reserves the
    /// program's largest retained payload, so that no capture grows it (specification 34 MEM-09).
    ///
    /// # Errors
    ///
    /// The [`RuntimeError`] of a checkpoint log that cannot be opened, of a retain map that does not
    /// read, or of a heap without room for the capture.
    pub fn with_retain(mut self, retain: RetainConfig) -> Result<Self, RuntimeError> {
        let mut store = RetainStore::open(retain)?;
        store.reserve(self.program.retain_payload_bound()?)?;
        self.retain = Some(store);
        Ok(self)
    }

    /// This cycle, which counts the allocator operations of each scan's program with
    /// `operations`, the platform's count in 32 bits that wrap, which must allocate nothing
    /// (specification 34, "The allocation count").
    #[must_use]
    pub fn with_allocator_operations(mut self, operations: fn() -> u32) -> Self {
        self.operations = Some(operations);
        self
    }

    /// The program the cycle runs.
    pub fn program(&self) -> &NativeProgram<'i, K> {
        &self.program
    }

    /// Arms the program's statements for the scan that begins, as the debugger gives them while
    /// the platform enables debugging, and publishes the I/O health a hold reports
    /// (specification 44 §13.2). A scan that no debugger can hold arms nothing.
    fn arm(&mut self) {
        let debugging = self
            .debug
            .hold
            .as_ref()
            .is_some_and(HoldHandle::scan_begins);
        if debugging {
            self.debug.drivers.set(self.io.driver_health());
        }
        let points = self.debug.stop_points(debugging);
        if self.debug.rearms(&points) {
            let wanted = debug::CompiledDebug::wanted(self.program.image(), &points);
            self.program.arm(points.every, &|id| {
                usize::try_from(id)
                    .ok()
                    .and_then(|id| wanted.get(id))
                    .copied()
                    .unwrap_or(false)
            });
        }
    }
}

impl<C: MonotonicClock, I: ProcessIo, K: ModuleCode> ResourceCycle for CompiledCycle<'_, C, I, K> {
    fn tasks(&self) -> Vec<TaskDescriptor> {
        self.tasks.clone()
    }

    fn cycle_interval(&self) -> Duration {
        self.settings.cycle_interval
    }

    fn watchdog(&self) -> WatchdogPolicy {
        self.settings.watchdog
    }

    fn fault_policy(&self) -> FaultPolicy {
        self.settings.fault_policy
    }

    /// The debugger's holds, which are no active time (specification 11 §6.6; specification 44
    /// §13.2).
    fn held_time(&self) -> TimeSpan {
        self.debug.holds.held_at(|| self.clock.now())
    }

    fn allocator_operations(&self) -> Option<u32> {
        self.operations.map(|operations| operations())
    }

    /// The allocator operations the program and the kernel made in the last scan, the drivers and
    /// the hooks not counted.
    fn last_scan_allocations(&self) -> Option<u64> {
        self.last_allocations
    }

    /// The image's memory figure (specification 44 §10.4) in the parts `status` reports: the areas
    /// as the persistent block, the stack bound as the activation stack and the kernel's bookkeeping
    /// as the engine; the module's code is not part of the figure (specification 44 §10.3). The
    /// capture of the retained values counts when the cycle has a store that persists them.
    fn program_memory(&self) -> Option<ProgramMemory> {
        let figure = self.program.image().memory_figure();
        let bytes = |value: u64| usize::try_from(value).unwrap_or(usize::MAX);
        Some(ProgramMemory {
            figure: MemoryFigure {
                program: 0,
                persistent_block: bytes(figure.areas),
                activation_stack: bytes(figure.stack),
                operand_stack: 0,
                engine: bytes(figure.bookkeeping),
                retain_capture: bytes(figure.retain_capture),
            },
            retains: self.retain.as_ref().is_some_and(RetainStore::persists),
        })
    }

    fn run_scan(
        &mut self,
        now: Duration,
        deadline: Option<MonotonicInstant>,
        observer: &mut dyn CycleHooks,
    ) -> Result<(), RuntimeError> {
        if let Some(fault) = &self.fault {
            return Err(fault.clone());
        }
        self.now = now;
        self.arm();
        let held_before = self.debug.holds.held_at(|| self.clock.now());
        observer.on_compiled_phase(CyclePhase::CycleStart, now, &self.images)?;
        observer.on_compiled_access(CompiledAccessPoint::CycleStart, now, &mut self.program)?;
        observer.on_compiled_phase(CyclePhase::BeforeInputSample, now, &self.images)?;
        self.io.read_inputs(&mut self.program.inputs)?;
        observer.on_compiled_access(CompiledAccessPoint::AfterInputs, now, &mut self.program)?;
        let mut phases = TaskPhases {
            observer: &mut *observer,
            now,
            images: &self.images,
            task_index: &self.task_index,
            error: None,
            operations: self.operations,
            observer_operations: 0,
            debug: &self.debug,
            clock: &self.clock,
        };
        let before = self.operations.map(|operations| operations());
        let result = self.program.scan(now, &mut phases);
        // The observer's own operations inside the scan are the services', not the program's.
        let observer_operations = phases.observer_operations;
        self.last_allocations = before
            .zip(self.operations.map(|operations| operations()))
            .map(|(before, after)| {
                u64::from(after.wrapping_sub(before).wrapping_sub(observer_operations))
            });
        if let Some(error) = phases.error {
            return Err(error);
        }
        result?;
        // The scan's deadline is checked before anything is published, as the engine checks it
        // before it publishes its outputs (specification 34 TIME-04); the debugger's holds in the
        // scan are no active time and move it (specification 11 §6.6).
        let held = self
            .debug
            .holds
            .held_at(|| self.clock.now())
            .saturating_sub(held_before);
        if deadline.is_some_and(|deadline| self.clock.now() > deadline.saturating_add(held)) {
            return Err(RuntimeError::ExecutionTimeout);
        }
        observer.on_compiled_access(CompiledAccessPoint::BeforeOutputs, now, &mut self.program)?;
        self.program.copy_images(&mut self.images);
        observer.on_compiled_phase(CyclePhase::BeforeOutputSample, now, &self.images)?;
        // The retained values are captured and offered where an engine cycle captures them; a
        // retain failure fails the scan before its outputs are driven (specification 11 §6.7).
        if let Some(retain) = self.retain.as_mut() {
            let program = &self.program;
            retain.capture_with(now, |capture| program.capture_retain(capture))?;
        }
        observer.on_compiled_phase(CyclePhase::AfterOutputSample, now, &self.images)?;
        self.io.write_outputs(&self.program.outputs)?;
        observer.on_compiled_phase(CyclePhase::CycleEnd, now, &self.images)
    }

    fn latch_fault(&mut self, fault: RuntimeError) {
        self.fault = Some(fault);
    }

    /// As trust-runtime applies `[[io.safe_state]]`: each safe value is written into the output
    /// image, and the drivers then drive that image.
    fn apply_safe_outputs(&mut self) -> Result<(), RuntimeError> {
        if self.settings.safe_outputs.is_empty() {
            return Ok(());
        }
        for (address, value) in &self.settings.safe_outputs {
            write_flat_image(&mut self.program.outputs, address, value.clone())?;
        }
        self.io.write_safe_outputs(&self.program.outputs)
    }

    /// Captures the values the program holds and completes the pending checkpoint and the newest
    /// image before the resource reports Stopped (specification 11 §6.7); a cycle without a
    /// checkpoint store saves nothing.
    fn save_retain(&mut self) -> Result<(), RuntimeError> {
        if let Some(retain) = self.retain.as_mut() {
            let program = &self.program;
            retain.save_with(self.now, |capture| program.capture_retain(capture))?;
        }
        Ok(())
    }

    /// Advances a pending checkpoint by at most one persister step, between scans (specification
    /// 34 §10); a cycle without a checkpoint store does nothing.
    fn step_retain(&mut self) {
        if let Some(retain) = self.retain.as_mut() {
            retain.step();
        }
    }

    /// A cold restart gives every variable its starting memory. A warm restart from the checkpoint
    /// restores the newest committed checkpoint by name, each value checked before any variable
    /// changes (specification 44 §11); a cycle without a checkpoint store restores the empty
    /// checkpoint, as an engine cycle does, and so keeps the retained values the program holds, as
    /// a warm restart from memory does (IEC 61131-3 §6.5.6.1). A restart that completes clears the
    /// latched fault and installs the next generation.
    fn restart(
        &mut self,
        mode: RestartMode,
        source: RetainSource,
    ) -> Result<Vec<RetainApplyEvent>, RuntimeError> {
        let generation = self.generation.next().ok_or(RuntimeError::Overflow)?;
        let events = match (mode, source) {
            (RestartMode::Cold, _) => {
                self.program.cold_start(Duration::ZERO)?;
                Vec::new()
            }
            (RestartMode::Warm, RetainSource::Checkpoint) => {
                let snapshot = match self.retain.as_mut() {
                    Some(retain) => retain.recover()?,
                    None => RetainSnapshot::default(),
                };
                self.program.warm_start_from(Duration::ZERO, &snapshot)?
            }
            (RestartMode::Warm, RetainSource::Memory) => {
                self.program.warm_start(Duration::ZERO)?;
                Vec::new()
            }
        };
        self.program.copy_images(&mut self.images);
        self.generation = generation;
        self.fault = None;
        Ok(events)
    }

    fn reset_task_timing(&mut self, now: Duration) {
        self.program.reset_task_timing(now);
    }

    /// The names are found in the image's symbol table before anything changes. A restart restarts
    /// the program at `restart_at`, as the conformance runner's kernel restarts it, and installs the
    /// next generation. The scan runs without hooks, so no driver, observer or capture of the
    /// retained values takes part, and the outputs it publishes stay in the output image; a step
    /// that runs a program instance runs its body by the number its root symbol carries.
    fn step(
        &mut self,
        request: &StepRequest,
        restart_at: Duration,
        deadline: Option<MonotonicInstant>,
    ) -> Result<StepOutcome, StepRefusal> {
        let image = self.program.image();
        let find = |name: &SmolStr| {
            image
                .find_variable(name)
                .map_err(|error| StepRefusal::name(name, error.to_string()))
        };
        let inputs = request
            .inputs
            .iter()
            .map(|(name, value)| Ok((name, find(name)?, value)))
            .collect::<Result<Vec<_>, StepRefusal>>()?;
        let watch = request
            .watch
            .iter()
            .map(|name| Ok((name, find(name)?)))
            .collect::<Result<Vec<_>, StepRefusal>>()?;
        let run = request
            .run
            .as_ref()
            .map(|name| {
                image
                    .find_program_instance(name)
                    .ok_or_else(|| StepRefusal::name(name, "names no program instance"))
            })
            .transpose()?;
        if let Some(mode) = request.restart {
            let generation = self
                .generation
                .next()
                .ok_or(StepRefusal::Restart(RuntimeError::Overflow))?;
            match mode {
                RestartMode::Cold => self.program.cold_start(restart_at),
                RestartMode::Warm => self.program.warm_start(restart_at),
            }
            .map_err(StepRefusal::Restart)?;
            self.generation = generation;
            self.fault = None;
        }
        for (name, symbol, value) in inputs {
            self.program
                .write_symbol(symbol, value)
                .map_err(|error| StepRefusal::write(name, &error))?;
        }
        for (text, address, value) in &request.direct_inputs {
            self.program
                .write_address(address, value.clone())
                .map_err(|error| StepRefusal::write(text, &error))?;
        }
        self.now = request.time;
        let mut error = match run {
            Some(instance) => self.program.run_instance(instance, request.time),
            None => self.program.scan(request.time, &mut NoHooks),
        }
        .err();
        if error.is_none() && deadline.is_some_and(|deadline| self.clock.now() > deadline) {
            error = Some(RuntimeError::ExecutionTimeout);
        }
        self.program.copy_images(&mut self.images);
        let program = &self.program;
        let values = watch
            .into_iter()
            .map(|(name, symbol)| {
                program
                    .read_symbol(symbol)
                    .map(|value| (name.clone(), value))
                    .map_err(|error| StepRefusal::read(name, &error))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let direct = request
            .watch_direct
            .iter()
            .map(|(text, address)| {
                program
                    .read_address(address)
                    .map(|value| (text.clone(), value))
                    .map_err(|error| StepRefusal::read(text, &error))
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(StepOutcome {
            values,
            direct,
            error,
        })
    }

    fn generation(&self) -> Generation {
        self.generation
    }

    fn io_drivers(&self) -> Vec<IoDriverStatus> {
        self.io.driver_health()
    }

    /// The loop's hold, which the stops of a debug image's scans go to (specification 44 §13.2).
    fn attach_hold(&mut self, hold: HoldHandle) {
        self.debug.hold = Some(hold);
    }

    /// The program's memory between scans, with the sources of its statement table, once the loop
    /// attached its debugger.
    fn compiled_view(&self) -> Option<CompiledView<'_>> {
        self.debug.hold.as_ref()?;
        Some(CompiledView {
            memory: &self.program,
            sources: self.debug.sources.as_ref(),
            release: self.debug.release,
        })
    }
}

/// The task phases of one scan, forwarded to the loop's observer with the image's task numbered as
/// the cycle's [`ResourceCycle::tasks`] number it. The first error the observer returns ends the
/// forwarding and fails the scan once the program returns. With the platform's allocator count,
/// it counts the observer's own operations, which are not the program's.
struct TaskPhases<'h, 'o> {
    observer: &'h mut (dyn CycleHooks + 'o),
    now: Duration,
    images: &'h FlatImages,
    task_index: &'h [Option<usize>],
    error: Option<RuntimeError>,
    operations: Option<fn() -> u32>,
    observer_operations: u32,
    debug: &'h debug::CompiledDebug,
    clock: &'h dyn MonotonicClock,
}

impl TaskPhases<'_, '_> {
    fn forward(&mut self, task: usize, phase: impl FnOnce(usize) -> CyclePhase) {
        if self.error.is_some() {
            return;
        }
        let Some(Some(index)) = self.task_index.get(task) else {
            return;
        };
        let before = self.operations.map(|operations| operations());
        let result = self
            .observer
            .on_compiled_phase(phase(*index), self.now, self.images);
        if let Some((before, after)) = before.zip(self.operations.map(|operations| operations())) {
            self.observer_operations = self
                .observer_operations
                .wrapping_add(after.wrapping_sub(before));
        }
        if let Err(error) = result {
            self.error = Some(error);
        }
    }
}

impl ScanHooks for TaskPhases<'_, '_> {
    fn task_overrun(&mut self, task: usize, missed: u64) {
        self.forward(task, |index| CyclePhase::TaskOverrun { index, missed });
    }

    fn task_started(&mut self, task: usize) {
        self.forward(task, CyclePhase::TaskStart);
    }

    fn task_finished(&mut self, task: usize) {
        self.forward(task, CyclePhase::TaskEnd);
    }

    /// A statement whose bit is set stops (specification 44 §13.2): the debugger decides there,
    /// and holds the scan while the loop serves the link. The Live Values queued during a hold
    /// reach the program when it ends, before the statement runs, as at a cycle start; the
    /// statements are then armed as the debugger gives them for the rest of the scan.
    fn on_stop(&mut self, stop: &mut Stop<'_>) -> Result<(), RuntimeError> {
        if self.debug.at_stop(stop, self.clock)? {
            self.observer
                .on_compiled_access(CompiledAccessPoint::HoldEnd, self.now, stop)?;
        }
        self.debug.arm_stop(stop);
        Ok(())
    }
}

mod change;
mod debug;
#[cfg(test)]
mod loop_tests;
#[cfg(test)]
mod retain_tests;
#[cfg(test)]
mod tests;
