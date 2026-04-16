use crate::error::RuntimeError;
use crate::stdlib::helpers::round_ties_to_even;
use crate::value::Value;
use trust_hir::TypeId;

use super::bitstring::bit_string_to_int;
use super::string::{parse_int_text, parse_real_text, string_input};
use super::ConversionMode;

pub(super) fn convert_to_int(
    value: &Value,
    dst: TypeId,
    mode: ConversionMode,
) -> Result<Value, RuntimeError> {
    match value {
        Value::Real(v) => real_to_int(*v as f64, dst, mode),
        Value::LReal(v) => real_to_int(*v, dst, mode),
        Value::Bool(v) => {
            let val = if *v { 1 } else { 0 };
            signed_int_from_i64(val, dst)
        }
        Value::SInt(v) => signed_int_from_i64(*v as i64, dst),
        Value::Int(v) => signed_int_from_i64(*v as i64, dst),
        Value::DInt(v) => signed_int_from_i64(*v as i64, dst),
        Value::LInt(v) => signed_int_from_i64(*v, dst),
        Value::USInt(v) => unsigned_int_from_u64(*v as u64, dst),
        Value::UInt(v) => unsigned_int_from_u64(*v as u64, dst),
        Value::UDInt(v) => unsigned_int_from_u64(*v as u64, dst),
        Value::ULInt(v) => unsigned_int_from_u64(*v, dst),
        Value::Char(v) => unsigned_int_from_u64(*v as u64, dst),
        Value::WChar(v) => unsigned_int_from_u64(*v as u64, dst),
        Value::Byte(_) | Value::Word(_) | Value::DWord(_) | Value::LWord(_) => {
            bit_string_to_int(value, dst)
        }
        Value::String(_) | Value::WString(_) => {
            let text = string_input(value)?;
            let parsed = parse_int_text(text)?;
            signed_int_from_i128(parsed, dst)
        }
        _ => Err(RuntimeError::TypeMismatch),
    }
}

pub(super) fn convert_to_real(value: &Value, dst: TypeId) -> Result<Value, RuntimeError> {
    match value {
        Value::DWord(v) if dst == TypeId::REAL => Ok(Value::Real(f32::from_bits(*v))),
        Value::LWord(v) if dst == TypeId::LREAL => Ok(Value::LReal(f64::from_bits(*v))),
        Value::Real(v) => match dst {
            TypeId::REAL => Ok(Value::Real(*v)),
            TypeId::LREAL => Ok(Value::LReal(*v as f64)),
            _ => Err(RuntimeError::TypeMismatch),
        },
        Value::LReal(v) => match dst {
            TypeId::REAL => Ok(Value::Real(*v as f32)),
            TypeId::LREAL => Ok(Value::LReal(*v)),
            _ => Err(RuntimeError::TypeMismatch),
        },
        Value::SInt(v) => real_from_int(*v as f64, dst),
        Value::Int(v) => real_from_int(*v as f64, dst),
        Value::DInt(v) => real_from_int(*v as f64, dst),
        Value::LInt(v) => real_from_int(*v as f64, dst),
        Value::USInt(v) => real_from_int(*v as f64, dst),
        Value::UInt(v) => real_from_int(*v as f64, dst),
        Value::UDInt(v) => real_from_int(*v as f64, dst),
        Value::ULInt(v) => real_from_int(*v as f64, dst),
        Value::String(_) | Value::WString(_) => {
            let text = string_input(value)?;
            let parsed = parse_real_text(text)?;
            real_from_int(parsed, dst)
        }
        _ => Err(RuntimeError::TypeMismatch),
    }
}

fn real_from_int(value: f64, dst: TypeId) -> Result<Value, RuntimeError> {
    match dst {
        TypeId::REAL => Ok(Value::Real(value as f32)),
        TypeId::LREAL => Ok(Value::LReal(value)),
        _ => Err(RuntimeError::TypeMismatch),
    }
}

pub(super) fn real_to_int(
    value: f64,
    dst: TypeId,
    mode: ConversionMode,
) -> Result<Value, RuntimeError> {
    if !value.is_finite() {
        return Err(RuntimeError::Overflow);
    }
    let rounded = match mode {
        ConversionMode::Round => round_ties_to_even(value),
        ConversionMode::Trunc => value.trunc(),
    };
    if rounded < i128::MIN as f64 || rounded > i128::MAX as f64 {
        return Err(RuntimeError::Overflow);
    }
    let int = rounded as i128;
    signed_int_from_i128(int, dst)
}

pub(super) fn signed_int_from_i64(value: i64, dst: TypeId) -> Result<Value, RuntimeError> {
    signed_int_from_i128(value as i128, dst)
}

pub(super) fn signed_int_from_i128(value: i128, dst: TypeId) -> Result<Value, RuntimeError> {
    match dst {
        TypeId::SINT => i8::try_from(value)
            .map(Value::SInt)
            .map_err(|_| RuntimeError::Overflow),
        TypeId::INT => i16::try_from(value)
            .map(Value::Int)
            .map_err(|_| RuntimeError::Overflow),
        TypeId::DINT => i32::try_from(value)
            .map(Value::DInt)
            .map_err(|_| RuntimeError::Overflow),
        TypeId::LINT => i64::try_from(value)
            .map(Value::LInt)
            .map_err(|_| RuntimeError::Overflow),
        TypeId::USINT | TypeId::UINT | TypeId::UDINT | TypeId::ULINT => {
            if value < 0 {
                return Err(RuntimeError::Overflow);
            }
            unsigned_int_from_u64(value as u64, dst)
        }
        _ => Err(RuntimeError::TypeMismatch),
    }
}

pub(super) fn unsigned_int_from_u64(value: u64, dst: TypeId) -> Result<Value, RuntimeError> {
    match dst {
        TypeId::USINT => u8::try_from(value)
            .map(Value::USInt)
            .map_err(|_| RuntimeError::Overflow),
        TypeId::UINT => u16::try_from(value)
            .map(Value::UInt)
            .map_err(|_| RuntimeError::Overflow),
        TypeId::UDINT => u32::try_from(value)
            .map(Value::UDInt)
            .map_err(|_| RuntimeError::Overflow),
        TypeId::ULINT => Ok(Value::ULInt(value)),
        TypeId::SINT | TypeId::INT | TypeId::DINT | TypeId::LINT => {
            if value > i64::MAX as u64 {
                return Err(RuntimeError::Overflow);
            }
            signed_int_from_i64(value as i64, dst)
        }
        _ => Err(RuntimeError::TypeMismatch),
    }
}
