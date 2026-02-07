//! Numeric and arithmetic standard functions.

#![allow(missing_docs)]

use crate::error::RuntimeError;
use crate::eval::ops::{apply_binary, BinaryOp};
use crate::stdlib::helpers::{
    require_arity, require_min, scale_time, signed_from_i128, to_f64, to_i64, to_u64,
    unsigned_from_u128, wider_numeric, NumericKind,
};
use crate::stdlib::StandardLibrary;
use crate::value::{DateTimeProfile, Value};

pub fn register(lib: &mut StandardLibrary) {
    lib.register("ABS", &["IN"], abs);
    lib.register("SQRT", &["IN"], sqrt);
    lib.register("LN", &["IN"], ln);
    lib.register("LOG", &["IN"], log10);
    lib.register("EXP", &["IN"], exp);
    lib.register("SIN", &["IN"], sin);
    lib.register("COS", &["IN"], cos);
    lib.register("TAN", &["IN"], tan);
    lib.register("ASIN", &["IN"], asin);
    lib.register("ACOS", &["IN"], acos);
    lib.register("ATAN", &["IN"], atan);
    lib.register("ATAN2", &["Y", "X"], atan2);

    lib.register_variadic("ADD", "IN", 1, 2, add);
    lib.register("SUB", &["IN1", "IN2"], sub);
    lib.register_variadic("MUL", "IN", 1, 2, mul);
    lib.register("DIV", &["IN1", "IN2"], div);
    lib.register("MOD", &["IN1", "IN2"], modulo);
    lib.register("EXPT", &["IN1", "IN2"], expt);
    lib.register("MOVE", &["IN"], mov);
}

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
        Value::Real(v) => Ok(Value::Real(v.abs())),
        Value::LReal(v) => Ok(Value::LReal(v.abs())),
        _ => Err(RuntimeError::TypeMismatch),
    }
}

fn sqrt(args: &[Value]) -> Result<Value, RuntimeError> {
    unary_real(args, |v| v.sqrt())
}

fn ln(args: &[Value]) -> Result<Value, RuntimeError> {
    unary_real(args, |v| v.ln())
}

fn log10(args: &[Value]) -> Result<Value, RuntimeError> {
    unary_real(args, |v| v.log10())
}

fn exp(args: &[Value]) -> Result<Value, RuntimeError> {
    unary_real(args, |v| v.exp())
}

fn sin(args: &[Value]) -> Result<Value, RuntimeError> {
    unary_real(args, |v| v.sin())
}

fn cos(args: &[Value]) -> Result<Value, RuntimeError> {
    unary_real(args, |v| v.cos())
}

fn tan(args: &[Value]) -> Result<Value, RuntimeError> {
    unary_real(args, |v| v.tan())
}

fn asin(args: &[Value]) -> Result<Value, RuntimeError> {
    unary_real(args, |v| v.asin())
}

fn acos(args: &[Value]) -> Result<Value, RuntimeError> {
    unary_real(args, |v| v.acos())
}

fn atan(args: &[Value]) -> Result<Value, RuntimeError> {
    unary_real(args, |v| v.atan())
}

fn atan2(args: &[Value]) -> Result<Value, RuntimeError> {
    require_arity(args, 2)?;
    match (&args[0], &args[1]) {
        (Value::Real(a), Value::Real(b)) => Ok(Value::Real(a.atan2(*b))),
        (Value::LReal(a), Value::LReal(b)) => Ok(Value::LReal(a.atan2(*b))),
        (Value::Real(a), Value::LReal(b)) => Ok(Value::LReal((*a as f64).atan2(*b))),
        (Value::LReal(a), Value::Real(b)) => Ok(Value::LReal(a.atan2(*b as f64))),
        _ => Err(RuntimeError::TypeMismatch),
    }
}

fn unary_real(args: &[Value], f: impl Fn(f64) -> f64) -> Result<Value, RuntimeError> {
    require_arity(args, 1)?;
    match args[0] {
        Value::Real(v) => {
            let result = f(v as f64);
            if !result.is_finite() {
                return Err(RuntimeError::Overflow);
            }
            Ok(Value::Real(result as f32))
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
            let result = (*v as f64).powf(exp);
            if !result.is_finite() {
                return Err(RuntimeError::Overflow);
            }
            Ok(Value::Real(result as f32))
        }
        Value::LReal(v) => {
            let result = v.powf(exp);
            if !result.is_finite() {
                return Err(RuntimeError::Overflow);
            }
            Ok(Value::LReal(result))
        }
        _ => Err(RuntimeError::TypeMismatch),
    }
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

// Shared coercion helper retained for upcoming numeric stdlib harmonization work.
#[allow(dead_code)]
fn coerce_numeric(value: &Value, target: NumericKind) -> Result<Value, RuntimeError> {
    match target {
        NumericKind::Real => Ok(Value::Real(to_f64(value)? as f32)),
        NumericKind::LReal => Ok(Value::LReal(to_f64(value)?)),
        NumericKind::SInt | NumericKind::Int | NumericKind::DInt | NumericKind::LInt => {
            let value = i128::from(to_i64(value)?);
            signed_from_i128(target, value)
        }
        NumericKind::USInt | NumericKind::UInt | NumericKind::UDInt | NumericKind::ULInt => {
            let value = u128::from(to_u64(value)?);
            unsigned_from_u128(target, value)
        }
    }
}

// Shared promotion helper retained for upcoming numeric stdlib harmonization work.
#[allow(dead_code)]
fn common_numeric_kind(values: &[Value]) -> Result<NumericKind, RuntimeError> {
    let mut common = None;
    for value in values {
        let kind = crate::stdlib::helpers::numeric_kind(value).ok_or(RuntimeError::TypeMismatch)?;
        common = Some(match common {
            None => kind,
            Some(existing) => wider_numeric(existing, kind),
        });
    }
    common.ok_or(RuntimeError::TypeMismatch)
}
