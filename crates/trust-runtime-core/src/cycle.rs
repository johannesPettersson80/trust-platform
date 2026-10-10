//! Portable cycle scheduling helpers.

use crate::value::Duration;

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
}
