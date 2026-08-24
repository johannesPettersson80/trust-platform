//! Retained snapshot capture, validation, migration, and atomic application.

#![allow(missing_docs)]

use indexmap::IndexMap;
use smol_str::SmolStr;
use std::sync::Arc;
use trust_hir::types::InitializerId;
use trust_hir::Type;

use crate::error;
use crate::memory::VariableStorage;
use crate::program_model::InitializerCatalog;
use crate::stdlib::StandardLibrary;
use crate::value::{
    truncate_string_elements, ArrayValue, DateTimeProfile, EnumValue, StructValue, Value,
};

use super::core::Runtime;
use super::restart::PreparedRestart;
use super::types::{RetainPolicy, RetainSnapshot};

pub(super) struct PreparedRetainSnapshot {
    storage: VariableStorage,
    events: Vec<crate::debug::RuntimeEvent>,
}

const PROGRAM_RETAIN_KEY_PREFIX: &str = "@program/";

impl Runtime {
    /// Capture retained values that can be preserved across reloads.
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
        for program in self.programs.values() {
            let Some(Value::Instance(instance)) = self.storage.get_global(program.name.as_ref())
            else {
                continue;
            };
            for var in &program.vars {
                if !retain_on_warm(var.retain) {
                    continue;
                }
                let Some(value) = self.storage.get_instance_var(*instance, var.name.as_ref())
                else {
                    continue;
                };
                if value_is_retainable(value) {
                    snapshot.insert(program_retain_key(&program.name, &var.name), value.clone());
                }
            }
        }
        snapshot
    }

    /// Apply a retained snapshot to the current runtime.
    pub fn apply_retain_snapshot(
        &mut self,
        snapshot: &RetainSnapshot,
    ) -> Result<(), error::RuntimeError> {
        let prepared = self.prepare_retain_snapshot(snapshot, &self.storage)?;
        self.commit_retain_snapshot(prepared);
        Ok(())
    }

    pub(super) fn prepare_retain_snapshot_for_restart(
        &self,
        snapshot: &RetainSnapshot,
        restart: &PreparedRestart,
    ) -> Result<PreparedRetainSnapshot, error::RuntimeError> {
        self.prepare_retain_snapshot(snapshot, &restart.storage)
    }

    fn prepare_retain_snapshot(
        &self,
        snapshot: &RetainSnapshot,
        storage: &VariableStorage,
    ) -> Result<PreparedRetainSnapshot, error::RuntimeError> {
        let mut prepared_storage = storage.clone();
        let mut staged_events = Vec::new();
        for (name, value) in snapshot.values() {
            if let Some((program_name, variable_name)) = parse_program_retain_key(name) {
                let Some(program) = self
                    .programs
                    .values()
                    .find(|program| program.name.eq_ignore_ascii_case(program_name))
                else {
                    staged_events.push(crate::debug::RuntimeEvent::RetainOrphanDropped {
                        name: name.clone(),
                        time: self.current_time,
                    });
                    continue;
                };
                let Some(variable) = program
                    .vars
                    .iter()
                    .find(|variable| variable.name.eq_ignore_ascii_case(variable_name))
                else {
                    staged_events.push(crate::debug::RuntimeEvent::RetainOrphanDropped {
                        name: name.clone(),
                        time: self.current_time,
                    });
                    continue;
                };
                let Some(Value::Instance(instance)) =
                    prepared_storage.get_global(program.name.as_ref())
                else {
                    staged_events.push(crate::debug::RuntimeEvent::RetainOrphanDropped {
                        name: name.clone(),
                        time: self.current_time,
                    });
                    continue;
                };
                let instance = *instance;
                if retain_on_warm(variable.retain) && value_is_retainable(value) {
                    let ctx = RetainMigrationContext {
                        storage: &prepared_storage,
                        registry: &self.registry,
                        initializer_catalog: &self.initializer_catalog,
                        profile: &self.profile,
                        stdlib: &self.stdlib,
                    };
                    let migrated = canonicalize_retained_value(&ctx, variable.type_id, value)
                        .map_err(|error| {
                            error::RuntimeError::RetainMigration(
                                format!(
                                    "invalid retained value for program variable \
                                 '{}.{}': {error}",
                                    program.name, variable.name
                                )
                                .into(),
                            )
                        })?;
                    if &migrated != value {
                        staged_events.push(crate::debug::RuntimeEvent::RetainMigrationApplied {
                            name: name.clone(),
                            detail: retain_migration_detail(value, &migrated),
                            time: self.current_time,
                        });
                    }
                    if !prepared_storage.set_instance_var(instance, variable.name.clone(), migrated)
                    {
                        return Err(error::RuntimeError::UndefinedVariable(
                            format!("{}.{}", program.name, variable.name).into(),
                        ));
                    }
                }
                continue;
            }
            let Some(meta) = self.globals.get(name) else {
                staged_events.push(crate::debug::RuntimeEvent::RetainOrphanDropped {
                    name: name.clone(),
                    time: self.current_time,
                });
                continue;
            };
            let retain = meta.retain;
            let type_id = meta.type_id;
            if retain_on_warm(retain) && value_is_retainable(value) {
                let migrated = {
                    let ctx = RetainMigrationContext {
                        storage: &prepared_storage,
                        registry: &self.registry,
                        initializer_catalog: &self.initializer_catalog,
                        profile: &self.profile,
                        stdlib: &self.stdlib,
                    };
                    canonicalize_retained_value(&ctx, type_id, value)
                }
                .map_err(|error| {
                    error::RuntimeError::RetainMigration(
                        format!("invalid retained value for global '{name}': {error}").into(),
                    )
                })?;
                if &migrated != value {
                    staged_events.push(crate::debug::RuntimeEvent::RetainMigrationApplied {
                        name: name.clone(),
                        detail: retain_migration_detail(value, &migrated),
                        time: self.current_time,
                    });
                }
                prepared_storage.set_global(name.clone(), migrated);
            }
        }
        Ok(PreparedRetainSnapshot {
            storage: prepared_storage,
            events: staged_events,
        })
    }

    pub(super) fn commit_retain_snapshot(&mut self, prepared: PreparedRetainSnapshot) {
        self.storage = prepared.storage;
        if let Some(debug) = &self.debug {
            for event in prepared.events {
                debug.push_runtime_event(event);
            }
        }
    }
}

