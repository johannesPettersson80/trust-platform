use smol_str::SmolStr;
use trust_hir::types::TypeRegistry;

use indexmap::IndexMap;

use crate::error::RuntimeError;
use crate::memory::{InstanceId, VariableStorage};
use crate::program_model::{
    apply_binary, apply_unary, ArgValue, BinaryOp, CallArg, Expr, SizeOfTarget,
};
use crate::stdlib::{conversions, StandardLibrary, StdParams};
use crate::value::{
    checked_array_offset_i64, parse_partial_access, read_partial_access, read_string_element,
    ref_indices_from_iter, size_of_type, ArrayValue, DateTimeProfile, PartialAccessError,
    RefSegment, SizeOfError, StructValue, Value, ValueRef,
};

use super::storage_lvalue::read_storage_lvalue;

pub(crate) fn eval_storage_expr(
    storage: &VariableStorage,
    registry: &TypeRegistry,
    profile: &DateTimeProfile,
    current_instance: Option<InstanceId>,
    expr: &Expr,
) -> Result<Value, RuntimeError> {
    eval_storage_expr_with_stdlib(storage, registry, profile, current_instance, None, expr)
}

pub(crate) fn eval_storage_expr_with_stdlib(
    storage: &VariableStorage,
    registry: &TypeRegistry,
    profile: &DateTimeProfile,
    current_instance: Option<InstanceId>,
    stdlib: Option<&StandardLibrary>,
    expr: &Expr,
) -> Result<Value, RuntimeError> {
    match expr {
        Expr::Literal(value) => Ok(value.clone()),
        Expr::ArrayInitializer(elements) => {
            let values = eval_array_initializer_elements(
                storage,
                registry,
                profile,
                current_instance,
                stdlib,
                elements,
            )?;
            let len = values.len() as i64;
            ArrayValue::from_untyped_parts(values, vec![(1, len)])
                .map(|value| Value::Array(Box::new(value)))
                .map_err(|_| RuntimeError::TypeMismatch)
        }
        Expr::StructInitializer(fields) => {
            eval_struct_initializer(storage, registry, profile, current_instance, stdlib, fields)
        }
        Expr::This => current_instance
            .map(Value::Instance)
            .ok_or(RuntimeError::TypeMismatch),
        Expr::Super => {
            let current = current_instance.ok_or(RuntimeError::TypeMismatch)?;
            let instance = storage
                .get_instance(current)
                .ok_or(RuntimeError::NullReference)?;
            instance
                .parent
                .map(Value::Instance)
                .ok_or(RuntimeError::TypeMismatch)
        }
        Expr::SizeOf(target) => {
            eval_size_of(storage, registry, profile, current_instance, stdlib, target)
        }
        Expr::Name(name) => read_name(storage, current_instance, name),
        Expr::Call { target, args } => eval_call(
            storage,
            registry,
            profile,
            current_instance,
            stdlib,
            target,
            args,
        ),
        Expr::Unary { op, expr } => {
            let value = eval_storage_expr_with_stdlib(
                storage,
                registry,
                profile,
                current_instance,
                stdlib,
                expr,
            )?;
            apply_unary(*op, value)
        }
        Expr::Binary { op, left, right } => {
            if *op == BinaryOp::And {
                let left_value = eval_storage_expr_with_stdlib(
                    storage,
                    registry,
                    profile,
                    current_instance,
                    stdlib,
                    left,
                )?;
                if matches!(left_value, Value::Bool(false)) {
                    return Ok(Value::Bool(false));
                }
                let right_value = eval_storage_expr_with_stdlib(
                    storage,
                    registry,
                    profile,
                    current_instance,
                    stdlib,
                    right,
                )?;
                return apply_binary(*op, left_value, right_value, profile);
            }
            if *op == BinaryOp::Or {
                let left_value = eval_storage_expr_with_stdlib(
                    storage,
                    registry,
                    profile,
                    current_instance,
                    stdlib,
                    left,
                )?;
                if matches!(left_value, Value::Bool(true)) {
                    return Ok(Value::Bool(true));
                }
                let right_value = eval_storage_expr_with_stdlib(
                    storage,
                    registry,
                    profile,
                    current_instance,
                    stdlib,
                    right,
                )?;
                return apply_binary(*op, left_value, right_value, profile);
            }
            let left_value = eval_storage_expr_with_stdlib(
                storage,
                registry,
                profile,
                current_instance,
                stdlib,
                left,
            )?;
            let right_value = eval_storage_expr_with_stdlib(
                storage,
                registry,
                profile,
                current_instance,
                stdlib,
                right,
            )?;
            apply_binary(*op, left_value, right_value, profile)
        }
        Expr::Index { target, indices } => {
            let target_value = eval_storage_expr_with_stdlib(
                storage,
                registry,
                profile,
                current_instance,
                stdlib,
                target,
            )?;
            let index_values = indices
                .iter()
                .map(|index| {
                    eval_storage_expr_with_stdlib(
                        storage,
                        registry,
                        profile,
                        current_instance,
                        stdlib,
                        index,
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;
            read_indices(target_value, &index_values)
        }
        Expr::Field { target, field } => {
            if let Some(qualified) = qualified_field_expr_name(expr) {
                if let Ok(value) = read_name(storage, current_instance, &qualified) {
                    return Ok(value);
                }
            }
            let target_value = eval_storage_expr_with_stdlib(
                storage,
                registry,
                profile,
                current_instance,
                stdlib,
                target,
            )?;
            read_field(storage, target_value, field)
        }
        Expr::Ref(target) => {
            resolve_lvalue_reference(storage, registry, profile, current_instance, stdlib, target)
                .map(|reference| Value::Reference(Some(reference)))
        }
        Expr::Deref(expr) => {
            let value = eval_storage_expr_with_stdlib(
                storage,
                registry,
                profile,
                current_instance,
                stdlib,
                expr,
            )?;
            match value {
                Value::Reference(Some(reference)) => storage
                    .materialize_by_ref(reference)
                    .ok_or(RuntimeError::NullReference),
                Value::Reference(None) => Err(RuntimeError::NullReference),
                _ => Err(RuntimeError::TypeMismatch),
            }
        }
    }
}

fn resolve_lvalue_reference(
    storage: &VariableStorage,
    registry: &TypeRegistry,
    profile: &DateTimeProfile,
    current_instance: Option<InstanceId>,
    stdlib: Option<&StandardLibrary>,
    target: &crate::program_model::LValue,
) -> Result<ValueRef, RuntimeError> {
    match target {
        crate::program_model::LValue::Name(name) => {
            resolve_name_reference(storage, current_instance, name)
                .ok_or_else(|| RuntimeError::UndefinedVariable(name.clone()))
        }
        crate::program_model::LValue::Index { target, indices } => {
            let base = resolve_lvalue_reference(
                storage,
                registry,
                profile,
                current_instance,
                stdlib,
                target,
            )?;
            let array_value =
                read_storage_lvalue(storage, registry, profile, current_instance, target)?;
            let Value::Array(array) = &array_value else {
                return Err(RuntimeError::TypeMismatch);
            };
            let index_values = indices
                .iter()
                .map(|expr| {
                    eval_storage_expr_with_stdlib(
                        storage,
                        registry,
                        profile,
                        current_instance,
                        stdlib,
                        expr,
                    )
                })
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
        crate::program_model::LValue::Field { target, field } => {
            if let Some(qualified) = target
                .qualified_name()
                .map(|prefix| SmolStr::new(format!("{prefix}.{field}")))
            {
                if let Some(reference) =
                    resolve_name_reference(storage, current_instance, &qualified)
                {
                    return Ok(reference);
                }
            }
            let base_value =
                read_storage_lvalue(storage, registry, profile, current_instance, target)?;
            match base_value {
                Value::Instance(id) => storage
                    .ref_for_instance_recursive(id, field.as_str())
                    .ok_or_else(|| RuntimeError::UndefinedField(field.clone())),
                Value::Struct(struct_value) => {
                    if !struct_value.contains_field(field.as_str()) {
                        return Err(RuntimeError::UndefinedField(field.clone()));
                    }
                    let mut value_ref = resolve_lvalue_reference(
                        storage,
                        registry,
                        profile,
                        current_instance,
                        stdlib,
                        target,
                    )?;
                    value_ref.path.push(RefSegment::Field(field.clone()));
                    Ok(value_ref)
                }
                _ => Err(RuntimeError::TypeMismatch),
            }
        }
        crate::program_model::LValue::Deref(expr) => match eval_storage_expr_with_stdlib(
            storage,
            registry,
            profile,
            current_instance,
            stdlib,
            expr,
        )? {
            Value::Reference(Some(reference)) => Ok(reference),
            Value::Reference(None) => Err(RuntimeError::NullReference),
            _ => Err(RuntimeError::TypeMismatch),
        },
    }
}

fn resolve_name_reference(
    storage: &VariableStorage,
    current_instance: Option<InstanceId>,
    name: &SmolStr,
) -> Option<ValueRef> {
    if let Some(reference) = storage.ref_for_local(name.as_str()) {
        return Some(reference);
    }
    if let Some(instance_id) = current_instance {
        if let Some(reference) = storage.ref_for_instance_recursive(instance_id, name.as_str()) {
            return Some(reference);
        }
    }
    storage.ref_for_global(name.as_str())
}

fn eval_struct_initializer(
    storage: &VariableStorage,
    registry: &TypeRegistry,
    profile: &DateTimeProfile,
    current_instance: Option<InstanceId>,
    stdlib: Option<&StandardLibrary>,
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
        let value = eval_storage_expr_with_stdlib(
            storage,
            registry,
            profile,
            current_instance,
            stdlib,
            expr,
        )?;
        values.insert(field.clone(), value);
    }
    Ok(Value::Struct(std::sync::Arc::new(
        StructValue::from_untyped_parts("".into(), values),
    )))
}

fn eval_array_initializer_elements(
    storage: &VariableStorage,
    registry: &TypeRegistry,
    profile: &DateTimeProfile,
    current_instance: Option<InstanceId>,
    stdlib: Option<&StandardLibrary>,
    elements: &[Expr],
) -> Result<Vec<Value>, RuntimeError> {
    let mut values = Vec::new();
    for expr in elements {
        if let Some((count, repeated_args)) = array_repeat_group(expr)? {
            for _ in 0..count {
                for arg in repeated_args {
                    let crate::program_model::ArgValue::Expr(value_expr) = &arg.value else {
                        return Err(RuntimeError::TypeMismatch);
                    };
                    values.push(eval_storage_expr_with_stdlib(
                        storage,
                        registry,
                        profile,
                        current_instance,
                        stdlib,
                        value_expr,
                    )?);
                }
            }
            continue;
        }
        values.push(eval_storage_expr_with_stdlib(
            storage,
            registry,
            profile,
            current_instance,
            stdlib,
            expr,
        )?);
    }
    Ok(values)
}

fn array_repeat_group(expr: &Expr) -> Result<Option<(usize, &[CallArg])>, RuntimeError> {
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
    let count = usize::try_from(count).map_err(|_| RuntimeError::TypeMismatch)?;
    Ok(Some((count, args)))
}

fn eval_call(
    storage: &VariableStorage,
    registry: &TypeRegistry,
    profile: &DateTimeProfile,
    current_instance: Option<InstanceId>,
    stdlib: Option<&StandardLibrary>,
    target: &Expr,
    args: &[CallArg],
) -> Result<Value, RuntimeError> {
    let Some(stdlib) = stdlib else {
        return Err(RuntimeError::TypeMismatch);
    };
    let Some(name) = call_target_name(target) else {
        return Err(RuntimeError::TypeMismatch);
    };
    let key = SmolStr::new(name.to_ascii_uppercase());
    let has_named = args.iter().any(|arg| arg.name.is_some());

    if let Some(entry) = stdlib.get(&key) {
        let values = if has_named {
            bind_stdlib_named_args(
                storage,
                registry,
                profile,
                current_instance,
                stdlib,
                &entry.params,
                args,
            )?
        } else {
            eval_positional_args(storage, registry, profile, current_instance, stdlib, args)?
        };
        return (entry.func)(&values);
    }

    if conversions::is_conversion_name(key.as_str()) {
        let params = StdParams::Fixed(vec![SmolStr::new("IN")]);
        let values = if has_named {
            bind_stdlib_named_args(
                storage,
                registry,
                profile,
                current_instance,
                stdlib,
                &params,
                args,
            )?
        } else {
            eval_positional_args(storage, registry, profile, current_instance, stdlib, args)?
        };
        return stdlib.call(key.as_str(), &values);
    }

    Err(RuntimeError::UndefinedFunction(name))
}

fn call_target_name(expr: &Expr) -> Option<SmolStr> {
    match expr {
        Expr::Name(name) => Some(name.clone()),
        Expr::Field { target, field } => {
            let prefix = call_target_name(target)?;
            let mut combined = String::with_capacity(prefix.len() + field.len() + 1);
            combined.push_str(prefix.as_str());
            combined.push('.');
            combined.push_str(field.as_str());
            Some(combined.into())
        }
        _ => None,
    }
}

fn eval_positional_args(
    storage: &VariableStorage,
    registry: &TypeRegistry,
    profile: &DateTimeProfile,
    current_instance: Option<InstanceId>,
    stdlib: &StandardLibrary,
    args: &[CallArg],
) -> Result<Vec<Value>, RuntimeError> {
    args.iter()
        .map(|arg| read_arg_value(storage, registry, profile, current_instance, stdlib, arg))
        .collect()
}

fn read_arg_value(
    storage: &VariableStorage,
    registry: &TypeRegistry,
    profile: &DateTimeProfile,
    current_instance: Option<InstanceId>,
    stdlib: &StandardLibrary,
    arg: &CallArg,
) -> Result<Value, RuntimeError> {
    match &arg.value {
        ArgValue::Expr(expr) => eval_storage_expr_with_stdlib(
            storage,
            registry,
            profile,
            current_instance,
            Some(stdlib),
            expr,
        ),
        ArgValue::Target(target) => {
            read_storage_lvalue(storage, registry, profile, current_instance, target)
        }
    }
}

fn bind_stdlib_named_args(
    storage: &VariableStorage,
    registry: &TypeRegistry,
    profile: &DateTimeProfile,
    current_instance: Option<InstanceId>,
    stdlib: &StandardLibrary,
    params: &StdParams,
    args: &[CallArg],
) -> Result<Vec<Value>, RuntimeError> {
    if args.iter().any(|arg| arg.name.is_none()) {
        return Err(RuntimeError::InvalidArgumentName("<unnamed>".into()));
    }
    match params {
        StdParams::Fixed(params) => bind_stdlib_named_args_fixed(
            storage,
            registry,
            profile,
            current_instance,
            stdlib,
            params,
            args,
        ),
        StdParams::Variadic {
            fixed,
            prefix,
            start,
            min,
        } => bind_stdlib_named_args_variadic(
            storage,
            registry,
            profile,
            current_instance,
            stdlib,
            fixed,
            prefix,
            *start,
            *min,
            args,
        ),
    }
}

fn bind_stdlib_named_args_fixed(
    storage: &VariableStorage,
    registry: &TypeRegistry,
    profile: &DateTimeProfile,
    current_instance: Option<InstanceId>,
    stdlib: &StandardLibrary,
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
        let value = read_arg_value(storage, registry, profile, current_instance, stdlib, arg)?;
        values[position] = Some(value);
    }

    let mut resolved = Vec::with_capacity(values.len());
    for value in values {
        let Some(value) = value else {
            return Err(RuntimeError::InvalidArgumentCount {
                expected: params.len(),
                got: args.len(),
            });
        };
        resolved.push(value);
    }
    Ok(resolved)
}

#[allow(clippy::too_many_arguments)]
fn bind_stdlib_named_args_variadic(
    storage: &VariableStorage,
    registry: &TypeRegistry,
    profile: &DateTimeProfile,
    current_instance: Option<InstanceId>,
    stdlib: &StandardLibrary,
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
            let value = read_arg_value(storage, registry, profile, current_instance, stdlib, arg)?;
            fixed_values[position] = Some(value);
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
            let value = read_arg_value(storage, registry, profile, current_instance, stdlib, arg)?;
            variadic_values[offset] = Some(value);
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
        let Some(value) = value else {
            return Err(RuntimeError::InvalidArgumentCount {
                expected: fixed.len() + count,
                got: args.len(),
            });
        };
        resolved.push(value);
    }
    for value in variadic_values.into_iter().take(count) {
        let Some(value) = value else {
            return Err(RuntimeError::InvalidArgumentCount {
                expected: fixed.len() + count,
                got: args.len(),
            });
        };
        resolved.push(value);
    }
    Ok(resolved)
}

fn eval_size_of(
    _storage: &VariableStorage,
    registry: &TypeRegistry,
    _profile: &DateTimeProfile,
    _current_instance: Option<InstanceId>,
    _stdlib: Option<&StandardLibrary>,
    target: &SizeOfTarget,
) -> Result<Value, RuntimeError> {
    let size = match target {
        SizeOfTarget::Type(type_id) => {
            size_of_type(*type_id, registry).map_err(size_error_to_runtime)?
        }
    };
    let size = i32::try_from(size).map_err(|_| RuntimeError::Overflow)?;
    Ok(Value::DInt(size))
}

fn read_name(
    storage: &VariableStorage,
    current_instance: Option<InstanceId>,
    name: &SmolStr,
) -> Result<Value, RuntimeError> {
    if let Some(value) = storage.get_local(name.as_ref()) {
        return Ok(value.clone());
    }
    if let Some(instance_id) = current_instance {
        if let Some(value) = storage.get_instance_var_recursive(instance_id, name.as_ref()) {
            return Ok(value.clone());
        }
    }
    if let Some(value) = storage.get_global(name.as_ref()) {
        return Ok(value.clone());
    }
    if let Some(value) = storage.get_retain(name.as_ref()) {
        return Ok(value.clone());
    }
    Err(RuntimeError::UndefinedVariable(name.clone()))
}

fn read_field(
    storage: &VariableStorage,
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
        Value::Instance(id) => storage
            .get_instance_var_recursive(id, field.as_ref())
            .cloned()
            .ok_or_else(|| RuntimeError::UndefinedField(field.clone())),
        _ => Err(RuntimeError::TypeMismatch),
    }
}

