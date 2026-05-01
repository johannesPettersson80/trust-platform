use indexmap::IndexMap;
use smol_str::SmolStr;
use trust_hir::TypeId;

use crate::error::RuntimeError;
use crate::instance::fb_initializer_target;
use crate::memory::{FrameId, InstanceId, MemoryLocation};
use crate::program_model::{
    apply_binary, apply_unary, static_storage_name, ArgValue, CallArg, Expr, FunctionBlockDef,
    LValue, SizeOfTarget,
};
use crate::stdlib::{conversions, StdParams};
use crate::value::{
    checked_array_offset_i64, materialize_value_path, parse_partial_access, read_partial_access,
    read_string_element, ref_indices_from_iter, size_of_type, ArrayValue, PartialAccessError,
    RefSegment, SizeOfError, StructValue, Value, ValueRef,
};

use super::super::frames::VmFrame;
use super::VmPouInitPlan;

pub(super) fn apply_fb_instance_initializer_from_vm_frame(
    runtime: &mut crate::Runtime,
    plan: &VmPouInitPlan,
    frame: &VmFrame,
    visible_slots: usize,
    target_instance_id: InstanceId,
    fb: &FunctionBlockDef,
    expr: &Expr,
) -> Result<(), RuntimeError> {
    let Expr::StructInitializer(fields) = expr else {
        return Err(RuntimeError::TypeMismatch);
    };

    let mut seen = Vec::<SmolStr>::new();
    for (name, value_expr) in fields {
        if seen
            .iter()
            .any(|existing| existing.eq_ignore_ascii_case(name.as_str()))
        {
            return Err(RuntimeError::TypeMismatch);
        }
        seen.push(name.clone());

        let (canonical_name, type_id) = fb_initializer_target(fb, name)?;
        let value = evaluate_initializer_from_vm_frame(
            runtime,
            plan,
            frame,
            visible_slots,
            value_expr,
            type_id,
        )?;
        let Some(reference) = runtime
            .storage()
            .ref_for_instance_recursive(target_instance_id, canonical_name.as_str())
        else {
            return Err(RuntimeError::TypeMismatch);
        };
        if !runtime.storage_mut().write_by_ref(reference, value) {
            return Err(RuntimeError::TypeMismatch);
        }
    }
    Ok(())
}

pub(super) fn evaluate_initializer_from_vm_frame(
    runtime: &crate::Runtime,
    plan: &VmPouInitPlan,
    frame: &VmFrame,
    visible_slots: usize,
    expr: &Expr,
    type_id: TypeId,
) -> Result<Value, RuntimeError> {
    let profile = runtime.profile();
    let ctx = VmLocalExprContext {
        runtime,
        plan,
        frame,
        visible_slots,
        profile,
    };
    let value = eval_vm_local_expr(&ctx, expr)?;
    crate::harness::initializer::apply_aggregate_overrides(
        runtime.storage(),
        runtime.registry(),
        runtime.initializer_catalog(),
        &ctx.profile,
        frame.runtime_instance,
        runtime.stdlib(),
        value,
        type_id,
    )
}

struct VmLocalExprContext<'a> {
    runtime: &'a crate::Runtime,
    plan: &'a VmPouInitPlan,
    frame: &'a VmFrame,
    visible_slots: usize,
    profile: crate::value::DateTimeProfile,
}

