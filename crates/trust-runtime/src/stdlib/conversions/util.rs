use crate::value::Value;
use trust_hir::TypeId;

pub(super) fn is_integer_type(ty: TypeId) -> bool {
    matches!(
        ty,
        TypeId::SINT
            | TypeId::INT
            | TypeId::DINT
            | TypeId::LINT
            | TypeId::USINT
            | TypeId::UINT
            | TypeId::UDINT
            | TypeId::ULINT
    )
}

pub(super) fn is_signed_int_type(ty: TypeId) -> bool {
    matches!(ty, TypeId::SINT | TypeId::INT | TypeId::DINT | TypeId::LINT)
}

pub(super) fn is_unsigned_int_type(ty: TypeId) -> bool {
    matches!(
        ty,
        TypeId::USINT | TypeId::UINT | TypeId::UDINT | TypeId::ULINT
    )
}

pub(super) fn is_bit_string_type(ty: TypeId) -> bool {
    matches!(
        ty,
        TypeId::BOOL | TypeId::BYTE | TypeId::WORD | TypeId::DWORD | TypeId::LWORD
    )
}

pub(super) fn is_conversion_allowed(src: TypeId, dst: TypeId) -> bool {
    if src == dst {
        return true;
    }

    if is_numeric_type(src) && is_numeric_type(dst) {
        return true;
    }

    if matches!(
        src,
        TypeId::BYTE | TypeId::WORD | TypeId::DWORD | TypeId::LWORD
    ) && matches!(
        dst,
        TypeId::BYTE | TypeId::WORD | TypeId::DWORD | TypeId::LWORD
    ) {
        return true;
    }

    if matches!(
        src,
        TypeId::BOOL | TypeId::BYTE | TypeId::WORD | TypeId::DWORD | TypeId::LWORD
    ) && matches!(
        dst,
        TypeId::SINT
            | TypeId::INT
            | TypeId::DINT
            | TypeId::LINT
            | TypeId::USINT
            | TypeId::UINT
            | TypeId::UDINT
            | TypeId::ULINT
    ) {
        return true;
    }

    if src == TypeId::DWORD && dst == TypeId::REAL {
        return true;
    }
    if src == TypeId::LWORD && dst == TypeId::LREAL {
        return true;
    }

    if matches!(
        dst,
        TypeId::BYTE | TypeId::WORD | TypeId::DWORD | TypeId::LWORD
    ) && matches!(
        src,
        TypeId::SINT
            | TypeId::INT
            | TypeId::DINT
            | TypeId::LINT
            | TypeId::USINT
            | TypeId::UINT
            | TypeId::UDINT
            | TypeId::ULINT
    ) {
        return true;
    }

    if src == TypeId::REAL && dst == TypeId::DWORD {
        return true;
    }
    if src == TypeId::LREAL && dst == TypeId::LWORD {
        return true;
    }

    if src == TypeId::LTIME && dst == TypeId::TIME {
        return true;
    }
    if src == TypeId::TIME && dst == TypeId::LTIME {
        return true;
    }
    if src == TypeId::TIME && dst == TypeId::DWORD {
        return true;
    }
    if src == TypeId::DWORD && dst == TypeId::TIME {
        return true;
    }
    if src == TypeId::LDT && dst == TypeId::DT {
        return true;
    }
    if src == TypeId::LDT && dst == TypeId::DATE {
        return true;
    }
    if src == TypeId::LDT && dst == TypeId::LTOD {
        return true;
    }
    if src == TypeId::LDT && dst == TypeId::TOD {
        return true;
    }
    if src == TypeId::DT && dst == TypeId::LDT {
        return true;
    }
    if src == TypeId::DT && dst == TypeId::DATE {
        return true;
    }
    if src == TypeId::DT && dst == TypeId::LTOD {
        return true;
    }
    if src == TypeId::DT && dst == TypeId::TOD {
        return true;
    }
    if src == TypeId::LTOD && dst == TypeId::TOD {
        return true;
    }
    if src == TypeId::TOD && dst == TypeId::LTOD {
        return true;
    }

    let src = normalize_string_type_id(src);
    let dst = normalize_string_type_id(dst);

    if matches!(dst, TypeId::STRING | TypeId::WSTRING)
        && (is_numeric_type(src)
            || matches!(
                src,
                TypeId::BYTE | TypeId::WORD | TypeId::DWORD | TypeId::LWORD
            ))
    {
        return true;
    }
    if matches!(src, TypeId::STRING | TypeId::WSTRING) && is_numeric_type(dst) {
        return true;
    }
    if matches!(dst, TypeId::CHAR | TypeId::WCHAR)
        && (is_numeric_type(src)
            || matches!(
                src,
                TypeId::BYTE | TypeId::WORD | TypeId::DWORD | TypeId::LWORD
            ))
    {
        return true;
    }
    if matches!(src, TypeId::CHAR | TypeId::WCHAR)
        && (is_numeric_type(dst)
            || matches!(
                dst,
                TypeId::BYTE | TypeId::WORD | TypeId::DWORD | TypeId::LWORD
            ))
    {
        return true;
    }

    if src == TypeId::WSTRING && matches!(dst, TypeId::STRING | TypeId::WCHAR) {
        return true;
    }
    if src == TypeId::STRING && matches!(dst, TypeId::WSTRING | TypeId::CHAR) {
        return true;
    }
    if src == TypeId::WCHAR && matches!(dst, TypeId::WSTRING | TypeId::CHAR) {
        return true;
    }
    if src == TypeId::CHAR && matches!(dst, TypeId::STRING | TypeId::WCHAR) {
        return true;
    }

    false
}

