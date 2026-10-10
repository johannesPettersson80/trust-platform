//! Selection standard functions (SEL, MIN, MAX, LIMIT, MUX).

use crate::error::RuntimeError;
use crate::stdlib::helpers::{
    coerce_to_common, common_kind, compare_common, require_arity, require_min, to_i64, CmpOp,
};
#[cfg(feature = "hir")]
use crate::stdlib::StandardLibrary;
use crate::value::Value;

/// Register the shared selection functions in a hosted registry.
#[cfg(feature = "hir")]
pub fn register(lib: &mut StandardLibrary) {
    lib.register_descriptors(FUNCTIONS);
}

pub(super) static FUNCTIONS: &[(&str, super::StdFunctionRef<'static>)] = &[
    super::registration::descriptor!("LIMIT", MN_IN_MX, limit),
    super::registration::descriptor!("MAX", VAR_IN_1_2, max),
    super::registration::descriptor!("MIN", VAR_IN_1_2, min),
    super::registration::descriptor!("MUX", VAR_K_IN_0_2, mux),
    super::registration::descriptor!("SEL", G_IN0_IN1, sel),
];

fn sel(args: &[Value]) -> Result<Value, RuntimeError> {
    require_arity(args, 3)?;
    let selector = match args[0] {
        Value::Bool(value) => value,
        _ => return Err(RuntimeError::TypeMismatch),
    };
    let kind = common_kind(&args[1..])?;
    let in0 = coerce_to_common(&args[1], &kind)?;
    let in1 = coerce_to_common(&args[2], &kind)?;
    Ok(if selector { in1 } else { in0 })
}

fn min(args: &[Value]) -> Result<Value, RuntimeError> {
    min_max(args, true)
}

fn max(args: &[Value]) -> Result<Value, RuntimeError> {
    min_max(args, false)
}

fn min_max(args: &[Value], is_min: bool) -> Result<Value, RuntimeError> {
    require_min(args, 2)?;
    let kind = common_kind(args)?;
    let mut best = coerce_to_common(&args[0], &kind)?;
    for value in &args[1..] {
        let candidate = coerce_to_common(value, &kind)?;
        let cmp = compare_common(
            &candidate,
            &best,
            &kind,
            if is_min { CmpOp::Lt } else { CmpOp::Gt },
        )?;
        if cmp {
            best = candidate;
        }
    }
    Ok(best)
}

fn limit(args: &[Value]) -> Result<Value, RuntimeError> {
    require_arity(args, 3)?;
    let kind = common_kind(args)?;
    let min = coerce_to_common(&args[0], &kind)?;
    let value = coerce_to_common(&args[1], &kind)?;
    let max = coerce_to_common(&args[2], &kind)?;
    if compare_common(&value, &min, &kind, CmpOp::Lt)? {
        Ok(min)
    } else if compare_common(&value, &max, &kind, CmpOp::Gt)? {
        Ok(max)
    } else {
        Ok(value)
    }
}

fn mux(args: &[Value]) -> Result<Value, RuntimeError> {
    require_min(args, 3)?;
    let selector = to_i64(&args[0])?;
    if selector < 0 {
        return Err(RuntimeError::IndexOutOfBounds {
            index: selector,
            lower: 0,
            upper: (args.len() - 2) as i64,
        });
    }
    let selector = selector as usize;
    let inputs = &args[1..];
    if selector >= inputs.len() {
        return Err(RuntimeError::IndexOutOfBounds {
            index: selector as i64,
            lower: 0,
            upper: (inputs.len() as i64) - 1,
        });
    }
    let kind = common_kind(inputs)?;
    coerce_to_common(&inputs[selector], &kind)
}
