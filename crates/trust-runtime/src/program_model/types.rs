use smol_str::SmolStr;
use trust_hir::symbols::ParamDirection;
use trust_hir::TypeId;

use crate::io::IoAddress;

use super::{expr, stmt};

/// Edge-qualified input metadata retained by the host runtime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EdgeInput {
    pub name: SmolStr,
    pub qualifier: trust_hir::symbols::EdgeQualifier,
}

/// Stable internal storage name for an edge input's previous sampled phase.
#[must_use]
pub fn edge_phase_storage_name(owner: &str, input: &str) -> SmolStr {
    SmolStr::new(format!("\0st_rt_edge_phase\0{owner}\0{input}"))
}

/// Parameter declaration for POUs.
#[derive(Debug, Clone)]
pub struct Param {
    pub name: SmolStr,
    pub type_id: TypeId,
    pub direction: ParamDirection,
    pub address: Option<IoAddress>,
    pub default: Option<expr::Expr>,
}

/// Variable declaration with optional initializer.
#[derive(Debug, Clone)]
pub struct VarDef {
    pub name: SmolStr,
    pub type_id: TypeId,
    pub initializer: Option<expr::Expr>,
    pub retain: crate::RetainPolicy,
    pub static_storage: bool,
    pub external: bool,
    pub in_out: bool,
    pub constant: bool,
    pub address: Option<IoAddress>,
}

/// Function definition (used by tests and runtime).
#[derive(Debug, Clone)]
pub struct FunctionDef {
    pub name: SmolStr,
    pub return_type: TypeId,
    pub params: Vec<Param>,
    pub locals: Vec<VarDef>,
    pub static_locals: Vec<VarDef>,
    pub using: Vec<SmolStr>,
    pub body: Vec<stmt::Stmt>,
}

/// Base type for a function block.
#[derive(Debug, Clone)]
pub enum FunctionBlockBase {
    FunctionBlock(SmolStr),
    Class(SmolStr),
}

/// Function block definition (used by tests and runtime).
#[derive(Debug, Clone)]
pub struct FunctionBlockDef {
    pub name: SmolStr,
    pub base: Option<FunctionBlockBase>,
    pub interfaces: Vec<SmolStr>,
    pub params: Vec<Param>,
    pub vars: Vec<VarDef>,
    pub temps: Vec<VarDef>,
    pub using: Vec<SmolStr>,
    pub methods: Vec<MethodDef>,
    pub body: Vec<stmt::Stmt>,
}

/// Method definition for classes and function blocks.
#[derive(Debug, Clone)]
pub struct MethodDef {
    pub name: SmolStr,
    pub return_type: Option<TypeId>,
    pub params: Vec<Param>,
    pub locals: Vec<VarDef>,
    pub static_locals: Vec<VarDef>,
    pub using: Vec<SmolStr>,
    pub body: Vec<stmt::Stmt>,
}

/// Class definition (used by tests and runtime).
#[derive(Debug, Clone)]
pub struct ClassDef {
    pub name: SmolStr,
    pub base: Option<SmolStr>,
    pub interfaces: Vec<SmolStr>,
    pub vars: Vec<VarDef>,
    pub using: Vec<SmolStr>,
    pub methods: Vec<MethodDef>,
}

/// Interface definition (used for metadata and bytecode emission).
#[derive(Debug, Clone)]
pub struct InterfaceDef {
    pub name: SmolStr,
    pub base: Option<SmolStr>,
    pub using: Vec<SmolStr>,
    pub methods: Vec<MethodDef>,
}

#[cfg(test)]
#[path = "types/contract_tests.rs"]
mod contract_tests;
