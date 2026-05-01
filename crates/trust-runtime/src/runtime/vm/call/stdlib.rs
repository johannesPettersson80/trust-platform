use smol_str::SmolStr;

use crate::error::RuntimeError;
use crate::stdlib::{conversions, time, StdParams};
use crate::value::Value;

use super::super::errors::VmTrap;
use super::super::frames::VmFrame;
use super::bindings::{
    require_output_target, resolve_vm_arg_value, write_output_int, VmNativeArg, VmWriteTarget,
};

pub(super) fn dispatch_native_stdlib_call(
    runtime: &mut super::super::super::core::Runtime,
    frame: &mut VmFrame,
    target_name: &SmolStr,
    normalized_target_name: &SmolStr,
    conversion_spec: Option<conversions::ConversionSpec>,
    args: &[VmNativeArg],
) -> Result<Value, VmTrap> {
    if time::is_runtime_clock_name(normalized_target_name.as_str()) {
        if !args.is_empty() {
            return Err(VmTrap::Runtime(RuntimeError::InvalidArgumentCount {
                expected: 0,
                got: args.len(),
            }));
        }
        return Ok(Value::Time(runtime.current_time()));
    }
    if time::is_split_name(normalized_target_name.as_str()) {
        return dispatch_native_split_call(runtime, frame, normalized_target_name.as_str(), args);
    }
    if let Some(conversion_spec) = conversion_spec {
        let value = bind_conversion_value(runtime, frame, args)?;
        return conversions::call_conversion_spec(conversion_spec, std::slice::from_ref(&value))
            .map_err(VmTrap::Runtime);
    }
    if let Some(entry) = runtime.stdlib().get(normalized_target_name.as_str()) {
        let params = entry.params.clone();
        let func = entry.func;
        let values = bind_stdlib_values(runtime, frame, &params, args)?;
        return func(&values).map_err(VmTrap::Runtime);
    }
    Err(VmTrap::Runtime(RuntimeError::UndefinedFunction(
        target_name.clone(),
    )))
}

pub(super) fn bind_conversion_value(
    runtime: &mut super::super::super::core::Runtime,
    frame: &VmFrame,
    args: &[VmNativeArg],
) -> Result<Value, VmTrap> {
    let positional = args.iter().all(|arg| arg.name.is_none());
    if !positional && args.iter().any(|arg| arg.name.is_none()) {
        return Err(VmTrap::Runtime(RuntimeError::InvalidArgumentName(
            "<unnamed>".into(),
        )));
    }
    if args.len() != 1 {
        return Err(VmTrap::Runtime(RuntimeError::InvalidArgumentCount {
            expected: 1,
            got: args.len(),
        }));
    }
    let arg = &args[0];
    if let Some(name) = arg.name.as_ref() {
        if !name.eq_ignore_ascii_case("IN") {
            return Err(VmTrap::Runtime(RuntimeError::InvalidArgumentName(
                name.clone(),
            )));
        }
    }
    resolve_vm_arg_value(runtime, frame, arg)
}

