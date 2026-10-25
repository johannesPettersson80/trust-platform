//! What the resource loop records of a scan at its boundaries (specification 44 §8.7): the overrun
//! events, and the task and overrun metrics, as trust-runtime records them. The events that every
//! scan makes, its cycle's and its task's start and end, are not recorded: the event store keeps
//! none of them (specification 11, the event store), and no caller drains them from a chip.

use smol_str::SmolStr;
use trust_runtime_core::cycle::{CycleHooks, CyclePhase};
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::io_image::FlatImages;
use trust_runtime_core::ports::{MonotonicClock, MonotonicInstant};
use trust_runtime_core::value::Duration;

use super::cycle::TaskDescriptor;
use super::debugger::ChipDebug;
use super::state::Records;
use super::threads::{background_thread, task_thread};
use crate::events::RuntimeEvent;

/// The loop's [`CycleHooks`] for one scan. It also tells the debugger which thread runs: a task
/// starts, or the programs that run outside any task do (specification 13, "Multi-Task Thread
/// Model").
pub(super) struct ScanObserver<'a, C> {
    tasks: &'a [TaskDescriptor],
    /// The program instance of each entry of the image, by which a program call is recorded.
    programs: &'a [SmolStr],
    records: &'a Records<C>,
    clock: &'a C,
    debug: &'a ChipDebug,
    task_started: Option<MonotonicInstant>,
    program_started: Option<MonotonicInstant>,
}

impl<'a, C: MonotonicClock> ScanObserver<'a, C> {
    /// An observer of a scan of the program whose tasks are `tasks`, which tells `debug` the
    /// thread that runs.
    pub(super) fn new(
        tasks: &'a [TaskDescriptor],
        programs: &'a [SmolStr],
        records: &'a Records<C>,
        clock: &'a C,
        debug: &'a ChipDebug,
    ) -> Self {
        Self {
            tasks,
            programs,
            records,
            clock,
            debug,
            task_started: None,
            program_started: None,
        }
    }

    fn task(&self, index: usize) -> Result<&'a TaskDescriptor, RuntimeError> {
        self.tasks
            .get(index)
            .ok_or(RuntimeError::InvalidExecutionState)
    }

    fn since(&self, started: MonotonicInstant) -> core::time::Duration {
        self.clock.now().saturating_since(started).as_duration()
    }

    /// Records `phase` of a scan at the logical time `time`.
    fn record(&mut self, phase: CyclePhase, time: Duration) -> Result<(), RuntimeError> {
        match phase {
            CyclePhase::TaskOverrun { index, missed } => {
                let task = self.task(index)?;
                self.records
                    .record(|metrics| metrics.record_overrun(&task.name, missed));
                self.records.push_event(RuntimeEvent::TaskOverrun {
                    name: task.name.clone(),
                    missed,
                    time,
                });
            }
            CyclePhase::TaskStart(index) => {
                self.debug.set_thread(task_thread(self.tasks, index));
                self.task_started = Some(self.clock.now());
            }
            CyclePhase::TaskEnd(index) => {
                let task = self.task(index)?;
                if let Some(started) = self.task_started.take() {
                    let duration = self.since(started);
                    self.records
                        .record(|metrics| metrics.record_task(&task.name, duration));
                }
            }
            CyclePhase::BackgroundStart => {
                self.debug.set_thread(background_thread(self.tasks));
            }
            CyclePhase::ProgramStart(_) => self.program_started = Some(self.clock.now()),
            // Each invoked program is a call of its instance, a faulted one too (specification 11,
            // runtime metrics).
            CyclePhase::ProgramEnd(entry) => {
                if let Some(started) = self.program_started.take()
                    && let Some(name) = self.programs.get(entry).filter(|name| !name.is_empty())
                {
                    let duration = self.since(started);
                    self.records
                        .record(|metrics| metrics.record_call("program", name, duration));
                }
            }
            CyclePhase::CycleStart
            | CyclePhase::BeforeInputSample
            | CyclePhase::BackgroundEnd
            | CyclePhase::BeforeOutputSample
            | CyclePhase::AfterOutputSample
            | CyclePhase::CycleEnd => {}
        }
        Ok(())
    }
}

impl<C: MonotonicClock> CycleHooks for ScanObserver<'_, C> {
    fn on_compiled_phase(
        &mut self,
        phase: CyclePhase,
        now: Duration,
        _images: &FlatImages,
    ) -> Result<(), RuntimeError> {
        self.record(phase, now)
    }
}
