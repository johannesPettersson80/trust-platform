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
    /// Its `METHOD`s (specification 04 §4) and its textual `ACTION`s, each an action as a method
    /// without parameters or result (specification 04 §8), in declaration order.
    pub methods: Vec<MethodDef>,
}

pub use trust_runtime_core::task::{TaskConfig, TaskState};
