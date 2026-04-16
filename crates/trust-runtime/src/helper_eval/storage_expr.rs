use smol_str::SmolStr;
use trust_hir::types::TypeRegistry;

use crate::error::RuntimeError;
use crate::memory::{InstanceId, VariableStorage};
use crate::program_model::{
    apply_binary, apply_unary, ArgValue, BinaryOp, CallArg, Expr, SizeOfTarget,
};
use crate::stdlib::{conversions, StandardLibrary, StdParams};
use crate::value::{
    parse_partial_access, read_partial_access, size_of_type, size_of_value, DateTimeProfile,
    PartialAccessError, SizeOfError, Value,
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
        Expr::Ref(_) => Err(RuntimeError::TypeMismatch),
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
    storage: &VariableStorage,
    registry: &TypeRegistry,
    profile: &DateTimeProfile,
    current_instance: Option<InstanceId>,
    stdlib: Option<&StandardLibrary>,
    target: &SizeOfTarget,
) -> Result<Value, RuntimeError> {
    let size = match target {
        SizeOfTarget::Type(type_id) => {
            size_of_type(*type_id, registry).map_err(size_error_to_runtime)?
        }
        SizeOfTarget::Expr(expr) => {
            let value = eval_storage_expr_with_stdlib(
                storage,
                registry,
                profile,
                current_instance,
                stdlib,
                expr,
            )?;
            size_of_value(registry, &value).map_err(size_error_to_runtime)?
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
            .fields
            .get(field)
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
            let offset = array_offset(&array.dimensions, indices)?;
            array
                .elements
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
    let mut offset: i128 = 0;
    let mut stride: i128 = 1;
    for ((lower, upper), index_value) in dimensions.iter().zip(indices).rev() {
        let idx = index_to_i64(index_value.clone())?;
        if idx < *lower || idx > *upper {
            return Err(RuntimeError::IndexOutOfBounds {
                index: idx,
                lower: *lower,
                upper: *upper,
            });
        }
        let len = (*upper - *lower + 1) as i128;
        offset += (idx - *lower) as i128 * stride;
        stride *= len;
    }
    usize::try_from(offset).map_err(|_| RuntimeError::TypeMismatch)
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
    let index = index_to_i64(indices[0].clone())?;
    if index < 1 {
        return Err(RuntimeError::IndexOutOfBounds {
            index,
            lower: 1,
            upper: i64::MAX,
        });
    }
    let position = usize::try_from(index - 1).map_err(|_| RuntimeError::Overflow)?;
    let Some(ch) = text.chars().nth(position) else {
        return Err(RuntimeError::IndexOutOfBounds {
            index,
            lower: 1,
            upper: i64::try_from(text.chars().count()).map_err(|_| RuntimeError::Overflow)?,
        });
    };
    if wide {
        let code = u16::try_from(ch as u32).map_err(|_| RuntimeError::Overflow)?;
        Ok(Value::WChar(code))
    } else {
        let code = u8::try_from(ch as u32).map_err(|_| RuntimeError::Overflow)?;
        Ok(Value::Char(code))
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
}