fn dispatch_native_split_call(
    runtime: &mut super::super::super::core::Runtime,
    frame: &mut VmFrame,
    name: &str,
    args: &[VmNativeArg],
) -> Result<Value, VmTrap> {
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
        _ => {
            return Err(VmTrap::Runtime(RuntimeError::UndefinedFunction(
                name.into(),
            )))
        }
    };

    let (input, outputs) = bind_split_vm_args(runtime, frame, params, args)?;
    match name {
        "SPLIT_DATE" => {
            let (year, month, day) = time::split_date(&input, runtime.profile)?;
            write_output_int(runtime, frame, &outputs[0], year)?;
            write_output_int(runtime, frame, &outputs[1], month)?;
            write_output_int(runtime, frame, &outputs[2], day)?;
        }
        "SPLIT_TOD" => {
            let (hour, minute, second, millis) = time::split_tod(&input, runtime.profile)?;
            write_output_int(runtime, frame, &outputs[0], hour)?;
            write_output_int(runtime, frame, &outputs[1], minute)?;
            write_output_int(runtime, frame, &outputs[2], second)?;
            write_output_int(runtime, frame, &outputs[3], millis)?;
        }
        "SPLIT_LTOD" => {
            let (hour, minute, second, millis) = time::split_ltod(&input)?;
            write_output_int(runtime, frame, &outputs[0], hour)?;
            write_output_int(runtime, frame, &outputs[1], minute)?;
            write_output_int(runtime, frame, &outputs[2], second)?;
            write_output_int(runtime, frame, &outputs[3], millis)?;
        }
        "SPLIT_DT" => {
            let (year, month, day, hour, minute, second, millis) =
                time::split_dt(&input, runtime.profile)?;
            write_output_int(runtime, frame, &outputs[0], year)?;
            write_output_int(runtime, frame, &outputs[1], month)?;
            write_output_int(runtime, frame, &outputs[2], day)?;
            write_output_int(runtime, frame, &outputs[3], hour)?;
            write_output_int(runtime, frame, &outputs[4], minute)?;
            write_output_int(runtime, frame, &outputs[5], second)?;
            write_output_int(runtime, frame, &outputs[6], millis)?;
        }
        "SPLIT_LDT" => {
            let (year, month, day, hour, minute, second, millis) = time::split_ldt(&input)?;
            write_output_int(runtime, frame, &outputs[0], year)?;
            write_output_int(runtime, frame, &outputs[1], month)?;
            write_output_int(runtime, frame, &outputs[2], day)?;
            write_output_int(runtime, frame, &outputs[3], hour)?;
            write_output_int(runtime, frame, &outputs[4], minute)?;
            write_output_int(runtime, frame, &outputs[5], second)?;
            write_output_int(runtime, frame, &outputs[6], millis)?;
        }
        _ => {}
    }
    Ok(Value::Null)
}

fn bind_split_vm_args(
    runtime: &mut super::super::super::core::Runtime,
    frame: &VmFrame,
    params: &[&str],
    args: &[VmNativeArg],
) -> Result<(Value, Vec<VmWriteTarget>), VmTrap> {
    let positional = args.iter().all(|arg| arg.name.is_none());
    if positional {
        if args.len() != params.len() {
            return Err(VmTrap::Runtime(RuntimeError::InvalidArgumentCount {
                expected: params.len(),
                got: args.len(),
            }));
        }
        let mut outputs = Vec::with_capacity(params.len().saturating_sub(1));
        let input = resolve_vm_arg_value(runtime, frame, &args[0])?;
        for arg in &args[1..] {
            outputs.push(require_output_target(arg)?);
        }
        return Ok((input, outputs));
    }

    if args.iter().any(|arg| arg.name.is_none()) {
        return Err(VmTrap::Runtime(RuntimeError::InvalidArgumentName(
            "<unnamed>".into(),
        )));
    }
    if args.len() != params.len() {
        return Err(VmTrap::Runtime(RuntimeError::InvalidArgumentCount {
            expected: params.len(),
            got: args.len(),
        }));
    }

    let mut assigned: Vec<Option<&VmNativeArg>> = vec![None; params.len()];
    for arg in args {
        let Some(name) = arg.name.as_ref() else {
            return Err(VmTrap::Runtime(RuntimeError::InvalidArgumentName(
                "<unnamed>".into(),
            )));
        };
        let key = name.to_ascii_uppercase();
        let position = params
            .iter()
            .position(|param| param.eq_ignore_ascii_case(&key))
            .ok_or_else(|| VmTrap::Runtime(RuntimeError::InvalidArgumentName(name.clone())))?;
        if assigned[position].is_some() {
            return Err(VmTrap::Runtime(RuntimeError::InvalidArgumentName(
                name.clone(),
            )));
        }
        assigned[position] = Some(arg);
    }

    let input = assigned[0]
        .ok_or({
            VmTrap::Runtime(RuntimeError::InvalidArgumentCount {
                expected: params.len(),
                got: args.len(),
            })
        })
        .and_then(|arg| resolve_vm_arg_value(runtime, frame, arg))?;
    let mut outputs = Vec::with_capacity(params.len().saturating_sub(1));
    for arg in assigned.into_iter().skip(1) {
        let arg = arg.ok_or({
            VmTrap::Runtime(RuntimeError::InvalidArgumentCount {
                expected: params.len(),
                got: args.len(),
            })
        })?;
        outputs.push(require_output_target(arg)?);
    }
    Ok((input, outputs))
}

