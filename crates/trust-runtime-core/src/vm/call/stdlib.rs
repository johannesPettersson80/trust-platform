use super::context::CallContext;
#[cfg(all(feature = "std", feature = "test-support"))]
use crate::vm::diagnostic_timing::attribution::{self, CopyConsumerKind};
use alloc::vec::Vec;
use smol_str::SmolStr;

use crate::error::RuntimeError;
use crate::stdlib::{conversions, time, StdParams};
use crate::value::Value;

use super::super::errors::VmTrap;
use super::super::frames::VmFrame;
use super::bindings::{
    require_output_target, resolve_vm_arg_value, write_output_int, VmNativeArg, VmWriteTarget,
};

/// Dispatch a standard function or recognized conversion with native output handling.
pub fn dispatch_native_stdlib_call(
    runtime: &mut impl CallContext,
    frame: &mut VmFrame,
    target_name: &SmolStr,
    normalized_target_name: &SmolStr,
    conversion_spec: Option<conversions::ConversionSpec>,
    args: &[VmNativeArg],
) -> Result<Value, VmTrap> {
    #[cfg(all(feature = "std", feature = "test-support"))]
    let _timing =
        crate::vm::diagnostic_timing::begin(crate::vm::diagnostic_timing::Phase::BuiltinExecute);
    #[cfg(all(feature = "std", feature = "test-support"))]
    let _consumer = attribution::consumer(CopyConsumerKind::StandardLibrary, None);
    if time::is_runtime_clock_name(normalized_target_name.as_str()) {
        if !args.is_empty() {
            return Err(VmTrap::Runtime(RuntimeError::InvalidArgumentCount {
                expected: 0,
                got: args.len(),
            }));
        }
        return time::runtime_clock_value(
            normalized_target_name.as_str(),
            runtime.current_time(),
            || runtime.current_dt(),
        )
        .map_err(VmTrap::Runtime);
    }
    if time::is_split_name(normalized_target_name.as_str()) {
        return dispatch_native_split_call(runtime, frame, normalized_target_name.as_str(), args);
    }
    if let Some(conversion_spec) = conversion_spec {
        let value = bind_conversion_value(runtime, frame, args)?;
        charge_text_arguments(runtime, core::slice::from_ref(&value))?;
        let result =
            conversions::call_conversion_spec(conversion_spec, core::slice::from_ref(&value))
                .map_err(VmTrap::Runtime)?;
        charge_text_return(runtime, &result)?;
        return Ok(result);
    }
    if let Some(entry) = runtime.stdlib().get(normalized_target_name.as_str()) {
        let params = entry.params.clone();
        let func = entry.func;
        // The evaluated arguments come from the call depth's reserved values and return to it on
        // every path: success, a binding refusal and a refusal by the function (spec 34 MEM-09;
        // census C9).
        let mut values = runtime.take_call_stdlib_values();
        let result =
            bind_stdlib_values_into(runtime, frame, &params, args, &mut values).and_then(|()| {
                charge_text_arguments(runtime, &values)?;
                let result = func(&values).map_err(VmTrap::Runtime)?;
                charge_text_return(runtime, &result)?;
                Ok(result)
            });
        runtime.recycle_call_stdlib_values(values);
        return result;
    }
    Err(VmTrap::Runtime(RuntimeError::UndefinedFunction(
        target_name.clone(),
    )))
}

/// UTF-8 byte length of a STRING or WSTRING value, which TIME-08 charges per byte.
#[inline(always)]
fn text_byte_len(value: &Value) -> Option<usize> {
    match value {
        Value::String(text) => Some(text.len()),
        Value::WString(text) => Some(text.len()),
        _ => None,
    }
}

/// Charge the text bytes of every argument before the function body runs (TIME-02, TIME-08).
/// One charge covers all arguments, so a refusal leaves the allowance and the body untouched.
/// A call without text arguments charges nothing, as an empty work charge consumes nothing.
#[inline(always)]
fn charge_text_arguments(runtime: &mut impl CallContext, values: &[Value]) -> Result<(), VmTrap> {
    let mut bytes = 0usize;
    for len in values.iter().filter_map(text_byte_len) {
        bytes = bytes
            .checked_add(len)
            .ok_or_else(|| VmTrap::Runtime(RuntimeError::Overflow))?;
    }
    if bytes == 0 {
        return Ok(());
    }
    runtime.charge_text_work(bytes).map_err(VmTrap::Runtime)
}