fn read_indices(target: Value, indices: &[Value]) -> Result<Value, RuntimeError> {
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
    let mut numeric_indices = Vec::with_capacity(indices.len());
    for index_value in indices {
        let idx = index_to_i64(index_value.clone())?;
        numeric_indices.push(idx);
    }
    checked_array_offset_i64(dimensions, &numeric_indices)
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

fn size_error_to_runtime(err: SizeOfError) -> RuntimeError {
    match err {
        SizeOfError::Overflow => RuntimeError::Overflow,
        SizeOfError::UnknownType | SizeOfError::UnsupportedType => RuntimeError::TypeMismatch,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::program_model::Expr;
    use crate::stdlib::StandardLibrary;
    use crate::value::Value;

    #[test]
    fn allows_pure_stdlib_calls_when_stdlib_is_provided() {
        let expr = Expr::Call {
            target: Box::new(Expr::Name("ABS".into())),
            args: vec![CallArg {
                name: None,
                value: ArgValue::Expr(Expr::Literal(Value::DInt(-1))),
            }],
        };

        let storage = VariableStorage::new();
        let registry = TypeRegistry::default();
        let profile = DateTimeProfile::default();
        let stdlib = StandardLibrary::new();

        let value = eval_storage_expr_with_stdlib(
            &storage,
            &registry,
            &profile,
            None,
            Some(&stdlib),
            &expr,
        )
        .unwrap();
        assert_eq!(value, Value::DInt(1));
    }

    #[test]
    fn rejects_calls_without_stdlib_surface() {
        let expr = Expr::Call {
            target: Box::new(Expr::Name("ABS".into())),
            args: vec![CallArg {
                name: None,
                value: ArgValue::Expr(Expr::Literal(Value::DInt(-1))),
            }],
        };

        let storage = VariableStorage::new();
        let registry = TypeRegistry::default();
        let profile = DateTimeProfile::default();

        assert!(matches!(
            eval_storage_expr_with_stdlib(&storage, &registry, &profile, None, None, &expr),
            Err(RuntimeError::TypeMismatch)
        ));
    }

    #[test]
    fn array_repetition_initializer_uses_expanded_value_shape() {
        let expr = Expr::ArrayInitializer(vec![Expr::Call {
            target: Box::new(Expr::Literal(Value::Int(3))),
            args: vec![
                CallArg {
                    name: None,
                    value: ArgValue::Expr(Expr::Literal(Value::Int(1))),
                },
                CallArg {
                    name: None,
                    value: ArgValue::Expr(Expr::Literal(Value::Int(2))),
                },
            ],
        }]);

        let storage = VariableStorage::new();
        let registry = TypeRegistry::default();
        let profile = DateTimeProfile::default();

        let value = eval_storage_expr_with_stdlib(&storage, &registry, &profile, None, None, &expr)
            .unwrap();
        let Value::Array(array) = value else {
            panic!("expected array value");
        };
        assert_eq!(array.dimensions(), &[(1, 6)]);
        assert_eq!(
            array.elements(),
            &[
                Value::Int(1),
                Value::Int(2),
                Value::Int(1),
                Value::Int(2),
                Value::Int(1),
                Value::Int(2),
            ]
        );
    }
}
