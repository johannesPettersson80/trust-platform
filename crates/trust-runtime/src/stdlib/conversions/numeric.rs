use crate::error::RuntimeError;
use crate::stdlib::helpers::round_ties_to_even;
use crate::value::Value;
use trust_hir::TypeId;

use super::bitstring::bit_string_to_int;
use super::string::{parse_int_text, parse_real_text, string_input};
use super::{ConversionMode, ConversionProfile};

pub(super) fn convert_to_int(
    value: &Value,
    dst: TypeId,
    mode: ConversionMode,
    profile: ConversionProfile,
) -> Result<Value, RuntimeError> {
    if profile == ConversionProfile::Codesys {
        // CODESYS keeps the low-order bits of the target width (section 2.7)
        let int = match value {
            Value::SInt(v) => Some(*v as i128),
            Value::Int(v) => Some(*v as i128),
            Value::DInt(v) => Some(*v as i128),
            Value::LInt(v) => Some(*v as i128),
            Value::USInt(v) => Some(*v as i128),
            Value::UInt(v) => Some(*v as i128),
            Value::UDInt(v) => Some(*v as i128),
            Value::ULInt(v) => Some(*v as i128),
            _ => None,
        };
        if let Some(int) = int {
            return wrap_int(int, dst);
        }
    }
    match value {
        Value::Real(v) => real_to_int(*v as f64, dst, mode, profile),
        Value::LReal(v) => real_to_int(*v, dst, mode, profile),
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

pub(super) fn convert_to_real(
    value: &Value,
    dst: TypeId,
    profile: ConversionProfile,
) -> Result<Value, RuntimeError> {
    match value {
        // CODESYS converts the number of a bit string (section 2.7)
        Value::Byte(v) if profile == ConversionProfile::Codesys => {
            finite_real_result(*v as f64, dst)
        }
        Value::Word(v) if profile == ConversionProfile::Codesys => {
            finite_real_result(*v as f64, dst)
        }
        Value::DWord(v) if profile == ConversionProfile::Codesys => {
            finite_real_result(*v as f64, dst)
        }
        Value::LWord(v) if profile == ConversionProfile::Codesys => {
            finite_real_result(*v as f64, dst)
        }
        Value::DWord(v) if dst == TypeId::REAL => {
            finite_real_result(f32::from_bits(*v) as f64, dst)
        }
        Value::LWord(v) if dst == TypeId::LREAL => finite_real_result(f64::from_bits(*v), dst),
        Value::Real(v) => finite_real_result(*v as f64, dst),
        Value::LReal(v) => finite_real_result(*v, dst),
        // vendor extension: BOOL_TO_REAL / BOOL_TO_LREAL
        Value::Bool(v) => finite_real_result(if *v { 1.0 } else { 0.0 }, dst),
        Value::SInt(v) => finite_real_result(*v as f64, dst),
        Value::Int(v) => finite_real_result(*v as f64, dst),
        Value::DInt(v) => finite_real_result(*v as f64, dst),
        Value::LInt(v) => finite_real_result(*v as f64, dst),
        Value::USInt(v) => finite_real_result(*v as f64, dst),
        Value::UInt(v) => finite_real_result(*v as f64, dst),
        Value::UDInt(v) => finite_real_result(*v as f64, dst),
        Value::ULInt(v) => finite_real_result(*v as f64, dst),
        Value::String(_) | Value::WString(_) => {
            let text = string_input(value)?;
            let parsed = parse_real_text(text)?;
            finite_real_result(parsed, dst)
        }
        _ => Err(RuntimeError::TypeMismatch),
    }
}

fn finite_real_result(value: f64, dst: TypeId) -> Result<Value, RuntimeError> {
    let result = match dst {
        TypeId::REAL => Value::Real(value as f32),
        TypeId::LREAL => Value::LReal(value),
        _ => return Err(RuntimeError::TypeMismatch),
    };
    match result {
        Value::Real(value) if value.is_finite() => Ok(Value::Real(value)),
        Value::LReal(value) if value.is_finite() => Ok(Value::LReal(value)),
        Value::Real(_) | Value::LReal(_) => Err(RuntimeError::Overflow),
        _ => unreachable!("finite_real_result only constructs REAL or LREAL"),
    }
}

pub(super) fn real_to_int(
    value: f64,
    dst: TypeId,
    mode: ConversionMode,
    profile: ConversionProfile,
) -> Result<Value, RuntimeError> {
    if !value.is_finite() {
        return Err(RuntimeError::Overflow);
    }
    let rounded = match (mode, profile) {
        // CODESYS: .1-.4 down, .5-.9 up, i.e. half away from zero (REAL_TO_INT(-1.5) = -2)
        (ConversionMode::Round, ConversionProfile::Codesys) => value.round(),
        (ConversionMode::Round, ConversionProfile::Iec) => round_ties_to_even(value),
        (ConversionMode::Trunc, _) => value.trunc(),
    };
    if rounded < i128::MIN as f64 || rounded > i128::MAX as f64 {
        return Err(RuntimeError::Overflow);
    }
    let int = rounded as i128;
    match profile {
        ConversionProfile::Codesys => wrap_int(int, dst),
        ConversionProfile::Iec => signed_int_from_i128(int, dst),
    }
}

/// The integer of type `dst` with the low-order bits of `value` (two's complement), as
/// CODESYS converts out-of-range values: the high-order bytes are dropped.
pub(super) fn wrap_int(value: i128, dst: TypeId) -> Result<Value, RuntimeError> {
    let (bits, signed) = match dst {
        TypeId::SINT => (8, true),
        TypeId::INT => (16, true),
        TypeId::DINT => (32, true),
        TypeId::LINT => (64, true),
        TypeId::USINT => (8, false),
        TypeId::UINT => (16, false),
        TypeId::UDINT => (32, false),
        TypeId::ULINT => (64, false),
        _ => return Err(RuntimeError::TypeMismatch),
    };
    let low = (value as u128) & ((1u128 << bits) - 1);
    if signed {
        let shift = 128 - bits;
        signed_int_from_i128(((low << shift) as i128) >> shift, dst)
    } else {
        signed_int_from_i128(low as i128, dst)
    }
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
            if value < 0 || value > i128::from(u64::MAX) {
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

/// Vendor extension `<number or bit string>_TO_BOOL`: TRUE when the value is not zero; a
/// non-finite REAL/LREAL is an overflow like in the other conversions from REAL.
pub(super) fn convert_to_bool(value: &Value) -> Result<Value, RuntimeError> {
    let nonzero = match value {
        Value::Bool(v) => *v,
        Value::SInt(v) => *v != 0,
        Value::Int(v) => *v != 0,
        Value::DInt(v) => *v != 0,
        Value::LInt(v) => *v != 0,
        Value::USInt(v) => *v != 0,
        Value::UInt(v) => *v != 0,
        Value::UDInt(v) => *v != 0,
        Value::ULInt(v) => *v != 0,
        Value::Byte(v) => *v != 0,
        Value::Word(v) => *v != 0,
        Value::DWord(v) => *v != 0,
        Value::LWord(v) => *v != 0,
        Value::Real(v) if v.is_finite() => *v != 0.0,
        Value::LReal(v) if v.is_finite() => *v != 0.0,
        Value::Real(_) | Value::LReal(_) => return Err(RuntimeError::Overflow),
        _ => return Err(RuntimeError::TypeMismatch),
    };
    Ok(Value::Bool(nonzero))
}
