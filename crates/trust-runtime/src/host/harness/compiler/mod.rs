//! Harness lowering and configuration compilation.

#![allow(missing_docs)]

mod config;
mod model;
mod pou;
mod types;
mod vars;

pub(super) use config::{
    lower_configuration, lower_root_global_var_blocks, resolve_program_type_name,
};
pub(super) use model::{
    AccessDecl, AccessPart, AccessPath, CompileTimeConsts, ConfigInit, GlobalInit, LoweringContext,
    LoweringInputs, ProgramInstanceConfig, ResolvedAccess, WildcardRequirement,
};
pub(super) use pou::{
    lower_classes, lower_function_blocks, lower_functions, lower_interfaces, lower_programs,
};
pub(super) use types::{
    class_type_name, function_block_type_name, interface_type_name, lower_type_decls,
    lower_type_ref, predeclare_classes, predeclare_function_blocks, predeclare_interfaces,
    resolve_named_type, resolve_type_name,
};