/// Charge a text result as a new value after the function body returns (TIME-08): its copy
/// allocation, the UTF-8 bytes of a STRING or two bytes per UTF-16 code unit of a WSTRING.
/// A refused charge discards the result.
#[inline(always)]
fn charge_text_return(runtime: &mut impl CallContext, result: &Value) -> Result<(), VmTrap> {
    if !matches!(result, Value::String(_) | Value::WString(_)) {
        return Ok(());
    }
    runtime.charge_text_result(result).map_err(VmTrap::Runtime)
}

/// Bind the single operand of a recognized conversion.
pub fn bind_conversion_value(
    runtime: &mut impl CallContext,
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
    if let Some(name) = arg.name.as_ref()
        && !name.eq_ignore_ascii_case("IN")
    {
        return Err(VmTrap::Runtime(RuntimeError::InvalidArgumentName(
            name.clone(),
        )));
    }
    #[cfg(all(feature = "std", feature = "test-support"))]
    let _parameter = attribution::parameter(0, 3, false);
    resolve_vm_arg_value(runtime, frame, arg)
}

/// The widest output list of a `SPLIT_*` function: `SPLIT_DT`'s and `SPLIT_LDT`'s seven
/// (spec 34 MEM-09; census C10).
const MAX_SPLIT_OUTPUTS: usize = 7;
/// The widest formal list of a `SPLIT_*` function: an input plus `SPLIT_DT`'s seven outputs.
const MAX_SPLIT_FORMALS: usize = MAX_SPLIT_OUTPUTS + 1;

/// One `SPLIT_*` call's bound output targets in declared order (spec 34 MEM-09; census C10). The
/// list is a fixed array, so binding a split call allocates no output vector; the binder fills
/// the first `count` slots, where `count` is the called function's declared output count.
struct SplitOutputs {
    targets: [Option<VmWriteTarget>; MAX_SPLIT_OUTPUTS],
    count: usize,
}

impl SplitOutputs {
    /// An empty output list.
    fn empty() -> Self {
        Self {
            targets: core::array::from_fn(|_| None),
            count: 0,
        }
    }

    /// Append a bound output target. The widest split has [`MAX_SPLIT_OUTPUTS`] outputs.
    fn push(&mut self, target: VmWriteTarget) -> Result<(), VmTrap> {
        let Some(slot) = self.targets.get_mut(self.count) else {
            return Err(VmTrap::InvalidNativeCall("split output count".into()));
        };
        *slot = Some(target);
        self.count += 1;
        Ok(())
    }

    /// Give the buffers of the targets' paths back to `runtime` (spec 34 MEM-09; C1-19c4).
    fn recycle(self, runtime: &impl CallContext) {
        for target in self.targets.into_iter().flatten() {
            target.recycle(runtime);
        }
    }
}

