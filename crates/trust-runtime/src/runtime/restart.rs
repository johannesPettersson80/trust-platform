//! Runtime restart and retention.

#![allow(missing_docs)]

use indexmap::IndexMap;
use smol_str::SmolStr;
use std::sync::Arc;
use trust_hir::Type;

use crate::error;
use crate::task::TaskState;
use crate::value::{ArrayValue, Duration, EnumValue, StructValue, Value};

use super::core::Runtime;
use super::types::{GlobalInitValue, RestartMode, RetainPolicy, RetainSnapshot};

impl Runtime {
    /// Restart the runtime in the given mode (cold or warm).
    pub fn restart(&mut self, mode: RestartMode) -> Result<(), error::RuntimeError> {
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
            }
        }

        // Compiled runtime metadata keeps ValueRef targets for program/FB instances.
        // Preserve those references across both cold and warm restarts by recreating
        // instances from a stable id sequence before execution resumes.
        self.storage.reset_runtime_values(true);

        for (name, meta) in globals {
            let keep = matches!(mode, RestartMode::Warm) && retain_on_warm(meta.retain);
            if keep {
                if let Some(value) = retained.get(&name) {
                    self.storage.set_global(name.clone(), value.clone());
                    continue;
                }
            }
            match meta.init {
                GlobalInitValue::Value(value) => {
                    self.storage.set_global(name.clone(), value);
                }
                GlobalInitValue::FunctionBlock { type_name } => {
                    let key = SmolStr::new(type_name.to_ascii_uppercase());
                    let fb = self
                        .function_blocks
                        .get(&key)
                        .ok_or(error::RuntimeError::UndefinedFunctionBlock(type_name))?;
                    let instance_id = crate::instance::create_fb_instance(
                        &mut self.storage,
                        &self.registry,
                        &self.profile,
                        &self.classes,
                        &self.function_blocks,
                        &self.functions,
                        &self.stdlib,
                        &self.initializer_catalog,
                        fb,
                    )?;
                    self.storage
                        .set_global(name.clone(), Value::Instance(instance_id));
                }
                GlobalInitValue::Class { type_name } => {
                    let key = SmolStr::new(type_name.to_ascii_uppercase());
                    let class_def = self
                        .classes
                        .get(&key)
                        .ok_or(error::RuntimeError::TypeMismatch)?;
                    let instance_id = crate::instance::create_class_instance(
                        &mut self.storage,
                        &self.registry,
                        &self.profile,
                        &self.classes,
                        &self.function_blocks,
                        &self.functions,
                        &self.stdlib,
                        &self.initializer_catalog,
                        class_def,
                    )?;
                    self.storage
                        .set_global(name.clone(), Value::Instance(instance_id));
                }
            }
        }

        let programs = self.programs.values().cloned().collect::<Vec<_>>();
        for program in programs {
            let instance_id = crate::instance::create_program_instance(
                &mut self.storage,
                &self.registry,
                &self.profile,
                &self.classes,
                &self.function_blocks,
                &self.functions,
                &self.stdlib,
                &self.initializer_catalog,
                &program,
            )?;
            self.storage
                .set_global(program.name.clone(), Value::Instance(instance_id));
        }
        for (program_name, var_name, value) in retained_program_vars {
            let Some(Value::Instance(id)) = self.storage.get_global(program_name.as_ref()) else {
                continue;
            };
            self.storage.set_instance_var(*id, var_name, value);
        }

        self.current_time = Duration::ZERO;
        for state in self.task_state.values_mut() {
            *state = TaskState::new(self.current_time);
        }
        self.faults.clear();
        self.cycle_counter = 0;
        Ok(())
    }

    /// Capture retained global values that can be preserved across reloads.
    #[must_use]
    pub fn retain_snapshot(&self) -> RetainSnapshot {
        let mut snapshot = RetainSnapshot::default();
        for (name, meta) in &self.globals {
            if !retain_on_warm(meta.retain) {
                continue;
            }
            let Some(value) = self.storage.get_global(name.as_ref()) else {
                continue;
            };
            if value_is_retainable(value) {
                snapshot.insert(name.clone(), value.clone());
            }
        }
        snapshot
    }

    /// Apply a retained snapshot to the current runtime.
    pub fn apply_retain_snapshot(
        &mut self,
        snapshot: &RetainSnapshot,
    ) -> Result<(), error::RuntimeError> {
        for (name, value) in snapshot.values() {
            let Some(meta) = self.globals.get(name) else {
                continue;
            };
            if retain_on_warm(meta.retain) && value_is_retainable(value) {
                let value = canonicalize_retained_value(&self.registry, meta.type_id, value)
                    .map_err(|error| {
                        error::RuntimeError::RetainStore(
                            format!("invalid retained value for global '{name}': {error}").into(),
                        )
                    })?;
                self.storage.set_global(name.clone(), value);
            }
        }
        Ok(())
    }
}

