//! Backend-agnostic runtime/program model and shared semantics.

#![allow(missing_docs)]

pub mod expr;
pub mod ops;
pub mod stmt;
mod types;
mod util;

pub use expr::{Expr, LValue, SizeOfTarget};
pub use ops::{apply_binary, apply_unary, BinaryOp, UnaryOp};
pub use stmt::{CaseLabel, Stmt, StmtResult};
pub use types::{
    ArgValue, CallArg, ClassDef, FunctionBlockBase, FunctionBlockDef, FunctionDef, InterfaceDef,
    MethodDef, Param, VarDef,
};
pub use util::{method_static_storage_owner, static_storage_name};
