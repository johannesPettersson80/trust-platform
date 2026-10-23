//! Task scheduling and cycle execution.

#![allow(missing_docs)]

use smol_str::SmolStr;

use crate::program_model::{MethodDef, Stmt, VarDef};

/// Program definition for execution.
#[derive(Debug, Clone)]
pub struct ProgramDef {
    pub name: SmolStr,
    pub vars: Vec<VarDef>,
    pub temps: Vec<VarDef>,
    pub using: Vec<SmolStr>,
    pub body: Vec<Stmt>,
    /// Program-scoped methods and textual actions (e!COCKPIT dialect):
    /// sub-bodies sharing the program singleton instance, dispatched through
    /// the program's method table.
    pub methods: Vec<MethodDef>,
}

#[cfg(test)]
pub(crate) use trust_runtime_core::task::evaluate_task_readiness;
pub use trust_runtime_core::task::{TaskConfig, TaskState};
