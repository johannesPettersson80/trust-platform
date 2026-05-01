//! Task scheduling and cycle execution.

#![allow(missing_docs)]

use smol_str::SmolStr;

use crate::program_model::{Stmt, VarDef};

/// Program definition for execution.
#[derive(Debug, Clone)]
pub struct ProgramDef {
    pub name: SmolStr,
    pub vars: Vec<VarDef>,
    pub temps: Vec<VarDef>,
    pub using: Vec<SmolStr>,
    pub body: Vec<Stmt>,
}

pub use trust_runtime_core::task::{evaluate_task_readiness, TaskConfig, TaskReadiness, TaskState};
