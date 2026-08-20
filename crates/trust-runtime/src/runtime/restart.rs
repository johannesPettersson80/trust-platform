//! Runtime restart and retention.

#![allow(missing_docs)]

use indexmap::IndexMap;
use smol_str::SmolStr;

use crate::error;
use crate::memory::VariableStorage;
use crate::program_model::edge_phase_storage_name;
use crate::task::TaskState;
use crate::value::Value;

use super::core::Runtime;
use super::retain_snapshot::{retain_on_warm, value_is_retainable};
use super::types::{GlobalInitValue, RestartMode};

pub(super) struct PreparedRestart {
    pub(super) storage: VariableStorage,
}

impl Runtime {
    /// Restart the runtime in the given mode (cold or warm).
    pub fn restart(&mut self, mode: RestartMode) -> Result<(), error::RuntimeError> {
        let prepared = self.prepare_restart(mode)?;
        self.commit_restart(prepared);
        Ok(())
    }

    pub(super) fn prepare_restart(
        &self,
        mode: RestartMode,
    ) -> Result<PreparedRestart, error::RuntimeError> {
        let globals = self.globals.clone();
        let mut retained = IndexMap::new();
        let mut retained_program_vars = Vec::new();
        if matches!(mode, RestartMode::Warm) {
            for (name, meta) in &globals {
                if retain_on_warm(meta.retain) {
                    if let Some(value) = self.storage.get_global(name.as_ref()) {
                        retained.insert(name.clone(), value.clone());
                    }
                }
            }
            for program in self.programs.values() {
                let Some(Value::Instance(id)) = self.storage.get_global(program.name.as_ref())
                else {
                    continue;
                };
                for var in &program.vars {
                    if !retain_on_warm(var.retain) {
                        continue;
                    }
                    let Some(value) = self.storage.get_instance_var(*id, var.name.as_ref()) else {
                        continue;
                    };
                    if value_is_retainable(value) {
                        retained_program_vars.push((
                            program.name.clone(),
                            var.name.clone(),
                            value.clone(),
                        ));
                    }
                }
                if let Some(inputs) = self.edge_inputs.get(&program.name) {
                    for input in inputs {
                        let Some(var) = program
                            .vars
                            .iter()
                            .find(|var| var.name.eq_ignore_ascii_case(input.name.as_str()))
                        else {
                            continue;
                        };
                        if !retain_on_warm(var.retain) {
                            continue;
                        }
                        let phase_name =
                            edge_phase_storage_name(program.name.as_str(), input.name.as_str());
                        let Some(value) = self
                            .storage
                            .get_instance_var(*id, phase_name.as_str())
                            .cloned()
                        else {
                            continue;
                        };
                        if value_is_retainable(&value) {
                            retained_program_vars.push((program.name.clone(), phase_name, value));
                        }
                    }
                }
            }
        }

        // Compiled runtime metadata keeps ValueRef targets for program/FB instances.
        // Preserve those references across both cold and warm restarts by recreating
        // instances from a stable id sequence before execution resumes.
        let mut storage = self.storage.clone();
        storage.reset_runtime_values(true);

        for (name, meta) in globals {
            let keep = matches!(mode, RestartMode::Warm) && retain_on_warm(meta.retain);
            if keep {
                if let Some(value) = retained.get(&name) {
                    storage.set_global(name.clone(), value.clone());
                    continue;
                }
            }
            match meta.init {
                GlobalInitValue::Value(value) => {
                    storage.set_global(name.clone(), value);
                }
                GlobalInitValue::FunctionBlock { type_name } => {
                    let key = SmolStr::new(type_name.to_ascii_uppercase());
                    let fb = self
                        .function_blocks
                        .get(&key)
                        .ok_or(error::RuntimeError::UndefinedFunctionBlock(type_name))?;
                    let instance_id = crate::instance::create_fb_instance(
                        &mut storage,
                        &self.registry,
                        &self.profile,
                        &self.classes,
                        &self.function_blocks,
                        &self.functions,
                        &self.stdlib,
                        &self.initializer_catalog,
                        fb,
                    )?;
                    storage.set_global(name.clone(), Value::Instance(instance_id));
                }
                GlobalInitValue::Class { type_name } => {
                    let key = SmolStr::new(type_name.to_ascii_uppercase());
                    let class_def = self
                        .classes
                        .get(&key)
                        .ok_or(error::RuntimeError::TypeMismatch)?;
                    let instance_id = crate::instance::create_class_instance(
                        &mut storage,
                        &self.registry,
                        &self.profile,
                        &self.classes,
                        &self.function_blocks,
                        &self.functions,
                        &self.stdlib,
                        &self.initializer_catalog,
                        class_def,
                    )?;
                    storage.set_global(name.clone(), Value::Instance(instance_id));
                }
            }
        }

        let programs = self.programs.values().cloned().collect::<Vec<_>>();
        for program in programs {
            let instance_id = crate::instance::create_program_instance(
                &mut storage,
                &self.registry,
                &self.profile,
                &self.classes,
                &self.function_blocks,
                &self.functions,
                &self.stdlib,
                &self.initializer_catalog,
                &program,
            )?;
            storage.set_global(program.name.clone(), Value::Instance(instance_id));
        }
        for (program_name, var_name, value) in retained_program_vars {
            let Some(Value::Instance(id)) = storage.get_global(program_name.as_ref()) else {
                continue;
            };
            storage.set_instance_var(*id, var_name, value);
        }

        Ok(PreparedRestart { storage })
    }