fn is_numeric_type(ty: TypeId) -> bool {
    matches!(
        ty,
        TypeId::SINT
            | TypeId::INT
            | TypeId::DINT
            | TypeId::LINT
            | TypeId::USINT
            | TypeId::UINT
            | TypeId::UDINT
            | TypeId::ULINT
            | TypeId::REAL
            | TypeId::LREAL
    )
}

fn normalize_string_type_id(ty: TypeId) -> TypeId {
    match ty {
        TypeId::STRING => TypeId::STRING,
        TypeId::WSTRING => TypeId::WSTRING,
        _ => ty,
    }
}

pub(super) fn value_type_id(value: &Value) -> Option<TypeId> {
    match value {
        Value::Bool(_) => Some(TypeId::BOOL),
        Value::SInt(_) => Some(TypeId::SINT),
        Value::Int(_) => Some(TypeId::INT),
        Value::DInt(_) => Some(TypeId::DINT),
        Value::LInt(_) => Some(TypeId::LINT),
        Value::USInt(_) => Some(TypeId::USINT),
        Value::UInt(_) => Some(TypeId::UINT),
        Value::UDInt(_) => Some(TypeId::UDINT),
        Value::ULInt(_) => Some(TypeId::ULINT),
        Value::Real(_) => Some(TypeId::REAL),
        Value::LReal(_) => Some(TypeId::LREAL),
        Value::Byte(_) => Some(TypeId::BYTE),
        Value::Word(_) => Some(TypeId::WORD),
        Value::DWord(_) => Some(TypeId::DWORD),
        Value::LWord(_) => Some(TypeId::LWORD),
        Value::Time(_) => Some(TypeId::TIME),
        Value::LTime(_) => Some(TypeId::LTIME),
        Value::Date(_) => Some(TypeId::DATE),
        Value::LDate(_) => Some(TypeId::LDATE),
        Value::Tod(_) => Some(TypeId::TOD),
        Value::LTod(_) => Some(TypeId::LTOD),
        Value::Dt(_) => Some(TypeId::DT),
        Value::Ldt(_) => Some(TypeId::LDT),
        Value::String(_) => Some(TypeId::STRING),
        Value::WString(_) => Some(TypeId::WSTRING),
        Value::Char(_) => Some(TypeId::CHAR),
        Value::WChar(_) => Some(TypeId::WCHAR),
        _ => None,
    }
}
