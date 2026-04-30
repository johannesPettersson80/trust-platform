//! Backend-agnostic runtime/program model and shared semantics.

#![allow(missing_docs)]

pub mod expr;
pub mod initializers;
pub mod stmt;
mod types;

pub mod ops {
    pub use trust_runtime_core::program_model::ops::*;
}

pub use expr::{Expr, LValue, SizeOfTarget};
pub use initializers::InitializerCatalog;
pub use ops::{apply_binary, apply_unary, BinaryOp, UnaryOp};
pub use stmt::{CaseLabel, Stmt, StmtResult};
pub use trust_runtime_core::program_model::{
    method_static_storage_owner, property_setter_method_name, static_storage_name,
};
pub use types::{
    ArgValue, CallArg, ClassDef, FunctionBlockBase, FunctionBlockDef, FunctionDef, InterfaceDef,
    MethodDef, Param, VarDef,
};
