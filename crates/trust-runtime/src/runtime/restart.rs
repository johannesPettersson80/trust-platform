//! Regression corpus for the installed common-engine lifecycle.
#[cfg(test)]
use crate::{error::RuntimeError, value::Value, RestartMode};
#[cfg(test)]
use smol_str::SmolStr;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::TestHarness;
    use crate::value::{ArrayValue, EnumValue, StructValue};
    use crate::RetainSnapshot;
    use std::sync::Arc;

    #[test]
    fn restart_store_failure_preserves_live_state_and_queued_forces() {
        struct FailedStore;
        impl crate::retain::RetainStore for FailedStore {
            fn load(&self) -> Result<crate::RetainSnapshot, RuntimeError> {
                Err(RuntimeError::RetainStore("unavailable".into()))
            }
            fn store(&self, _: &crate::RetainSnapshot) -> Result<(), RuntimeError> {
                Ok(())
            }
        }
        let mut harness = TestHarness::from_source("FUNCTION_BLOCK CounterFb VAR count : INT; END_VAR END_FUNCTION_BLOCK VAR_GLOBAL g_fb : CounterFb; END_VAR PROGRAM Main END_PROGRAM").unwrap();
        harness
            .runtime_mut()
            .write_named_path(
                "g_fb",
                &[crate::value::RefSegment::Field("count".into())],
                Value::Int(7),
            )
            .unwrap();
        let Some(Value::Instance(instance)) = harness.runtime().storage().get_global("g_fb") else {
            panic!("FB")
        };
        let instance = *instance;
        harness.runtime_mut().cycle_counter = 9;
        let debug = harness.runtime_mut().enable_debug();
        debug.force_instance(instance, "count", Value::Int(99));
        harness
            .runtime_mut()
            .set_retain_store(Some(Box::new(FailedStore)), None);
        let error = harness
            .runtime_mut()
            .restart_with_retain_store(RestartMode::Warm)
            .unwrap_err();
        assert!(matches!(error, RuntimeError::RetainStore(_)));
        assert_eq!(harness.runtime().cycle_counter, 9);
        assert_eq!(
            harness
                .runtime()
                .storage()
                .get_instance_var(instance, "count"),
            Some(&Value::Int(7))
        );
        let cycle = harness.cycle();
        assert!(
            cycle.errors.is_empty(),
            "old runtime must remain executable"
        );
        assert_eq!(
            harness
                .runtime()
                .storage()
                .get_instance_var(instance, "count"),
            Some(&Value::Int(99))
        );
        assert_eq!(harness.runtime().cycle_counter, 10);
    }

    #[test]
    fn apply_retain_snapshot_canonicalizes_legacy_enum_type_name() {
        let source = r#"
TYPE Solo : (S0, S1)
END_TYPE

VAR_GLOBAL RETAIN
    state : Solo := S0;
END_VAR

PROGRAM Main
VAR
    matched : BOOL := FALSE;
END_VAR
matched := state = S1;
END_PROGRAM
"#;

        let mut harness = TestHarness::from_source(source).expect("compile harness");
        harness.cycle();
        let state_name = harness
            .runtime()
            .storage()
            .globals()
            .keys()
            .find(|name| name.as_str().eq_ignore_ascii_case("state"))
            .cloned()
            .expect("state global key");
        let mut snapshot = RetainSnapshot::default();
        snapshot.insert(
            state_name,
            Value::Enum(Box::new(EnumValue::from_canonical_parts(
                "SOLO".into(),
                "S1".into(),
                1,
            ))),
        );

        harness
            .runtime_mut()
            .apply_retain_snapshot(&snapshot)
            .expect("apply retained enum");
        let result = harness.cycle();
        assert!(
            result.errors.is_empty(),
            "unexpected runtime errors: {:?}",
            result.errors
        );

        assert_eq!(harness.get_output("matched"), Some(Value::Bool(true)));
        let Some(Value::Enum(value)) = harness.get_output("state") else {
            panic!("expected retained enum state");
        };
        assert_eq!(value.type_name().as_str(), "Solo");
        assert_eq!(value.variant_name().as_str(), "S1");
    }

    #[test]
    fn apply_retain_snapshot_canonicalizes_nested_enum_in_struct() {
        let source = r#"
TYPE
    Solo : (S0, S1);
    Holder : STRUCT
        state : Solo;
    END_STRUCT;
END_TYPE

VAR_GLOBAL RETAIN
    retained_holder : Holder;
END_VAR

PROGRAM Main
END_PROGRAM
"#;

        let mut harness = TestHarness::from_source(source).expect("compile harness");
        let holder_name = harness
            .runtime()
            .storage()
            .globals()
            .keys()
            .find(|name| name.as_str().eq_ignore_ascii_case("retained_holder"))
            .cloned()
            .expect("holder global key");
        let fields = [(
            SmolStr::new("STATE"),
            Value::Enum(Box::new(EnumValue::from_canonical_parts(
                "SOLO".into(),
                "s1".into(),
                1,
            ))),
        )]
        .into_iter()
        .collect();
        let mut snapshot = RetainSnapshot::default();
        snapshot.insert(
            holder_name,
            Value::Struct(Arc::new(StructValue::from_untyped_parts(
                "HOLDER".into(),
                fields,
            ))),
        );

        harness
            .runtime_mut()
            .apply_retain_snapshot(&snapshot)
            .expect("apply retained struct");

        let Some(Value::Struct(holder)) = harness.get_output("retained_holder") else {
            panic!("expected retained holder");
        };
        assert_eq!(holder.type_name().as_str(), "Holder");
        let Some(Value::Enum(state)) = holder.fields().get("state") else {
            panic!("expected retained holder.state enum");
        };
        assert_eq!(state.type_name().as_str(), "Solo");
        assert_eq!(state.variant_name().as_str(), "S1");
    }

    #[test]
    fn apply_retain_snapshot_rejects_struct_field_type_drift() {
        let source = r#"
TYPE Holder : STRUCT
    count : INT;
END_STRUCT;
END_TYPE

VAR_GLOBAL RETAIN
    retained_holder : Holder;
END_VAR

PROGRAM Main
END_PROGRAM
"#;

        let mut harness = TestHarness::from_source(source).expect("compile harness");
        let holder_name = harness
            .runtime()
            .storage()
            .globals()
            .keys()
            .find(|name| name.as_str().eq_ignore_ascii_case("retained_holder"))
            .cloned()
            .expect("holder global key");
        let mut snapshot = RetainSnapshot::default();
        snapshot.insert(
            holder_name,
            Value::Struct(Arc::new(StructValue::from_untyped_parts(
                "Holder".into(),
                [(SmolStr::new("count"), Value::Bool(true))]
                    .into_iter()
                    .collect(),
            ))),
        );

        let error = harness
            .runtime_mut()
            .apply_retain_snapshot(&snapshot)
            .expect_err("struct field type drift must fail retain apply");
        assert!(format!("{error}").contains("invalid retained value"));
    }

    #[test]
    fn apply_retain_snapshot_canonicalizes_array_of_struct_and_rejects_bad_element() {
        let source = r#"
TYPE
    Point : STRUCT
        x : INT;
    END_STRUCT;
    Points : ARRAY[1..2] OF Point;
END_TYPE

VAR_GLOBAL RETAIN
    retained_points : Points;
END_VAR

PROGRAM Main
END_PROGRAM
"#;

        let mut harness = TestHarness::from_source(source).expect("compile harness");
        let points_name = harness
            .runtime()
            .storage()
            .globals()
            .keys()
            .find(|name| name.as_str().eq_ignore_ascii_case("retained_points"))
            .cloned()
            .expect("points global key");
        let point = |field_name: &str, value: Value| {
            Value::Struct(Arc::new(StructValue::from_untyped_parts(
                "POINT".into(),
                [(SmolStr::new(field_name), value)].into_iter().collect(),
            )))
        };
        let mut snapshot = RetainSnapshot::default();
        snapshot.insert(
            points_name.clone(),
            Value::Array(Box::new(
                ArrayValue::from_untyped_parts(
                    vec![point("X", Value::Int(1)), point("X", Value::Int(2))],
                    vec![(1, 2)],
                )
                .expect("raw retained point array"),
            )),
        );

        harness
            .runtime_mut()
            .apply_retain_snapshot(&snapshot)
            .expect("apply retained array of struct");

        let Some(Value::Array(points)) = harness.get_output("retained_points") else {
            panic!("expected retained points array");
        };
        assert_eq!(points.dimensions(), &[(1, 2)]);
        for element in points.elements() {
            let Value::Struct(point) = element else {
                panic!("expected retained point struct");
            };
            assert_eq!(point.type_name().as_str(), "Point");
            assert!(point.fields().contains_key("x"));
        }

        let mut bad_harness = TestHarness::from_source(source).expect("compile harness");
        let mut bad_snapshot = RetainSnapshot::default();
        bad_snapshot.insert(
            points_name,
            Value::Array(Box::new(
                ArrayValue::from_untyped_parts(
                    vec![point("X", Value::Int(1)), Value::Bool(false)],
                    vec![(1, 2)],
                )
                .expect("raw retained bad point array"),
            )),
        );
        let error = bad_harness
            .runtime_mut()
            .apply_retain_snapshot(&bad_snapshot)
            .expect_err("array element type drift must fail retain apply");
        assert!(format!("{error}").contains("invalid retained value"));
    }
}
