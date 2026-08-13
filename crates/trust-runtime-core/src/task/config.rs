//! Portable task configuration metadata.

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
