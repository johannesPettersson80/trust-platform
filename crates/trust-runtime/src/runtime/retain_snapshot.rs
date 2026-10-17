//! Retain key helpers of the host tests. The text of committed retain migration events is shared
//! runtime code (`trust_runtime_services::events::retain_apply_event`).
#[cfg(test)]
use crate::value::Value;
#[cfg(test)]
use trust_runtime_core::retain::{retain_on_warm, value_is_retainable};
#[cfg(test)]
use {crate::RetainPolicy, indexmap::IndexMap, smol_str::SmolStr};
#[cfg(test)]
const PROGRAM_RETAIN_KEY_PREFIX: &str = "@program/";
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