fn eval_vm_local_expr(ctx: &VmLocalExprContext<'_>, expr: &Expr) -> Result<Value, RuntimeError> {
    match expr {
        Expr::Literal(value) => Ok(value.clone()),
        Expr::ArrayInitializer(elements) => {
            let values = eval_vm_local_array_elements(ctx, elements)?;
            let len = values.len() as i64;
            ArrayValue::from_untyped_parts(values, vec![(1, len)])
                .map(|value| Value::Array(Box::new(value)))
                .map_err(|_| RuntimeError::TypeMismatch)
        }
        Expr::StructInitializer(fields) => eval_vm_local_struct_initializer(ctx, fields),
        Expr::This => ctx
            .frame
            .runtime_instance
            .map(Value::Instance)
            .ok_or(RuntimeError::TypeMismatch),
        Expr::Super => {
            let current = ctx
                .frame
                .runtime_instance
                .ok_or(RuntimeError::TypeMismatch)?;
            let instance = ctx
                .runtime
                .storage()
                .get_instance(current)
                .ok_or(RuntimeError::NullReference)?;
            instance
                .parent
                .map(Value::Instance)
                .ok_or(RuntimeError::TypeMismatch)
        }
        Expr::SizeOf(target) => eval_vm_local_size_of(ctx, target),
        Expr::Name(name) => read_vm_local_name(ctx, name),
        Expr::Call { target, args } => eval_vm_local_call(ctx, target, args),
        Expr::Unary { op, expr } => {
            let value = eval_vm_local_expr(ctx, expr)?;
            apply_unary(*op, value)
        }
        Expr::Binary { op, left, right } => {
            let left_value = eval_vm_local_expr(ctx, left)?;
            if *op == crate::program_model::BinaryOp::And
                && matches!(left_value, Value::Bool(false))
            {
                return Ok(Value::Bool(false));
            }
            if *op == crate::program_model::BinaryOp::Or && matches!(left_value, Value::Bool(true))
            {
                return Ok(Value::Bool(true));
            }
            let right_value = eval_vm_local_expr(ctx, right)?;
            apply_binary(*op, left_value, right_value, &ctx.profile)
        }
        Expr::Index { target, indices } => {
            let target_value = eval_vm_local_expr(ctx, target)?;
            let index_values = indices
                .iter()
                .map(|index| eval_vm_local_expr(ctx, index))
                .collect::<Result<Vec<_>, _>>()?;
            read_vm_local_indices(target_value, &index_values)
        }
        Expr::Field { target, field } => {
            if let Some(qualified) = qualified_field_expr_name(expr) {
                if let Ok(value) = read_vm_local_name(ctx, &qualified) {
                    return Ok(value);
                }
            }
            let target_value = eval_vm_local_expr(ctx, target)?;
            read_vm_local_field(ctx, target_value, field)
        }
        Expr::Ref(target) => resolve_vm_local_lvalue_reference(ctx, target)
            .map(|reference| Value::Reference(Some(reference))),
        Expr::Deref(expr) => match eval_vm_local_expr(ctx, expr)? {
            Value::Reference(Some(reference)) => {
                materialize_vm_local_ref(ctx, &reference).ok_or(RuntimeError::NullReference)
            }
            Value::Reference(None) => Err(RuntimeError::NullReference),
            _ => Err(RuntimeError::TypeMismatch),
        },
    }
}

fn eval_vm_local_struct_initializer(
    ctx: &VmLocalExprContext<'_>,
    fields: &[(SmolStr, Expr)],
) -> Result<Value, RuntimeError> {
    let mut values = IndexMap::new();
    for (field, expr) in fields {
        if values
            .keys()
            .any(|existing: &SmolStr| existing.eq_ignore_ascii_case(field.as_str()))
        {
            return Err(RuntimeError::TypeMismatch);
        }
        values.insert(field.clone(), eval_vm_local_expr(ctx, expr)?);
    }
    Ok(Value::Struct(std::sync::Arc::new(
        StructValue::from_untyped_parts("".into(), values),
    )))
}

fn eval_vm_local_array_elements(
    ctx: &VmLocalExprContext<'_>,
    elements: &[Expr],
) -> Result<Vec<Value>, RuntimeError> {
    let mut values = Vec::new();
    for expr in elements {
        if let Some((count, repeated_args)) = vm_local_array_repeat_group(expr)? {
            for _ in 0..count {
                for arg in repeated_args {
                    let ArgValue::Expr(value_expr) = &arg.value else {
                        return Err(RuntimeError::TypeMismatch);
                    };
                    values.push(eval_vm_local_expr(ctx, value_expr)?);
                }
            }
            continue;
        }
        values.push(eval_vm_local_expr(ctx, expr)?);
    }
    Ok(values)
}

