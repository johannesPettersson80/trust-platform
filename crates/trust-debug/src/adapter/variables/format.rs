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
        RuntimeValue::SInt(value) => value.to_string(),
        RuntimeValue::Int(value) => value.to_string(),
        RuntimeValue::DInt(value) => value.to_string(),
        RuntimeValue::LInt(value) => value.to_string(),
        RuntimeValue::USInt(value) => value.to_string(),
        RuntimeValue::UInt(value) => value.to_string(),
        RuntimeValue::UDInt(value) => value.to_string(),
        RuntimeValue::ULInt(value) => value.to_string(),
        RuntimeValue::Real(value) => value.to_string(),
        RuntimeValue::LReal(value) => value.to_string(),
        RuntimeValue::Byte(value) => value.to_string(),
        RuntimeValue::Word(value) => value.to_string(),
        RuntimeValue::DWord(value) => value.to_string(),
        RuntimeValue::LWord(value) => value.to_string(),
        RuntimeValue::Time(value) => format_duration("T", value.as_nanos()),
        RuntimeValue::LTime(value) => format_duration("LT", value.as_nanos()),
        RuntimeValue::Date(value) => format!("D#{}", value.ticks()),
        RuntimeValue::LDate(value) => format!("LD#{}", value.nanos()),
        RuntimeValue::Tod(value) => format!("TOD#{}", value.ticks()),
        RuntimeValue::LTod(value) => format!("LTOD#{}", value.nanos()),
        RuntimeValue::Dt(value) => format!("DT#{}", value.ticks()),
        RuntimeValue::Ldt(value) => format!("LDT#{}", value.nanos()),
        RuntimeValue::String(value) => value.to_string(),
        RuntimeValue::WString(value) => value.clone(),
        RuntimeValue::Char(value) => (*value as char).to_string(),
        RuntimeValue::WChar(value) => char::from_u32((*value).into()).unwrap_or('?').to_string(),
        RuntimeValue::Array(value) => format!("[{}]", value.elements().len()),
        RuntimeValue::Struct(value) => format!("{} {{...}}", value.type_name()),
        RuntimeValue::Enum(value) => format!("{}::{}", value.type_name(), value.variant_name()),
        RuntimeValue::Reference(Some(_)) => "REF".to_string(),
        RuntimeValue::Reference(None) => "NULL_REF".to_string(),
        RuntimeValue::Instance(_) => "Instance".to_string(),
        RuntimeValue::Null => "NULL".to_string(),
    }
}

fn format_duration(prefix: &str, nanos: i64) -> String {
    if nanos % 1_000_000_000 == 0 {
        return format!("{prefix}#{}s", nanos / 1_000_000_000);
    }
    if nanos % 1_000_000 == 0 {
        return format!("{prefix}#{}ms", nanos / 1_000_000);
    }
    if nanos % 1_000 == 0 {
        return format!("{prefix}#{}us", nanos / 1_000);
    }
    format!("{prefix}#{nanos}ns")
}

pub(in crate::adapter) fn type_id_for_value(value: &RuntimeValue) -> Option<TypeId> {
    primitive_type_info(value).map(|(_, type_id)| type_id)
}

#[cfg(test)]
mod tests {
    use trust_runtime::memory::InstanceId;
    use trust_runtime::value::{
        DateTimeValue, DateValue, Duration, LDateTimeValue, LDateValue, LTimeOfDayValue,
        TimeOfDayValue, Value as RuntimeValue,
    };

    use super::format_value;

    #[test]
    fn format_value_uses_user_facing_primitive_strings() {
        assert_eq!(format_value(&RuntimeValue::Int(1)), "1");
        assert_eq!(format_value(&RuntimeValue::Real(1.5)), "1.5");
        assert_eq!(format_value(&RuntimeValue::Word(16)), "16");
        assert_eq!(
            format_value(&RuntimeValue::Time(Duration::from_millis(250))),
            "T#250ms"
        );
        assert_eq!(
            format_value(&RuntimeValue::LTime(Duration::from_nanos(7))),
            "LT#7ns"
        );
        assert_eq!(format_value(&RuntimeValue::Date(DateValue::new(3))), "D#3");
        assert_eq!(
            format_value(&RuntimeValue::LDate(LDateValue::new(4))),
            "LD#4"
        );
        assert_eq!(
            format_value(&RuntimeValue::Tod(TimeOfDayValue::new(5))),
            "TOD#5"
        );
        assert_eq!(
            format_value(&RuntimeValue::LTod(LTimeOfDayValue::new(6))),
            "LTOD#6"
        );
        assert_eq!(
            format_value(&RuntimeValue::Dt(DateTimeValue::new(7))),
            "DT#7"
        );
        assert_eq!(
            format_value(&RuntimeValue::Ldt(LDateTimeValue::new(8))),
            "LDT#8"
        );
        assert_eq!(
            format_value(&RuntimeValue::Instance(InstanceId(7))),
            "Instance"
        );
    }
}