fn bind_stdlib_values(
    runtime: &mut super::super::super::core::Runtime,
    frame: &VmFrame,
    params: &StdParams,
    args: &[VmNativeArg],
) -> Result<Vec<Value>, VmTrap> {
    let positional = args.iter().all(|arg| arg.name.is_none());
    if positional {
        return bind_stdlib_positional_values(runtime, frame, params, args);
    }
    if args.iter().any(|arg| arg.name.is_none()) {
        return Err(VmTrap::Runtime(RuntimeError::InvalidArgumentName(
            "<unnamed>".into(),
        )));
    }
    bind_stdlib_named_values(runtime, frame, params, args)
}

pub(super) fn bind_stdlib_positional_values(
    runtime: &mut super::super::super::core::Runtime,
    frame: &VmFrame,
    params: &StdParams,
    args: &[VmNativeArg],
) -> Result<Vec<Value>, VmTrap> {
    match params {
        StdParams::Fixed(expected) => {
            if args.len() != expected.len() {
                return Err(VmTrap::Runtime(RuntimeError::InvalidArgumentCount {
                    expected: expected.len(),
                    got: args.len(),
                }));
            }
        }
        StdParams::Variadic { fixed, min, .. } => {
            let expected = fixed.len() + *min;
            if args.len() < expected {
                return Err(VmTrap::Runtime(RuntimeError::InvalidArgumentCount {
                    expected,
                    got: args.len(),
                }));
            }
        }
    }
    let mut values = Vec::with_capacity(args.len());
    for arg in args {
        values.push(resolve_vm_arg_value(runtime, frame, arg)?);
    }
    Ok(values)
}

pub(super) fn bind_stdlib_named_values(
    runtime: &mut super::super::super::core::Runtime,
    frame: &VmFrame,
    params: &StdParams,
    args: &[VmNativeArg],
) -> Result<Vec<Value>, VmTrap> {
    match params {
        StdParams::Fixed(params) => bind_stdlib_named_values_fixed(runtime, frame, params, args),
        StdParams::Variadic {
            fixed,
            prefix,
            start,
            min,
        } => bind_stdlib_named_values_variadic(runtime, frame, fixed, prefix, *start, *min, args),
    }
}

fn bind_stdlib_named_values_fixed(
    runtime: &mut super::super::super::core::Runtime,
    frame: &VmFrame,
    params: &[SmolStr],
    args: &[VmNativeArg],
) -> Result<Vec<Value>, VmTrap> {
    if args.len() != params.len() {
        return Err(VmTrap::Runtime(RuntimeError::InvalidArgumentCount {
            expected: params.len(),
            got: args.len(),
        }));
    }

    let mut values: Vec<Option<Value>> = vec![None; params.len()];
    for arg in args {
        let Some(name) = arg.name.as_ref() else {
            return Err(VmTrap::Runtime(RuntimeError::InvalidArgumentName(
                "<unnamed>".into(),
            )));
        };
        let key = name.to_ascii_uppercase();
        let position = params
            .iter()
            .position(|param| param.as_str() == key)
            .ok_or_else(|| VmTrap::Runtime(RuntimeError::InvalidArgumentName(name.clone())))?;
        if values[position].is_some() {
            return Err(VmTrap::Runtime(RuntimeError::InvalidArgumentName(
                name.clone(),
            )));
        }
        values[position] = Some(resolve_vm_arg_value(runtime, frame, arg)?);
    }

    let mut resolved = Vec::with_capacity(values.len());
    for value in values {
        let Some(value) = value else {
            return Err(VmTrap::Runtime(RuntimeError::InvalidArgumentCount {
                expected: params.len(),
                got: args.len(),
            }));
        };
        resolved.push(value);
    }
    Ok(resolved)
}

