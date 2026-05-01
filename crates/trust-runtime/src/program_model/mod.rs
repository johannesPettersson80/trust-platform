//! Backend-agnostic runtime/program model and shared semantics.

#![allow(missing_docs)]

pub mod stmt;
mod types;

pub mod expr {
    pub use trust_runtime_core::program_model::expr::*;
}

pub mod initializers {
    pub use trust_runtime_core::program_model::initializers::*;
}

pub mod ops {
    pub use trust_runtime_core::program_model::ops::*;
}

pub use expr::{ArgValue, CallArg, Expr, LValue, SizeOfTarget};
pub use initializers::InitializerCatalog;
pub use ops::{apply_binary, apply_unary, BinaryOp, UnaryOp};
pub use stmt::{CaseLabel, Stmt, StmtResult};
pub use trust_runtime_core::program_model::{
    method_static_storage_owner, property_setter_method_name, static_storage_name,
};
pub use types::{
    ClassDef, FunctionBlockBase, FunctionBlockDef, FunctionDef, InterfaceDef, MethodDef, Param,
    VarDef,
};
