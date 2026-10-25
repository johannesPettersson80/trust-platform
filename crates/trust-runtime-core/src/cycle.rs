//! Portable cycle scheduling helpers, and the boundaries of a scan that a device's services observe
//! (specification 44 §8.7).

use crate::error::RuntimeError;
use crate::io_image::FlatImages;
use crate::native::variables::{CompiledAccessPoint, CompiledVariables};
use crate::value::Duration;

/// A boundary of a scan at which a device's services act (specification 44 §8.7).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CyclePhase {
    /// Apply pending engineering writes before physical input acquisition.
    CycleStart,
    /// Acquire physical inputs, staged debug I/O and input forces.
    BeforeInputSample,
    /// Record dropped activations in declaration order before priority execution.
    TaskOverrun {
        /// Task index in the resource's declaration order.
        index: usize,
        /// Periodic activations dropped before this single ready activation.
        missed: u64,
    },
    /// Begin a due task, identified by its index in the resource's declaration order.
    TaskStart(usize),
    /// Finish a due task with the same index.
    TaskEnd(usize),
    /// Begin the body of a program instance in a task or outside every task, by its entry's index
    /// in the image's entry table (specification 44 §5.4); a function block instance that a task
    /// runs on its own is a body of the program instance that holds it.
    ProgramStart(usize),
    /// Finish the body of the same entry, also when it faulted.
    ProgramEnd(usize),
    /// Begin the programs that run outside every task.
    BackgroundStart,
    /// Finish the programs that run outside every task.
    BackgroundEnd,
    /// Save retain state before any output image sampling.
    BeforeOutputSample,
    /// The outputs are sampled; apply forces, capture protocols and drive I/O.
    AfterOutputSample,
    /// Publish the successful cycle's final observations.
    CycleEnd,
}

/// What a device's services do at the boundaries of a compiled program's scan (specification 44
/// §8.7, §13.3). A returned error fails the scan.
pub trait CycleHooks {
    /// A boundary of the scan at its logical time `now`, with the program's process images `%I`,
    /// `%Q` and `%M` as the last completed scan left them, the scan's own at `AfterOutputSample`.
    /// The default does nothing.
    ///
    /// # Errors
    ///
    /// An error fails the scan.
    fn on_compiled_phase(
        &mut self,
        _phase: CyclePhase,
        _now: Duration,
        _images: &FlatImages,
    ) -> Result<(), RuntimeError> {
        Ok(())
    }

    /// A boundary of the scan at which Live Values reach the program's variables and process images
    /// through `variables` (specification 44 §13.3), at the scan's logical time `now`. The default
    /// does nothing.
    ///
    /// # Errors
    ///
    /// An error fails the scan.
    fn on_compiled_access(
        &mut self,
        _point: CompiledAccessPoint,
        _now: Duration,
        _variables: &mut dyn CompiledVariables,
    ) -> Result<(), RuntimeError> {
        Ok(())
    }
}

/// Task selected for execution in the current runtime cycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReadyTask {
    /// Index of the ready task in the host task table.
    pub index: usize,
    /// Logical time at which the task became due.
    pub due_at: Duration,
}

/// Sort ready tasks by priority, due time, and stable task-table order.
pub fn sort_ready_tasks_by_priority(
    ready: &mut [ReadyTask],
    mut priority_for_index: impl FnMut(usize) -> u32,
) {
    let result: Result<(), core::convert::Infallible> =
        try_sort_ready_tasks_by_priority(ready, &mut priority_for_index, || Ok(()));
    match result {
        Ok(()) => (),
        Err(impossible) => match impossible {},
    }
}

/// The engine uses the same ordering with fallible per-operation budget charges.
pub(crate) fn try_sort_ready_tasks_by_priority<E>(
    ready: &mut [ReadyTask],
    mut priority_for_index: impl FnMut(usize) -> u32,
    mut charge: impl FnMut() -> Result<(), E>,
) -> Result<(), E> {
    // Fewer than two tasks are in order: the sort would compare, swap and charge nothing.
    if ready.len() < 2 {
        return Ok(());
    }
    crate::sort::heap_sort(ready.len(), &mut |operation| {
        use crate::sort::Operation;
        match operation {
            Operation::Charge => {
                charge()?;
                Ok(false)
            }
            Operation::Less(left, right) => {
                let left = ready[left];
                let right = ready[right];
                Ok((
                    priority_for_index(left.index),
                    left.due_at.as_nanos(),
                    left.index,
                ) < (
                    priority_for_index(right.index),
                    right.due_at.as_nanos(),
                    right.index,
                ))
            }
            Operation::Swap(left, right) => {
                ready.swap(left, right);
                Ok(false)
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{sort_ready_tasks_by_priority, ReadyTask};
    use crate::value::Duration;

    #[test]
    fn ready_task_sort_preserves_priority_due_time_and_stable_index_order() {
        let mut ready = [
            ReadyTask {
                index: 4,
                due_at: Duration::from_millis(10),
            },
            ReadyTask {
                index: 2,
                due_at: Duration::from_millis(5),
            },
            ReadyTask {
                index: 1,
                due_at: Duration::from_millis(5),
            },
            ReadyTask {
                index: 3,
                due_at: Duration::from_millis(2),
            },
        ];
        let priorities = [0, 10, 5, 5, 0];

        sort_ready_tasks_by_priority(&mut ready, |index| priorities[index]);

        assert_eq!(
            ready.map(|entry| entry.index),
            [4, 3, 2, 1],
            "priority wins first, then earlier due_at, then lower task index"
        );
    }
    #[test]
    fn shared_ready_sort_matches_priority_due_and_index_order_on_reversed_tasks() {
        let mut ready = (0..513)
            .rev()
            .map(|index| ReadyTask {
                index,
                due_at: Duration::from_millis((index % 11) as i64),
            })
            .collect::<alloc::vec::Vec<_>>();
        let mut expected = ready.clone();
        expected.sort_by_key(|entry| {
            (
                (entry.index % 7) as u32,
                entry.due_at.as_nanos(),
                entry.index,
            )
        });
        sort_ready_tasks_by_priority(&mut ready, |index| (index % 7) as u32);
        assert_eq!(ready, expected);
    }

    #[test]
    fn engine_ready_sort_stops_before_mutation_when_work_is_exhausted() {
        let mut ready = [
            ReadyTask {
                index: 1,
                due_at: Duration::ZERO,
            },
            ReadyTask {
                index: 0,
                due_at: Duration::ZERO,
            },
        ];
        let before = ready;
        let error = super::try_sort_ready_tasks_by_priority(&mut ready, |_| 0, || Err("exhausted"));
        assert_eq!(error, Err("exhausted"));
        assert_eq!(ready, before);
    }

    /// One ready task, or none, is in order: the sort compares, swaps and charges nothing, so an
    /// exhausted allowance does not refuse it, as the heap sort makes no operation for fewer than
    /// two entries. Catches a charge made for a sort with nothing to order.
    #[test]
    fn a_single_ready_task_is_in_order_without_a_charge() {
        let mut one = [ReadyTask {
            index: 3,
            due_at: Duration::ZERO,
        }];
        let sorted = super::try_sort_ready_tasks_by_priority(&mut one, |_| 0, || Err("exhausted"));
        assert_eq!(sorted, Ok(()));
        assert_eq!(one.map(|entry| entry.index), [3]);
        let mut none: [ReadyTask; 0] = [];
        let sorted = super::try_sort_ready_tasks_by_priority(&mut none, |_| 0, || Err("exhausted"));
        assert_eq!(sorted, Ok(()));
    }
}
