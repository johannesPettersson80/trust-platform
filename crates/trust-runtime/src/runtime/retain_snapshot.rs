//! Host presentation of committed common-engine retain migration events.
use crate::value::Value;
#[cfg(test)]
use trust_runtime_core::retain::{retain_on_warm, value_is_retainable};
#[cfg(test)]
use {crate::RetainPolicy, indexmap::IndexMap, smol_str::SmolStr};
#[cfg(test)]
const PROGRAM_RETAIN_KEY_PREFIX: &str = "@program/";
fn retain_value_kind(value: &Value) -> &'static str {
    match value {
        Value::Bool(_) => "BOOL",
        Value::SInt(_) => "SINT",
        Value::Int(_) => "INT",
        Value::DInt(_) => "DINT",
        Value::LInt(_) => "LINT",
        Value::USInt(_) => "USINT",
        Value::UInt(_) => "UINT",
        Value::UDInt(_) => "UDINT",
        Value::ULInt(_) => "ULINT",
        Value::Real(_) => "REAL",
        Value::LReal(_) => "LREAL",
        Value::Byte(_) => "BYTE",
        Value::Word(_) => "WORD",
        Value::DWord(_) => "DWORD",
        Value::LWord(_) => "LWORD",
        Value::Time(_) => "TIME",
        Value::LTime(_) => "LTIME",
        Value::Date(_) => "DATE",
        Value::LDate(_) => "LDATE",
        Value::Tod(_) => "TOD",
        Value::LTod(_) => "LTOD",
        Value::Dt(_) => "DT",
        Value::Ldt(_) => "LDT",
        Value::String(_) => "STRING",
        Value::WString(_) => "WSTRING",
        Value::Char(_) => "CHAR",
        Value::WChar(_) => "WCHAR",
        Value::Array(_) => "ARRAY",
        Value::Struct(_) => "STRUCT",
        Value::Enum(_) => "ENUM",
        Value::Reference(_) => "REFERENCE",
        Value::Instance(_) => "INSTANCE",
        Value::Null => "NULL",
    }
}

pub(super) fn retain_migration_detail(before: &Value, after: &Value) -> String {
    if let (Value::Struct(before), Value::Struct(after)) = (before, after) {
        let mut changes = Vec::new();
        for name in before.fields().keys() {
            if !after
                .fields()
                .keys()
                .any(|after_name| after_name.eq_ignore_ascii_case(name.as_str()))
            {
                changes.push(format!("dropped field {name}"));
            }
        }
        for name in after.fields().keys() {
            if !before
                .fields()
                .keys()
                .any(|before_name| before_name.eq_ignore_ascii_case(name.as_str()))
            {
                changes.push(format!("added field {name}"));
            }
        }
        if !changes.is_empty() {
            return changes.join(", ");
        }
    }
    format!(
        "migrated retained value from {} to {}",
        retain_value_kind(before),
        retain_value_kind(after)
    )
}

#[cfg(test)]
fn program_retain_key(program: &str, variable: &str) -> SmolStr {
    SmolStr::new(format!("{PROGRAM_RETAIN_KEY_PREFIX}{program}/{variable}"))
}

#[cfg(test)]
fn parse_program_retain_key(key: &str) -> Option<(&str, &str)> {
    let remainder = key.strip_prefix(PROGRAM_RETAIN_KEY_PREFIX)?;
    let (program, variable) = remainder.rsplit_once('/')?;
    if program.is_empty() || variable.is_empty() {
        return None;
    }
    Some((program, variable))
}

#[cfg(test)]
mod contract_tests {
    use super::*;

    use std::sync::Arc;

    use crate::value::{ArrayValue, StructValue};

    #[test]
    fn program_retain_key_round_trips_qualified_program_path() {
        let key = program_retain_key("Cell/Motor", "Speed");
        assert_eq!(key, "@program/Cell/Motor/Speed");
        assert_eq!(
            parse_program_retain_key(key.as_str()),
            Some(("Cell/Motor", "Speed"))
        );

        for invalid in [
            "Cell/Motor/Speed",
            "@program/",
            "@program/Main",
            "@program//Speed",
            "@program/Main/",
        ] {
            assert_eq!(parse_program_retain_key(invalid), None, "{invalid}");
        }
    }

    #[test]
    fn warm_restart_retains_only_retain_and_persistent_policies() {
        assert!(retain_on_warm(RetainPolicy::Retain));
        assert!(retain_on_warm(RetainPolicy::Persistent));
        assert!(!retain_on_warm(RetainPolicy::NonRetain));
        assert!(!retain_on_warm(RetainPolicy::Unspecified));
    }

    #[test]
    fn retainability_rejects_reference_or_instance_at_any_aggregate_depth() {
        assert!(value_is_retainable(&Value::DInt(7)));
        assert!(!value_is_retainable(&Value::Reference(None)));
        assert!(!value_is_retainable(&Value::Instance(
            crate::memory::InstanceId(1)
        )));

        let scalar_array =
            ArrayValue::from_untyped_parts(vec![Value::DInt(1)], vec![(1, 1)]).unwrap();
        assert!(value_is_retainable(&Value::Array(Box::new(scalar_array))));
        let reference_array =
            ArrayValue::from_untyped_parts(vec![Value::Reference(None)], vec![(1, 1)]).unwrap();
        assert!(!value_is_retainable(&Value::Array(Box::new(
            reference_array
        ))));

        let scalar_struct = StructValue::from_untyped_parts(
            "State".into(),
            IndexMap::from([("Count".into(), Value::DInt(1))]),
        );
        assert!(value_is_retainable(&Value::Struct(Arc::new(scalar_struct))));
        let instance_struct = StructValue::from_untyped_parts(
            "State".into(),
            IndexMap::from([(
                "Owner".into(),
                Value::Instance(crate::memory::InstanceId(2)),
            )]),
        );
        assert!(!value_is_retainable(&Value::Struct(Arc::new(
            instance_struct
        ))));
    }
}