fn vm_local_array_repeat_group(expr: &Expr) -> Result<Option<(usize, &[CallArg])>, RuntimeError> {
    let Expr::Call { target, args } = expr else {
        return Ok(None);
    };
    if args.iter().any(|arg| arg.name.is_some()) {
        return Err(RuntimeError::TypeMismatch);
    }
    let count = match target.as_ref() {
        Expr::Literal(Value::SInt(v)) => i64::from(*v),
        Expr::Literal(Value::Int(v)) => i64::from(*v),
        Expr::Literal(Value::DInt(v)) => i64::from(*v),
        Expr::Literal(Value::LInt(v)) => *v,
        Expr::Literal(Value::USInt(v)) => i64::from(*v),
        Expr::Literal(Value::UInt(v)) => i64::from(*v),
        Expr::Literal(Value::UDInt(v)) => i64::from(*v),
        Expr::Literal(Value::ULInt(v)) => {
            i64::try_from(*v).map_err(|_| RuntimeError::TypeMismatch)?
        }
        _ => return Ok(None),
    };
    if count < 0 {
        return Err(RuntimeError::TypeMismatch);
    }
    usize::try_from(count)
        .map(Some)
        .map(|count| count.map(|count| (count, args.as_slice())))
        .map_err(|_| RuntimeError::TypeMismatch)
}

fn eval_vm_local_size_of(
    ctx: &VmLocalExprContext<'_>,
    target: &SizeOfTarget,
) -> Result<Value, RuntimeError> {
    let size = match target {
        SizeOfTarget::Type(type_id) => {
            size_of_type(*type_id, ctx.runtime.registry()).map_err(size_error_to_runtime)?
        }
    };
    let size = i32::try_from(size).map_err(|_| RuntimeError::Overflow)?;
    Ok(Value::DInt(size))
}

fn eval_vm_local_call(
    ctx: &VmLocalExprContext<'_>,
    target: &Expr,
    args: &[CallArg],
) -> Result<Value, RuntimeError> {
    let Some(name) = call_target_name(target) else {
        return Err(RuntimeError::TypeMismatch);
    };
    let key = SmolStr::new(name.to_ascii_uppercase());
    let has_named = args.iter().any(|arg| arg.name.is_some());

    if let Some(entry) = ctx.runtime.stdlib().get(&key) {
        let values = if has_named {
            bind_vm_local_stdlib_named_args(ctx, &entry.params, args)?
        } else {
            eval_vm_local_positional_args(ctx, args)?
        };
        return (entry.func)(&values);
    }

    if conversions::is_conversion_name(key.as_str()) {
        let params = StdParams::Fixed(vec![SmolStr::new("IN")]);
        let values = if has_named {
            bind_vm_local_stdlib_named_args(ctx, &params, args)?
        } else {
            eval_vm_local_positional_args(ctx, args)?
        };
        return ctx.runtime.stdlib().call(key.as_str(), &values);
    }

    Err(RuntimeError::UndefinedFunction(name))
}

fn eval_vm_local_positional_args(
    ctx: &VmLocalExprContext<'_>,
    args: &[CallArg],
) -> Result<Vec<Value>, RuntimeError> {
    args.iter()
        .map(|arg| read_vm_local_arg_value(ctx, arg))
        .collect()
}

fn bind_vm_local_stdlib_named_args(
    ctx: &VmLocalExprContext<'_>,
    params: &StdParams,
    args: &[CallArg],
) -> Result<Vec<Value>, RuntimeError> {
    if args.iter().any(|arg| arg.name.is_none()) {
        return Err(RuntimeError::InvalidArgumentName("<unnamed>".into()));
    }
    match params {
        StdParams::Fixed(params) => bind_vm_local_stdlib_named_args_fixed(ctx, params, args),
        StdParams::Variadic {
            fixed,
            prefix,
            start,
            min,
        } => bind_vm_local_stdlib_named_args_variadic(ctx, fixed, prefix, *start, *min, args),
    }
}

