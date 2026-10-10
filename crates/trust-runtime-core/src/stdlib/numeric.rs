//! Numeric and arithmetic standard functions.

use crate::error::RuntimeError;
use crate::numeric::math;
use crate::program_model::{apply_binary, BinaryOp};
use crate::stdlib::helpers::{require_arity, require_min, scale_time, to_f64};
#[cfg(feature = "hir")]
use crate::stdlib::StandardLibrary;
use crate::value::{DateTimeProfile, Value};

/// Register the shared numeric functions in a hosted registry.
#[cfg(feature = "hir")]
pub fn register(lib: &mut StandardLibrary) {
    lib.register_descriptors(FUNCTIONS);
}

pub(super) static FUNCTIONS: &[(&str, super::StdFunctionRef<'static>)] = &[
    super::registration::descriptor!("ABS", IN, abs),
    super::registration::descriptor!("ACOS", IN, acos),
    super::registration::descriptor!("ADD", VAR_IN_1_2, add),
    super::registration::descriptor!("ASIN", IN, asin),
    super::registration::descriptor!("ATAN", IN, atan),
    super::registration::descriptor!("ATAN2", Y_X, atan2),
    super::registration::descriptor!("COS", IN, cos),
    super::registration::descriptor!("DIV", IN1_IN2, div),
    super::registration::descriptor!("EXP", IN, exp),
    super::registration::descriptor!("EXPT", IN1_IN2, expt),
    super::registration::descriptor!("LN", IN, ln),
    super::registration::descriptor!("LOG", IN, log10),
    super::registration::descriptor!("MOD", IN1_IN2, modulo),
    super::registration::descriptor!("MOVE", IN, mov),
    super::registration::descriptor!("MUL", VAR_IN_1_2, mul),
    super::registration::descriptor!("SIN", IN, sin),
    super::registration::descriptor!("SQRT", IN, sqrt),
    super::registration::descriptor!("SUB", IN1_IN2, sub),
    super::registration::descriptor!("TAN", IN, tan),
];

fn abs(args: &[Value]) -> Result<Value, RuntimeError> {
    require_arity(args, 1)?;
    match args[0] {
        Value::SInt(v) => v
            .checked_abs()
            .map(Value::SInt)
            .ok_or(RuntimeError::Overflow),
        Value::Int(v) => v
            .checked_abs()
            .map(Value::Int)
            .ok_or(RuntimeError::Overflow),
        Value::DInt(v) => v
            .checked_abs()
            .map(Value::DInt)
            .ok_or(RuntimeError::Overflow),
        Value::LInt(v) => v
            .checked_abs()
            .map(Value::LInt)
            .ok_or(RuntimeError::Overflow),
        Value::USInt(v) => Ok(Value::USInt(v)),
        Value::UInt(v) => Ok(Value::UInt(v)),
        Value::UDInt(v) => Ok(Value::UDInt(v)),
        Value::ULInt(v) => Ok(Value::ULInt(v)),
        Value::Real(v) => Ok(Value::Real(math::fabsf(v))),
        Value::LReal(v) => Ok(Value::LReal(math::fabs(v))),
        _ => Err(RuntimeError::TypeMismatch),
    }
}

fn sqrt(args: &[Value]) -> Result<Value, RuntimeError> {
    unary_real(args, math::sqrt)
}

fn ln(args: &[Value]) -> Result<Value, RuntimeError> {
    unary_real(args, math::log)
}

fn log10(args: &[Value]) -> Result<Value, RuntimeError> {
    unary_real(args, math::log10)
}

fn exp(args: &[Value]) -> Result<Value, RuntimeError> {
    unary_real(args, math::exp)
}

fn sin(args: &[Value]) -> Result<Value, RuntimeError> {
    unary_real(args, math::sin)
}

fn cos(args: &[Value]) -> Result<Value, RuntimeError> {
    unary_real(args, math::cos)
}

fn tan(args: &[Value]) -> Result<Value, RuntimeError> {
    unary_real(args, math::tan)
}

fn asin(args: &[Value]) -> Result<Value, RuntimeError> {
    unary_real(args, math::asin)
}

fn acos(args: &[Value]) -> Result<Value, RuntimeError> {
    unary_real(args, math::acos)
}

fn atan(args: &[Value]) -> Result<Value, RuntimeError> {
    unary_real(args, math::atan)
}

fn atan2(args: &[Value]) -> Result<Value, RuntimeError> {
    require_arity(args, 2)?;
    match (&args[0], &args[1]) {
        (Value::Real(a), Value::Real(b)) => Ok(Value::Real(math::atan2f(*a, *b))),
        (Value::LReal(a), Value::LReal(b)) => Ok(Value::LReal(math::atan2(*a, *b))),
        (Value::Real(a), Value::LReal(b)) => Ok(Value::LReal(math::atan2(f64::from(*a), *b))),
        (Value::LReal(a), Value::Real(b)) => Ok(Value::LReal(math::atan2(*a, f64::from(*b)))),
        _ => Err(RuntimeError::TypeMismatch),
    }
}

fn unary_real(args: &[Value], f: impl Fn(f64) -> f64) -> Result<Value, RuntimeError> {
    require_arity(args, 1)?;
    match args[0] {
        Value::Real(v) => {
            let result = f(v as f64);
            checked_real_result(result)
        }
        Value::LReal(v) => {
            let result = f(v);
            if !result.is_finite() {
                return Err(RuntimeError::Overflow);
            }
            Ok(Value::LReal(result))
        }
        _ => Err(RuntimeError::TypeMismatch),
    }
}

fn add(args: &[Value]) -> Result<Value, RuntimeError> {
    require_min(args, 2)?;
    let profile = DateTimeProfile::default();
    if args.iter().any(is_time_related) {
        require_arity(args, 2)?;
        return apply_binary(BinaryOp::Add, args[0].clone(), args[1].clone(), &profile);
    }
    fold_binary(BinaryOp::Add, args, &profile)
}

fn sub(args: &[Value]) -> Result<Value, RuntimeError> {
    require_arity(args, 2)?;
    let profile = DateTimeProfile::default();
    apply_binary(BinaryOp::Sub, args[0].clone(), args[1].clone(), &profile)
}

fn mul(args: &[Value]) -> Result<Value, RuntimeError> {
    require_min(args, 2)?;
    if args.iter().any(is_time_duration) {
        require_arity(args, 2)?;
        return mul_time_duration(&args[0], &args[1]);
    }
    let profile = DateTimeProfile::default();
    fold_binary(BinaryOp::Mul, args, &profile)
}

fn div(args: &[Value]) -> Result<Value, RuntimeError> {
    require_arity(args, 2)?;
    if is_time_duration(&args[0]) {
        return div_time_duration(&args[0], &args[1]);
    }
    let profile = DateTimeProfile::default();
    apply_binary(BinaryOp::Div, args[0].clone(), args[1].clone(), &profile)
}

fn modulo(args: &[Value]) -> Result<Value, RuntimeError> {
    require_arity(args, 2)?;
    let profile = DateTimeProfile::default();
    apply_binary(BinaryOp::Mod, args[0].clone(), args[1].clone(), &profile)
}

fn expt(args: &[Value]) -> Result<Value, RuntimeError> {
    require_arity(args, 2)?;
    let base = &args[0];
    let exp = &args[1];
    let exp = to_f64(exp)?;
    match base {
        Value::Real(v) => {
            let result = math::pow(f64::from(*v), exp);
            checked_real_result(result)
        }
        Value::LReal(v) => {
            let result = math::pow(*v, exp);
            if !result.is_finite() {
                return Err(RuntimeError::Overflow);
            }
            Ok(Value::LReal(result))
        }
        _ => Err(RuntimeError::TypeMismatch),
    }
}

fn checked_real_result(result: f64) -> Result<Value, RuntimeError> {
    if !result.is_finite() {
        return Err(RuntimeError::Overflow);
    }
    let result = result as f32;
    if !result.is_finite() {
        return Err(RuntimeError::Overflow);
    }
    Ok(Value::Real(result))
}

fn mov(args: &[Value]) -> Result<Value, RuntimeError> {
    require_arity(args, 1)?;
    Ok(args[0].clone())
}

fn fold_binary(
    op: BinaryOp,
    args: &[Value],
    profile: &DateTimeProfile,
) -> Result<Value, RuntimeError> {
    let mut acc = args[0].clone();
    for value in &args[1..] {
        acc = apply_binary(op, acc, value.clone(), profile)?;
    }
    Ok(acc)
}

fn is_time_related(value: &Value) -> bool {
    matches!(
        value,
        Value::Time(_)
            | Value::LTime(_)
            | Value::Date(_)
            | Value::LDate(_)
            | Value::Tod(_)
            | Value::LTod(_)
            | Value::Dt(_)
            | Value::Ldt(_)
    )
}

fn is_time_duration(value: &Value) -> bool {
    matches!(value, Value::Time(_) | Value::LTime(_))
}

fn mul_time_duration(lhs: &Value, rhs: &Value) -> Result<Value, RuntimeError> {
    match (lhs, rhs) {
        (Value::Time(duration), other) => scale_time(*duration, other, true).map(Value::Time),
        (Value::LTime(duration), other) => scale_time(*duration, other, true).map(Value::LTime),
        (other, Value::Time(duration)) => scale_time(*duration, other, true).map(Value::Time),
        (other, Value::LTime(duration)) => scale_time(*duration, other, true).map(Value::LTime),
        _ => Err(RuntimeError::TypeMismatch),
    }
}

fn div_time_duration(lhs: &Value, rhs: &Value) -> Result<Value, RuntimeError> {
    match (lhs, rhs) {
        (Value::Time(duration), other) => scale_time(*duration, other, false).map(Value::Time),
        (Value::LTime(duration), other) => scale_time(*duration, other, false).map(Value::LTime),
        _ => Err(RuntimeError::TypeMismatch),
    }
}