fn dispatch_native_split_call(
    runtime: &mut impl CallContext,
    frame: &mut VmFrame,
    name: &str,
    args: &[VmNativeArg],
) -> Result<Value, VmTrap> {
    let params = time::split_parameter_names(name)
        .ok_or_else(|| VmTrap::Runtime(RuntimeError::UndefinedFunction(name.into())))?;

    let (input, outputs) = bind_split_vm_args(runtime, frame, params, args)?;
    let result = runtime.with_output_transaction(frame, |runtime, frame| {
        match name {
            "SPLIT_DATE" => {
                let [Some(year_output), Some(month_output), Some(day_output)] =
                    &outputs.targets[..outputs.count]
                else {
                    return Err(VmTrap::InvalidNativeCall("split output count".into()));
                };
                let (year, month, day) = time::split_date(&input, *runtime.profile())?;
                write_output_int(runtime, frame, year_output, year)?;
                write_output_int(runtime, frame, month_output, month)?;
                write_output_int(runtime, frame, day_output, day)?;
            }
            "SPLIT_TOD" => {
                let [Some(hour_output), Some(minute_output), Some(second_output), Some(millis_output)] =
                    &outputs.targets[..outputs.count]
                else {
                    return Err(VmTrap::InvalidNativeCall("split output count".into()));
                };
                let (hour, minute, second, millis) = time::split_tod(&input, *runtime.profile())?;
                write_output_int(runtime, frame, hour_output, hour)?;
                write_output_int(runtime, frame, minute_output, minute)?;
                write_output_int(runtime, frame, second_output, second)?;
                write_output_int(runtime, frame, millis_output, millis)?;
            }
            "SPLIT_LTOD" => {
                let [Some(hour_output), Some(minute_output), Some(second_output), Some(millis_output)] =
                    &outputs.targets[..outputs.count]
                else {
                    return Err(VmTrap::InvalidNativeCall("split output count".into()));
                };
                let (hour, minute, second, millis) = time::split_ltod(&input)?;
                write_output_int(runtime, frame, hour_output, hour)?;
                write_output_int(runtime, frame, minute_output, minute)?;
                write_output_int(runtime, frame, second_output, second)?;
                write_output_int(runtime, frame, millis_output, millis)?;
            }
            "SPLIT_DT" => {
                let [
                    Some(year_output),
                    Some(month_output),
                    Some(day_output),
                    Some(hour_output),
                    Some(minute_output),
                    Some(second_output),
                    Some(millis_output),
                ] = &outputs.targets[..outputs.count]
                else {
                    return Err(VmTrap::InvalidNativeCall("split output count".into()));
                };
                let (year, month, day, hour, minute, second, millis) =
                    time::split_dt(&input, *runtime.profile())?;
                write_output_int(runtime, frame, year_output, year)?;
                write_output_int(runtime, frame, month_output, month)?;
                write_output_int(runtime, frame, day_output, day)?;
                write_output_int(runtime, frame, hour_output, hour)?;
                write_output_int(runtime, frame, minute_output, minute)?;
                write_output_int(runtime, frame, second_output, second)?;
                write_output_int(runtime, frame, millis_output, millis)?;
            }
            "SPLIT_LDT" => {
                let [
                    Some(year_output),
                    Some(month_output),
                    Some(day_output),
                    Some(hour_output),
                    Some(minute_output),
                    Some(second_output),
                    Some(millis_output),
                ] = &outputs.targets[..outputs.count]
                else {
                    return Err(VmTrap::InvalidNativeCall("split output count".into()));
                };
                let (year, month, day, hour, minute, second, millis) = time::split_ldt(&input)?;
                write_output_int(runtime, frame, year_output, year)?;
                write_output_int(runtime, frame, month_output, month)?;
                write_output_int(runtime, frame, day_output, day)?;
                write_output_int(runtime, frame, hour_output, hour)?;
                write_output_int(runtime, frame, minute_output, minute)?;
                write_output_int(runtime, frame, second_output, second)?;
                write_output_int(runtime, frame, millis_output, millis)?;
            }
            _ => {
                return Err(VmTrap::InvalidNativeCall(
                    "invalid split function".into(),
                ));
            }
        }
        Ok(Value::Null)
    });
    outputs.recycle(runtime);
    result
}

