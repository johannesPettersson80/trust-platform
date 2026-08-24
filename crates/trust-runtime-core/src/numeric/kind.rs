use crate::value::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumericKind {
    SInt,
    Int,
    DInt,
    LInt,
    USInt,
    UInt,
    UDInt,
    ULInt,
    Real,
    LReal,
}

pub fn numeric_kind(value: &Value) -> Option<NumericKind> {
    match value {
        Value::SInt(_) => Some(NumericKind::SInt),
        Value::Int(_) => Some(NumericKind::Int),
        Value::DInt(_) => Some(NumericKind::DInt),
        Value::LInt(_) => Some(NumericKind::LInt),
        Value::USInt(_) => Some(NumericKind::USInt),
        Value::UInt(_) => Some(NumericKind::UInt),
        Value::UDInt(_) => Some(NumericKind::UDInt),
        Value::ULInt(_) => Some(NumericKind::ULInt),
        Value::Real(_) => Some(NumericKind::Real),
        Value::LReal(_) => Some(NumericKind::LReal),
        _ => None,
    }
}

pub fn wider_numeric(left: NumericKind, right: NumericKind) -> Option<NumericKind> {
    if left == right {
        return Some(left);
    }
    if is_accuracy_preserving_widening(left, right) {
        return Some(left);
    }
    if is_accuracy_preserving_widening(right, left) {
        return Some(right);
    }
    None
}

pub(super) fn is_accuracy_preserving_widening(target: NumericKind, source: NumericKind) -> bool {
    matches!(
        (target, source),
        (NumericKind::Int, NumericKind::SInt)
            | (NumericKind::DInt, NumericKind::SInt | NumericKind::Int)
            | (
                NumericKind::LInt,
                NumericKind::SInt | NumericKind::Int | NumericKind::DInt
            )
            | (NumericKind::UInt, NumericKind::USInt)
            | (NumericKind::UDInt, NumericKind::USInt | NumericKind::UInt)
            | (
                NumericKind::ULInt,
                NumericKind::USInt | NumericKind::UInt | NumericKind::UDInt
            )
            | (NumericKind::Real, NumericKind::SInt | NumericKind::Int)
            | (
                NumericKind::LReal,
                NumericKind::SInt | NumericKind::Int | NumericKind::DInt | NumericKind::Real
            )
    )
}
