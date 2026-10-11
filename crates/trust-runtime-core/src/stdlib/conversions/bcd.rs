use super::ConversionType;
use crate::error::RuntimeError;
use crate::stdlib::helpers::to_u64;
use crate::value::Value;

use super::bitstring::{bit_string_from_u64, bit_string_to_u64};
use super::numeric::unsigned_int_from_u64;
use super::util::{is_bit_string_type, is_unsigned_int_type, value_type_id};

pub(super) fn to_bcd(
    value: &Value,
    src: Option<ConversionType>,
    dst: ConversionType,
) -> Result<Value, RuntimeError> {
    let actual_src = value_type_id(value).ok_or(RuntimeError::TypeMismatch)?;
    if let Some(expected) = src {
        if actual_src != expected {
            return Err(RuntimeError::TypeMismatch);
        }
    }
    if !is_unsigned_int_type(actual_src) {
        return Err(RuntimeError::TypeMismatch);
    }
    if !is_bit_string_type(dst) || dst == ConversionType::Bool {
        return Err(RuntimeError::TypeMismatch);
    }
    let input = to_u64(value)?;
    let digits = bcd_digits_for(dst)?;
    let bits = u64_to_bcd(input, digits)?;
    bit_string_from_u64(bits, dst)
}

pub(super) fn from_bcd(
    value: &Value,
    src: Option<ConversionType>,
    dst: ConversionType,
) -> Result<Value, RuntimeError> {
    let actual_src = value_type_id(value).ok_or(RuntimeError::TypeMismatch)?;
    if let Some(expected) = src {
        if actual_src != expected {
            return Err(RuntimeError::TypeMismatch);
        }
    }
    if !is_bit_string_type(actual_src) || actual_src == ConversionType::Bool {
        return Err(RuntimeError::TypeMismatch);
    }
    if !is_unsigned_int_type(dst) {
        return Err(RuntimeError::TypeMismatch);
    }
    let bits = bit_string_to_u64(value)?;
    let digits = bcd_digits_for(actual_src)?;
    let decoded = bcd_to_u64(bits, digits)?;
    unsigned_int_from_u64(decoded, dst)
}

fn bcd_digits_for(ty: ConversionType) -> Result<usize, RuntimeError> {
    match ty {
        ConversionType::Byte => Ok(2),
        ConversionType::Word => Ok(4),
        ConversionType::DWord => Ok(8),
        ConversionType::LWord => Ok(16),
        _ => Err(RuntimeError::TypeMismatch),
    }
}

fn u64_to_bcd(mut value: u64, digits: usize) -> Result<u64, RuntimeError> {
    let mut result = 0u64;
    for i in 0..digits {
        let digit = value % 10;
        value /= 10;
        result |= digit << (i * 4);
    }
    if value != 0 {
        return Err(RuntimeError::Overflow);
    }
    Ok(result)
}

fn bcd_to_u64(value: u64, digits: usize) -> Result<u64, RuntimeError> {
    let mut result = 0u64;
    let mut multiplier = 1u64;
    for i in 0..digits {
        let digit = (value >> (i * 4)) & 0xF;
        if digit > 9 {
            return Err(RuntimeError::TypeMismatch);
        }
        result = result
            .checked_add(digit * multiplier)
            .ok_or(RuntimeError::Overflow)?;
        multiplier = multiplier.checked_mul(10).ok_or(RuntimeError::Overflow)?;
    }
    Ok(result)
}