fn bind_split_vm_args(
    runtime: &mut impl CallContext,
    frame: &VmFrame,
    params: &[&str],
    args: &[VmNativeArg],
) -> Result<(Value, SplitOutputs), VmTrap> {
    if params.len() > MAX_SPLIT_FORMALS {
        return Err(VmTrap::InvalidNativeCall("split formals count".into()));
    }
    let positional = args.iter().all(|arg| arg.name.is_none());
    if positional {
        if args.len() != params.len() {
            return Err(VmTrap::Runtime(RuntimeError::InvalidArgumentCount {
                expected: params.len(),
                got: args.len(),
            }));
        }
        let mut outputs = SplitOutputs::empty();
        #[cfg(all(feature = "std", feature = "test-support"))]
        let _parameter = attribution::parameter(0, 3, false);
        let input = resolve_vm_arg_value(runtime, frame, &args[0])?;
        for arg in &args[1..] {
            outputs.push(require_output_target(runtime, arg)?)?;
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

    let mut assigned: [Option<&VmNativeArg>; MAX_SPLIT_FORMALS] = [None; MAX_SPLIT_FORMALS];
    for arg in args {
        let Some(name) = arg.name.as_ref() else {
            return Err(VmTrap::Runtime(RuntimeError::InvalidArgumentName(
                "<unnamed>".into(),
            )));
        };
        let position = params
            .iter()
            .position(|param| param.eq_ignore_ascii_case(name))
            .ok_or_else(|| VmTrap::Runtime(RuntimeError::InvalidArgumentName(name.clone())))?;
        if assigned[position].is_some() {
            return Err(VmTrap::Runtime(RuntimeError::InvalidArgumentName(
                name.clone(),
            )));
        }
        assigned[position] = Some(arg);
    }

    #[cfg(all(feature = "std", feature = "test-support"))]
    let _parameter = attribution::parameter(0, 3, false);
    let input = assigned[0]
        .ok_or({
            VmTrap::Runtime(RuntimeError::InvalidArgumentCount {
                expected: params.len(),
                got: args.len(),
            })
        })
        .and_then(|arg| resolve_vm_arg_value(runtime, frame, arg))?;
    let mut outputs = SplitOutputs::empty();
    for arg in assigned
        .into_iter()
        .skip(1)
        .take(params.len().saturating_sub(1))
    {
        let arg = arg.ok_or({
            VmTrap::Runtime(RuntimeError::InvalidArgumentCount {
                expected: params.len(),
                got: args.len(),
            })
        })?;
        outputs.push(require_output_target(runtime, arg)?)?;
    }
    Ok((input, outputs))
}

/// Bind standard-function arguments into the call depth's reserved value vector (spec 34
/// MEM-09; census C9).
fn bind_stdlib_values_into(
    runtime: &mut impl CallContext,
    frame: &VmFrame,
    params: &StdParams,
    args: &[VmNativeArg],
    values: &mut Vec<Value>,
) -> Result<(), VmTrap> {
    let positional = args.iter().all(|arg| arg.name.is_none());
    if positional {
        return bind_stdlib_positional_values_into(runtime, frame, params, args, values);
    }
    if args.iter().any(|arg| arg.name.is_none()) {
        return Err(VmTrap::Runtime(RuntimeError::InvalidArgumentName(
            "<unnamed>".into(),
        )));
    }
    bind_stdlib_named_values_into(runtime, frame, params, args, values)
}

/// Bind standard-function arguments by declared positional order into a fresh vector. The hosted
/// facade and its tests use this form; the engine binds into its reserved values (spec 34
/// MEM-09; census C9).
#[cfg(any(feature = "hir", test))]
pub fn bind_stdlib_positional_values(
    runtime: &mut impl CallContext,
    frame: &VmFrame,
    params: &StdParams,
    args: &[VmNativeArg],
) -> Result<Vec<Value>, VmTrap> {
    let mut values = Vec::new();
    bind_stdlib_positional_values_into(runtime, frame, params, args, &mut values)?;
    Ok(values)
}

fn bind_stdlib_positional_values_into(
    runtime: &mut impl CallContext,
    frame: &VmFrame,
    params: &StdParams,
    args: &[VmNativeArg],
    values: &mut Vec<Value>,
) -> Result<(), VmTrap> {
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
    values.clear();
    #[cfg(all(feature = "std", feature = "test-support"))]
    let mut diagnostic_formals = 0..;
    for arg in args {
        #[cfg(all(feature = "std", feature = "test-support"))]
        let _parameter =
            attribution::parameter(diagnostic_formals.next().unwrap_or(usize::MAX), 3, false);
        values.push(resolve_vm_arg_value(runtime, frame, arg)?);
    }
    Ok(())
}

/// Bind standard-function arguments by their declared names into a fresh vector. The hosted
/// facade and its tests use this form; the engine binds into its reserved values (spec 34
/// MEM-09; census C9).
#[cfg(any(feature = "hir", test))]
pub fn bind_stdlib_named_values(
    runtime: &mut impl CallContext,
    frame: &VmFrame,
    params: &StdParams,
    args: &[VmNativeArg],
) -> Result<Vec<Value>, VmTrap> {
    let mut values = Vec::new();
    bind_stdlib_named_values_into(runtime, frame, params, args, &mut values)?;
    Ok(values)
}

fn bind_stdlib_named_values_into(
    runtime: &mut impl CallContext,
    frame: &VmFrame,
    params: &StdParams,
    args: &[VmNativeArg],
    values: &mut Vec<Value>,
) -> Result<(), VmTrap> {
    match params {
        StdParams::Fixed(params) => {
            bind_stdlib_named_values_fixed_into(runtime, frame, params, args, values)
        }
        StdParams::Variadic {
            fixed,
            prefix,
            start,
            min,
        } => bind_stdlib_named_values_variadic_into(
            runtime, frame, fixed, prefix, *start, *min, args, values,
        ),
    }
}

fn bind_stdlib_named_values_fixed_into(
    runtime: &mut impl CallContext,
    frame: &VmFrame,
    params: &[SmolStr],
    args: &[VmNativeArg],
    values: &mut Vec<Value>,
) -> Result<(), VmTrap> {
    if args.len() != params.len() {
        return Err(VmTrap::Runtime(RuntimeError::InvalidArgumentCount {
            expected: params.len(),
            got: args.len(),
        }));
    }

    values.clear();
    values.resize(params.len(), Value::Null);
    let mut assigned = runtime.take_call_flags(params.len());
    let result =
        bind_stdlib_named_values_fixed_inner(runtime, frame, params, args, values, &mut assigned);
    runtime.recycle_call_flags(assigned);
    result
}

/// The binding loop of [`bind_stdlib_named_values_fixed_into`], writing the reserved values and
/// marking the formals they fill with the call depth's flags.
fn bind_stdlib_named_values_fixed_inner(
    runtime: &mut impl CallContext,
    frame: &VmFrame,
    params: &[SmolStr],
    args: &[VmNativeArg],
    values: &mut [Value],
    assigned: &mut [bool],
) -> Result<(), VmTrap> {
    for arg in args {
        let Some(name) = arg.name.as_ref() else {
            return Err(VmTrap::Runtime(RuntimeError::InvalidArgumentName(
                "<unnamed>".into(),
            )));
        };
        let position = params
            .iter()
            .position(|param| formal_matches(param.as_str(), name))
            .ok_or_else(|| VmTrap::Runtime(RuntimeError::InvalidArgumentName(name.clone())))?;
        if assigned[position] {
            return Err(VmTrap::Runtime(RuntimeError::InvalidArgumentName(
                name.clone(),
            )));
        }
        #[cfg(all(feature = "std", feature = "test-support"))]
        let _parameter = attribution::parameter(position, 3, false);
        values[position] = resolve_vm_arg_value(runtime, frame, arg)?;
        assigned[position] = true;
    }

    if assigned.iter().any(|assigned| !*assigned) {
        return Err(VmTrap::Runtime(RuntimeError::InvalidArgumentCount {
            expected: params.len(),
            got: args.len(),
        }));
    }
    Ok(())
}

#[allow(
    clippy::too_many_arguments,
    reason = "the variadic binding plan and the call depth's reserved values and flags"
)]
fn bind_stdlib_named_values_variadic_into(
    runtime: &mut impl CallContext,
    frame: &VmFrame,
    fixed: &[SmolStr],
    prefix: &SmolStr,
    start: usize,
    min: usize,
    args: &[VmNativeArg],
    values: &mut Vec<Value>,
) -> Result<(), VmTrap> {
    values.clear();
    values.resize(fixed.len(), Value::Null);
    let mut assigned = runtime.take_call_flags(fixed.len());
    let result = bind_stdlib_named_values_variadic_inner(
        runtime,
        frame,
        fixed,
        prefix.as_str(),
        start,
        min,
        args,
        values,
        &mut assigned,
    );
    runtime.recycle_call_flags(assigned);
    result
}

