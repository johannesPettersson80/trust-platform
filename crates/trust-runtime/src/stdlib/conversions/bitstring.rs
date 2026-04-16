use crate::error::RuntimeError;
use crate::value::Value;
use trust_hir::TypeId;

use super::numeric::{signed_int_from_i64, unsigned_int_from_u64};

pub(super) fn convert_to_bit_string(value: &Value, dst: TypeId) -> Result<Value, RuntimeError> {
    match value {
        Value::Byte(v) => bit_string_from_u64(*v as u64, dst),
        Value::Word(v) => bit_string_from_u64(*v as u64, dst),
        Value::DWord(v) => bit_string_from_u64(*v as u64, dst),
        Value::LWord(v) => bit_string_from_u64(*v, dst),
        Value::Char(v) => bit_string_from_u64(*v as u64, dst),
        Value::WChar(v) => bit_string_from_u64(*v as u64, dst),
        Value::SInt(v) => integer_to_bit_string(*v as i64, dst),
        Value::Int(v) => integer_to_bit_string(*v as i64, dst),
        Value::DInt(v) => integer_to_bit_string(*v as i64, dst),
        Value::LInt(v) => integer_to_bit_string(*v, dst),
        Value::USInt(v) => unsigned_to_bit_string(*v as u64, dst),
        Value::UInt(v) => unsigned_to_bit_string(*v as u64, dst),
        Value::UDInt(v) => unsigned_to_bit_string(*v as u64, dst),
        Value::ULInt(v) => unsigned_to_bit_string(*v, dst),
        Value::Real(v) if dst == TypeId::DWORD => Ok(Value::DWord(v.to_bits())),
        Value::LReal(v) if dst == TypeId::LWORD => Ok(Value::LWord(v.to_bits())),
        Value::Time(duration) if dst == TypeId::DWORD => {
            let millis = duration.as_millis();
            let millis = u32::try_from(millis).map_err(|_| RuntimeError::Overflow)?;
            Ok(Value::DWord(millis))
        }
        _ => Err(RuntimeError::TypeMismatch),
    }
}

pub(super) fn bit_string_to_int(value: &Value, dst: TypeId) -> Result<Value, RuntimeError> {
    let bits = bit_string_to_u64(value)?;
    let src_width = bit_width_from_value(value)?;
    let dst_width = int_width_from_type(dst)?;
    let masked = if dst_width >= src_width {
        bits
    } else {
        bits & mask_for(dst_width)
    };
    if super::util::is_signed_int_type(dst) {
        let signed = sign_extend(masked, dst_width)?;
        signed_int_from_i64(signed, dst)
    } else {
        unsigned_int_from_u64(masked, dst)
    }
}

pub(super) fn integer_to_bit_string(value: i64, dst: TypeId) -> Result<Value, RuntimeError> {
    let width = bit_width_from_type(dst)?;
    let mask = mask_for(width);
    let bits = (value as i128) & (mask as i128);
    bit_string_from_u64(bits as u64, dst)
}

pub(super) fn unsigned_to_bit_string(value: u64, dst: TypeId) -> Result<Value, RuntimeError> {
    let width = bit_width_from_type(dst)?;
    let mask = mask_for(width);
    let bits = value & mask;
    bit_string_from_u64(bits, dst)
}

pub(super) fn bit_string_to_u64(value: &Value) -> Result<u64, RuntimeError> {
    match value {
        Value::Byte(v) => Ok(*v as u64),
        Value::Word(v) => Ok(*v as u64),
        Value::DWord(v) => Ok(*v as u64),
        Value::LWord(v) => Ok(*v),
        _ => Err(RuntimeError::TypeMismatch),
    }
}

pub(super) fn bit_string_from_u64(value: u64, dst: TypeId) -> Result<Value, RuntimeError> {
    match dst {
        TypeId::BYTE => Ok(Value::Byte(value as u8)),
        TypeId::WORD => Ok(Value::Word(value as u16)),
        TypeId::DWORD => Ok(Value::DWord(value as u32)),
        TypeId::LWORD => Ok(Value::LWord(value)),
        _ => Err(RuntimeError::TypeMismatch),
    }
}

fn bit_width_from_value(value: &Value) -> Result<u32, RuntimeError> {
    match value {
        Value::Byte(_) => Ok(8),
        Value::Word(_) => Ok(16),
        Value::DWord(_) => Ok(32),
        Value::LWord(_) => Ok(64),
        _ => Err(RuntimeError::TypeMismatch),
    }
}

fn bit_width_from_type(ty: TypeId) -> Result<u32, RuntimeError> {
    match ty {
        TypeId::BYTE => Ok(8),
        TypeId::WORD => Ok(16),
        TypeId::DWORD => Ok(32),
        TypeId::LWORD => Ok(64),
        _ => Err(RuntimeError::TypeMismatch),
    }
}

fn int_width_from_type(ty: TypeId) -> Result<u32, RuntimeError> {
    match ty {
        TypeId::SINT | TypeId::USINT => Ok(8),
        TypeId::INT | TypeId::UINT => Ok(16),
        TypeId::DINT | TypeId::UDINT => Ok(32),
        TypeId::LINT | TypeId::ULINT => Ok(64),
        _ => Err(RuntimeError::TypeMismatch),
    }
}

fn mask_for(width: u32) -> u64 {
    if width >= 64 {
        u64::MAX
    } else {
        (1u64 << width) - 1
    }
}

fn sign_extend(value: u64, width: u32) -> Result<i64, RuntimeError> {
    if width == 64 {
        return Ok(value as i64);
    }
    let shift = 64 - width;
    let extended = ((value << shift) as i64) >> shift;
    Ok(extended)
}
