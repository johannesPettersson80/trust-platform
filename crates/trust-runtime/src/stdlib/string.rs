//! String standard functions.

#![allow(missing_docs)]

use crate::error::RuntimeError;
use crate::stdlib::helpers::{require_arity, require_min, to_i64};
use crate::stdlib::StandardLibrary;
use crate::value::{
    string_delete, string_element_count, string_find, string_insert, string_left, string_mid,
    string_replace, string_right, Value,
};
use smol_str::SmolStr;

pub fn register(lib: &mut StandardLibrary) {
    lib.register("LEN", &["IN"], len);
    lib.register("LEFT", &["IN", "L"], left);
    lib.register("RIGHT", &["IN", "L"], right);
    lib.register("MID", &["IN", "L", "P"], mid);
    lib.register_variadic("CONCAT", "IN", 1, 2, concat);
    lib.register("INSERT", &["IN1", "IN2", "P"], insert);
    lib.register("DELETE", &["IN", "L", "P"], delete);
    lib.register("REPLACE", &["IN1", "IN2", "L", "P"], replace);
    lib.register("FIND", &["IN1", "IN2"], find);
}

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
        Value::String(value) => Ok(Value::String(SmolStr::new(string_left(
            value.as_str(),
            count,
        )))),
        Value::WString(value) => Ok(Value::WString(string_left(value.as_str(), count))),
        _ => Err(RuntimeError::TypeMismatch),
    }
}

fn right(args: &[Value]) -> Result<Value, RuntimeError> {
    require_arity(args, 2)?;
    let count = to_i64(&args[1])?;
    match &args[0] {
        Value::String(value) => Ok(Value::String(SmolStr::new(string_right(
            value.as_str(),
            count,
        )))),
        Value::WString(value) => Ok(Value::WString(string_right(value.as_str(), count))),
        _ => Err(RuntimeError::TypeMismatch),
    }
}

fn mid(args: &[Value]) -> Result<Value, RuntimeError> {
    require_arity(args, 3)?;
    let length = to_i64(&args[1])?;
    let position = to_i64(&args[2])?;
    match &args[0] {
        Value::String(value) => Ok(Value::String(SmolStr::new(string_mid(
            value.as_str(),
            length,
            position,
        )))),
        Value::WString(value) => Ok(Value::WString(string_mid(value.as_str(), length, position))),
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
        Ok(Value::WString(result))
    } else {
        let mut result = String::new();
        for value in args {
            match value {
                Value::String(s) => result.push_str(s.as_str()),
                _ => return Err(RuntimeError::TypeMismatch),
            }
        }
        Ok(Value::String(SmolStr::new(result)))
    }
}

fn insert(args: &[Value]) -> Result<Value, RuntimeError> {
    require_arity(args, 3)?;
    let position = to_i64(&args[2])?;
    match (&args[0], &args[1]) {
        (Value::String(in1), Value::String(in2)) => Ok(Value::String(SmolStr::new(string_insert(
            in1.as_str(),
            in2.as_str(),
            position,
        )))),
        (Value::WString(in1), Value::WString(in2)) => Ok(Value::WString(string_insert(
            in1.as_str(),
            in2.as_str(),
            position,
        ))),
        _ => Err(RuntimeError::TypeMismatch),
    }
}

fn delete(args: &[Value]) -> Result<Value, RuntimeError> {
    require_arity(args, 3)?;
    let length = to_i64(&args[1])?;
    let position = to_i64(&args[2])?;
    match &args[0] {
        Value::String(input) => Ok(Value::String(SmolStr::new(string_delete(
            input.as_str(),
            length,
            position,
        )))),
        Value::WString(input) => Ok(Value::WString(string_delete(
            input.as_str(),
            length,
            position,
        ))),
        _ => Err(RuntimeError::TypeMismatch),
    }
}

fn replace(args: &[Value]) -> Result<Value, RuntimeError> {
    require_arity(args, 4)?;
    let length = to_i64(&args[2])?;
    let position = to_i64(&args[3])?;
    match (&args[0], &args[1]) {
        (Value::String(input), Value::String(repl)) => Ok(Value::String(SmolStr::new(
            string_replace(input.as_str(), repl.as_str(), length, position),
        ))),
        (Value::WString(input), Value::WString(repl)) => Ok(Value::WString(string_replace(
            input.as_str(),
            repl.as_str(),
            length,
            position,
        ))),
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
