use super::ConversionType;
use crate::error::RuntimeError;
use crate::stdlib::helpers::round_ties_to_even;
use crate::value::Value;

use super::bitstring::bit_string_to_int;
use super::string::{parse_int_text, parse_real_text, string_input};
use super::{ConversionMode, ConversionProfile};

pub(super) fn convert_to_int(
    value: &Value,
    dst: ConversionType,
    mode: ConversionMode,
    profile: ConversionProfile,
) -> Result<Value, RuntimeError> {
    if profile == ConversionProfile::Codesys {
        // CODESYS keeps the low-order bits of the target width
        // (docs/specs/07-standard-functions.md 2.10)
        let int = match value {
            Value::SInt(v) => Some(i128::from(*v)),
            Value::Int(v) => Some(i128::from(*v)),
            Value::DInt(v) => Some(i128::from(*v)),
            Value::LInt(v) => Some(i128::from(*v)),
            Value::USInt(v) => Some(i128::from(*v)),
            Value::UInt(v) => Some(i128::from(*v)),
            Value::UDInt(v) => Some(i128::from(*v)),
            Value::ULInt(v) => Some(i128::from(*v)),
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
    dst: ConversionType,
    profile: ConversionProfile,
) -> Result<Value, RuntimeError> {
    let codesys = profile == ConversionProfile::Codesys;
    match value {
        // CODESYS converts the number of a bit string (docs/specs/07-standard-functions.md 2.10)
        Value::Byte(v) if codesys => finite_real_result(f64::from(*v), dst),
        Value::Word(v) if codesys => finite_real_result(f64::from(*v), dst),
        Value::DWord(v) if codesys => finite_real_result(f64::from(*v), dst),
        Value::LWord(v) if codesys => finite_real_result(*v as f64, dst),
        Value::DWord(v) if dst == ConversionType::Real => {
            finite_real_result(f32::from_bits(*v) as f64, dst)
        }
        Value::LWord(v) if dst == ConversionType::LReal => {
            finite_real_result(f64::from_bits(*v), dst)
        }
        Value::Real(v) => finite_real_result(*v as f64, dst),
        Value::LReal(v) => finite_real_result(*v, dst),
        // vendor extension (docs/specs/07-standard-functions.md 2.9): BOOL_TO_REAL / BOOL_TO_LREAL
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

pub(super) fn finite_real_result(value: f64, dst: ConversionType) -> Result<Value, RuntimeError> {
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
    profile: ConversionProfile,
) -> Result<Value, RuntimeError> {
    if !value.is_finite() {
        return Err(RuntimeError::Overflow);
    }
    let rounded = match (mode, profile) {
        // CODESYS: .1-.4 down, .5-.9 up, i.e. half away from zero (REAL_TO_INT(-1.5) = -2)
        (ConversionMode::Round, ConversionProfile::Codesys) => crate::numeric::math::round(value),
        (ConversionMode::Round, ConversionProfile::Iec) => round_ties_to_even(value),
        (ConversionMode::Trunc, _) => crate::numeric::math::trunc(value),
    };
    if rounded < i128::MIN as f64 || rounded > i128::MAX as f64 {
        return Err(RuntimeError::Overflow);
    }
    if profile == ConversionProfile::Codesys {
        return wrap_int(rounded as i128, dst);
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

/// The integer of type `dst` with the low-order bits of `value` (two's complement), as
/// CODESYS converts out-of-range values: the high-order bytes are dropped.
pub(super) fn wrap_int(value: i128, dst: ConversionType) -> Result<Value, RuntimeError> {
    let (bits, signed) = match dst {
        ConversionType::SInt => (8, true),
        ConversionType::Int => (16, true),
        ConversionType::DInt => (32, true),
        ConversionType::LInt => (64, true),
        ConversionType::USInt => (8, false),
        ConversionType::UInt => (16, false),
        ConversionType::UDInt => (32, false),
        ConversionType::ULInt => (64, false),
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

/// `<source>_TO_BOOL`. A bit string transfers its rightmost bit (IEC 61131-3 Ed.3 Table 24
/// rows 4, 8, 12 and 16); the CODESYS profile tests it for zero instead
/// (docs/specs/07-standard-functions.md 2.10). For a number, a truST extension (2.9): TRUE
/// when the value is not zero; a non-finite REAL/LREAL is an overflow like in the other
/// conversions from REAL.
pub(super) fn convert_to_bool(
    value: &Value,
    profile: ConversionProfile,
) -> Result<Value, RuntimeError> {
    let bits = match value {
        Value::Byte(v) => Some(u64::from(*v)),
        Value::Word(v) => Some(u64::from(*v)),
        Value::DWord(v) => Some(u64::from(*v)),
        Value::LWord(v) => Some(*v),
        _ => None,
    };
    if let Some(bits) = bits {
        let mask = match profile {
            ConversionProfile::Iec => 1,
            ConversionProfile::Codesys => u64::MAX,
        };
        return Ok(Value::Bool(bits & mask != 0));
    }
    let result = match value {
        Value::Bool(v) => *v,
        Value::SInt(v) => *v != 0,
        Value::Int(v) => *v != 0,
        Value::DInt(v) => *v != 0,
        Value::LInt(v) => *v != 0,
        Value::USInt(v) => *v != 0,
        Value::UInt(v) => *v != 0,
        Value::UDInt(v) => *v != 0,
        Value::ULInt(v) => *v != 0,
        Value::Real(v) if v.is_finite() => *v != 0.0,
        Value::LReal(v) if v.is_finite() => *v != 0.0,
        Value::Real(_) | Value::LReal(_) => return Err(RuntimeError::Overflow),
        _ => return Err(RuntimeError::TypeMismatch),
    };
    Ok(Value::Bool(result))
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
                            real_to_int(value, dst, mode, ConversionProfile::Iec),
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
