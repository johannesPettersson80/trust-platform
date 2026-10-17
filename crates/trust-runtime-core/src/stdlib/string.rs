//! String standard functions.

use crate::error::RuntimeError;
use crate::stdlib::helpers::{require_arity, require_min, to_i64};
#[cfg(feature = "hir")]
use crate::stdlib::StandardLibrary;
use crate::value::{
    check_string_result, string_capacity_cut, string_delete, string_element_count, string_find,
    string_insert, string_left, string_mid, string_replace, string_right, Value,
};
use alloc::string::String;
use smol_str::SmolStr;

/// Register the shared string functions in a hosted registry.
#[cfg(feature = "hir")]
pub fn register(lib: &mut StandardLibrary) {
    lib.register_descriptors(&FUNCTIONS);
}

pub(super) static FUNCTIONS: [(&str, super::StdFunctionRef<'static>); 10] = [
    super::registration::descriptor!("CONCAT", VAR_IN_1_2, concat),
    super::registration::descriptor!("DELETE", IN_L_P, delete),
    super::registration::descriptor!("FIND", IN1_IN2, find),
    super::registration::descriptor!("INSERT", IN1_IN2_P, insert),
    super::registration::descriptor!("LEFT", IN_L, left),
    super::registration::descriptor!("LEN", IN, len),
    super::registration::descriptor!("MID", IN_L_P, mid),
    super::registration::descriptor!("REPLACE", IN1_IN2_L_P, replace),
    super::registration::descriptor!("RIGHT", IN_L, right),
    super::registration::descriptor!("__TRUST_LIMIT_STRING", IN_L, limit_string),
];

fn len(args: &[Value]) -> Result<Value, RuntimeError> {
    require_arity(args, 1)?;
    let length = match &args[0] {
        Value::String(value) => string_element_count(value.as_str()),
        Value::WString(value) => string_element_count(value.as_str()),
        _ => return Err(RuntimeError::TypeMismatch),
    };
    if length > i16::MAX as usize {
        return Err(RuntimeError::Overflow);
    }
    Ok(Value::Int(length as i16))
}

fn left(args: &[Value]) -> Result<Value, RuntimeError> {
    require_arity(args, 2)?;
    let count = to_i64(&args[1])?;
    match &args[0] {
        Value::String(value) => narrow_result(string_left(value.as_str(), count)),
        Value::WString(value) => wide_result(string_left(value.as_str(), count)),
        _ => Err(RuntimeError::TypeMismatch),
    }
}

fn right(args: &[Value]) -> Result<Value, RuntimeError> {
    require_arity(args, 2)?;
    let count = to_i64(&args[1])?;
    match &args[0] {
        Value::String(value) => narrow_result(string_right(value.as_str(), count)),
        Value::WString(value) => wide_result(string_right(value.as_str(), count)),
        _ => Err(RuntimeError::TypeMismatch),
    }
}

fn mid(args: &[Value]) -> Result<Value, RuntimeError> {
    require_arity(args, 3)?;
    let length = to_i64(&args[1])?;
    let position = to_i64(&args[2])?;
    match &args[0] {
        Value::String(value) => narrow_result(string_mid(value.as_str(), length, position)),
        Value::WString(value) => wide_result(string_mid(value.as_str(), length, position)),
        _ => Err(RuntimeError::TypeMismatch),
    }
}

fn concat(args: &[Value]) -> Result<Value, RuntimeError> {
    require_min(args, 2)?;
    let is_wide = match &args[0] {
        Value::String(_) => false,
        Value::WString(_) => true,
        _ => return Err(RuntimeError::TypeMismatch),
    };
    if is_wide {
        let mut result = String::new();
        for value in args {
            match value {
                Value::WString(s) => result.push_str(s),
                _ => return Err(RuntimeError::TypeMismatch),
            }
        }
        wide_result(result)
    } else {
        let mut result = String::new();
        for value in args {
            match value {
                Value::String(s) => result.push_str(s.as_str()),
                _ => return Err(RuntimeError::TypeMismatch),
            }
        }
        narrow_result(result)
    }
}

/// A `STRING` result, refused above the implementer maximum (IEC 61131-3 Ed.3 §6.6.2.5.11).
fn narrow_result(text: String) -> Result<Value, RuntimeError> {
    check_string_result(&text, false)?;
    Ok(Value::String(SmolStr::new(text)))
}

/// A `WSTRING` result, refused above the implementer maximum (IEC 61131-3 Ed.3 §6.6.2.5.11).
fn wide_result(text: String) -> Result<Value, RuntimeError> {
    check_string_result(&text, true)?;
    Ok(Value::WString(text))
}

/// Cut a value to the capacity of its destination: UTF-8 bytes for `STRING`, UTF-16 code units
/// for `WSTRING`, never splitting a character. A value that fits is returned unchanged.
fn limit_string(args: &[Value]) -> Result<Value, RuntimeError> {
    require_arity(args, 2)?;
    let capacity = u32::try_from(to_i64(&args[1])?).map_err(|_| RuntimeError::Overflow)?;
    let capacity = usize::try_from(capacity).map_err(|_| RuntimeError::Overflow)?;
    match &args[0] {
        Value::String(text) => Ok(Value::String(
            match string_capacity_cut(text.as_str(), false, capacity) {
                Some(end) => SmolStr::new(&text[..end]),
                None => text.clone(),
            },
        )),
        Value::WString(text) => Ok(Value::WString(
            match string_capacity_cut(text, true, capacity) {
                Some(end) => String::from(&text[..end]),
                None => text.clone(),
            },
        )),
        _ => Err(RuntimeError::TypeMismatch),
    }
}

fn insert(args: &[Value]) -> Result<Value, RuntimeError> {
    require_arity(args, 3)?;
    let position = to_i64(&args[2])?;
    match (&args[0], &args[1]) {
        (Value::String(in1), Value::String(in2)) => {
            narrow_result(string_insert(in1.as_str(), in2.as_str(), position))
        }
        (Value::WString(in1), Value::WString(in2)) => {
            wide_result(string_insert(in1.as_str(), in2.as_str(), position))
        }
        _ => Err(RuntimeError::TypeMismatch),
    }
}

fn delete(args: &[Value]) -> Result<Value, RuntimeError> {
    require_arity(args, 3)?;
    let length = to_i64(&args[1])?;
    let position = to_i64(&args[2])?;
    match &args[0] {
        Value::String(input) => narrow_result(string_delete(input.as_str(), length, position)),
        Value::WString(input) => wide_result(string_delete(input.as_str(), length, position)),
        _ => Err(RuntimeError::TypeMismatch),
    }
}

fn replace(args: &[Value]) -> Result<Value, RuntimeError> {
    require_arity(args, 4)?;
    let length = to_i64(&args[2])?;
    let position = to_i64(&args[3])?;
    match (&args[0], &args[1]) {
        (Value::String(input), Value::String(repl)) => narrow_result(string_replace(
            input.as_str(),
            repl.as_str(),
            length,
            position,
        )),
        (Value::WString(input), Value::WString(repl)) => wide_result(string_replace(
            input.as_str(),
            repl.as_str(),
            length,
            position,
        )),
        _ => Err(RuntimeError::TypeMismatch),
    }
}

fn find(args: &[Value]) -> Result<Value, RuntimeError> {
    require_arity(args, 2)?;
    match (&args[0], &args[1]) {
        (Value::String(in1), Value::String(in2)) => {
            Ok(Value::Int(string_find(in1.as_str(), in2.as_str())?))
        }
        (Value::WString(in1), Value::WString(in2)) => {
            Ok(Value::Int(string_find(in1.as_str(), in2.as_str())?))
        }
        _ => Err(RuntimeError::TypeMismatch),
    }
}
