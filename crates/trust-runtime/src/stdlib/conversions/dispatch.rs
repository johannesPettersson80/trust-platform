use crate::error::RuntimeError;
use crate::stdlib::helpers::require_arity;
use crate::value::Value;
use trust_hir::TypeId;

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
    src: Option<TypeId>,
    dst: TypeId,
    mode: ConversionMode,
) -> Result<Value, RuntimeError> {
    let (value, actual_src) = normalize_source_value(value, src, mode)?;
    if !is_conversion_allowed(actual_src, dst) {
        return Err(RuntimeError::TypeMismatch);
    }
    convert_value(&value, dst, mode)
}

fn trunc_convert(value: &Value, src: Option<TypeId>, dst: TypeId) -> Result<Value, RuntimeError> {
    let (value, actual_src) = normalize_source_value(value, src, ConversionMode::Trunc)?;
    if !matches!(actual_src, TypeId::REAL | TypeId::LREAL) {
        return Err(RuntimeError::TypeMismatch);
    }
    if !is_integer_type(dst) {
        return Err(RuntimeError::TypeMismatch);
    }
    convert_value(&value, dst, ConversionMode::Trunc)
}

fn normalize_source_value(
    value: &Value,
    src: Option<TypeId>,
    mode: ConversionMode,
) -> Result<(Value, TypeId), RuntimeError> {
    let actual_src = value_type_id(value).ok_or(RuntimeError::TypeMismatch)?;
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

fn convert_value(value: &Value, dst: TypeId, mode: ConversionMode) -> Result<Value, RuntimeError> {
    if let Some(src) = value_type_id(value) {
        if src == dst {
            return Ok(value.clone());
        }
    }

    match dst {
        TypeId::SINT
        | TypeId::INT
        | TypeId::DINT
        | TypeId::LINT
        | TypeId::USINT
        | TypeId::UINT
        | TypeId::UDINT
        | TypeId::ULINT => convert_to_int(value, dst, mode),
        TypeId::REAL | TypeId::LREAL => convert_to_real(value, dst),
        TypeId::BYTE | TypeId::WORD | TypeId::DWORD | TypeId::LWORD => {
            convert_to_bit_string(value, dst)
        }
        TypeId::TIME | TypeId::LTIME => convert_to_time(value, dst),
        TypeId::DATE | TypeId::LDATE => convert_to_date(value, dst),
        TypeId::TOD | TypeId::LTOD => convert_to_tod(value, dst),
        TypeId::DT | TypeId::LDT => convert_to_dt(value, dst),
        TypeId::STRING | TypeId::WSTRING => convert_to_string(value, dst),
        TypeId::CHAR | TypeId::WCHAR => convert_to_char(value, dst),
        TypeId::BOOL => Err(RuntimeError::TypeMismatch),
        _ => Err(RuntimeError::TypeMismatch),
    }
}
