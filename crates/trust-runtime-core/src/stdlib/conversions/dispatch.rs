use super::ConversionType;
use crate::error::RuntimeError;
use crate::stdlib::helpers::require_arity;
use crate::stdlib::text_kernels::TextResult;
use crate::value::{check_string_result, Value};
use alloc::string::String;

use super::bcd::{from_bcd, to_bcd};
use super::bitstring::convert_to_bit_string;
use super::numeric::{convert_to_int, convert_to_real, finite_real_result, signed_int_from_i128};
use super::spec::ConversionSpec;
use super::string::{
    convert_to_char, convert_to_string, convert_to_string_into, parse_int_text,
    parse_real_text_with, string_to_char,
};
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

/// [`apply_conversion`] of `spec` for `value`, an integer, when `spec` converts it to an integer type
/// (checklist P1-9): the same checks and result, through the integer branch that
/// [`convert_with_mode`] takes for them, without the general binding of a conversion call. `None`
/// for any other conversion, a source type that is not the value's, and any other value.
pub(crate) fn convert_integer(
    spec: ConversionSpec,
    value: &Value,
) -> Option<Result<Value, RuntimeError>> {
    let ConversionSpec::Convert { src, dst } = spec else {
        return None;
    };
    let actual = value_type_id(value)?;
    if !is_integer_type(actual) || !is_integer_type(dst) || src.is_some_and(|src| src != actual) {
        return None;
    }
    Some(if !is_conversion_allowed(actual, dst) {
        Err(RuntimeError::TypeMismatch)
    } else if actual == dst {
        Ok(value.clone())
    } else {
        convert_to_int(value, dst, ConversionMode::Round)
    })
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

/// [`apply_conversion`] of `ConversionSpec::Convert { src, dst }` for the value `value`, a text
/// result written into `out` instead of a new value (C1-12): what it gives, the text in `out`.
pub(crate) fn convert_value_into(
    src: Option<ConversionType>,
    dst: ConversionType,
    value: &Value,
    out: &mut String,
) -> Result<TextResult, RuntimeError> {
    let (value, actual_src) = normalize_source_value(value, src, ConversionMode::Round)?;
    if !is_conversion_allowed(actual_src, dst) {
        return Err(RuntimeError::TypeMismatch);
    }
    converted_into(value, dst, out)
}

/// [`apply_conversion`] of `ConversionSpec::Convert { src, dst }` for a text read where it lies,
/// a `WSTRING` when `wide` (C1-12): parsed without a copy, a text result written into `out`, a real
/// with separators cleaned in `scratch`. What it gives for the text's value.
pub(crate) fn convert_text_into(
    src: Option<ConversionType>,
    dst: ConversionType,
    text: &str,
    wide: bool,
    out: &mut String,
    scratch: &mut String,
) -> Result<TextResult, RuntimeError> {
    let mut actual = if wide {
        ConversionType::WString
    } else {
        ConversionType::String
    };
    if let Some(expected) = src
        && expected != actual
    {
        // The source is first converted to the type the conversion names, as for its value.
        if !is_conversion_allowed(actual, expected) {
            return Err(RuntimeError::TypeMismatch);
        }
        if !matches!(expected, ConversionType::String | ConversionType::WString) {
            let value = text_value_as(expected, text, scratch)?;
            if !is_conversion_allowed(expected, dst) {
                return Err(RuntimeError::TypeMismatch);
            }
            return converted_into(value, dst, out);
        }
        check_string_result(text, expected == ConversionType::WString)?;
        actual = expected;
    }
    if !is_conversion_allowed(actual, dst) {
        return Err(RuntimeError::TypeMismatch);
    }
    if actual == dst {
        out.push_str(text);
        return Ok(TextResult::Text {
            wide: dst == ConversionType::WString,
        });
    }
    match dst {
        ConversionType::String | ConversionType::WString => {
            let wide = dst == ConversionType::WString;
            check_string_result(text, wide)?;
            out.push_str(text);
            Ok(TextResult::Text { wide })
        }
        _ => text_value_as(dst, text, scratch).map(TextResult::Value),
    }
}

/// The value of the text `text` converted to the type `ty` that is no text: an integer, a real or
/// a character, as `convert_value` gives it for the text's value.
fn text_value_as(
    ty: ConversionType,
    text: &str,
    scratch: &mut String,
) -> Result<Value, RuntimeError> {
    match ty {
        ConversionType::SInt
        | ConversionType::Int
        | ConversionType::DInt
        | ConversionType::LInt
        | ConversionType::USInt
        | ConversionType::UInt
        | ConversionType::UDInt
        | ConversionType::ULInt => signed_int_from_i128(parse_int_text(text)?, ty),
        ConversionType::Real | ConversionType::LReal => {
            finite_real_result(parse_real_text_with(text, scratch)?, ty)
        }
        ConversionType::Char | ConversionType::WChar => {
            string_to_char(text, ty == ConversionType::WChar)
        }
        _ => Err(RuntimeError::TypeMismatch),
    }
}

/// The last step of a conversion of the value `value` to `dst`: a text into `out`, any other
/// result as a value (C1-12).
fn converted_into(
    value: Value,
    dst: ConversionType,
    out: &mut String,
) -> Result<TextResult, RuntimeError> {
    if value_type_id(&value) == Some(dst) {
        return Ok(TextResult::Value(value));
    }
    match dst {
        ConversionType::String | ConversionType::WString => {
            convert_to_string_into(&value, dst, out).map(|wide| TextResult::Text { wide })
        }
        _ => convert_value(&value, dst, ConversionMode::Round).map(TextResult::Value),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stdlib::conversions::call_conversion_spec;
    use crate::stdlib::conversions::ConversionType::{
        DInt, Int, LInt, SInt, UDInt, UInt, ULInt, USInt,
    };
    use alloc::vec;
    use alloc::vec::Vec;

    const INTEGERS: [ConversionType; 8] = [SInt, Int, DInt, LInt, USInt, UInt, UDInt, ULInt];

    /// Integers of every type at its bounds and between them.
    fn samples() -> Vec<Value> {
        vec![
            Value::SInt(i8::MIN),
            Value::SInt(-1),
            Value::SInt(i8::MAX),
            Value::Int(i16::MIN),
            Value::Int(300),
            Value::Int(i16::MAX),
            Value::DInt(i32::MIN),
            Value::DInt(70_000),
            Value::DInt(i32::MAX),
            Value::LInt(i64::MIN),
            Value::LInt(-1),
            Value::LInt(i64::MAX),
            Value::USInt(0),
            Value::USInt(u8::MAX),
            Value::UInt(40_000),
            Value::UInt(u16::MAX),
            Value::UDInt(0),
            Value::UDInt(u32::MAX),
            Value::ULInt(1 << 63),
            Value::ULInt(u64::MAX),
        ]
    }

    /// Specification 34 PERF-05: compact code converts an integer to an integer type with the
    /// general conversion's result. Every sample converted to every integer type, with the
    /// conversion's source type named and left out, gives what `call_conversion_spec` gives.
    /// Catches a range check of the wrong width, a sign lost on the way and a result of the wrong
    /// type.
    #[test]
    fn an_integer_converts_as_the_general_conversion_converts_it() {
        for value in samples() {
            let actual = value_type_id(&value).expect("an integer");
            for dst in INTEGERS {
                for src in [None, Some(actual)] {
                    let spec = ConversionSpec::Convert { src, dst };
                    assert_eq!(
                        convert_integer(spec, &value),
                        Some(call_conversion_spec(spec, core::slice::from_ref(&value))),
                        "{value:?} to {dst:?} (source {src:?})"
                    );
                }
            }
        }
    }

    /// Worked cases of specification 07 §2.1: a representable integer keeps its value exactly, and
    /// a value outside the target's range is `RuntimeError::Overflow`, never wrapped or saturated:
    /// UDINT_TO_DINT(70000) is 70000, INT_TO_DINT(-1234) is -1234, UDINT_TO_DINT(4294967295),
    /// DINT_TO_SINT(128) and LINT_TO_USINT(-1) overflow. Catches wrapping, saturation and a sign
    /// extended or dropped.
    #[test]
    fn worked_integer_conversions_of_specification_07() {
        let convert = |value: Value, src, dst| {
            convert_integer(
                ConversionSpec::Convert {
                    src: Some(src),
                    dst,
                },
                &value,
            )
        };
        assert_eq!(
            convert(Value::UDInt(70_000), UDInt, DInt),
            Some(Ok(Value::DInt(70_000)))
        );
        assert_eq!(
            convert(Value::Int(-1234), Int, DInt),
            Some(Ok(Value::DInt(-1234)))
        );
        assert_eq!(
            convert(Value::UDInt(u32::MAX), UDInt, DInt),
            Some(Err(RuntimeError::Overflow))
        );
        assert_eq!(
            convert(Value::DInt(128), DInt, SInt),
            Some(Err(RuntimeError::Overflow))
        );
        assert_eq!(
            convert(Value::LInt(-1), LInt, USInt),
            Some(Err(RuntimeError::Overflow))
        );
    }

    /// Any other conversion or value is left to the general conversion: a source type that is not
    /// the value's, a target that is not an integer, a value that is not an integer, a truncation
    /// and a BCD conversion. Catches the integer branch taking a conversion it does not cover.
    #[test]
    fn only_an_integer_to_an_integer_type_takes_the_integer_branch() {
        let five = Value::Int(5);
        let convert = |src, dst| ConversionSpec::Convert { src, dst };
        assert_eq!(convert_integer(convert(Some(DInt), Int), &five), None);
        assert_eq!(
            convert_integer(convert(None, ConversionType::Real), &five),
            None
        );
        assert_eq!(
            convert_integer(convert(None, ConversionType::DWord), &five),
            None
        );
        assert_eq!(convert_integer(convert(None, Int), &Value::Real(5.0)), None);
        assert_eq!(
            convert_integer(convert(None, Int), &Value::Bool(true)),
            None
        );
        assert_eq!(
            convert_integer(
                ConversionSpec::Trunc {
                    src: None,
                    dst: Int
                },
                &five
            ),
            None
        );
        assert_eq!(
            convert_integer(
                ConversionSpec::ToBcd {
                    src: None,
                    dst: ConversionType::Word
                },
                &five
            ),
            None
        );
    }
}