/// The binding loop of [`bind_stdlib_named_values_variadic_into`], writing the reserved values
/// and marking the formals they fill with the call depth's flags. A variadic index beyond the
/// reservation grows both vectors, as a fresh binding vector grew today; only a refused call
/// reaches it.
#[allow(
    clippy::too_many_arguments,
    reason = "the variadic binding plan and the call depth's reserved values and flags"
)]
fn bind_stdlib_named_values_variadic_inner(
    runtime: &mut impl CallContext,
    frame: &VmFrame,
    fixed: &[SmolStr],
    prefix: &str,
    start: usize,
    min: usize,
    args: &[VmNativeArg],
    values: &mut Vec<Value>,
    assigned: &mut Vec<bool>,
) -> Result<(), VmTrap> {
    let mut max_index: Option<usize> = None;

    for arg in args {
        let Some(name) = arg.name.as_ref() else {
            return Err(VmTrap::Runtime(RuntimeError::InvalidArgumentName(
                "<unnamed>".into(),
            )));
        };
        if let Some(position) = fixed
            .iter()
            .position(|param| formal_matches(param.as_str(), name))
        {
            if assigned[position] {
                return Err(VmTrap::Runtime(RuntimeError::InvalidArgumentName(
                    name.clone(),
                )));
            }
            #[cfg(all(feature = "std", feature = "test-support"))]
            let _parameter = attribution::parameter(position, 3, false);
            values[position] = resolve_vm_arg_value(runtime, frame, arg)?;
            assigned[position] = true;
            continue;
        }

        let Some(suffix) = variadic_suffix(name.as_str(), prefix) else {
            return Err(VmTrap::Runtime(RuntimeError::InvalidArgumentName(
                name.clone(),
            )));
        };
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
        let slot = fixed.len() + offset;
        if values.len() <= slot {
            values.resize(slot + 1, Value::Null);
            assigned.resize(slot + 1, false);
        }
        if assigned[slot] {
            return Err(VmTrap::Runtime(RuntimeError::InvalidArgumentName(
                name.clone(),
            )));
        }
        #[cfg(all(feature = "std", feature = "test-support"))]
        let _parameter = attribution::variadic_parameter(fixed.len(), offset);
        values[slot] = resolve_vm_arg_value(runtime, frame, arg)?;
        assigned[slot] = true;
        max_index = Some(max_index.map_or(offset, |max| max.max(offset)));
    }

    if assigned.iter().take(fixed.len()).any(|assigned| !*assigned) {
        return Err(VmTrap::Runtime(RuntimeError::InvalidArgumentCount {
            expected: fixed.len() + min,
            got: args.len(),
        }));
    }

    let count = max_index.map_or(0, |index| index + 1);
    if count < min {
        return Err(VmTrap::Runtime(RuntimeError::InvalidArgumentCount {
            expected: fixed.len() + min,
            got: args.len(),
        }));
    }
    for idx in 0..count {
        if !assigned[fixed.len() + idx] {
            return Err(VmTrap::Runtime(RuntimeError::InvalidArgumentCount {
                expected: fixed.len() + count,
                got: args.len(),
            }));
        }
    }

    Ok(())
}

