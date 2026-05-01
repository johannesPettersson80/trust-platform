//! Portable task configuration records.

use alloc::vec::Vec;
use smol_str::SmolStr;

use crate::value::{Duration, ValueRef};

/// Configuration for a task (periodic and/or event-driven).
#[derive(Debug, Clone)]
pub struct TaskConfig {
    /// Task name.
    pub name: SmolStr,
    /// Periodic interval. Zero means no periodic interval.
    pub interval: Duration,
    /// Optional event input name.
    pub single: Option<SmolStr>,
    /// Lower values run before higher values when tasks are ready together.
    pub priority: u32,
    /// Program instances executed by this task.
    pub programs: Vec<SmolStr>,
    /// Function block instances executed by this task.
    pub fb_instances: Vec<ValueRef>,
}

#[cfg(test)]
mod tests {
    use super::TaskConfig;
    use crate::value::Duration;
    use alloc::vec;
    use smol_str::SmolStr;

    #[test]
    fn task_config_preserves_periodic_and_event_fields() {
        let task = TaskConfig {
            name: SmolStr::new("Fast"),
            interval: Duration::from_millis(10),
            single: Some(SmolStr::new("Start")),
            priority: 2,
            programs: vec![SmolStr::new("Main")],
            fb_instances: Vec::new(),
        };

        assert_eq!(task.name.as_str(), "Fast");
        assert_eq!(task.interval, Duration::from_millis(10));
        assert_eq!(task.single.as_deref(), Some("Start"));
        assert_eq!(task.priority, 2);
        assert_eq!(task.programs, vec![SmolStr::new("Main")]);
        assert!(task.fb_instances.is_empty());
    }
}
