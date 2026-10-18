//! The debugger thread of each task (specification 13, "Multi-Task Thread Model"; specification 11
//! §6.13.3).
//!
//! Every task of the resource is one debugger thread, as trust-runtime's `task_thread_id` assigns
//! them: the ids are 1, 2, ... in the order the tasks are declared, a name that a later task
//! repeats keeps the id of its first task, and the programs that run outside any task are one more
//! thread after the last task.

use super::cycle::TaskDescriptor;

/// The thread of task `index` of `tasks`, in declaration order. None for an index beyond the
/// tasks. It is out of line, so the scan's observer, which runs from RAM on a chip, only calls it.
#[inline(never)]
pub(super) fn task_thread(tasks: &[TaskDescriptor], index: usize) -> Option<u32> {
    let name = &tasks.get(index)?.name;
    let mut next = 1_u32;
    for (position, task) in tasks.iter().enumerate() {
        if tasks
            .iter()
            .take(position)
            .any(|earlier| earlier.name == task.name)
        {
            continue;
        }
        if task.name == *name {
            return Some(next);
        }
        next = next.checked_add(1)?;
    }
    None
}

/// The thread of the programs that run outside any task: the one after the last task's. It is out of
/// line for the reason of [`task_thread`].
#[inline(never)]
pub(super) fn background_thread(tasks: &[TaskDescriptor]) -> Option<u32> {
    let mut threads = 0_u32;
    for (position, task) in tasks.iter().enumerate() {
        let repeated = tasks
            .iter()
            .take(position)
            .any(|earlier| earlier.name == task.name);
        if !repeated {
            threads = threads.checked_add(1)?;
        }
    }
    threads.checked_add(1)
}

#[cfg(test)]
mod tests {
    use alloc::vec::Vec;

    use smol_str::SmolStr;

    use super::*;

    fn tasks(names: &[&str]) -> Vec<TaskDescriptor> {
        names
            .iter()
            .map(|name| TaskDescriptor {
                name: SmolStr::new(name),
                priority: 0,
            })
            .collect()
    }

    #[test]
    fn tasks_take_thread_ids_in_declaration_order() {
        // Specification 11 §6.13.3: the threads are numbered 1, 2, ... in the order the resource
        // declares its tasks, whatever their names or priorities. `Slow`, `Fast` and `Mid` are
        // threads 1, 2 and 3, and a task index beyond the resource names no thread. Wrong behaviour
        // caught: ids in name order (`Fast` first), ids from 0, and an out-of-range index that names a
        // thread.
        let tasks = tasks(&["Slow", "Fast", "Mid"]);

        assert_eq!(
            [
                task_thread(&tasks, 0),
                task_thread(&tasks, 1),
                task_thread(&tasks, 2),
                task_thread(&tasks, 3)
            ],
            [Some(1), Some(2), Some(3), None]
        );
    }

    #[test]
    fn a_repeated_task_name_keeps_the_thread_of_its_first_task() {
        // Specification 11 §6.13.3: a name that a later task repeats takes no number of its own. For
        // `A`, `B`, `A`, `C` the threads are 1, 2, 1 and 3. Wrong behaviour caught: a repeated name
        // that takes the next number (the third task would be 3 and `C` 4).
        let tasks = tasks(&["A", "B", "A", "C"]);

        assert_eq!(
            [
                task_thread(&tasks, 0),
                task_thread(&tasks, 1),
                task_thread(&tasks, 2),
                task_thread(&tasks, 3)
            ],
            [Some(1), Some(2), Some(1), Some(3)]
        );
    }

    #[test]
    fn the_programs_outside_every_task_are_one_thread_after_the_last_task() {
        // Specification 11 §6.13.3: the programs that run outside every task are one more thread,
        // numbered after the last task. Three distinct tasks make it thread 4, the repeated name
        // counts once (`A`, `B`, `A` make it 3), and a resource without a task makes it thread 1.
        // Wrong behaviour caught: a background thread that reuses the last task's number, and one that
        // counts a repeated name twice.
        assert_eq!(background_thread(&tasks(&["Slow", "Fast", "Mid"])), Some(4));
        assert_eq!(background_thread(&tasks(&["A", "B", "A"])), Some(3));
        assert_eq!(background_thread(&tasks(&[])), Some(1));
    }
}