fn bind_stdlib_named_values_variadic(
    runtime: &mut super::super::super::core::Runtime,
    frame: &VmFrame,
    fixed: &[SmolStr],
    prefix: &SmolStr,
    start: usize,
    min: usize,
    args: &[VmNativeArg],
) -> Result<Vec<Value>, VmTrap> {
    let mut fixed_values: Vec<Option<Value>> = vec![None; fixed.len()];
    let mut variadic_values: Vec<Option<Value>> = Vec::new();
    let mut max_index: Option<usize> = None;

    for arg in args {
        let Some(name) = arg.name.as_ref() else {
            return Err(VmTrap::Runtime(RuntimeError::InvalidArgumentName(
                "<unnamed>".into(),
            )));
        };
        let key = name.to_ascii_uppercase();
        if let Some(position) = fixed.iter().position(|param| param.as_str() == key) {
            if fixed_values[position].is_some() {
                return Err(VmTrap::Runtime(RuntimeError::InvalidArgumentName(
                    name.clone(),
                )));
            }
            fixed_values[position] = Some(resolve_vm_arg_value(runtime, frame, arg)?);
            continue;
        }

        let prefix_str = prefix.as_str();
        if let Some(suffix) = key.strip_prefix(prefix_str) {
            if suffix.is_empty() {
                return Err(VmTrap::Runtime(RuntimeError::InvalidArgumentName(
                    name.clone(),
                )));
            }
            let index = suffix
                .parse::<usize>()
                .map_err(|_| VmTrap::Runtime(RuntimeError::InvalidArgumentName(name.clone())))?;
            if index < start {
                return Err(VmTrap::Runtime(RuntimeError::InvalidArgumentName(
                    name.clone(),
                )));
            }
            let offset = index - start;
            if variadic_values.len() <= offset {
                variadic_values.resize(offset + 1, None);
            }
            if variadic_values[offset].is_some() {
                return Err(VmTrap::Runtime(RuntimeError::InvalidArgumentName(
                    name.clone(),
                )));
            }
            variadic_values[offset] = Some(resolve_vm_arg_value(runtime, frame, arg)?);
            max_index = Some(max_index.map_or(offset, |max| max.max(offset)));
            continue;
        }

        return Err(VmTrap::Runtime(RuntimeError::InvalidArgumentName(
            name.clone(),
        )));
    }

    for value in &fixed_values {
        if value.is_none() {
            return Err(VmTrap::Runtime(RuntimeError::InvalidArgumentCount {
                expected: fixed.len() + min,
                got: args.len(),
            }));
        }
    }

    let count = max_index.map(|idx| idx + 1).unwrap_or(0);
    if count < min {
        return Err(VmTrap::Runtime(RuntimeError::InvalidArgumentCount {
            expected: fixed.len() + min,
            got: args.len(),
        }));
    }
    for idx in 0..count {
        if variadic_values
            .get(idx)
            .and_then(|value| value.as_ref())
            .is_none()
        {
            return Err(VmTrap::Runtime(RuntimeError::InvalidArgumentCount {
                expected: fixed.len() + count,
                got: args.len(),
            }));
        }
    }

    let mut resolved = Vec::with_capacity(fixed.len() + count);
    resolved.extend(
        fixed_values
            .into_iter()
            .map(|value| value.expect("fixed variadic values were validated")),
    );
    resolved.extend(
        variadic_values
            .into_iter()
            .take(count)
            .map(|value| value.expect("variadic values were validated")),
    );
    Ok(resolved)
}