/// Whether a formal name equals the actual argument name with its ASCII letters made uppercase,
/// compared byte by byte (spec 34 MEM-09, §6.3; census C9). The comparison builds no uppercase
/// name, and a lowercase formal never matches, exactly as a named argument never matched one when
/// the actual name was uppercased into a new string.
#[inline]
pub(crate) fn formal_matches(formal: &str, actual: &str) -> bool {
    formal.len() == actual.len()
        && formal
            .bytes()
            .zip(actual.bytes())
            .all(|(formal, actual)| formal == actual.to_ascii_uppercase())
}

/// The suffix of `name` after `prefix`, matching the prefix with its ASCII letters made
/// uppercase, or `None` when it does not match (spec 34 MEM-09, §6.3; census C9). The matched
/// prefix bytes are ASCII, so the suffix starts on a character boundary.
#[inline]
fn variadic_suffix<'a>(name: &'a str, prefix: &str) -> Option<&'a str> {
    let bytes = name.as_bytes();
    let prefix_bytes = prefix.as_bytes();
    if bytes.len() < prefix_bytes.len()
        || !prefix_bytes
            .iter()
            .zip(bytes)
            .all(|(prefix, byte)| *prefix == byte.to_ascii_uppercase())
    {
        return None;
    }
    Some(&name[prefix_bytes.len()..])
}
