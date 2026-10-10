use super::ConversionType;
use crate::error::RuntimeError;
use crate::stdlib::helpers::round_ties_to_even;
use crate::value::Value;

use super::bitstring::bit_string_to_int;
use super::string::{parse_int_text, parse_real_text, string_input};
use super::ConversionMode;

pub(super) fn convert_to_int(
    value: &Value,
    dst: ConversionType,
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

pub(super) fn convert_to_real(value: &Value, dst: ConversionType) -> Result<Value, RuntimeError> {
    match value {
        Value::DWord(v) if dst == ConversionType::Real => {
            finite_real_result(f32::from_bits(*v) as f64, dst)
        }
        Value::LWord(v) if dst == ConversionType::LReal => {
            finite_real_result(f64::from_bits(*v), dst)
        }
        Value::Real(v) => finite_real_result(*v as f64, dst),
        Value::LReal(v) => finite_real_result(*v, dst),
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

fn finite_real_result(value: f64, dst: ConversionType) -> Result<Value, RuntimeError> {
    let result = match dst {
        ConversionType::Real => Value::Real(value as f32),
        ConversionType::LReal => Value::LReal(value),
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
    dst: ConversionType,
    mode: ConversionMode,
) -> Result<Value, RuntimeError> {
    if !value.is_finite() {
        return Err(RuntimeError::Overflow);
    }
    let rounded = match mode {
        ConversionMode::Round => round_ties_to_even(value),
        ConversionMode::Trunc => crate::numeric::math::trunc(value),
    };
    if rounded < i128::MIN as f64 || rounded > i128::MAX as f64 {
        return Err(RuntimeError::Overflow);
    }
    // The preceding wide bound preserves the old error precedence even for
    // unsupported destinations. Actual IEC integer results require at most 64 bits.
    if super::util::is_signed_int_type(dst) {
        if !(-9_223_372_036_854_775_808.0..9_223_372_036_854_775_808.0).contains(&rounded) {
            return Err(RuntimeError::Overflow);
        }
        signed_int_from_i64(rounded as i64, dst)
    } else if super::util::is_unsigned_int_type(dst) {
        if !(0.0..18_446_744_073_709_551_616.0).contains(&rounded) {
            return Err(RuntimeError::Overflow);
        }
        unsigned_int_from_u64(rounded as u64, dst)
    } else {
        Err(RuntimeError::TypeMismatch)
    }
}

pub(super) fn signed_int_from_i64(value: i64, dst: ConversionType) -> Result<Value, RuntimeError> {
    match dst {
        ConversionType::SInt => i8::try_from(value)
            .map(Value::SInt)
            .map_err(|_| RuntimeError::Overflow),
        ConversionType::Int => i16::try_from(value)
            .map(Value::Int)
            .map_err(|_| RuntimeError::Overflow),
        ConversionType::DInt => i32::try_from(value)
            .map(Value::DInt)
            .map_err(|_| RuntimeError::Overflow),
        ConversionType::LInt => Ok(Value::LInt(value)),
        ConversionType::USInt
        | ConversionType::UInt
        | ConversionType::UDInt
        | ConversionType::ULInt => {
            let value = u64::try_from(value).map_err(|_| RuntimeError::Overflow)?;
            unsigned_int_from_u64(value, dst)
        }
        _ => Err(RuntimeError::TypeMismatch),
    }
}

pub(super) fn signed_int_from_i128(
    value: i128,
    dst: ConversionType,
) -> Result<Value, RuntimeError> {
    match dst {
        ConversionType::SInt => i8::try_from(value)
            .map(Value::SInt)
            .map_err(|_| RuntimeError::Overflow),
        ConversionType::Int => i16::try_from(value)
            .map(Value::Int)
            .map_err(|_| RuntimeError::Overflow),
        ConversionType::DInt => i32::try_from(value)
            .map(Value::DInt)
            .map_err(|_| RuntimeError::Overflow),
        ConversionType::LInt => i64::try_from(value)
            .map(Value::LInt)
            .map_err(|_| RuntimeError::Overflow),
        ConversionType::USInt
        | ConversionType::UInt
        | ConversionType::UDInt
        | ConversionType::ULInt => {
            if value < 0 || value > i128::from(u64::MAX) {
                return Err(RuntimeError::Overflow);
            }
            unsigned_int_from_u64(value as u64, dst)
        }
        _ => Err(RuntimeError::TypeMismatch),
    }
}

pub(super) fn unsigned_int_from_u64(
    value: u64,
    dst: ConversionType,
) -> Result<Value, RuntimeError> {
    match dst {
        ConversionType::USInt => u8::try_from(value)
            .map(Value::USInt)
            .map_err(|_| RuntimeError::Overflow),
        ConversionType::UInt => u16::try_from(value)
            .map(Value::UInt)
            .map_err(|_| RuntimeError::Overflow),
        ConversionType::UDInt => u32::try_from(value)
            .map(Value::UDInt)
            .map_err(|_| RuntimeError::Overflow),
        ConversionType::ULInt => Ok(Value::ULInt(value)),
        ConversionType::SInt
        | ConversionType::Int
        | ConversionType::DInt
        | ConversionType::LInt => {
            if value > i64::MAX as u64 {
                return Err(RuntimeError::Overflow);
            }
            signed_int_from_i64(value as i64, dst)
        }
        _ => Err(RuntimeError::TypeMismatch),
    }
}

#[cfg(test)]
mod width_tests {
    use super::*;

    fn wide_reference(
        value: f64,
        dst: ConversionType,
        mode: ConversionMode,
    ) -> Result<Value, RuntimeError> {
        if !value.is_finite() {
            return Err(RuntimeError::Overflow);
        }
        let rounded = match mode {
            ConversionMode::Round => round_ties_to_even(value),
            ConversionMode::Trunc => crate::numeric::math::trunc(value),
        };
        if rounded < i128::MIN as f64 || rounded > i128::MAX as f64 {
            return Err(RuntimeError::Overflow);
        }
        signed_int_from_i128(rounded as i128, dst)
    }

    #[test]
    fn narrowed_float_conversion_matches_wide_reference_at_all_integer_boundaries() {
        let types = [
            ConversionType::SInt,
            ConversionType::Int,
            ConversionType::DInt,
            ConversionType::LInt,
            ConversionType::USInt,
            ConversionType::UInt,
            ConversionType::UDInt,
            ConversionType::ULInt,
            ConversionType::Bool,
        ];
        let points = [
            f64::NAN,
            f64::INFINITY,
            f64::NEG_INFINITY,
            -0.0,
            0.0,
            -0.5,
            0.5,
            -1.5,
            1.5,
            2.5,
            i8::MIN as f64,
            i8::MAX as f64,
            u8::MAX as f64,
            i16::MIN as f64,
            i16::MAX as f64,
            u16::MAX as f64,
            i32::MIN as f64,
            i32::MAX as f64,
            u32::MAX as f64,
            i64::MIN as f64,
            i64::MAX as f64,
            u64::MAX as f64,
            i128::MIN as f64,
            i128::MAX as f64,
        ];
        for point in points {
            for value in [point.next_down(), point, point.next_up()] {
                for dst in types {
                    for mode in [ConversionMode::Round, ConversionMode::Trunc] {
                        assert_eq!(
                            real_to_int(value, dst, mode),
                            wide_reference(value, dst, mode),
                            "value={value:?} target={dst:?} mode={mode:?}"
                        );
                    }
                }
            }
        }
        for value in [i64::MIN, -1, 0, 1, i64::MAX] {
            for dst in types {
                assert_eq!(
                    signed_int_from_i64(value, dst),
                    signed_int_from_i128(i128::from(value), dst)
                );
            }
        }
        assert_eq!(
            unsigned_int_from_u64(u64::MAX, ConversionType::ULInt),
            Ok(Value::ULInt(u64::MAX))
        );
    }
}
