//! The messages of failed assertions (specification 44 Appendix A; specification 07, MP-014): the
//! runtime's words for each, the operands read from the fault record through the type table, a
//! text from the stack area where the module laid it, and written as the runtime writes a value
//! for its user.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec;

use smol_str::SmolStr;
use trust_plc_abi::{FaultCode, FaultRecord};

use super::FailedAssertion;
use crate::error::RuntimeError;
use crate::native::image::type_kind;
use crate::native::values::{damaged, read_value, AreaBytes, Types};
use crate::value::{float_text, format_user_value, Value};

/// The message of the failed assertion of `record`, of fault `code`, as the runtime's assertion
/// functions write it; a text operand lies in `stack`, the stack area.
pub(super) fn message<A: AreaBytes + ?Sized>(
    types: &Types<'_>,
    record: &FaultRecord,
    code: u32,
    stack: &A,
) -> String {
    let [left, right, described] = record.detail;
    if code == FaultCode::AssertionNear as u32 {
        return near(
            f64::from_bits(left.cast_unsigned()),
            f64::from_bits(right.cast_unsigned()),
            f64::from_bits(described.cast_unsigned()),
        );
    }
    let [left, right] = operands(types, record, stack)
        .map(|value| value.map_or_else(|_| "?".to_string(), |value| format_user_value(&value)));
    match described.cast_unsigned() & 0xFF {
        1 => "ASSERT_TRUE expected TRUE, got FALSE".to_string(),
        2 => "ASSERT_FALSE expected FALSE, got TRUE".to_string(),
        3 => format!("ASSERT_EQUAL failed: expected {left}, actual {right}"),
        4 => format!("ASSERT_NOT_EQUAL failed: values should differ, left {left}, right {right}"),
        5 => format!("ASSERT_GREATER failed: value {left} is not greater than bound {right}"),
        6 => format!("ASSERT_LESS failed: value {left} is not less than bound {right}"),
        7 => format!("ASSERT_GREATER_OR_EQUAL failed: value {left} is not >= bound {right}"),
        _ => format!("ASSERT_LESS_OR_EQUAL failed: value {left} is not <= bound {right}"),
    }
}

/// The values of the failed assertion of `record`, of fault `code` (specification 22): the inputs
/// the message names, as typed values; a text operand lies in `stack`, the stack area.
pub(super) fn failed<A: AreaBytes + ?Sized>(
    types: &Types<'_>,
    record: &FaultRecord,
    code: u32,
    stack: &A,
) -> FailedAssertion {
    let [left, right, described] = record.detail;
    if code == FaultCode::AssertionNear as u32 {
        let real = |bits: i64| Some(Value::LReal(f64::from_bits(bits.cast_unsigned())));
        return FailedAssertion {
            name: "ASSERT_NEAR",
            expected: real(left),
            actual: real(right),
            bound: None,
            delta: real(described),
        };
    }
    let [left, right] = operands(types, record, stack).map(Result::ok);
    let (name, expected, actual, bound) = match described.cast_unsigned() & 0xFF {
        1 => (
            "ASSERT_TRUE",
            Some(Value::Bool(true)),
            Some(Value::Bool(false)),
            None,
        ),
        2 => (
            "ASSERT_FALSE",
            Some(Value::Bool(false)),
            Some(Value::Bool(true)),
            None,
        ),
        3 => ("ASSERT_EQUAL", left, right, None),
        4 => ("ASSERT_NOT_EQUAL", left, right, None),
        5 => ("ASSERT_GREATER", None, left, right),
        6 => ("ASSERT_LESS", None, left, right),
        7 => ("ASSERT_GREATER_OR_EQUAL", None, left, right),
        _ => ("ASSERT_LESS_OR_EQUAL", None, left, right),
    };
    FailedAssertion {
        name,
        expected,
        actual,
        bound,
        delta: None,
    }
}

/// The two operands of a failed assertion other than `ASSERT_NEAR`, read through the type table by
/// the types its third number names: a text from `stack`, where the module laid it, and any other
/// value from its own bits.
fn operands<A: AreaBytes + ?Sized>(
    types: &Types<'_>,
    record: &FaultRecord,
    stack: &A,
) -> [Result<Value, RuntimeError>; 2] {
    let [left, right, described] = record.detail;
    let described = described.cast_unsigned();
    let left_type = u32::try_from((described >> 8) & 0x00FF_FFFF).unwrap_or(0);
    let right_type = u32::try_from((described >> 32) & 0x00FF_FFFF).unwrap_or(0);
    let operand = |bits: i64, index: u32| match types.get(index) {
        Ok(ty) if ty.kind == type_kind::STRING => text(stack, bits),
        _ => read_value(types, &bits.to_le_bytes()[..], 0, index),
    };
    [operand(left, left_type), operand(right, right_type)]
}

/// The text the module laid at `offset` of the stack area for a failed assertion of texts: its
/// length in bytes in two bytes, then its bytes, as a string lies (specification 44 Appendix A).
fn text<A: AreaBytes + ?Sized>(stack: &A, offset: i64) -> Result<Value, RuntimeError> {
    let offset = usize::try_from(offset).map_err(|_| damaged("stack area"))?;
    let mut length = [0u8; 2];
    if !stack.read(offset, &mut length) {
        return Err(damaged("stack area"));
    }
    let mut bytes = vec![0u8; usize::from(u16::from_le_bytes(length))];
    if !stack.read(offset + 2, &mut bytes) {
        return Err(damaged("stack area"));
    }
    let text = String::from_utf8(bytes).map_err(|_| RuntimeError::TypeMismatch)?;
    Ok(Value::String(SmolStr::new(text)))
}

/// The message of a failed `ASSERT_NEAR` of `expected`, `actual` and `delta`.
fn near(expected: f64, actual: f64, delta: f64) -> String {
    if delta < 0.0 {
        return "ASSERT_NEAR failed: DELTA must be non-negative".to_string();
    }
    let difference = (expected - actual).abs();
    format!(
        "ASSERT_NEAR failed: expected {}, actual {}, delta {}, diff {}",
        float_text::display(expected),
        float_text::display(actual),
        float_text::display(delta),
        float_text::display(difference)
    )
}
