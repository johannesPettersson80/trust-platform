//! Bitwise and shift standard functions.

#![allow(missing_docs)]

use crate::error::RuntimeError;
use crate::stdlib::helpers::{
    bit_value, bit_value_to_result, mask_for, require_arity, require_min, to_i64,
};
use crate::stdlib::StandardLibrary;
use crate::value::Value;

pub fn register(lib: &mut StandardLibrary) {
    lib.register("SHL", &["IN", "N"], shl);
    lib.register("SHR", &["IN", "N"], shr);
    lib.register("ROL", &["IN", "N"], rol);
    lib.register("ROR", &["IN", "N"], ror);

    lib.register_variadic("AND", "IN", 1, 2, bit_and);
    lib.register_variadic("OR", "IN", 1, 2, bit_or);
    lib.register_variadic("XOR", "IN", 1, 2, bit_xor);
    lib.register("NOT", &["IN"], bit_not);
}

fn shl(args: &[Value]) -> Result<Value, RuntimeError> {
    shift(args, ShiftOp::Left)
}

fn shr(args: &[Value]) -> Result<Value, RuntimeError> {
    shift(args, ShiftOp::Right)
}

fn rol(args: &[Value]) -> Result<Value, RuntimeError> {
    shift(args, ShiftOp::RotateLeft)
}

fn ror(args: &[Value]) -> Result<Value, RuntimeError> {
    shift(args, ShiftOp::RotateRight)
}

fn bit_and(args: &[Value]) -> Result<Value, RuntimeError> {
    bitwise_variadic(args, |a, b| a & b)
}

fn bit_or(args: &[Value]) -> Result<Value, RuntimeError> {
    bitwise_variadic(args, |a, b| a | b)
}

fn bit_xor(args: &[Value]) -> Result<Value, RuntimeError> {
    bitwise_variadic(args, |a, b| a ^ b)
}

fn bit_not(args: &[Value]) -> Result<Value, RuntimeError> {
    require_arity(args, 1)?;
    let (value, width) = bit_value(&args[0])?;
    let mask = mask_for(width);
    let result = (!value) & mask;
    Ok(bit_value_to_result(result, width))
}

fn bitwise_variadic(args: &[Value], op: impl Fn(u64, u64) -> u64) -> Result<Value, RuntimeError> {
    require_min(args, 2)?;
    let (mut acc, mut width) = bit_value(&args[0])?;
    for value in &args[1..] {
        let (bits, bits_width) = bit_value(value)?;
        if bits_width > width {
            width = bits_width;
        }
        acc = op(acc, bits);
    }
    let mask = mask_for(width);
    acc &= mask;
    Ok(bit_value_to_result(acc, width))
}

#[derive(Debug, Clone, Copy)]
enum ShiftOp {
    Left,
    Right,
    RotateLeft,
    RotateRight,
}

fn shift(args: &[Value], op: ShiftOp) -> Result<Value, RuntimeError> {
    require_arity(args, 2)?;
    let (value, width) = bit_value(&args[0])?;
    let count = to_i64(&args[1])?;
    if count < 0 {
        return Err(RuntimeError::TypeMismatch);
    }
    let count = count as u32;
    let mask = mask_for(width);
    let result = match op {
        ShiftOp::Left => {
            if count >= width {
                0
            } else {
                (value << count) & mask
            }
        }
        ShiftOp::Right => {
            if count >= width {
                0
            } else {
                (value >> count) & mask
            }
        }
        ShiftOp::RotateLeft => {
            let shift = count % width;
            if shift == 0 {
                value & mask
            } else {
                ((value << shift) | (value >> (width - shift))) & mask
            }
        }
        ShiftOp::RotateRight => {
            let shift = count % width;
            if shift == 0 {
                value & mask
            } else {
                ((value >> shift) | (value << (width - shift))) & mask
            }
        }
    };
    Ok(bit_value_to_result(result, width))
}