fn bind_vm_local_stdlib_named_args_fixed(
    ctx: &VmLocalExprContext<'_>,
    params: &[SmolStr],
    args: &[CallArg],
) -> Result<Vec<Value>, RuntimeError> {
    if args.len() != params.len() {
        return Err(RuntimeError::InvalidArgumentCount {
            expected: params.len(),
            got: args.len(),
        });
    }

    let mut values: Vec<Option<Value>> = vec![None; params.len()];
    for arg in args {
        let Some(name) = arg.name.as_ref() else {
            return Err(RuntimeError::InvalidArgumentName("<unnamed>".into()));
        };
        let key = name.to_ascii_uppercase();
        let position = params
            .iter()
            .position(|param| param.as_str() == key)
            .ok_or_else(|| RuntimeError::InvalidArgumentName(name.clone()))?;
        if values[position].is_some() {
            return Err(RuntimeError::InvalidArgumentName(name.clone()));
        }
        values[position] = Some(read_vm_local_arg_value(ctx, arg)?);
    }
    values
        .into_iter()
        .map(|value| {
            value.ok_or(RuntimeError::InvalidArgumentCount {
                expected: params.len(),
                got: args.len(),
            })
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn bind_vm_local_stdlib_named_args_variadic(
    ctx: &VmLocalExprContext<'_>,
    fixed: &[SmolStr],
    prefix: &SmolStr,
    start: usize,
    min: usize,
    args: &[CallArg],
) -> Result<Vec<Value>, RuntimeError> {
    let mut fixed_values: Vec<Option<Value>> = vec![None; fixed.len()];
    let mut variadic_values: Vec<Option<Value>> = Vec::new();
    let mut max_index: Option<usize> = None;

    for arg in args {
        let Some(name) = arg.name.as_ref() else {
            return Err(RuntimeError::InvalidArgumentName("<unnamed>".into()));
        };
        let key = name.to_ascii_uppercase();
        if let Some(position) = fixed.iter().position(|param| param.as_str() == key) {
            if fixed_values[position].is_some() {
                return Err(RuntimeError::InvalidArgumentName(name.clone()));
            }
            fixed_values[position] = Some(read_vm_local_arg_value(ctx, arg)?);
            continue;
        }

        let prefix_str = prefix.as_str();
        if let Some(suffix) = key.strip_prefix(prefix_str) {
            if suffix.is_empty() {
                return Err(RuntimeError::InvalidArgumentName(name.clone()));
            }
            let index = suffix
                .parse::<usize>()
                .map_err(|_| RuntimeError::InvalidArgumentName(name.clone()))?;
            if index < start {
                return Err(RuntimeError::InvalidArgumentName(name.clone()));
            }
            let offset = index - start;
            if variadic_values.len() <= offset {
                variadic_values.resize(offset + 1, None);
            }
            if variadic_values[offset].is_some() {
                return Err(RuntimeError::InvalidArgumentName(name.clone()));
            }
            variadic_values[offset] = Some(read_vm_local_arg_value(ctx, arg)?);
            max_index = Some(max_index.map_or(offset, |max| max.max(offset)));
            continue;
        }

        return Err(RuntimeError::InvalidArgumentName(name.clone()));
    }

    for value in &fixed_values {
        if value.is_none() {
            return Err(RuntimeError::InvalidArgumentCount {
                expected: fixed.len() + min,
                got: args.len(),
            });
        }
    }

    let count = max_index.map(|idx| idx + 1).unwrap_or(0);
    if count < min {
        return Err(RuntimeError::InvalidArgumentCount {
            expected: fixed.len() + min,
            got: args.len(),
        });
    }
    for idx in 0..count {
        if variadic_values
            .get(idx)
            .and_then(|value| value.as_ref())
            .is_none()
        {
            return Err(RuntimeError::InvalidArgumentCount {
                expected: fixed.len() + count,
                got: args.len(),
            });
        }
    }

    let mut resolved = Vec::with_capacity(fixed.len() + count);
    for value in fixed_values {
        resolved.push(value.ok_or(RuntimeError::InvalidArgumentCount {
            expected: fixed.len() + count,
            got: args.len(),
        })?);
    }
    for value in variadic_values.into_iter().take(count) {
        resolved.push(value.ok_or(RuntimeError::InvalidArgumentCount {
            expected: fixed.len() + count,
            got: args.len(),
        })?);
    }
    Ok(resolved)
}

fn read_vm_local_arg_value(
    ctx: &VmLocalExprContext<'_>,
    arg: &CallArg,
) -> Result<Value, RuntimeError> {
    match &arg.value {
        ArgValue::Expr(expr) => eval_vm_local_expr(ctx, expr),
        ArgValue::Target(target) => read_vm_local_lvalue(ctx, target),
    }
}

fn read_vm_local_lvalue(
    ctx: &VmLocalExprContext<'_>,
    target: &LValue,
) -> Result<Value, RuntimeError> {
    let reference = resolve_vm_local_lvalue_reference(ctx, target)?;
    materialize_vm_local_ref(ctx, &reference).ok_or(RuntimeError::NullReference)
}

fn read_vm_local_name(ctx: &VmLocalExprContext<'_>, name: &SmolStr) -> Result<Value, RuntimeError> {
    if let Some(slot) = vm_local_slot_for_name(ctx.plan, name) {
        if slot < ctx.visible_slots {
            return ctx
                .frame
                .locals
                .get(slot)
                .cloned()
                .ok_or_else(|| RuntimeError::UndefinedVariable(name.clone()));
        }
    }
    if let Some(value) = read_static_local(ctx, name) {
        return Ok(value);
    }
    if let Some(instance_id) = ctx.frame.runtime_instance {
        if let Some(value) = ctx
            .runtime
            .storage()
            .get_instance_var_recursive(instance_id, name.as_str())
        {
            return Ok(value.clone());
        }
    }
    if let Some(value) = ctx.runtime.storage().get_global(name.as_str()) {
        return Ok(value.clone());
    }
    if let Some(value) = ctx.runtime.storage().get_retain(name.as_str()) {
        return Ok(value.clone());
    }
    Err(RuntimeError::UndefinedVariable(name.clone()))
}

fn read_static_local(ctx: &VmLocalExprContext<'_>, name: &SmolStr) -> Option<Value> {
    let local = ctx
        .plan
        .static_locals()
        .iter()
        .find(|local| local.name.eq_ignore_ascii_case(name.as_str()))?;
    let key = static_storage_name(&ctx.plan.static_owner(), &local.name);
    match ctx.frame.runtime_instance {
        Some(instance_id) => ctx
            .runtime
            .storage()
            .get_instance_var(instance_id, key.as_str())
            .cloned(),
        None => ctx.runtime.storage().get_global(key.as_str()).cloned(),
    }
}

fn vm_local_slot_for_name(plan: &VmPouInitPlan, name: &SmolStr) -> Option<usize> {
    let mut slot = 0usize;
    if let Some((return_name, _)) = plan.return_slot() {
        if return_name.eq_ignore_ascii_case(name.as_str()) {
            return Some(slot);
        }
        slot = slot.saturating_add(1);
    }
    for param in plan.params() {
        if param.name.eq_ignore_ascii_case(name.as_str()) {
            return Some(slot);
        }
        slot = slot.saturating_add(1);
    }
    for local in plan.locals() {
        if local.name.eq_ignore_ascii_case(name.as_str()) {
            return Some(slot);
        }
        slot = slot.saturating_add(1);
    }
    None
}

fn resolve_vm_local_lvalue_reference(
    ctx: &VmLocalExprContext<'_>,
    target: &LValue,
) -> Result<ValueRef, RuntimeError> {
    match target {
        LValue::Name(name) => resolve_vm_local_name_reference(ctx, name)
            .ok_or_else(|| RuntimeError::UndefinedVariable(name.clone())),
        LValue::Index { target, indices } => {
            let base = resolve_vm_local_lvalue_reference(ctx, target)?;
            let array_value = read_vm_local_lvalue(ctx, target)?;
            let Value::Array(array) = &array_value else {
                return Err(RuntimeError::TypeMismatch);
            };
            let index_values = indices
                .iter()
                .map(|expr| eval_vm_local_expr(ctx, expr))
                .collect::<Result<Vec<_>, _>>()?;
            checked_array_offset_i64(
                array.dimensions(),
                &index_values
                    .iter()
                    .cloned()
                    .map(index_to_i64)
                    .collect::<Result<Vec<_>, _>>()?,
            )?;
            let mut value_ref = base;
            value_ref.path.push(RefSegment::Index(ref_indices_from_iter(
                index_values
                    .into_iter()
                    .map(index_to_i64)
                    .collect::<Result<Vec<_>, _>>()?,
            )));
            Ok(value_ref)
        }
        LValue::Field { target, field } => {
            if let Some(qualified) = target
                .qualified_name()
                .map(|prefix| SmolStr::new(format!("{prefix}.{field}")))
            {
                if let Some(reference) = resolve_vm_local_name_reference(ctx, &qualified) {
                    return Ok(reference);
                }
            }
            let base_value = read_vm_local_lvalue(ctx, target)?;
            match base_value {
                Value::Instance(id) => ctx
                    .runtime
                    .storage()
                    .ref_for_instance_recursive(id, field.as_str())
                    .ok_or_else(|| RuntimeError::UndefinedField(field.clone())),
                Value::Struct(struct_value) => {
                    if !struct_value.contains_field(field.as_str()) {
                        return Err(RuntimeError::UndefinedField(field.clone()));
                    }
                    let mut value_ref = resolve_vm_local_lvalue_reference(ctx, target)?;
                    value_ref.path.push(RefSegment::Field(field.clone()));
                    Ok(value_ref)
                }
                _ => Err(RuntimeError::TypeMismatch),
            }
        }
        LValue::Deref(expr) => match eval_vm_local_expr(ctx, expr)? {
            Value::Reference(Some(reference)) => Ok(reference),
            Value::Reference(None) => Err(RuntimeError::NullReference),
            _ => Err(RuntimeError::TypeMismatch),
        },
    }
}

fn resolve_vm_local_name_reference(
    ctx: &VmLocalExprContext<'_>,
    name: &SmolStr,
) -> Option<ValueRef> {
    if let Some(slot) = vm_local_slot_for_name(ctx.plan, name) {
        if slot < ctx.visible_slots {
            return Some(ValueRef {
                location: MemoryLocation::Local(FrameId(
                    super::super::call::VM_LOCAL_SENTINEL_FRAME_ID,
                )),
                offset: slot,
                path: Vec::new(),
            });
        }
    }
    if let Some(instance_id) = ctx.frame.runtime_instance {
        if let Some(reference) = ctx
            .runtime
            .storage()
            .ref_for_instance_recursive(instance_id, name.as_str())
        {
            return Some(reference);
        }
    }
    ctx.runtime.storage().ref_for_global(name.as_str())
}

fn materialize_vm_local_ref(ctx: &VmLocalExprContext<'_>, reference: &ValueRef) -> Option<Value> {
    if is_vm_local_frame_ref(reference) {
        let value = ctx.frame.locals.get(reference.offset)?;
        if reference.path.is_empty() {
            return Some(value.clone());
        }
        return materialize_value_path(value, &reference.path);
    }
    ctx.runtime.storage().materialize_by_ref_ref(reference)
}

fn is_vm_local_frame_ref(reference: &ValueRef) -> bool {
    matches!(
        reference.location,
        MemoryLocation::Local(FrameId(super::super::call::VM_LOCAL_SENTINEL_FRAME_ID))
    )
}

fn read_vm_local_field(
    ctx: &VmLocalExprContext<'_>,
    target: Value,
    field: &SmolStr,
) -> Result<Value, RuntimeError> {
    if let Some(access) = parse_partial_access(field.as_str()) {
        return read_partial_access(&target, access).map_err(partial_access_error_to_runtime);
    }
    match target {
        Value::Struct(struct_value) => struct_value
            .field(field.as_str())
            .cloned()
            .ok_or_else(|| RuntimeError::UndefinedField(field.clone())),
        Value::Instance(id) => ctx
            .runtime
            .storage()
            .get_instance_var_recursive(id, field.as_str())
            .cloned()
            .ok_or_else(|| RuntimeError::UndefinedField(field.clone())),
        _ => Err(RuntimeError::TypeMismatch),
    }
}

fn read_vm_local_indices(target: Value, indices: &[Value]) -> Result<Value, RuntimeError> {
    match target {
        Value::Array(array) => {
            let offset = array_offset(array.dimensions(), indices)?;
            array
                .elements()
                .get(offset)
                .cloned()
                .ok_or(RuntimeError::TypeMismatch)
        }
        Value::String(text) => read_string_index(text.as_str(), indices, false),
        Value::WString(text) => read_string_index(text.as_str(), indices, true),
        _ => Err(RuntimeError::TypeMismatch),
    }
}

fn array_offset(dimensions: &[(i64, i64)], indices: &[Value]) -> Result<usize, RuntimeError> {
    if dimensions.len() != indices.len() {
        return Err(RuntimeError::TypeMismatch);
    }
    checked_array_offset_i64(
        dimensions,
        &indices
            .iter()
            .cloned()
            .map(index_to_i64)
            .collect::<Result<Vec<_>, _>>()?,
    )
}

fn index_to_i64(value: Value) -> Result<i64, RuntimeError> {
    match value {
        Value::SInt(v) => Ok(v as i64),
        Value::Int(v) => Ok(v as i64),
        Value::DInt(v) => Ok(v as i64),
        Value::LInt(v) => Ok(v),
        Value::USInt(v) => Ok(v as i64),
        Value::UInt(v) => Ok(v as i64),
        Value::UDInt(v) => Ok(v as i64),
        Value::ULInt(v) => Ok(v as i64),
        Value::Byte(v) => Ok(v as i64),
        Value::Word(v) => Ok(v as i64),
        Value::DWord(v) => Ok(v as i64),
        Value::LWord(v) => Ok(v as i64),
        _ => Err(RuntimeError::TypeMismatch),
    }
}

fn read_string_index(text: &str, indices: &[Value], wide: bool) -> Result<Value, RuntimeError> {
    if indices.len() != 1 {
        return Err(RuntimeError::TypeMismatch);
    }
    read_string_element(text, index_to_i64(indices[0].clone())?, wide)
}

fn call_target_name(expr: &Expr) -> Option<SmolStr> {
    match expr {
        Expr::Name(name) => Some(name.clone()),
        Expr::Field { target, field } => {
            let prefix = call_target_name(target)?;
            Some(SmolStr::new(format!("{prefix}.{field}")))
        }
        _ => None,
    }
}

fn qualified_field_expr_name(expr: &Expr) -> Option<SmolStr> {
    match expr {
        Expr::Name(name) => Some(name.clone()),
        Expr::Field { target, field } => {
            let prefix = qualified_field_expr_name(target)?;
            Some(SmolStr::new(format!("{prefix}.{field}")))
        }
        _ => None,
    }
}

fn size_error_to_runtime(err: SizeOfError) -> RuntimeError {
    match err {
        SizeOfError::Overflow => RuntimeError::Overflow,
        SizeOfError::UnknownType | SizeOfError::UnsupportedType => RuntimeError::TypeMismatch,
    }
}

fn partial_access_error_to_runtime(err: PartialAccessError) -> RuntimeError {
    match err {
        PartialAccessError::IndexOutOfBounds {
            index,
            lower,
            upper,
        } => RuntimeError::IndexOutOfBounds {
            index,
            lower,
            upper,
        },
        PartialAccessError::TypeMismatch => RuntimeError::TypeMismatch,
    }
}
