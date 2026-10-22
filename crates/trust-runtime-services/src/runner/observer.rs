//! What the resource loop records of a scan at the engine's cycle phases: the overrun events, and
//! the task, program and overrun metrics, as trust-runtime's hosted cycle records them
//! (`runtime/installed_cycle.rs`). The events that every scan makes, its cycle's and its task's
//! start and end, are not recorded: the event store keeps none of them (specification 11, the event
//! store), and no caller drains them from a chip.

use trust_runtime_core::error::RuntimeError;
#[cfg(feature = "native")]
use trust_runtime_core::io_image::FlatImages;
use trust_runtime_core::ports::{MonotonicClock, MonotonicInstant};
use trust_runtime_core::value::Duration;
use trust_runtime_core::vm::{CycleAccess, CycleHooks, CyclePhase};

use super::cycle::TaskDescriptor;
use super::state::Records;
use crate::events::RuntimeEvent;

/// The loop's [`CycleHooks`] for one scan.
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
    task_started: Option<MonotonicInstant>,
    program_started: Option<MonotonicInstant>,
}

impl<'a, C: MonotonicClock> ScanObserver<'a, C> {
    /// An observer of a scan of the program whose tasks are `tasks`.
    pub(super) fn new(tasks: &'a [TaskDescriptor], records: &'a Records<C>, clock: &'a C) -> Self {
        Self {
            tasks,
            records,
            clock,
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
            CyclePhase::TaskStart(_) => self.task_started = Some(self.clock.now()),
            CyclePhase::TaskEnd(index) => {
                let task = self.task(index)?;
                if let Some(started) = self.task_started.take() {
                    let duration = self.since(started);
                    self.records
                        .record(|metrics| metrics.record_task(&task.name, duration));
                }
            }
            CyclePhase::CycleStart
            | CyclePhase::BeforeInputSample
            | CyclePhase::AfterInputSample
            | CyclePhase::BackgroundStart(_)
            | CyclePhase::BackgroundEnd(_)
            | CyclePhase::BeforeOutputSample
            | CyclePhase::AfterOutputSample
            | CyclePhase::CycleEnd => {}
        }
        Ok(())
    }
}

impl<C: MonotonicClock> CycleHooks for ScanObserver<'_, C> {
    fn has_program_hooks(&self) -> bool {
        true
    }

    fn program_started(&mut self, _name: &str) {
        self.program_started = Some(self.clock.now());
    }

    fn program_finished(&mut self, name: &str) {
        if let Some(started) = self.program_started.take() {
            let duration = self.since(started);
            self.records
                .record(|metrics| metrics.record_call("program", name, duration));
        }
    }

    /// The phases whose events and metrics it records.
    fn observes(&self, phase: CyclePhase) -> bool {
        matches!(
            phase,
            CyclePhase::TaskOverrun { .. } | CyclePhase::TaskStart(_) | CyclePhase::TaskEnd(_)
        )
    }

    fn on_phase(
        &mut self,
        phase: CyclePhase,
        access: &mut CycleAccess<'_, '_>,
    ) -> Result<(), RuntimeError> {
        self.record(phase, access.logical_time())
    }

    /// A compiled program's scan is recorded at the same phases (specification 44 §8.7).
    #[cfg(feature = "native")]
    fn on_compiled_phase(
        &mut self,
        phase: CyclePhase,
        now: Duration,
        _images: &FlatImages,
    ) -> Result<(), RuntimeError> {
        self.record(phase, now)
    }
}