fn canonicalize_retained_value(
    registry: &trust_hir::types::TypeRegistry,
    type_id: trust_hir::TypeId,
    value: &Value,
) -> Result<Value, error::RuntimeError> {
    let Some(ty) = registry.get(type_id) else {
        return Ok(value.clone());
    };
    match ty {
        Type::Alias { target, .. } => canonicalize_retained_value(registry, *target, value),
        Type::Enum { .. } => canonicalize_retained_enum(registry, type_id, value),
        Type::Array { element, .. } => {
            let Value::Array(array) = value else {
                return Err(error::RuntimeError::RetainStore(
                    format!(
                        "retained value kind does not match declared array type id {}",
                        type_id.0
                    )
                    .into(),
                ));
            };
            let elements = array
                .elements()
                .iter()
                .map(|element_value| canonicalize_retained_value(registry, *element, element_value))
                .collect::<Result<Vec<_>, _>>()?;
            let array = ArrayValue::from_serialized_parts(
                registry,
                type_id,
                array.dimensions().to_vec(),
                elements,
            )
            .map_err(retain_value_error)?;
            Ok(Value::Array(Box::new(array)))
        }
        Type::Struct { fields, .. } => {
            let Value::Struct(struct_value) = value else {
                return Err(error::RuntimeError::RetainStore(
                    format!(
                        "retained value kind does not match declared struct type id {}",
                        type_id.0
                    )
                    .into(),
                ));
            };
            let mut values = IndexMap::new();
            for (name, field_value) in struct_value.fields() {
                let value = if let Some(field) = fields
                    .iter()
                    .find(|field| field.name.eq_ignore_ascii_case(name.as_str()))
                {
                    canonicalize_retained_value(registry, field.type_id, field_value)?
                } else {
                    field_value.clone()
                };
                values.insert(name.clone(), value);
            }
            let value = StructValue::new(registry, type_id, values).map_err(retain_value_error)?;
            Ok(Value::Struct(Arc::new(value)))
        }
        Type::Union { variants, .. } => {
            let Value::Struct(struct_value) = value else {
                return Err(error::RuntimeError::RetainStore(
                    format!(
                        "retained value kind does not match declared union type id {}",
                        type_id.0
                    )
                    .into(),
                ));
            };
            let mut values = IndexMap::new();
            for (name, variant_value) in struct_value.fields() {
                let value = if let Some(variant) = variants
                    .iter()
                    .find(|variant| variant.name.eq_ignore_ascii_case(name.as_str()))
                {
                    canonicalize_retained_value(registry, variant.type_id, variant_value)?
                } else {
                    variant_value.clone()
                };
                values.insert(name.clone(), value);
            }
            let value = StructValue::new(registry, type_id, values).map_err(retain_value_error)?;
            Ok(Value::Struct(Arc::new(value)))
        }
        _ => Ok(value.clone()),
    }
}

fn canonicalize_retained_enum(
    registry: &trust_hir::types::TypeRegistry,
    declared_type_id: trust_hir::TypeId,
    value: &Value,
) -> Result<Value, error::RuntimeError> {
    let Value::Enum(enum_value) = value else {
        return Err(error::RuntimeError::RetainStore(
            format!(
                "retained value kind does not match declared enum type id {}",
                declared_type_id.0
            )
            .into(),
        ));
    };
    let retained = EnumValue::from_serialized_parts(
        registry,
        enum_value.type_name().as_str(),
        enum_value.variant_name().as_str(),
        enum_value.numeric_value(),
    )
    .map_err(retain_enum_error)?;
    let declared = EnumValue::new_with_numeric(
        registry,
        declared_type_id,
        retained.variant_name().as_str(),
        retained.numeric_value(),
    )
    .map_err(retain_enum_error)?;
    if retained.type_name() != declared.type_name() {
        return Err(error::RuntimeError::RetainStore(
            format!(
                "retained enum type '{}' does not match declared type '{}'",
                retained.type_name(),
                declared.type_name()
            )
            .into(),
        ));
    }
    Ok(Value::Enum(Box::new(declared)))
}

fn retain_enum_error(error: crate::value::EnumValueError) -> error::RuntimeError {
    error::RuntimeError::RetainStore(format!("invalid retained enum value: {error}").into())
}

fn retain_value_error(error: crate::value::ValueConstructionError) -> error::RuntimeError {
    error::RuntimeError::RetainStore(format!("invalid retained value: {error}").into())
}

fn retain_on_warm(policy: RetainPolicy) -> bool {
    matches!(policy, RetainPolicy::Retain | RetainPolicy::Persistent)
}

fn value_is_retainable(value: &Value) -> bool {
    match value {
        Value::Array(array) => array.elements().iter().all(value_is_retainable),
        Value::Struct(value) => value.fields().values().all(value_is_retainable),
        Value::Reference(_) | Value::Instance(_) => false,
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::TestHarness;

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
