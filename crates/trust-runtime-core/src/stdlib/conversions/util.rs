use super::ConversionType;
use crate::value::Value;

pub(super) fn is_integer_type(ty: ConversionType) -> bool {
    matches!(
        ty,
        ConversionType::SInt
            | ConversionType::Int
            | ConversionType::DInt
            | ConversionType::LInt
            | ConversionType::USInt
            | ConversionType::UInt
            | ConversionType::UDInt
            | ConversionType::ULInt
    )
}

pub(super) fn is_signed_int_type(ty: ConversionType) -> bool {
    matches!(
        ty,
        ConversionType::SInt | ConversionType::Int | ConversionType::DInt | ConversionType::LInt
    )
}

pub(super) fn is_unsigned_int_type(ty: ConversionType) -> bool {
    matches!(
        ty,
        ConversionType::USInt
            | ConversionType::UInt
            | ConversionType::UDInt
            | ConversionType::ULInt
    )
}

pub(super) fn is_bit_string_type(ty: ConversionType) -> bool {
    matches!(
        ty,
        ConversionType::Bool
            | ConversionType::Byte
            | ConversionType::Word
            | ConversionType::DWord
            | ConversionType::LWord
    )
}

pub(super) fn is_conversion_allowed(src: ConversionType, dst: ConversionType) -> bool {
    if src == dst {
        return true;
    }

    if is_numeric_type(src) && is_numeric_type(dst) {
        return true;
    }

    if matches!(
        src,
        ConversionType::Byte | ConversionType::Word | ConversionType::DWord | ConversionType::LWord
    ) && matches!(
        dst,
        ConversionType::Byte | ConversionType::Word | ConversionType::DWord | ConversionType::LWord
    ) {
        return true;
    }

    if matches!(
        src,
        ConversionType::Bool
            | ConversionType::Byte
            | ConversionType::Word
            | ConversionType::DWord
            | ConversionType::LWord
    ) && matches!(
        dst,
        ConversionType::SInt
            | ConversionType::Int
            | ConversionType::DInt
            | ConversionType::LInt
            | ConversionType::USInt
            | ConversionType::UInt
            | ConversionType::UDInt
            | ConversionType::ULInt
    ) {
        return true;
    }

    if src == ConversionType::DWord && dst == ConversionType::Real {
        return true;
    }
    if src == ConversionType::LWord && dst == ConversionType::LReal {
        return true;
    }

    if matches!(
        dst,
        ConversionType::Byte | ConversionType::Word | ConversionType::DWord | ConversionType::LWord
    ) && matches!(
        src,
        ConversionType::SInt
            | ConversionType::Int
            | ConversionType::DInt
            | ConversionType::LInt
            | ConversionType::USInt
            | ConversionType::UInt
            | ConversionType::UDInt
            | ConversionType::ULInt
    ) {
        return true;
    }

    if src == ConversionType::Real && dst == ConversionType::DWord {
        return true;
    }
    if src == ConversionType::LReal && dst == ConversionType::LWord {
        return true;
    }

    if src == ConversionType::LTime && dst == ConversionType::Time {
        return true;
    }
    if src == ConversionType::Time && dst == ConversionType::LTime {
        return true;
    }
    if src == ConversionType::Time && dst == ConversionType::DWord {
        return true;
    }
    if src == ConversionType::DWord && dst == ConversionType::Time {
        return true;
    }
    if src == ConversionType::Ldt && dst == ConversionType::Dt {
        return true;
    }
    if src == ConversionType::Ldt && dst == ConversionType::Date {
        return true;
    }
    if src == ConversionType::Ldt && dst == ConversionType::LTod {
        return true;
    }
    if src == ConversionType::Ldt && dst == ConversionType::Tod {
        return true;
    }
    if src == ConversionType::Dt && dst == ConversionType::Ldt {
        return true;
    }
    if src == ConversionType::Dt && dst == ConversionType::Date {
        return true;
    }
    if src == ConversionType::Dt && dst == ConversionType::LTod {
        return true;
    }
    if src == ConversionType::Dt && dst == ConversionType::Tod {
        return true;
    }
    if src == ConversionType::LTod && dst == ConversionType::Tod {
        return true;
    }
    if src == ConversionType::Tod && dst == ConversionType::LTod {
        return true;
    }

    if matches!(dst, ConversionType::String | ConversionType::WString)
        && (is_numeric_type(src)
            || matches!(
                src,
                ConversionType::Byte
                    | ConversionType::Word
                    | ConversionType::DWord
                    | ConversionType::LWord
            ))
    {
        return true;
    }
    if matches!(src, ConversionType::String | ConversionType::WString) && is_numeric_type(dst) {
        return true;
    }
    if matches!(dst, ConversionType::Char | ConversionType::WChar)
        && (is_numeric_type(src)
            || matches!(
                src,
                ConversionType::Byte
                    | ConversionType::Word
                    | ConversionType::DWord
                    | ConversionType::LWord
            ))
    {
        return true;
    }
    if matches!(src, ConversionType::Char | ConversionType::WChar)
        && (is_numeric_type(dst)
            || matches!(
                dst,
                ConversionType::Byte
                    | ConversionType::Word
                    | ConversionType::DWord
                    | ConversionType::LWord
            ))
    {
        return true;
    }

    if src == ConversionType::WString
        && matches!(dst, ConversionType::String | ConversionType::WChar)
    {
        return true;
    }
    if src == ConversionType::String
        && matches!(dst, ConversionType::WString | ConversionType::Char)
    {
        return true;
    }
    if src == ConversionType::WChar && matches!(dst, ConversionType::WString | ConversionType::Char)
    {
        return true;
    }
    if src == ConversionType::Char && matches!(dst, ConversionType::String | ConversionType::WChar)
    {
        return true;
    }

    false
}

