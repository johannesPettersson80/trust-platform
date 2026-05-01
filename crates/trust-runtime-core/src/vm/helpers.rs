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
        | 0x31
        | 0x32
        | 0x33
        | 0x61
        | 0x40..=0x4E
        | 0x50..=0x55 => Some(0),
        0x02..=0x05 | 0x07 | 0x10 | 0x20..=0x22 | 0x30 | 0x60 | 0x62 | 0x63 | 0x70 => Some(4),
        0x08 => Some(8),
        0x09 => Some(12),
        0x16 => Some(1),
        _ => None,
    }
}

/// Materialize a borrowed VM value, reporting whether a clone was required.
#[must_use]
pub fn materialize_borrowed_value(value: &Value) -> (Value, bool) {
    match value {
        Value::Bool(value) => (Value::Bool(*value), false),
        Value::SInt(value) => (Value::SInt(*value), false),
        Value::Int(value) => (Value::Int(*value), false),
        Value::DInt(value) => (Value::DInt(*value), false),
        Value::LInt(value) => (Value::LInt(*value), false),
        Value::USInt(value) => (Value::USInt(*value), false),
        Value::UInt(value) => (Value::UInt(*value), false),
        Value::UDInt(value) => (Value::UDInt(*value), false),
        Value::ULInt(value) => (Value::ULInt(*value), false),
        Value::Real(value) => (Value::Real(*value), false),
        Value::LReal(value) => (Value::LReal(*value), false),
        Value::Byte(value) => (Value::Byte(*value), false),
        Value::Word(value) => (Value::Word(*value), false),
        Value::DWord(value) => (Value::DWord(*value), false),
        Value::LWord(value) => (Value::LWord(*value), false),
        Value::Time(value) => (Value::Time(*value), false),
        Value::LTime(value) => (Value::LTime(*value), false),
        Value::Date(value) => (Value::Date(*value), false),
        Value::LDate(value) => (Value::LDate(*value), false),
        Value::Tod(value) => (Value::Tod(*value), false),
        Value::LTod(value) => (Value::LTod(*value), false),
        Value::Dt(value) => (Value::Dt(*value), false),
        Value::Ldt(value) => (Value::Ldt(*value), false),
        Value::Char(value) => (Value::Char(*value), false),
        Value::WChar(value) => (Value::WChar(*value), false),
        Value::Struct(value) => (Value::Struct(value.clone()), false),
        Value::Instance(value) => (Value::Instance(*value), false),
        Value::Null => (Value::Null, false),
        _ => (value.clone(), true),
    }
}
