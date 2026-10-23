//! Entry points select the ABI once; recursive evaluation retains that context.
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
    value::{DateTimeProfile, StorageAbi, Value},
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
#[cfg(test)]
pub(super) fn eval_storage_expr_with_stdlib(
    storage: &VariableStorage,
    registry: &TypeRegistry,
    profile: &DateTimeProfile,
    instance: Option<InstanceId>,
    stdlib: Option<&StandardLibrary>,
    expr: &Expr,
) -> Result<Value, RuntimeError> {
    eval_storage_expr_with_abi(
        storage,
        registry,
        StorageAbi::default(),
        profile,
        instance,
        stdlib,
        expr,
    )
}
pub(crate) fn eval_storage_expr_with_abi(
    storage: &VariableStorage,
    registry: &TypeRegistry,
    abi: StorageAbi,
    profile: &DateTimeProfile,
    instance: Option<InstanceId>,
    stdlib: Option<&StandardLibrary>,
    expr: &Expr,
) -> Result<Value, RuntimeError> {
    eval_storage_expr_with_types(
        storage,
        &EvaluationTypes { registry, abi },
        profile,
        instance,
        stdlib,
        expr,
    )
}
#[cfg(test)]
pub(super) fn read_storage_lvalue(
    storage: &VariableStorage,
    registry: &TypeRegistry,
    profile: &DateTimeProfile,
    instance: Option<InstanceId>,
    target: &LValue,
) -> Result<Value, RuntimeError> {
    read_storage_lvalue_with_abi(
        storage,
        registry,
        StorageAbi::default(),
        profile,
        instance,
        target,
    )
}
pub(crate) fn read_storage_lvalue_with_abi(
    storage: &VariableStorage,
    registry: &TypeRegistry,
    abi: StorageAbi,
    profile: &DateTimeProfile,
    instance: Option<InstanceId>,
    target: &LValue,
) -> Result<Value, RuntimeError> {
    read_storage_lvalue_with_types(
        storage,
        &EvaluationTypes { registry, abi },
        profile,
        instance,
        target,
    )
}
#[cfg(test)]
pub(super) fn write_storage_lvalue(
    storage: &mut VariableStorage,
    registry: &TypeRegistry,
    profile: &DateTimeProfile,
    instance: Option<InstanceId>,
    target: &LValue,
    value: Value,
) -> Result<(), RuntimeError> {
    write_storage_lvalue_with_abi(
        storage,
        registry,
        StorageAbi::default(),
        profile,
        instance,
        target,
        value,
    )
}
pub(crate) fn write_storage_lvalue_with_abi(
    storage: &mut VariableStorage,
    registry: &TypeRegistry,
    abi: StorageAbi,
    profile: &DateTimeProfile,
    instance: Option<InstanceId>,
    target: &LValue,
    value: Value,
) -> Result<(), RuntimeError> {
    write_storage_lvalue_with_types(
        storage,
        &EvaluationTypes { registry, abi },
        profile,
        instance,
        target,
        value,
    )
}
