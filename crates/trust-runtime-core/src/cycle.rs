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
    ready.sort_by_key(|entry| {
        (
            priority_for_index(entry.index),
            entry.due_at.as_nanos(),
            entry.index,
        )
    });
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
}
