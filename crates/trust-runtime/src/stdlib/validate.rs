//! Validate standard functions.

#![allow(missing_docs)]

use crate::error::RuntimeError;
use crate::stdlib::helpers::require_arity;
use crate::stdlib::StandardLibrary;
use crate::value::Value;

pub fn register(lib: &mut StandardLibrary) {
    lib.register("IS_VALID", &["IN"], is_valid);
    lib.register("IS_VALID_BCD", &["IN"], is_valid_bcd);
}

fn is_valid(args: &[Value]) -> Result<Value, RuntimeError> {
    require_arity(args, 1)?;
    let valid = match &args[0] {
        Value::Real(v) => v.is_finite(),
        Value::LReal(v) => v.is_finite(),
        _ => return Err(RuntimeError::TypeMismatch),
    };
    Ok(Value::Bool(valid))
}

fn is_valid_bcd(args: &[Value]) -> Result<Value, RuntimeError> {
    require_arity(args, 1)?;
    let (value, width) = match &args[0] {
        Value::Byte(value) => (u64::from(*value), 8),
        Value::Word(value) => (u64::from(*value), 16),
        Value::DWord(value) => (u64::from(*value), 32),
        Value::LWord(value) => (*value, 64),
        _ => return Err(RuntimeError::TypeMismatch),
    };
    let digits = (width / 4) as usize;
    let mut valid = true;
    for i in 0..digits {
        let nibble = (value >> (i * 4)) & 0xF;
        if nibble > 9 {
            valid = false;
            break;
        }
    }
    Ok(Value::Bool(valid))
}
