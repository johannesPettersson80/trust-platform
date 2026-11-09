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
    /// The retention of a function block's or a program's input or output: the `RETAIN` or
    /// `NON_RETAIN` of its section (IEC 61131-3 Ed.3 §6.5.6.1, §6.5.6.2); unspecified for an
    /// in-out and for a function's or a method's parameters, which a call owns.
    pub retain: crate::RetainPolicy,
    /// `{attribute 'noinit'}` on the line above a function block's input or output: a restart
    /// gives it back the value it held (specification 10 §4.8); `false` for any other parameter.
    pub noinit: bool,
    /// `{attribute 'hide'}` on the line above its declaration: the lists a reader browses leave it
    /// out, and it is read and written by its name as any other (specification 10 §4.8).
    pub hidden: bool,
}

/// An argument an instance's declaration gives its `FB_init` (specification 10 §4.8): the name of
/// the input when the argument names it, and the value.
#[derive(Debug, Clone)]
pub struct InitArg {
    pub name: Option<SmolStr>,
    pub value: expr::Expr,
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
    /// The `FB_init` arguments of an instance's declaration, `fb : FB_X(5)`.
    pub init_args: Vec<InitArg>,
    /// `{attribute 'noinit'}` on the line above its declaration: a restart gives the variable back
    /// the value it held (specification 10 §4.8).
    pub noinit: bool,
    /// `{attribute 'hide'}` on the line above its declaration, or a `VAR` local of a POU marked
    /// `{attribute 'hide_all_locals'}`: the lists a reader browses leave it out, and it is read and
    /// written by its name as any other (specification 10 §4.8).
    pub hidden: bool,
    /// `{attribute 'instance-path'}` with `noinit` on a variable of a POU marked
    /// `{attribute 'reflection'}`: it starts with the path of its instance (specification 10 §4.8).
    pub instance_path: bool,
    /// `{attribute 'no-exit'}` on the line above an instance's declaration: the exit of a restart or a
    /// download calls no `FB_exit` of the instance or of what it holds (specification 10 §4.8).
    pub no_exit: bool,
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
    /// The properties a name read reads as one of its variables (specification 44 §5.8).
    pub property_aliases: Vec<PropertyAlias>,
    /// `{attribute 'call_after_init'}` in the block's first line: its marked methods run after each
    /// instance's initialization (specification 10 §4.8).
    pub call_after_init: bool,
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
    /// `{attribute 'call_after_init'}` in the method's first line (specification 10 §4.8).
    pub call_after_init: bool,
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
    /// The properties a name read reads as one of its variables (specification 44 §5.8).
    pub property_aliases: Vec<PropertyAlias>,
}

/// A property of a function block or class whose GET only returns one of its variables
/// (`Prop := Var;`): a name read reads it as that variable, and a name write writes the variable when
/// its SET only assigns it (`Var := Prop;`), as CODESYS monitors such a property (specification 44
/// §5.8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PropertyAlias {
    /// The property's name.
    pub name: SmolStr,
    /// The variable its GET returns.
    pub member: SmolStr,
    /// Whether its SET only assigns that variable.
    pub writable: bool,
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
