//! Value formatting and primitive type mapping.
//! - format_value: format runtime values for DAP
//! - value_type_name/type_id_for_value: primitive mapping

use trust_hir::TypeId;
use trust_runtime::value::Value as RuntimeValue;

fn primitive_type_info(value: &RuntimeValue) -> Option<(&'static str, TypeId)> {
    match value {
        RuntimeValue::Bool(_) => Some(("BOOL", TypeId::BOOL)),
        RuntimeValue::SInt(_) => Some(("SINT", TypeId::SINT)),
        RuntimeValue::Int(_) => Some(("INT", TypeId::INT)),
        RuntimeValue::DInt(_) => Some(("DINT", TypeId::DINT)),
        RuntimeValue::LInt(_) => Some(("LINT", TypeId::LINT)),
        RuntimeValue::USInt(_) => Some(("USINT", TypeId::USINT)),
        RuntimeValue::UInt(_) => Some(("UINT", TypeId::UINT)),
        RuntimeValue::UDInt(_) => Some(("UDINT", TypeId::UDINT)),
        RuntimeValue::ULInt(_) => Some(("ULINT", TypeId::ULINT)),
        RuntimeValue::Real(_) => Some(("REAL", TypeId::REAL)),
        RuntimeValue::LReal(_) => Some(("LREAL", TypeId::LREAL)),
        RuntimeValue::Byte(_) => Some(("BYTE", TypeId::BYTE)),
        RuntimeValue::Word(_) => Some(("WORD", TypeId::WORD)),
        RuntimeValue::DWord(_) => Some(("DWORD", TypeId::DWORD)),
        RuntimeValue::LWord(_) => Some(("LWORD", TypeId::LWORD)),
        RuntimeValue::Time(_) => Some(("TIME", TypeId::TIME)),
        RuntimeValue::LTime(_) => Some(("LTIME", TypeId::LTIME)),
        RuntimeValue::Date(_) => Some(("DATE", TypeId::DATE)),
        RuntimeValue::LDate(_) => Some(("LDATE", TypeId::LDATE)),
        RuntimeValue::Tod(_) => Some(("TOD", TypeId::TOD)),
        RuntimeValue::LTod(_) => Some(("LTOD", TypeId::LTOD)),
        RuntimeValue::Dt(_) => Some(("DT", TypeId::DT)),
        RuntimeValue::Ldt(_) => Some(("LDT", TypeId::LDT)),
        RuntimeValue::String(_) => Some(("STRING", TypeId::STRING)),
        RuntimeValue::WString(_) => Some(("WSTRING", TypeId::WSTRING)),
        RuntimeValue::Char(_) => Some(("CHAR", TypeId::CHAR)),
        RuntimeValue::WChar(_) => Some(("WCHAR", TypeId::WCHAR)),
        _ => None,
    }
}

pub(in crate::adapter) fn value_type_name(value: &RuntimeValue) -> Option<String> {
    if let Some((name, _)) = primitive_type_info(value) {
        return Some(name.to_string());
    }
    let type_name = match value {
        RuntimeValue::Array(_) => "ARRAY",
        RuntimeValue::Struct(value) => return Some(value.type_name().to_string()),
        RuntimeValue::Enum(value) => return Some(value.type_name().to_string()),
        RuntimeValue::Reference(_) => "REF",
        RuntimeValue::Instance(_) => "INSTANCE",
        RuntimeValue::Null => "NULL",
        _ => return None,
    };
    Some(type_name.to_string())
}

pub(in crate::adapter) fn format_value(value: &RuntimeValue) -> String {
    match value {
        RuntimeValue::Bool(value) => {
            if *value {
                "TRUE".to_string()
            } else {
                "FALSE".to_string()
            }
        }
        RuntimeValue::String(value) => value.to_string(),
        RuntimeValue::WString(value) => value.clone(),
        RuntimeValue::Char(value) => (*value as char).to_string(),
        RuntimeValue::WChar(value) => char::from_u32((*value).into()).unwrap_or('?').to_string(),
        RuntimeValue::Array(value) => format!("[{}]", value.elements().len()),
        RuntimeValue::Struct(value) => format!("{} {{...}}", value.type_name()),
        RuntimeValue::Enum(value) => format!("{}::{}", value.type_name(), value.variant_name()),
        RuntimeValue::Reference(Some(_)) => "REF".to_string(),
        RuntimeValue::Reference(None) => "NULL_REF".to_string(),
        RuntimeValue::Instance(value) => format!("Instance({})", value.0),
        RuntimeValue::Null => "NULL".to_string(),
        _ => format!("{value:?}"),
    }
}

pub(in crate::adapter) fn type_id_for_value(value: &RuntimeValue) -> Option<TypeId> {
    primitive_type_info(value).map(|(_, type_id)| type_id)
}
