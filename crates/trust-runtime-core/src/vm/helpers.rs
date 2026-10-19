use crate::value::Value;

/// Return the number of immediate operand bytes for one VM opcode.
#[must_use]
pub const fn opcode_operand_len(opcode: u8) -> Option<usize> {
    match opcode {
        0x00
        | 0x01
        | 0x06
        | 0x11
        | 0x12
        | 0x13
        | 0x14
        | 0x15
        | 0x23
        | 0x24
        | 0x25
        | 0x31
        | 0x32
        | 0x33
        | 0x61
        | 0x40..=0x4E
        | 0x50..=0x55 => Some(0),
        0x02..=0x05
        | 0x07
        | 0x10
        | 0x20..=0x22
        | 0x30
        | 0x60
        | 0x62..=crate::bytecode::opcodes::STRUCT_SET
        | 0x70 => Some(4),
        0x08 => Some(8),
        0x09 => Some(12),
        0x16 => Some(1),
        _ => None,
    }
}

/// Materialize a borrowed VM value, reporting whether a clone was required.
#[must_use]
pub fn materialize_borrowed_value(value: &Value) -> (Value, bool) {
    (value.clone(), borrowed_value_is_cloned(value))
}

/// Whether [`materialize_borrowed_value`] copies a payload of `value` rather than its scalar bits
/// or a shared handle: a text, an array, an enumeration or a reference is cloned, a scalar, a
/// structure, an instance and the null value are not.
pub fn borrowed_value_is_cloned(value: &Value) -> bool {
    !matches!(
        value,
        Value::Bool(_)
            | Value::SInt(_)
            | Value::Int(_)
            | Value::DInt(_)
            | Value::LInt(_)
            | Value::USInt(_)
            | Value::UInt(_)
            | Value::UDInt(_)
            | Value::ULInt(_)
            | Value::Real(_)
            | Value::LReal(_)
            | Value::Byte(_)
            | Value::Word(_)
            | Value::DWord(_)
            | Value::LWord(_)
            | Value::Time(_)
            | Value::LTime(_)
            | Value::Date(_)
            | Value::LDate(_)
            | Value::Tod(_)
            | Value::LTod(_)
            | Value::Dt(_)
            | Value::Ldt(_)
            | Value::Char(_)
            | Value::WChar(_)
            | Value::Struct(_)
            | Value::Instance(_)
            | Value::Null
    )
}
