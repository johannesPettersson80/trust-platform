#![cfg_attr(not(test), deny(clippy::unwrap_used))]

use smol_str::SmolStr;

use crate::error::RuntimeError;
use crate::eval::{ArgValue, CallArg, EvalContext};
use crate::memory::InstanceId;
use crate::stdlib::{time, StdParams};
use crate::value::Value;

use super::ast::{Expr, LValue};
use super::lvalue::{read_lvalue, resolve_reference_for_lvalue, write_lvalue};

pub(super) fn call_target_name(expr: &Expr) -> Option<SmolStr> {
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

pub(super) fn eval_positional_args(
    ctx: &mut EvalContext<'_>,
    args: &[CallArg],
) -> Result<Vec<Value>, RuntimeError> {
    let mut values = Vec::with_capacity(args.len());
    for arg in args {
        let value = read_arg_value(ctx, arg)?;
        values.push(value);
    }
    Ok(values)
}

pub(super) fn resolve_using_function<'a>(
    functions: &'a indexmap::IndexMap<SmolStr, crate::eval::FunctionDef>,
    name: &str,
    using: &[SmolStr],
) -> Option<&'a crate::eval::FunctionDef> {
    for namespace in using {
        let qualified = format!("{namespace}.{name}");
        let key = SmolStr::new(qualified.to_ascii_uppercase());
        if let Some(func) = functions.get(&key) {
            return Some(func);
        }
    }
    None
}

pub(super) fn resolve_instance_method(
    ctx: &EvalContext<'_>,
    instance_id: InstanceId,
    name: &SmolStr,
) -> Option<crate::eval::MethodDef> {
    let instance = ctx.storage.get_instance(instance_id)?;
    let key = SmolStr::new(instance.type_name.to_ascii_uppercase());

    if let Some(function_blocks) = ctx.function_blocks {
        if let Some(fb) = function_blocks.get(&key) {
            let classes = ctx.classes?;
            return resolve_fb_method(function_blocks, classes, fb, name);
        }
    }

    let classes = ctx.classes?;
    let class_def = classes.get(&key)?;
    resolve_class_method(classes, class_def, name)
}

pub(super) fn resolve_fb_method(
    function_blocks: &indexmap::IndexMap<SmolStr, crate::eval::FunctionBlockDef>,
    classes: &indexmap::IndexMap<SmolStr, crate::eval::ClassDef>,
    fb: &crate::eval::FunctionBlockDef,
    name: &SmolStr,
) -> Option<crate::eval::MethodDef> {
    let mut current = Some(fb);
    while let Some(def) = current {
        if let Some(method) = def
            .methods
            .iter()
            .find(|method| method.name.eq_ignore_ascii_case(name))
        {
            return Some(method.clone());
        }
        let Some(base) = &def.base else {
            break;
        };
        match base {
            crate::eval::FunctionBlockBase::FunctionBlock(base_name) => {
                let base_key = SmolStr::new(base_name.to_ascii_uppercase());
                current = function_blocks.get(&base_key);
            }
            crate::eval::FunctionBlockBase::Class(base_name) => {
                let base_key = SmolStr::new(base_name.to_ascii_uppercase());
                let class_def = classes.get(&base_key)?;
                return resolve_class_method(classes, class_def, name);
            }
        }
    }
    None
}

pub(super) fn resolve_class_method(
    classes: &indexmap::IndexMap<SmolStr, crate::eval::ClassDef>,
    class_def: &crate::eval::ClassDef,
    name: &SmolStr,
) -> Option<crate::eval::MethodDef> {
    let mut current = class_def;
    loop {
        if let Some(method) = current
            .methods
            .iter()
            .find(|method| method.name.eq_ignore_ascii_case(name))
        {
            return Some(method.clone());
        }
        let Some(base) = &current.base else {
            break;
        };
        let base_key = SmolStr::new(base.to_ascii_uppercase());
        let Some(base_def) = classes.get(&base_key) else {
            break;
        };
        current = base_def;
    }
    None
}