fn is_numeric_type(ty: ConversionType) -> bool {
    matches!(
        ty,
        ConversionType::SInt
            | ConversionType::Int
            | ConversionType::DInt
            | ConversionType::LInt
            | ConversionType::USInt
            | ConversionType::UInt
            | ConversionType::UDInt
            | ConversionType::ULInt
            | ConversionType::Real
            | ConversionType::LReal
    )
}

pub(super) fn value_type_id(value: &Value) -> Option<ConversionType> {
    match value {
        Value::Bool(_) => Some(ConversionType::Bool),
        Value::SInt(_) => Some(ConversionType::SInt),
        Value::Int(_) => Some(ConversionType::Int),
        Value::DInt(_) => Some(ConversionType::DInt),
        Value::LInt(_) => Some(ConversionType::LInt),
        Value::USInt(_) => Some(ConversionType::USInt),
        Value::UInt(_) => Some(ConversionType::UInt),
        Value::UDInt(_) => Some(ConversionType::UDInt),
        Value::ULInt(_) => Some(ConversionType::ULInt),
        Value::Real(_) => Some(ConversionType::Real),
        Value::LReal(_) => Some(ConversionType::LReal),
        Value::Byte(_) => Some(ConversionType::Byte),
        Value::Word(_) => Some(ConversionType::Word),
        Value::DWord(_) => Some(ConversionType::DWord),
        Value::LWord(_) => Some(ConversionType::LWord),
        Value::Time(_) => Some(ConversionType::Time),
        Value::LTime(_) => Some(ConversionType::LTime),
        Value::Date(_) => Some(ConversionType::Date),
        Value::LDate(_) => Some(ConversionType::LDate),
        Value::Tod(_) => Some(ConversionType::Tod),
        Value::LTod(_) => Some(ConversionType::LTod),
        Value::Dt(_) => Some(ConversionType::Dt),
        Value::Ldt(_) => Some(ConversionType::Ldt),
        Value::String(_) => Some(ConversionType::String),
        Value::WString(_) => Some(ConversionType::WString),
        Value::Char(_) => Some(ConversionType::Char),
        Value::WChar(_) => Some(ConversionType::WChar),
        _ => None,
    }
}