    pub(super) fn commit_restart(&mut self, prepared: PreparedRestart) {
        if let Some(debug) = &self.debug {
            debug.clear_runtime_mutations();
        }
        self.storage = prepared.storage;
        self.openot_telemetry.reset_scan_state();
        for state in self.task_state.values_mut() {
            *state = TaskState::new(self.current_time);
        }
        self.faults.clear();
        self.cycle_counter = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::TestHarness;
    use crate::value::{ArrayValue, EnumValue, StructValue};
    use crate::RetainSnapshot;
    use std::sync::Arc;

    #[test]
    fn restart_instance_construction_failure_preserves_live_state() {
        let source = r#"
FUNCTION_BLOCK CounterFb
VAR
    count : INT := INT#0;
END_VAR
END_FUNCTION_BLOCK

VAR_GLOBAL
    g_fb : CounterFb;
END_VAR

PROGRAM Main
END_PROGRAM
"#;

        let mut harness = TestHarness::from_source(source).expect("compile harness");
        let instance_id = match harness.runtime().storage.get_global("g_fb") {
            Some(Value::Instance(id)) => *id,
            other => panic!("expected g_fb instance, got {other:?}"),
        };
        harness
            .runtime_mut()
            .storage
            .set_instance_var(instance_id, "count", Value::Int(7));
        harness.runtime_mut().cycle_counter = 9;
        let debug = harness.runtime_mut().enable_debug();
        debug.force_instance(instance_id, "count", Value::Int(99));

        let fb_key = harness
            .runtime()
            .function_blocks
            .keys()
            .find(|name| name.eq_ignore_ascii_case("CounterFb"))
            .cloned()
            .expect("CounterFb definition key");
        let fb = harness
            .runtime_mut()
            .function_blocks
            .shift_remove(&fb_key)
            .expect("remove CounterFb definition");

        let error = harness
            .runtime_mut()
            .restart(RestartMode::Warm)
            .expect_err("missing FB definition must reject restart");
        assert!(
            matches!(error, error::RuntimeError::UndefinedFunctionBlock(_)),
            "expected undefined FB restart failure, got {error:?}"
        );
        assert_eq!(harness.runtime().cycle_counter, 9);
        assert_eq!(
            harness
                .runtime()
                .storage
                .get_instance_var(instance_id, "count"),
            Some(&Value::Int(7)),
            "failed restart must preserve live instance storage"
        );

        harness.runtime_mut().function_blocks.insert(fb_key, fb);
        let cycle = harness.cycle();
        assert!(
            cycle.errors.is_empty(),
            "old runtime must remain executable"
        );
        assert_eq!(
            harness
                .runtime()
                .storage
                .get_instance_var(instance_id, "count"),
            Some(&Value::Int(99)),
            "failed restart must preserve queued debug forces"
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
            .globals
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
            .globals
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
            .globals
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
            .globals
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