pub(super) fn bind_stdlib_named_args(
    ctx: &mut EvalContext<'_>,
    params: &StdParams,
    args: &[CallArg],
) -> Result<Vec<Value>, RuntimeError> {
    if args.iter().any(|arg| arg.name.is_none()) {
        return Err(RuntimeError::InvalidArgumentName("<unnamed>".into()));
    }
    match params {
        StdParams::Fixed(params) => bind_stdlib_named_args_fixed(ctx, params, args),
        StdParams::Variadic {
            fixed,
            prefix,
            start,
            min,
        } => bind_stdlib_named_args_variadic(ctx, fixed, prefix, *start, *min, args),
    }
}

fn bind_stdlib_named_args_fixed(
    ctx: &mut EvalContext<'_>,
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
        let value = read_arg_value(ctx, arg)?;
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

fn bind_stdlib_named_args_variadic(
    ctx: &mut EvalContext<'_>,
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
            let value = read_arg_value(ctx, arg)?;
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
            let value = read_arg_value(ctx, arg)?;
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

pub(super) fn eval_split_call(
    ctx: &mut EvalContext<'_>,
    name: &str,
    args: &[CallArg],
) -> Result<Value, RuntimeError> {
    let params: &[&str] = match name {
        "SPLIT_DATE" => &["IN", "YEAR", "MONTH", "DAY"],
        "SPLIT_TOD" | "SPLIT_LTOD" => &["IN", "HOUR", "MINUTE", "SECOND", "MILLISECOND"],
        "SPLIT_DT" | "SPLIT_LDT" => &[
            "IN",
            "YEAR",
            "MONTH",
            "DAY",
            "HOUR",
            "MINUTE",
            "SECOND",
            "MILLISECOND",
        ],
        _ => return Err(RuntimeError::UndefinedFunction(name.into())),
    };

    let (input, outputs) = bind_split_args(ctx, params, args)?;

    match name {
        "SPLIT_DATE" => {
            let (year, month, day) = time::split_date(&input, ctx.profile)?;
            write_output_int(ctx, &outputs[0], year)?;
            write_output_int(ctx, &outputs[1], month)?;
            write_output_int(ctx, &outputs[2], day)?;
        }
        "SPLIT_TOD" => {
            let (hour, minute, second, millis) = time::split_tod(&input, ctx.profile)?;
            write_output_int(ctx, &outputs[0], hour)?;
            write_output_int(ctx, &outputs[1], minute)?;
            write_output_int(ctx, &outputs[2], second)?;
            write_output_int(ctx, &outputs[3], millis)?;
        }
        "SPLIT_LTOD" => {
            let (hour, minute, second, millis) = time::split_ltod(&input)?;
            write_output_int(ctx, &outputs[0], hour)?;
            write_output_int(ctx, &outputs[1], minute)?;
            write_output_int(ctx, &outputs[2], second)?;
            write_output_int(ctx, &outputs[3], millis)?;
        }
        "SPLIT_DT" => {
            let (year, month, day, hour, minute, second, millis) =
                time::split_dt(&input, ctx.profile)?;
            write_output_int(ctx, &outputs[0], year)?;
            write_output_int(ctx, &outputs[1], month)?;
            write_output_int(ctx, &outputs[2], day)?;
            write_output_int(ctx, &outputs[3], hour)?;
            write_output_int(ctx, &outputs[4], minute)?;
            write_output_int(ctx, &outputs[5], second)?;
            write_output_int(ctx, &outputs[6], millis)?;
        }
        "SPLIT_LDT" => {
            let (year, month, day, hour, minute, second, millis) = time::split_ldt(&input)?;
            write_output_int(ctx, &outputs[0], year)?;
            write_output_int(ctx, &outputs[1], month)?;
            write_output_int(ctx, &outputs[2], day)?;
            write_output_int(ctx, &outputs[3], hour)?;
            write_output_int(ctx, &outputs[4], minute)?;
            write_output_int(ctx, &outputs[5], second)?;
            write_output_int(ctx, &outputs[6], millis)?;
        }
        _ => {}
    }

    Ok(Value::Null)
}

fn bind_split_args(
    ctx: &mut EvalContext<'_>,
    params: &[&str],
    args: &[CallArg],
) -> Result<(Value, Vec<LValue>), RuntimeError> {
    let positional = args.iter().all(|arg| arg.name.is_none());
    if positional {
        if args.len() != params.len() {
            return Err(RuntimeError::InvalidArgumentCount {
                expected: params.len(),
                got: args.len(),
            });
        }
        let mut input = None;
        let mut outputs = Vec::with_capacity(params.len().saturating_sub(1));
        for (idx, arg) in args.iter().enumerate() {
            if idx == 0 {
                input = Some(read_arg_value(ctx, arg)?);
            } else {
                outputs.push(require_output_target(arg)?);
            }
        }
        let input = input.ok_or(RuntimeError::InvalidArgumentCount {
            expected: params.len(),
            got: args.len(),
        })?;
        return Ok((input, outputs));
    }

    if args.iter().any(|arg| arg.name.is_none()) {
        return Err(RuntimeError::InvalidArgumentName("<unnamed>".into()));
    }
    if args.len() != params.len() {
        return Err(RuntimeError::InvalidArgumentCount {
            expected: params.len(),
            got: args.len(),
        });
    }

    let mut assigned: Vec<Option<&CallArg>> = vec![None; params.len()];
    for arg in args {
        let Some(name) = arg.name.as_ref() else {
            return Err(RuntimeError::InvalidArgumentName("<unnamed>".into()));
        };
        let key = name.to_ascii_uppercase();
        let position = params
            .iter()
            .position(|param| param.eq_ignore_ascii_case(&key))
            .ok_or_else(|| RuntimeError::InvalidArgumentName(name.clone()))?;
        if assigned[position].is_some() {
            return Err(RuntimeError::InvalidArgumentName(name.clone()));
        }
        assigned[position] = Some(arg);
    }

    let mut input = None;
    let mut outputs = Vec::with_capacity(params.len().saturating_sub(1));
    for (idx, arg) in assigned.iter().enumerate() {
        let Some(arg) = arg else {
            return Err(RuntimeError::InvalidArgumentCount {
                expected: params.len(),
                got: args.len(),
            });
        };
        if idx == 0 {
            input = Some(read_arg_value(ctx, arg)?);
        } else {
            outputs.push(require_output_target(arg)?);
        }
    }
    let input = input.ok_or(RuntimeError::InvalidArgumentCount {
        expected: params.len(),
        got: args.len(),
    })?;
    Ok((input, outputs))
}

pub(crate) fn read_arg_value(
    ctx: &mut EvalContext<'_>,
    arg: &CallArg,
) -> Result<Value, RuntimeError> {
    match &arg.value {
        ArgValue::Expr(expr) => super::eval::eval_expr(ctx, expr),
        ArgValue::Target(target) => read_lvalue(ctx, target),
    }
}

fn require_output_target(arg: &CallArg) -> Result<LValue, RuntimeError> {
    match &arg.value {
        ArgValue::Target(target) => Ok(target.clone()),
        _ => Err(RuntimeError::TypeMismatch),
    }
}

fn write_output_int(
    ctx: &mut EvalContext<'_>,
    target: &LValue,
    value: i64,
) -> Result<(), RuntimeError> {
    let current = read_lvalue(ctx, target)?;
    let converted = match current {
        Value::SInt(_) => Value::SInt(i8::try_from(value).map_err(|_| RuntimeError::Overflow)?),
        Value::Int(_) => Value::Int(i16::try_from(value).map_err(|_| RuntimeError::Overflow)?),
        Value::DInt(_) => Value::DInt(i32::try_from(value).map_err(|_| RuntimeError::Overflow)?),
        Value::LInt(_) => Value::LInt(value),
        Value::USInt(_) => {
            if value < 0 {
                return Err(RuntimeError::Overflow);
            }
            Value::USInt(u8::try_from(value).map_err(|_| RuntimeError::Overflow)?)
        }
        Value::UInt(_) => {
            if value < 0 {
                return Err(RuntimeError::Overflow);
            }
            Value::UInt(u16::try_from(value).map_err(|_| RuntimeError::Overflow)?)
        }
        Value::UDInt(_) => {
            if value < 0 {
                return Err(RuntimeError::Overflow);
            }
            Value::UDInt(u32::try_from(value).map_err(|_| RuntimeError::Overflow)?)
        }
        Value::ULInt(_) => {
            if value < 0 {
                return Err(RuntimeError::Overflow);
            }
            Value::ULInt(value as u64)
        }
        _ => return Err(RuntimeError::TypeMismatch),
    };
    write_lvalue(ctx, target, converted)
}

pub(super) fn eval_ref_call(
    ctx: &mut EvalContext<'_>,
    args: &[CallArg],
) -> Result<Value, RuntimeError> {
    if args.len() != 1 {
        return Err(RuntimeError::InvalidArgumentCount {
            expected: 1,
            got: args.len(),
        });
    }
    let arg = &args[0];
    let ArgValue::Target(target) = &arg.value else {
        return Err(RuntimeError::TypeMismatch);
    };
    let reference = resolve_reference_for_lvalue(ctx, target)?;
    Ok(Value::Reference(Some(reference)))
}

#[cfg(test)]
mod tests {
    use super::{bind_split_args, bind_stdlib_named_args, ArgValue, CallArg, EvalContext, Expr};
    use crate::error::RuntimeError;
    use crate::memory::VariableStorage;
    use crate::stdlib::StdParams;
    use crate::value::{DateTimeProfile, Duration, Value};
    use trust_hir::types::TypeRegistry;

    fn make_context<'a>(
        storage: &'a mut VariableStorage,
        registry: &'a TypeRegistry,
    ) -> EvalContext<'a> {
        EvalContext {
            storage,
            registry,
            profile: DateTimeProfile::default(),
            now: Duration::ZERO,
            debug: None,
            call_depth: 0,
            functions: None,
            stdlib: None,
            function_blocks: None,
            classes: None,
            using: None,
            access: None,
            current_instance: None,
            return_name: None,
            loop_depth: 0,
            pause_requested: false,
        }
    }

    fn unnamed_literal_arg(value: Value) -> CallArg {
        CallArg {
            name: None,
            value: ArgValue::Expr(Expr::Literal(value)),
        }
    }

    #[test]
    fn bind_stdlib_named_args_rejects_unnamed_arg_without_panic() {
        let mut storage = VariableStorage::new();
        let registry = TypeRegistry::new();
        let mut ctx = make_context(&mut storage, &registry);
        let params = StdParams::Fixed(vec!["IN".into()]);
        let args = vec![unnamed_literal_arg(Value::Int(1))];

        let result = bind_stdlib_named_args(&mut ctx, &params, &args);
        assert!(matches!(
            result,
            Err(RuntimeError::InvalidArgumentName(name)) if name.as_str() == "<unnamed>"
        ));
    }

    #[test]
    fn bind_split_args_rejects_unnamed_named_call_without_panic() {
        let mut storage = VariableStorage::new();
        let registry = TypeRegistry::new();
        let mut ctx = make_context(&mut storage, &registry);
        let args = vec![
            CallArg {
                name: Some("IN".into()),
                value: ArgValue::Expr(Expr::Literal(Value::Int(1))),
            },
            unnamed_literal_arg(Value::Int(2)),
        ];

        let result = bind_split_args(&mut ctx, &["IN", "YEAR"], &args);
        assert!(matches!(
            result,
            Err(RuntimeError::InvalidArgumentName(name)) if name.as_str() == "<unnamed>"
        ));
    }
}
