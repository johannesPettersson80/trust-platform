//! Comparison standard functions (GT, GE, EQ, LE, LT, NE).

use crate::error::RuntimeError;
use crate::stdlib::helpers::{
    coerce_to_common, common_kind, compare_common, require_arity, require_min, CmpOp,
};
#[cfg(feature = "hir")]
use crate::stdlib::StandardLibrary;
use crate::value::Value;

/// Register the shared comparison functions in a hosted registry.
#[cfg(feature = "hir")]
pub fn register(lib: &mut StandardLibrary) {
    lib.register_descriptors(FUNCTIONS);
}

pub(super) static FUNCTIONS: &[(&str, super::StdFunctionRef<'static>)] = &[
    super::registration::descriptor!("EQ", VAR_IN_1_2, eq),
    super::registration::descriptor!("GE", VAR_IN_1_2, ge),
    super::registration::descriptor!("GT", VAR_IN_1_2, gt),
    super::registration::descriptor!("LE", VAR_IN_1_2, le),
    super::registration::descriptor!("LT", VAR_IN_1_2, lt),
    super::registration::descriptor!("NE", IN1_IN2, ne),
];

fn gt(args: &[Value]) -> Result<Value, RuntimeError> {
    compare_chain(args, CmpOp::Gt)
}

fn ge(args: &[Value]) -> Result<Value, RuntimeError> {
    compare_chain(args, CmpOp::Ge)
}

fn eq(args: &[Value]) -> Result<Value, RuntimeError> {
    compare_chain(args, CmpOp::Eq)
}

fn le(args: &[Value]) -> Result<Value, RuntimeError> {
    compare_chain(args, CmpOp::Le)
}

fn lt(args: &[Value]) -> Result<Value, RuntimeError> {
    compare_chain(args, CmpOp::Lt)
}

fn ne(args: &[Value]) -> Result<Value, RuntimeError> {
    require_arity(args, 2)?;
    let kind = common_kind(args)?;
    let left = coerce_to_common(&args[0], &kind)?;
    let right = coerce_to_common(&args[1], &kind)?;
    Ok(Value::Bool(compare_common(
        &left,
        &right,
        &kind,
        CmpOp::Ne,
    )?))
}

fn compare_chain(args: &[Value], op: CmpOp) -> Result<Value, RuntimeError> {
    require_min(args, 2)?;
    let kind = common_kind(args)?;
    let mut previous = coerce_to_common(&args[0], &kind)?;
    for value in &args[1..] {
        let current = coerce_to_common(value, &kind)?;
        let matched = compare_common(&previous, &current, &kind, op)?;
        if !matched {
            return Ok(Value::Bool(false));
        }
        previous = current;
    }
    Ok(Value::Bool(true))
}
