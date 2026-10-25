use super::ConversionType;
use crate::error::RuntimeError;
use crate::stdlib::helpers::require_arity;
use crate::value::Value;

use super::bcd::{from_bcd, to_bcd};
use super::bitstring::convert_to_bit_string;
use super::numeric::{convert_to_int, convert_to_real};
use super::spec::ConversionSpec;
use super::string::{convert_to_char, convert_to_string};
use super::time::{convert_to_date, convert_to_dt, convert_to_time, convert_to_tod};
use super::util::{is_conversion_allowed, is_integer_type, value_type_id};
use super::ConversionMode;

pub(super) fn apply_conversion(
    spec: ConversionSpec,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    require_arity(args, 1)?;
    let value = &args[0];
    match spec {
        ConversionSpec::Convert { src, dst } => {
            convert_with_mode(value, src, dst, ConversionMode::Round)
        }
        ConversionSpec::Trunc { src, dst } => trunc_convert(value, src, dst),
        ConversionSpec::ToBcd { src, dst } => to_bcd(value, src, dst),
        ConversionSpec::BcdTo { src, dst } => from_bcd(value, src, dst),
    }
}

fn convert_with_mode(
    value: &Value,
    src: Option<ConversionType>,
    dst: ConversionType,
    mode: ConversionMode,
) -> Result<Value, RuntimeError> {
    let (value, actual_src) = normalize_source_value(value, src, mode)?;
    if !is_conversion_allowed(actual_src, dst) {
        return Err(RuntimeError::TypeMismatch);
    }
    convert_value(&value, dst, mode)
}

fn trunc_convert(
    value: &Value,
    src: Option<ConversionType>,
    dst: ConversionType,
) -> Result<Value, RuntimeError> {
    let (value, actual_src) = normalize_source_value(value, src, ConversionMode::Trunc)?;
    if !matches!(actual_src, ConversionType::Real | ConversionType::LReal) {
        return Err(RuntimeError::TypeMismatch);
    }
    if !is_integer_type(dst) {
        return Err(RuntimeError::TypeMismatch);
    }
    convert_value(&value, dst, ConversionMode::Trunc)
}

fn normalize_source_value(
    value: &Value,
    src: Option<ConversionType>,
    mode: ConversionMode,
) -> Result<(Value, ConversionType), RuntimeError> {
    let actual_src = value_type_id(value).ok_or_else(|| RuntimeError::TypeMismatch)?;
    let Some(expected) = src else {
        return Ok((value.clone(), actual_src));
    };
    if actual_src == expected {
        return Ok((value.clone(), actual_src));
    }
    if !is_conversion_allowed(actual_src, expected) {
        return Err(RuntimeError::TypeMismatch);
    }
    // Runtime storage can widen scalar values (for example INT -> DINT after arithmetic).
    // Exact-source conversions should normalize back to the requested source type first.
    let coerced = convert_value(value, expected, mode)?;
    Ok((coerced, expected))
}

fn convert_value(
    value: &Value,
    dst: ConversionType,
    mode: ConversionMode,
) -> Result<Value, RuntimeError> {
    if let Some(src) = value_type_id(value)
        && src == dst
    {
        return Ok(value.clone());
    }

    match dst {
        ConversionType::SInt
        | ConversionType::Int
        | ConversionType::DInt
        | ConversionType::LInt
        | ConversionType::USInt
        | ConversionType::UInt
        | ConversionType::UDInt
        | ConversionType::ULInt => convert_to_int(value, dst, mode),
        ConversionType::Real | ConversionType::LReal => convert_to_real(value, dst),
        ConversionType::Byte
        | ConversionType::Word
        | ConversionType::DWord
        | ConversionType::LWord => convert_to_bit_string(value, dst),
        ConversionType::Time | ConversionType::LTime => convert_to_time(value, dst),
        ConversionType::Date | ConversionType::LDate => convert_to_date(value, dst),
        ConversionType::Tod | ConversionType::LTod => convert_to_tod(value, dst),
        ConversionType::Dt | ConversionType::Ldt => convert_to_dt(value, dst),
        ConversionType::String | ConversionType::WString => convert_to_string(value, dst),
        ConversionType::Char | ConversionType::WChar => convert_to_char(value, dst),
        ConversionType::Bool => Err(RuntimeError::TypeMismatch),
        _ => Err(RuntimeError::TypeMismatch),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stdlib::conversions::ConversionType::{DInt, Int, LInt, SInt, UDInt, USInt};

    /// Worked cases of specification 07 §2.1: a representable integer keeps its value exactly, and
    /// a value outside the target's range is `RuntimeError::Overflow`, never wrapped or saturated:
    /// UDINT_TO_DINT(70000) is 70000, INT_TO_DINT(-1234) is -1234, UDINT_TO_DINT(4294967295),
    /// DINT_TO_SINT(128) and LINT_TO_USINT(-1) overflow. Catches wrapping, saturation and a sign
    /// extended or dropped.
    #[test]
    fn worked_integer_conversions_of_specification_07() {
        let convert = |value: Value, src, dst| {
            apply_conversion(
                ConversionSpec::Convert {
                    src: Some(src),
                    dst,
                },
                core::slice::from_ref(&value),
            )
        };
        assert_eq!(
            convert(Value::UDInt(70_000), UDInt, DInt),
            Ok(Value::DInt(70_000))
        );
        assert_eq!(
            convert(Value::Int(-1234), Int, DInt),
            Ok(Value::DInt(-1234))
        );
        assert_eq!(
            convert(Value::UDInt(u32::MAX), UDInt, DInt),
            Err(RuntimeError::Overflow)
        );
        assert_eq!(
            convert(Value::DInt(128), DInt, SInt),
            Err(RuntimeError::Overflow)
        );
        assert_eq!(
            convert(Value::LInt(-1), LInt, USInt),
            Err(RuntimeError::Overflow)
        );
    }
}