fn program_retain_key(program: &str, variable: &str) -> SmolStr {
    SmolStr::new(format!("{PROGRAM_RETAIN_KEY_PREFIX}{program}/{variable}"))
}

fn parse_program_retain_key(key: &str) -> Option<(&str, &str)> {
    let remainder = key.strip_prefix(PROGRAM_RETAIN_KEY_PREFIX)?;
    let (program, variable) = remainder.rsplit_once('/')?;
    if program.is_empty() || variable.is_empty() {
        return None;
    }
    Some((program, variable))
}

struct RetainMigrationContext<'a> {
    storage: &'a VariableStorage,
    registry: &'a trust_hir::types::TypeRegistry,
    initializer_catalog: &'a InitializerCatalog,
    profile: &'a DateTimeProfile,
    stdlib: &'a StandardLibrary,
}

fn canonicalize_retained_value(
    ctx: &RetainMigrationContext<'_>,
    type_id: trust_hir::TypeId,
    value: &Value,
) -> Result<Value, error::RuntimeError> {
    let Some(ty) = ctx.registry.get(type_id) else {
        return Ok(value.clone());
    };
    match ty {
        Type::Alias { target, .. } => canonicalize_retained_value(ctx, *target, value),
        Type::Subrange { base, lower, upper } => {
            let value = canonicalize_retained_value(ctx, *base, value)?;
            let Some(numeric) = retained_integer(&value) else {
                return Err(error::RuntimeError::RetainMigration(
                    format!(
                        "retained {} does not match declared subrange type id {}",
                        retain_value_kind(&value),
                        type_id.0
                    )
                    .into(),
                ));
            };
            let lower = i128::from(*lower);
            let upper = i128::from(*upper);
            if numeric < lower || numeric > upper {
                return Err(error::RuntimeError::RetainMigration(
                    format!("retained value {numeric} outside declared subrange {lower}..{upper}")
                        .into(),
                ));
            }
            Ok(value)
        }
        Type::Bool
        | Type::SInt
        | Type::Int
        | Type::DInt
        | Type::LInt
        | Type::USInt
        | Type::UInt
        | Type::UDInt
        | Type::ULInt
        | Type::Real
        | Type::LReal
        | Type::Byte
        | Type::Word
        | Type::DWord
        | Type::LWord
        | Type::Time
        | Type::LTime
        | Type::Date
        | Type::LDate
        | Type::Tod
        | Type::LTod
        | Type::Dt
        | Type::Ldt
        | Type::String { .. }
        | Type::WString { .. }
        | Type::Char
        | Type::WChar => canonicalize_retained_scalar(ty, value),
        Type::Enum { .. } => canonicalize_retained_enum(ctx.registry, type_id, value),
        Type::Array { element, .. } => {
            let Value::Array(array) = value else {
                return Err(error::RuntimeError::RetainMigration(
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
                .map(|element_value| canonicalize_retained_value(ctx, *element, element_value))
                .collect::<Result<Vec<_>, _>>()?;
            let array = ArrayValue::from_serialized_parts(
                ctx.registry,
                type_id,
                array.dimensions().to_vec(),
                elements,
            )
            .map_err(retain_value_error)?;
            Ok(Value::Array(Box::new(array)))
        }
        Type::Struct { fields, .. } => {
            let Value::Struct(struct_value) = value else {
                return Err(error::RuntimeError::RetainMigration(
                    format!(
                        "retained value kind does not match declared struct type id {}",
                        type_id.0
                    )
                    .into(),
                ));
            };
            let mut values = IndexMap::new();
            for field in fields {
                let value = if let Some((_, field_value)) = struct_value
                    .fields()
                    .iter()
                    .find(|(name, _)| field.name.eq_ignore_ascii_case(name.as_str()))
                {
                    canonicalize_retained_value(ctx, field.type_id, field_value)?
                } else {
                    materialize_retain_member_default(
                        ctx,
                        field.default_initializer,
                        field.type_id,
                        field.name.as_str(),
                    )?
                };
                values.insert(field.name.clone(), value);
            }
            let value =
                StructValue::new(ctx.registry, type_id, values).map_err(retain_value_error)?;
            Ok(Value::Struct(Arc::new(value)))
        }
        Type::Union { variants, .. } => {
            let Value::Struct(struct_value) = value else {
                return Err(error::RuntimeError::RetainMigration(
                    format!(
                        "retained value kind does not match declared union type id {}",
                        type_id.0
                    )
                    .into(),
                ));
            };
            let mut values = IndexMap::new();
            for variant in variants {
                let value = if let Some((_, variant_value)) = struct_value
                    .fields()
                    .iter()
                    .find(|(name, _)| variant.name.eq_ignore_ascii_case(name.as_str()))
                {
                    canonicalize_retained_value(ctx, variant.type_id, variant_value)?
                } else {
                    materialize_retain_member_default(
                        ctx,
                        variant.default_initializer,
                        variant.type_id,
                        variant.name.as_str(),
                    )?
                };
                values.insert(variant.name.clone(), value);
            }
            let value =
                StructValue::new(ctx.registry, type_id, values).map_err(retain_value_error)?;
            Ok(Value::Struct(Arc::new(value)))
        }
        _ => Err(error::RuntimeError::RetainMigration(
            format!(
                "declared retain type id {} is not retain-migratable",
                type_id.0
            )
            .into(),
        )),
    }
}

fn materialize_retain_member_default(
    ctx: &RetainMigrationContext<'_>,
    initializer: Option<InitializerId>,
    type_id: trust_hir::TypeId,
    field_name: &str,
) -> Result<Value, error::RuntimeError> {
    if let Some(initializer_id) = initializer {
        let expr = ctx
            .initializer_catalog
            .initializer(initializer_id)
            .ok_or_else(|| {
                error::RuntimeError::RetainMigration(
                    format!("missing initializer record for retained field '{field_name}'").into(),
                )
            })?;
        return crate::harness::initializer::evaluate_initializer(
            ctx.storage,
            ctx.registry,
            ctx.initializer_catalog,
            ctx.profile,
            None,
            ctx.stdlib,
            expr,
            type_id,
        )
        .map_err(|error| {
            error::RuntimeError::RetainMigration(
                format!("default initializer for retained field '{field_name}' failed: {error}")
                    .into(),
            )
        });
    }
    crate::harness::initializer::default_value_for_type_id(
        ctx.storage,
        ctx.registry,
        ctx.initializer_catalog,
        ctx.profile,
        None,
        ctx.stdlib,
        type_id,
    )
    .map_err(|error| {
        error::RuntimeError::RetainMigration(
            format!("default value for retained field '{field_name}' failed: {error}").into(),
        )
    })
}

fn canonicalize_retained_scalar(
    declared: &Type,
    value: &Value,
) -> Result<Value, error::RuntimeError> {
    let migrated = match (declared, value) {
        (
            Type::String {
                max_len: Some(max_len),
            },
            Value::String(text),
        ) => Value::String(truncate_string_elements(text.as_str(), *max_len).into()),
        (
            Type::WString {
                max_len: Some(max_len),
            },
            Value::WString(text),
        ) => Value::WString(truncate_string_elements(text, *max_len)),
        (Type::Bool, Value::Bool(_))
        | (Type::SInt, Value::SInt(_))
        | (Type::Int, Value::Int(_))
        | (Type::DInt, Value::DInt(_))
        | (Type::LInt, Value::LInt(_))
        | (Type::USInt, Value::USInt(_))
        | (Type::UInt, Value::UInt(_))
        | (Type::UDInt, Value::UDInt(_))
        | (Type::ULInt, Value::ULInt(_))
        | (Type::Real, Value::Real(_))
        | (Type::LReal, Value::LReal(_))
        | (Type::Byte, Value::Byte(_))
        | (Type::Word, Value::Word(_))
        | (Type::DWord, Value::DWord(_))
        | (Type::LWord, Value::LWord(_))
        | (Type::Time, Value::Time(_))
        | (Type::LTime, Value::LTime(_))
        | (Type::Date, Value::Date(_))
        | (Type::LDate, Value::LDate(_))
        | (Type::Tod, Value::Tod(_))
        | (Type::LTod, Value::LTod(_))
        | (Type::Dt, Value::Dt(_))
        | (Type::Ldt, Value::Ldt(_))
        | (Type::String { .. }, Value::String(_))
        | (Type::WString { .. }, Value::WString(_))
        | (Type::Char, Value::Char(_))
        | (Type::WChar, Value::WChar(_)) => value.clone(),
        (Type::Int, Value::SInt(value)) => Value::Int(i16::from(*value)),
        (Type::DInt, Value::SInt(value)) => Value::DInt(i32::from(*value)),
        (Type::DInt, Value::Int(value)) => Value::DInt(i32::from(*value)),
        (Type::LInt, Value::SInt(value)) => Value::LInt(i64::from(*value)),
        (Type::LInt, Value::Int(value)) => Value::LInt(i64::from(*value)),
        (Type::LInt, Value::DInt(value)) => Value::LInt(i64::from(*value)),
        (Type::UInt, Value::USInt(value)) => Value::UInt(u16::from(*value)),
        (Type::UDInt, Value::USInt(value)) => Value::UDInt(u32::from(*value)),
        (Type::UDInt, Value::UInt(value)) => Value::UDInt(u32::from(*value)),
        (Type::ULInt, Value::USInt(value)) => Value::ULInt(u64::from(*value)),
        (Type::ULInt, Value::UInt(value)) => Value::ULInt(u64::from(*value)),
        (Type::ULInt, Value::UDInt(value)) => Value::ULInt(u64::from(*value)),
        (Type::LReal, Value::Real(value)) => Value::LReal(f64::from(*value)),
        (Type::Word, Value::Byte(value)) => Value::Word(u16::from(*value)),
        (Type::DWord, Value::Byte(value)) => Value::DWord(u32::from(*value)),
        (Type::DWord, Value::Word(value)) => Value::DWord(u32::from(*value)),
        (Type::LWord, Value::Byte(value)) => Value::LWord(u64::from(*value)),
        (Type::LWord, Value::Word(value)) => Value::LWord(u64::from(*value)),
        (Type::LWord, Value::DWord(value)) => Value::LWord(u64::from(*value)),
        _ => {
            return Err(error::RuntimeError::RetainMigration(
                format!(
                    "cannot migrate retained {} to declared {}",
                    retain_value_kind(value),
                    retain_type_name(declared)
                )
                .into(),
            ));
        }
    };
    Ok(migrated)
}

fn retained_integer(value: &Value) -> Option<i128> {
    match value {
        Value::SInt(value) => Some(i128::from(*value)),
        Value::Int(value) => Some(i128::from(*value)),
        Value::DInt(value) => Some(i128::from(*value)),
        Value::LInt(value) => Some(i128::from(*value)),
        Value::USInt(value) => Some(i128::from(*value)),
        Value::UInt(value) => Some(i128::from(*value)),
        Value::UDInt(value) => Some(i128::from(*value)),
        Value::ULInt(value) => Some(i128::from(*value)),
        _ => None,
    }
}

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

fn retain_migration_detail(before: &Value, after: &Value) -> String {
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

fn retain_type_name(ty: &Type) -> &'static str {
    match ty {
        Type::Bool => "BOOL",
        Type::SInt => "SINT",
        Type::Int => "INT",
        Type::DInt => "DINT",
        Type::LInt => "LINT",
        Type::USInt => "USINT",
        Type::UInt => "UINT",
        Type::UDInt => "UDINT",
        Type::ULInt => "ULINT",
        Type::Real => "REAL",
        Type::LReal => "LREAL",
        Type::Byte => "BYTE",
        Type::Word => "WORD",
        Type::DWord => "DWORD",
        Type::LWord => "LWORD",
        Type::Time => "TIME",
        Type::LTime => "LTIME",
        Type::Date => "DATE",
        Type::LDate => "LDATE",
        Type::Tod => "TOD",
        Type::LTod => "LTOD",
        Type::Dt => "DT",
        Type::Ldt => "LDT",
        Type::String { .. } => "STRING",
        Type::WString { .. } => "WSTRING",
        Type::Char => "CHAR",
        Type::WChar => "WCHAR",
        _ => "non-scalar",
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
        return Err(error::RuntimeError::RetainMigration(
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

pub(super) fn retain_on_warm(policy: RetainPolicy) -> bool {
    matches!(policy, RetainPolicy::Retain | RetainPolicy::Persistent)
}

pub(super) fn value_is_retainable(value: &Value) -> bool {
    match value {
        Value::Array(array) => array.elements().iter().all(value_is_retainable),
        Value::Struct(value) => value.fields().values().all(value_is_retainable),
        Value::Reference(_) | Value::Instance(_) => false,
        _ => true,
    }
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
