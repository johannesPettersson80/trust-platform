use indexmap::IndexMap;
use trust_hir::types::{ArrayDimensionExt, StructField, UnionVariant};
use trust_hir::{Type, TypeId};

use crate::error::RuntimeError;
use crate::value::{ArrayValue, StructValue, Value};

use super::{coerce_initializer_value, evaluate_initializer, InitContext, MAX_INITIALIZER_DEPTH};

pub(super) fn materialize_default_value(
    ctx: &InitContext<'_>,
    type_id: TypeId,
    depth: u8,
) -> Result<Value, RuntimeError> {
    if depth > MAX_INITIALIZER_DEPTH {
        return Err(RuntimeError::TypeMismatch);
    }
    if let Some(value) = materialize_type_default(ctx, type_id, depth)? {
        return Ok(value);
    }

    let ty = ctx
        .registry
        .get(type_id)
        .ok_or(RuntimeError::TypeMismatch)?;
    match ty {
        Type::Alias { target, .. } => materialize_default_value(ctx, *target, depth + 1),
        Type::Array {
            element,
            dimensions,
        } => materialize_array_default(ctx, *element, dimensions, depth + 1),
        Type::Struct { name, fields } => materialize_struct_default(ctx, name, fields, depth),
        Type::Union { name, variants } => materialize_union_default(ctx, name, variants, depth),
        // Unresolved generic counter FB slots are typed by the call argument on execution.
        Type::AnyInt => Ok(Value::Null),
        _ => crate::value::default_value_for_type_id(type_id, ctx.registry, ctx.profile)
            .map_err(|_| RuntimeError::TypeMismatch),
    }
}

fn materialize_type_default(
    ctx: &InitContext<'_>,
    type_id: TypeId,
    depth: u8,
) -> Result<Option<Value>, RuntimeError> {
    let Some(initializer_id) = ctx.catalog.type_default(type_id) else {
        return Ok(None);
    };
    let expr = ctx
        .catalog
        .initializer(initializer_id)
        .ok_or(RuntimeError::TypeMismatch)?;
    let value = crate::helper_eval::eval_storage_expr_with_stdlib(
        ctx.storage,
        ctx.registry,
        ctx.profile,
        ctx.current_instance,
        Some(ctx.stdlib),
        expr,
    )?;
    coerce_initializer_value(ctx, value, type_id, depth + 1).map(Some)
}

fn materialize_array_default(
    ctx: &InitContext<'_>,
    element: TypeId,
    dimensions: &[(i64, i64)],
    depth: u8,
) -> Result<Value, RuntimeError> {
    if dimensions.iter().any(ArrayDimensionExt::is_wildcard) {
        return Ok(Value::Array(Box::new(ArrayValue::from_canonical_parts(
            Vec::new(),
            dimensions.to_vec(),
        ))));
    }
    let total = array_len(dimensions)?;
    let mut elements = Vec::with_capacity(total);
    for _ in 0..total {
        elements.push(materialize_default_value(ctx, element, depth + 1)?);
    }
    Ok(Value::Array(Box::new(ArrayValue::from_canonical_parts(
        elements,
        dimensions.to_vec(),
    ))))
}

fn materialize_struct_default(
    ctx: &InitContext<'_>,
    name: &smol_str::SmolStr,
    fields: &[StructField],
    depth: u8,
) -> Result<Value, RuntimeError> {
    let values = materialize_member_defaults(ctx, fields, depth + 1)?;
    Ok(Value::Struct(std::sync::Arc::new(
        StructValue::from_canonical_parts(name.clone(), values),
    )))
}

fn materialize_union_default(
    ctx: &InitContext<'_>,
    name: &smol_str::SmolStr,
    variants: &[UnionVariant],
    depth: u8,
) -> Result<Value, RuntimeError> {
    let values = materialize_variant_defaults(ctx, variants, depth + 1)?;
    Ok(Value::Struct(std::sync::Arc::new(
        StructValue::from_canonical_parts(name.clone(), values),
    )))
}

pub(super) fn materialize_member_defaults(
    ctx: &InitContext<'_>,
    fields: &[StructField],
    depth: u8,
) -> Result<IndexMap<smol_str::SmolStr, Value>, RuntimeError> {
    let mut values = IndexMap::new();
    for field in fields {
        let value =
            materialize_member_default(ctx, field.default_initializer, field.type_id, depth)?;
        values.insert(field.name.clone(), value);
    }
    Ok(values)
}

pub(super) fn materialize_variant_defaults(
    ctx: &InitContext<'_>,
    variants: &[UnionVariant],
    depth: u8,
) -> Result<IndexMap<smol_str::SmolStr, Value>, RuntimeError> {
    let mut values = IndexMap::new();
    for variant in variants {
        let value =
            materialize_member_default(ctx, variant.default_initializer, variant.type_id, depth)?;
        values.insert(variant.name.clone(), value);
    }
    Ok(values)
}

fn materialize_member_default(
    ctx: &InitContext<'_>,
    initializer_id: Option<trust_hir::types::InitializerId>,
    type_id: TypeId,
    depth: u8,
) -> Result<Value, RuntimeError> {
    if let Some(initializer_id) = initializer_id {
        let expr = ctx
            .catalog
            .initializer(initializer_id)
            .ok_or(RuntimeError::TypeMismatch)?;
        return evaluate_initializer(
            ctx.storage,
            ctx.registry,
            ctx.catalog,
            ctx.profile,
            ctx.current_instance,
            ctx.stdlib,
            expr,
            type_id,
        );
    }
    materialize_default_value(ctx, type_id, depth + 1)
}

pub(super) fn array_len(dimensions: &[(i64, i64)]) -> Result<usize, RuntimeError> {
    dimensions.iter().try_fold(1usize, |acc, (lower, upper)| {
        if upper < lower {
            return Err(RuntimeError::TypeMismatch);
        }
        let width = upper
            .checked_sub(*lower)
            .and_then(|value| value.checked_add(1))
            .ok_or(RuntimeError::TypeMismatch)?;
        let width = usize::try_from(width).map_err(|_| RuntimeError::TypeMismatch)?;
        acc.checked_mul(width).ok_or(RuntimeError::TypeMismatch)
    })
}
