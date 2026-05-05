//! Evaluator entry point.

#![allow(missing_docs)]

#[cfg(test)]
use indexmap::IndexMap;
#[cfg(test)]
use smol_str::SmolStr;
#[cfg(test)]
use trust_hir::symbols::ParamDirection;
#[cfg(test)]
use trust_hir::types::TypeRegistry;
#[cfg(test)]
use trust_hir::TypeId;

#[cfg(test)]
use crate::error::RuntimeError;
#[cfg(test)]
use crate::instance::{create_class_instance, create_fb_instance};
#[cfg(test)]
use crate::memory::{InstanceId, VariableStorage};
#[cfg(test)]
use crate::stdlib::{fbs, StandardLibrary};
#[cfg(test)]
use crate::value::{default_value_for_type_id, DateTimeProfile, Duration, Value};

pub use crate::program_model::{
    method_static_storage_owner, static_storage_name, ArgValue, CallArg, ClassDef,
    FunctionBlockBase, FunctionBlockDef, FunctionDef, InterfaceDef, MethodDef, Param, VarDef,
};

pub mod expr;
pub mod ops;
pub mod stmt;

#[cfg(test)]
fn init_failed_display(
    owner: &SmolStr,
    variable: &SmolStr,
    error: impl core::fmt::Display,
) -> RuntimeError {
    RuntimeError::InitFailed {
        owner: owner.clone(),
        variable: variable.clone(),
        error: SmolStr::new(error.to_string()),
    }
}

#[cfg(test)]
fn init_failed_debug(
    owner: &SmolStr,
    variable: &SmolStr,
    error: impl core::fmt::Debug,
) -> RuntimeError {
    RuntimeError::InitFailed {
        owner: owner.clone(),
        variable: variable.clone(),
        error: SmolStr::new(format!("{error:?}")),
    }
}

#[cfg(test)]
fn current_init_owner(ctx: &EvalContext<'_>) -> SmolStr {
    ctx.storage
        .current_frame()
        .map(|frame| frame.owner.clone())
        .unwrap_or_else(|| SmolStr::new("eval"))
}

#[cfg(test)]
include!("types.rs");
#[cfg(test)]
include!("calls.rs");
#[cfg(test)]
include!("bindings.rs");
#[cfg(test)]
include!("locals.rs");
#[cfg(test)]
include!("outputs.rs");

#[cfg(test)]
mod tests;
