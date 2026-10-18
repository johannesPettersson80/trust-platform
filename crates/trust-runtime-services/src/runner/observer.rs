//! What the resource loop records of a scan at the engine's cycle phases: the scan and task
//! events, and the task, program and overrun metrics, as trust-runtime's hosted cycle records
//! them (`runtime/installed_cycle.rs`). The loop itself records the end of a successful scan.

use smol_str::SmolStr;
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::ports::{MonotonicClock, MonotonicInstant};
use trust_runtime_core::vm::{CycleAccess, CycleHooks, CyclePhase};

use super::cycle::TaskDescriptor;
use super::debugger::ChipDebug;
use super::state::Records;
use super::threads::{background_thread, task_thread};
use crate::events::RuntimeEvent;

/// The loop's [`CycleHooks`] for one scan. It also tells the debugger which thread runs: a task
/// starts, or the programs that run outside any task do (specification 13, "Multi-Task Thread
/// Model").
///
/// It keeps the engine's default `has_output_hooks`, true, as trust-runtime's hosted cycle does:
/// the engine then checks the scan's deadline after the outputs are staged and before they are
/// published, and restores the staged outputs after a failed publication, so a fault is recorded
/// at the same point as on the host. The cost is the engine's journal of the staged outputs,
/// which the device measurements of slice S1b include.
pub(super) struct ScanObserver<'a, C> {
    tasks: &'a [TaskDescriptor],
    records: &'a Records<C>,
    clock: &'a C,
    debug: &'a ChipDebug,
    cycle: u64,
    task_started: Option<MonotonicInstant>,
    program_started: Option<MonotonicInstant>,
}

impl<'a, C: MonotonicClock> ScanObserver<'a, C> {
    /// An observer of scan number `cycle`, counted from the last start of the program.
    pub(super) fn new(
        tasks: &'a [TaskDescriptor],
        records: &'a Records<C>,
        clock: &'a C,
        debug: &'a ChipDebug,
        cycle: u64,
    ) -> Self {
        Self {
            tasks,
            records,
            clock,
            debug,
            cycle,
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
}

impl<C: MonotonicClock> CycleHooks for ScanObserver<'_, C> {
    fn has_program_hooks(&self) -> bool {
        true
    }

    fn program_started(&mut self, _name: &SmolStr) {
        self.program_started = Some(self.clock.now());
    }

    fn program_finished(&mut self, name: &SmolStr) {
        if let Some(started) = self.program_started.take() {
            let duration = self.since(started);
            self.records
                .record(|metrics| metrics.record_call("program", name, duration));
        }
    }

    fn on_phase(
        &mut self,
        phase: CyclePhase,
        access: &mut CycleAccess<'_, '_>,
    ) -> Result<(), RuntimeError> {
        let time = access.logical_time();
        match phase {
            CyclePhase::CycleStart => self.records.push_event(RuntimeEvent::CycleStart {
                cycle: self.cycle,
                time,
            }),
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
                let task = self.task(index)?;
                self.debug.set_thread(task_thread(self.tasks, index));
                self.task_started = Some(self.clock.now());
                self.records.push_event(RuntimeEvent::TaskStart {
                    name: task.name.clone(),
                    priority: task.priority,
                    time,
                });
            }
            CyclePhase::TaskEnd(index) => {
                let task = self.task(index)?;
                if let Some(started) = self.task_started.take() {
                    let duration = self.since(started);
                    self.records
                        .record(|metrics| metrics.record_task(&task.name, duration));
                }
                self.records.push_event(RuntimeEvent::TaskEnd {
                    name: task.name.clone(),
                    priority: task.priority,
                    time,
                });
            }
            CyclePhase::BackgroundStart(_) => {
                self.debug.set_thread(background_thread(self.tasks));
            }
            CyclePhase::BeforeInputSample
            | CyclePhase::AfterInputSample
            | CyclePhase::BackgroundEnd(_)
            | CyclePhase::BeforeOutputSample
            | CyclePhase::AfterOutputSample
            | CyclePhase::CycleEnd => {}
        }
        Ok(())
    }
}
