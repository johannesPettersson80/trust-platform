//! Entry points of the detached helper evaluation, which pass their type context down.
use super::{
    storage_context::EvaluationTypes,
    storage_expr::eval_storage_expr_with_types,
    storage_lvalue::{read_storage_lvalue_with_types, write_storage_lvalue_with_types},
};
use crate::{
    error::RuntimeError,
    memory::{InstanceId, VariableStorage},
    program_model::{Expr, LValue},
    stdlib::StandardLibrary,
    value::{DateTimeProfile, Value},
};
use trust_hir::types::TypeRegistry;

#[cfg(test)]
pub(super) fn eval_storage_expr(
    storage: &VariableStorage,
    registry: &TypeRegistry,
    profile: &DateTimeProfile,
    instance: Option<InstanceId>,
    expr: &Expr,
) -> Result<Value, RuntimeError> {
    eval_storage_expr_with_stdlib(storage, registry, profile, instance, None, expr)
}
pub(crate) fn eval_storage_expr_with_stdlib(
    storage: &VariableStorage,
    registry: &TypeRegistry,
    profile: &DateTimeProfile,
    instance: Option<InstanceId>,
    stdlib: Option<&StandardLibrary>,
    expr: &Expr,
) -> Result<Value, RuntimeError> {
    eval_storage_expr_with_types(
        storage,
        &EvaluationTypes { registry },
        profile,
        instance,
        stdlib,
        expr,
    )
}
pub(crate) fn read_storage_lvalue(
    storage: &VariableStorage,
    registry: &TypeRegistry,
    profile: &DateTimeProfile,
    instance: Option<InstanceId>,
    target: &LValue,
) -> Result<Value, RuntimeError> {
    read_storage_lvalue_with_types(
        storage,
        &EvaluationTypes { registry },
        profile,
        instance,
        target,
    )
}
pub(crate) fn write_storage_lvalue(
    storage: &mut VariableStorage,
    registry: &TypeRegistry,
    profile: &DateTimeProfile,
    instance: Option<InstanceId>,
    target: &LValue,
    value: Value,
) -> Result<(), RuntimeError> {
    write_storage_lvalue_with_types(
        storage,
        &EvaluationTypes { registry },
        profile,
        instance,
        target,
        value,
    )
}
