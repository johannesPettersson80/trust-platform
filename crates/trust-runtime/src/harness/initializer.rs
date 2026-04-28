use trust_hir::types::{StructField, TypeRegistry, UnionVariant};
use trust_hir::{Type, TypeId};

use crate::error::RuntimeError;
use crate::memory::{InstanceId, VariableStorage};
use crate::program_model::{Expr, InitializerCatalog};
use crate::stdlib::StandardLibrary;
use crate::value::{ArrayValue, DateTimeProfile, StructValue, Value};

mod defaults;

use defaults::{
    array_len, materialize_default_value, materialize_member_defaults, materialize_variant_defaults,
};

const MAX_INITIALIZER_DEPTH: u8 = 64;

pub(super) struct InitContext<'a> {
    pub(super) storage: &'a VariableStorage,
    pub(super) registry: &'a TypeRegistry,
    pub(super) catalog: &'a InitializerCatalog,
    pub(super) profile: &'a DateTimeProfile,
    pub(super) current_instance: Option<InstanceId>,
    pub(super) stdlib: &'a StandardLibrary,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn evaluate_initializer(
    storage: &VariableStorage,
    registry: &TypeRegistry,
    catalog: &InitializerCatalog,
    profile: &DateTimeProfile,
    current_instance: Option<InstanceId>,
    stdlib: &StandardLibrary,
    expr: &Expr,
    type_id: TypeId,
) -> Result<Value, RuntimeError> {
    let value = crate::helper_eval::eval_storage_expr_with_stdlib(
        storage,
        registry,
        profile,
        current_instance,
        Some(stdlib),
        expr,
    )?;
    apply_aggregate_overrides(
        storage,
        registry,
        catalog,
        profile,
        current_instance,
        stdlib,
        value,
        type_id,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn apply_aggregate_overrides(
    storage: &VariableStorage,
    registry: &TypeRegistry,
    catalog: &InitializerCatalog,
    profile: &DateTimeProfile,
    current_instance: Option<InstanceId>,
    stdlib: &StandardLibrary,
    value: Value,
    type_id: TypeId,
) -> Result<Value, RuntimeError> {
    let ctx = InitContext {
        storage,
        registry,
        catalog,
        profile,
        current_instance,
        stdlib,
    };
    coerce_initializer_value(&ctx, value, type_id, 0)
}

pub(crate) fn default_value_for_type_id(
    storage: &VariableStorage,
    registry: &TypeRegistry,
    catalog: &InitializerCatalog,
    profile: &DateTimeProfile,
    current_instance: Option<InstanceId>,
    stdlib: &StandardLibrary,
    type_id: TypeId,
) -> Result<Value, RuntimeError> {
    let ctx = InitContext {
        storage,
        registry,
        catalog,
        profile,
        current_instance,
        stdlib,
    };
    materialize_default_value(&ctx, type_id, 0)
}

pub(crate) fn coerce_evaluated_initializer_value(
    value: Value,
    type_id: TypeId,
    registry: &TypeRegistry,
    profile: &DateTimeProfile,
) -> Result<Value, crate::harness::types::CompileError> {
    crate::harness::coerce_initializer_value_to_type(value, type_id, registry, profile)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn coerce_initializer_value(
    ctx: &InitContext<'_>,
    value: Value,
    type_id: TypeId,
    depth: u8,
) -> Result<Value, RuntimeError> {
    if depth > MAX_INITIALIZER_DEPTH {
        return Err(RuntimeError::TypeMismatch);
    }
    let ty = ctx
        .registry
        .get(type_id)
        .ok_or(RuntimeError::TypeMismatch)?;
    match ty {
        Type::Alias { target, .. } => coerce_initializer_value(ctx, value, *target, depth + 1),
        Type::Array {
            element,
            dimensions,
        } => coerce_array_initializer(ctx, value, *element, dimensions, depth + 1),
        Type::Struct { fields, .. } => {
            coerce_struct_initializer(ctx, value, type_id, fields, depth + 1)
        }
        Type::Union { variants, .. } => {
            coerce_union_initializer(ctx, value, type_id, variants, depth + 1)
        }
        _ => crate::harness::coerce_initializer_value_to_type(
            value,
            type_id,
            ctx.registry,
            ctx.profile,
        )
        .map_err(|_| RuntimeError::TypeMismatch),
    }
}

fn coerce_array_initializer(
    ctx: &InitContext<'_>,
    value: Value,
    element: TypeId,
    dimensions: &[(i64, i64)],
    depth: u8,
) -> Result<Value, RuntimeError> {
    let Value::Array(array) = value else {
        return Err(RuntimeError::TypeMismatch);
    };
    let expected_len = array_len(dimensions)?;
    if array.elements().len() > expected_len {
        return Err(RuntimeError::TypeMismatch);
    }
    let mut elements = Vec::with_capacity(expected_len);
    for value in array.elements() {
        elements.push(coerce_initializer_value(
            ctx,
            value.clone(),
            element,
            depth + 1,
        )?);
    }
    while elements.len() < expected_len {
        elements.push(materialize_default_value(ctx, element, depth + 1)?);
    }
    Ok(Value::Array(Box::new(ArrayValue::from_canonical_parts(
        elements,
        dimensions.to_vec(),
    ))))
}

fn coerce_struct_initializer(
    ctx: &InitContext<'_>,
    value: Value,
    type_id: TypeId,
    fields: &[StructField],
    depth: u8,
) -> Result<Value, RuntimeError> {
    let Value::Struct(struct_value) = value else {
        return Err(RuntimeError::TypeMismatch);
    };
    let mut values = materialize_member_defaults(ctx, fields, depth + 1)?;
    for (name, value) in struct_value.fields() {
        let field = fields
            .iter()
            .find(|field| field.name.eq_ignore_ascii_case(name.as_str()))
            .ok_or(RuntimeError::TypeMismatch)?;
        values.insert(
            field.name.clone(),
            coerce_initializer_value(ctx, value.clone(), field.type_id, depth + 1)?,
        );
    }
    StructValue::new(ctx.registry, type_id, values)
        .map(|value| Value::Struct(std::sync::Arc::new(value)))
        .map_err(|_| RuntimeError::TypeMismatch)
}

fn coerce_union_initializer(
    ctx: &InitContext<'_>,
    value: Value,
    type_id: TypeId,
    variants: &[UnionVariant],
    depth: u8,
) -> Result<Value, RuntimeError> {
    let Value::Struct(struct_value) = value else {
        return Err(RuntimeError::TypeMismatch);
    };
    let mut values = materialize_variant_defaults(ctx, variants, depth + 1)?;
    for (name, value) in struct_value.fields() {
        let variant = variants
            .iter()
            .find(|variant| variant.name.eq_ignore_ascii_case(name.as_str()))
            .ok_or(RuntimeError::TypeMismatch)?;
        values.insert(
            variant.name.clone(),
            coerce_initializer_value(ctx, value.clone(), variant.type_id, depth + 1)?,
        );
    }
    StructValue::new(ctx.registry, type_id, values)
        .map(|value| Value::Struct(std::sync::Arc::new(value)))
        .map_err(|_| RuntimeError::TypeMismatch)
}
