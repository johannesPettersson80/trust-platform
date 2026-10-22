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
//! The retained values stay in the program's memory across a warm restart; their checkpoint comes
//! with the retain map (specification 44 §11, milestone M2), so this cycle saves none.

use alloc::vec::Vec;

use smol_str::SmolStr;
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::io_image::{write_flat_image, FlatImages};
use trust_runtime_core::memory::Generation;
use trust_runtime_core::native::image::BACKGROUND_PRIORITY;
use trust_runtime_core::native::program::{ModuleCode, NativeProgram, ScanHooks};
use trust_runtime_core::ports::{MonotonicClock, MonotonicInstant, TimeSpan};
use trust_runtime_core::retain::RestartMode;
use trust_runtime_core::value::Duration;
use trust_runtime_core::vm::{CycleHooks, CyclePhase, MemoryFigure, RetainApplyEvent};
use trust_runtime_core::watchdog::{FaultPolicy, WatchdogPolicy};

use super::cycle::{ResourceCycle, RetainSource, TaskDescriptor};
use super::engine::{CycleSettings, ProcessIo};
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
        Ok(Self {
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
        })
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

    /// No debugger holds a compiled program's scan until its breakpoints come (specification 44
    /// §13.2, milestone M3).
    fn held_time(&self) -> TimeSpan {
        TimeSpan::ZERO
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
    /// as the engine; the module's code is not part of the figure (specification 44 §10.3).
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
            retains: false,
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
        observer.on_compiled_phase(CyclePhase::CycleStart, now, &self.images)?;
        observer.on_compiled_phase(CyclePhase::BeforeInputSample, now, &self.images)?;
        self.io.read_inputs(&mut self.program.inputs)?;
        let mut phases = TaskPhases {
            observer: &mut *observer,
            now,
            images: &self.images,
            task_index: &self.task_index,
            error: None,
            operations: self.operations,
            observer_operations: 0,
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
        // before it publishes its outputs (specification 34 TIME-04).
        if deadline.is_some_and(|deadline| self.clock.now() > deadline) {
            return Err(RuntimeError::ExecutionTimeout);
        }
        self.program.copy_images(&mut self.images);
        observer.on_compiled_phase(CyclePhase::BeforeOutputSample, now, &self.images)?;
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

    /// The checkpoint of the retained values comes with the retain map (specification 44 §11,
    /// milestone M2); until then a stop saves nothing.
    fn save_retain(&mut self) -> Result<(), RuntimeError> {
        Ok(())
    }

    /// A cold restart gives every area its starting memory; a warm restart keeps the retained
    /// values the program holds in its memory (IEC 61131-3 §6.5.6.1), from either source, since no
    /// checkpoint holds them yet. A restart that completes clears the latched fault and installs the
    /// next generation.
    fn restart(
        &mut self,
        mode: RestartMode,
        _source: RetainSource,
    ) -> Result<Vec<RetainApplyEvent>, RuntimeError> {
        let generation = self.generation.next().ok_or(RuntimeError::Overflow)?;
        match mode {
            RestartMode::Cold => self.program.cold_start(Duration::ZERO)?,
            RestartMode::Warm => self.program.warm_start(Duration::ZERO)?,
        }
        self.program.copy_images(&mut self.images);
        self.generation = generation;
        self.fault = None;
        Ok(Vec::new())
    }

    fn reset_task_timing(&mut self, now: Duration) {
        self.program.reset_task_timing(now);
    }

    fn generation(&self) -> Generation {
        self.generation
    }

    fn io_drivers(&self) -> Vec<IoDriverStatus> {
        self.io.driver_health()
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
}

#[cfg(test)]
mod tests;
