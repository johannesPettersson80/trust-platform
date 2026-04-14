//! Task scheduling and cycle execution.

#![allow(missing_docs)]

use smol_str::SmolStr;

use crate::program_model::{Stmt, VarDef};
use crate::value::{Duration, ValueRef};

/// Program definition for execution.
#[derive(Debug, Clone)]
pub struct ProgramDef {
    pub name: SmolStr,
    pub vars: Vec<VarDef>,
    pub temps: Vec<VarDef>,
    pub using: Vec<SmolStr>,
    pub body: Vec<Stmt>,
}

/// Configuration for a task (periodic and/or event-driven).
#[derive(Debug, Clone)]
pub struct TaskConfig {
    pub name: SmolStr,
    pub interval: Duration,
    pub single: Option<SmolStr>,
    pub priority: u32,
    pub programs: Vec<SmolStr>,
    pub fb_instances: Vec<ValueRef>,
}

/// Scheduling state for a task.
#[derive(Debug, Clone)]
pub struct TaskState {
    pub last_single: bool,
    pub last_run: Duration,
    pub overrun_count: u64,
}

impl TaskState {
    #[must_use]
    pub fn new(current_time: Duration) -> Self {
        Self {
            last_single: false,
            last_run: current_time,
            overrun_count: 0,
        }
    }
}
