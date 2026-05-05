use std::sync::Arc;

use smol_str::SmolStr;
use trust_hir::types::TypeRegistry;

use crate::error::RuntimeError;
use crate::memory::{InstanceId, VariableStorage};
use crate::program_model::{Expr, LValue};
use crate::value::{
    checked_array_offset_i64, parse_partial_access, ref_indices_from_iter, write_partial_access,
    DateTimeProfile, PartialAccessError, RefSegment, Value, ValueRef,
};

use super::storage_expr::eval_storage_expr;

pub(crate) fn read_storage_lvalue(
    storage: &VariableStorage,
    registry: &TypeRegistry,
    profile: &DateTimeProfile,
    current_instance: Option<InstanceId>,
    target: &LValue,
) -> Result<Value, RuntimeError> {
    let expr = expr_from_lvalue(target);
    eval_storage_expr(storage, registry, profile, current_instance, &expr)
}

fn expr_from_lvalue(target: &LValue) -> Expr {
    match target {
        LValue::Name(name) => Expr::Name(name.clone()),
        LValue::Index { target, indices } => Expr::Index {
            target: Box::new(expr_from_lvalue(target)),
            indices: indices.clone(),
        },
        LValue::Field { target, field } => Expr::Field {
            target: Box::new(expr_from_lvalue(target)),
            field: field.clone(),
        },
        LValue::Deref(expr) => Expr::Deref(expr.clone()),
    }
}

pub(crate) fn write_storage_lvalue(
    storage: &mut VariableStorage,
    registry: &TypeRegistry,
    profile: &DateTimeProfile,
    current_instance: Option<InstanceId>,
    target: &LValue,
    value: Value,
) -> Result<(), RuntimeError> {
    match target {
        LValue::Name(name) => write_name(storage, current_instance, name, value),
        LValue::Index { target, indices } => {
            let array_value =
                read_storage_lvalue(storage, registry, profile, current_instance, target)?;
            let index_values = indices
                .iter()
                .map(|expr| eval_storage_expr(storage, registry, profile, current_instance, expr))
                .collect::<Result<Vec<_>, _>>()?;
            let updated = write_indices(array_value, &index_values, value)?;
            write_storage_lvalue(
                storage,
                registry,
                profile,
                current_instance,
                target,
                updated,
            )
        }
        LValue::Field { target, field } => {
            let base_value =
                read_storage_lvalue(storage, registry, profile, current_instance, target)?;
            match base_value {
                Value::Instance(id) => {
                    let Some(reference) = storage.ref_for_instance_recursive(id, field.as_str())
                    else {
                        return Err(RuntimeError::UndefinedField(field.clone()));
                    };
                    if storage.write_by_ref(reference, value) {
                        Ok(())
                    } else {
                        Err(RuntimeError::NullReference)
                    }
                }
                other => {
                    let updated = write_field(other, field, value)?;
                    write_storage_lvalue(
                        storage,
                        registry,
                        profile,
                        current_instance,
                        target,
                        updated,
                    )
                }
            }
        }
        LValue::Deref(expr) => {
            let reference =
                resolve_reference_expr(storage, registry, profile, current_instance, expr)?;
            if storage.write_by_ref(reference, value) {
                Ok(())
            } else {
                Err(RuntimeError::NullReference)
            }
        }
    }
}

fn write_name(
    storage: &mut VariableStorage,
    current_instance: Option<InstanceId>,
    name: &SmolStr,
    value: Value,
) -> Result<(), RuntimeError> {
    if storage.get_local(name.as_str()).is_some() {
        storage.set_local(name.clone(), value);
        return Ok(());
    }
    if let Some(instance_id) = current_instance {
        if let Some(reference) = storage.ref_for_instance_recursive(instance_id, name.as_str()) {
            if storage.write_by_ref(reference, value) {
                return Ok(());
            }
            return Err(RuntimeError::NullReference);
        }
    }
    if storage.get_global(name.as_str()).is_some() {
        storage.set_global(name.clone(), value);
        return Ok(());
    }
    if storage.get_retain(name.as_str()).is_some() {
        storage.set_retain(name.clone(), value);
        return Ok(());
    }
    Err(RuntimeError::UndefinedVariable(name.clone()))
}

fn write_indices(target: Value, indices: &[Value], value: Value) -> Result<Value, RuntimeError> {
    match target {
        Value::Array(mut array) => {
            let offset = array_offset(array.dimensions(), indices)?;
            if let Some(slot) = array.elements_mut().get_mut(offset) {
                *slot = value;
                Ok(Value::Array(array))
            } else {
                Err(RuntimeError::TypeMismatch)
            }
        }
        _ => Err(RuntimeError::TypeMismatch),
    }
}

fn write_field(target: Value, field: &SmolStr, value: Value) -> Result<Value, RuntimeError> {
    if let Some(access) = parse_partial_access(field.as_str()) {
        return write_partial_access(target, access, value)
            .map_err(partial_access_error_to_runtime);
    }
    match target {
        Value::Struct(mut struct_value) => {
            let struct_value_mut = Arc::make_mut(&mut struct_value);
            if struct_value_mut.set_existing_field(field.clone(), value) {
                Ok(Value::Struct(struct_value))
            } else {
                Err(RuntimeError::UndefinedField(field.clone()))
            }
        }
        _ => Err(RuntimeError::TypeMismatch),
    }
}

fn resolve_reference_expr(
    storage: &mut VariableStorage,
    registry: &TypeRegistry,
    profile: &DateTimeProfile,
    current_instance: Option<InstanceId>,
    expr: &Expr,
) -> Result<ValueRef, RuntimeError> {
    match expr {
        Expr::Ref(target) => {
            resolve_lvalue_reference(storage, registry, profile, current_instance, target)
        }
        _ => match eval_storage_expr(storage, registry, profile, current_instance, expr)? {
            Value::Reference(Some(reference)) => Ok(reference),
            Value::Reference(None) => Err(RuntimeError::NullReference),
            _ => Err(RuntimeError::TypeMismatch),
        },
    }
}

fn resolve_lvalue_reference(
    storage: &mut VariableStorage,
    registry: &TypeRegistry,
    profile: &DateTimeProfile,
    current_instance: Option<InstanceId>,
    target: &LValue,
) -> Result<ValueRef, RuntimeError> {
    match target {
        LValue::Name(name) => resolve_name_reference(storage, current_instance, name)
            .ok_or_else(|| RuntimeError::UndefinedVariable(name.clone())),
        LValue::Index { target, indices } => {
            let base =
                resolve_lvalue_reference(storage, registry, profile, current_instance, target)?;
            let array_value =
                read_storage_lvalue(storage, registry, profile, current_instance, target)?;
            let Value::Array(array) = &array_value else {
                return Err(RuntimeError::TypeMismatch);
            };
            let index_values = indices
                .iter()
                .map(|expr| eval_storage_expr(storage, registry, profile, current_instance, expr))
                .collect::<Result<Vec<_>, _>>()?;
            array_offset(array.dimensions(), &index_values)?;
            let mut index_path = Vec::with_capacity(index_values.len());
            for value in index_values {
                index_path.push(index_to_i64(value)?);
            }
            let mut value_ref = base;
            value_ref
                .path
                .push(RefSegment::Index(ref_indices_from_iter(index_path)));
            Ok(value_ref)
        }
        LValue::Field { target, field } => {
            if let Some(qualified) = target
                .qualified_name()
                .map(|prefix| SmolStr::new(format!("{prefix}.{field}")))
            {
                if let Some(reference) =
                    resolve_name_reference(storage, current_instance, &qualified)
                {
                    return Ok(reference);
                }
            }
            let base_value =
                read_storage_lvalue(storage, registry, profile, current_instance, target)?;
            match base_value {
                Value::Instance(id) => storage
                    .ref_for_instance_recursive(id, field.as_str())
                    .ok_or_else(|| RuntimeError::UndefinedField(field.clone())),
                Value::Struct(struct_value) => {
                    if !struct_value.contains_field(field.as_str()) {
                        return Err(RuntimeError::UndefinedField(field.clone()));
                    }
                    let mut value_ref = resolve_lvalue_reference(
                        storage,
                        registry,
                        profile,
                        current_instance,
                        target,
                    )?;
                    value_ref.path.push(RefSegment::Field(field.clone()));
                    Ok(value_ref)
                }
                _ => Err(RuntimeError::TypeMismatch),
            }
        }
        LValue::Deref(expr) => {
            resolve_reference_expr(storage, registry, profile, current_instance, expr)
        }
    }
}

fn resolve_name_reference(
    storage: &VariableStorage,
    current_instance: Option<InstanceId>,
    name: &SmolStr,
) -> Option<ValueRef> {
    if let Some(reference) = storage.ref_for_local(name.as_str()) {
        return Some(reference);
    }
    if let Some(instance_id) = current_instance {
        if let Some(reference) = storage.ref_for_instance_recursive(instance_id, name.as_str()) {
            return Some(reference);
        }
    }
    storage.ref_for_global(name.as_str())
}

fn index_to_i64(value: Value) -> Result<i64, RuntimeError> {
    match value {
        Value::SInt(v) => Ok(v as i64),
        Value::Int(v) => Ok(v as i64),
        Value::DInt(v) => Ok(v as i64),
        Value::LInt(v) => Ok(v),
        Value::USInt(v) => Ok(v as i64),
        Value::UInt(v) => Ok(v as i64),
        Value::UDInt(v) => Ok(v as i64),
        Value::ULInt(v) => Ok(v as i64),
        Value::Byte(v) => Ok(v as i64),
        Value::Word(v) => Ok(v as i64),
        Value::DWord(v) => Ok(v as i64),
        Value::LWord(v) => Ok(v as i64),
        _ => Err(RuntimeError::TypeMismatch),
    }
}

fn array_offset(dimensions: &[(i64, i64)], indices: &[Value]) -> Result<usize, RuntimeError> {
    if dimensions.len() != indices.len() {
        return Err(RuntimeError::TypeMismatch);
    }
    let mut numeric_indices = Vec::with_capacity(indices.len());
    for index_value in indices {
        let idx = index_to_i64(index_value.clone())?;
        numeric_indices.push(idx);
    }
    checked_array_offset_i64(dimensions, &numeric_indices)
}

fn partial_access_error_to_runtime(err: PartialAccessError) -> RuntimeError {
    match err {
        PartialAccessError::IndexOutOfBounds {
            index,
            lower,
            upper,
        } => RuntimeError::IndexOutOfBounds {
            index,
            lower,
            upper,
        },
        PartialAccessError::TypeMismatch => RuntimeError::TypeMismatch,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_local_name_without_eval_context() {
        let mut storage = VariableStorage::new();
        storage.push_frame("MAIN");
        storage.set_local("x", Value::DInt(1));
        let registry = TypeRegistry::new();

        write_storage_lvalue(
            &mut storage,
            &registry,
            &DateTimeProfile::default(),
            None,
            &LValue::Name("x".into()),
            Value::DInt(7),
        )
        .unwrap();

        assert_eq!(storage.get_local("x"), Some(&Value::DInt(7)));
    }

    #[test]
    #[ignore = "red test for runtime-safety fail-closed Phase 1"]
    fn unknown_name_write_fails_without_creating_global() {
        let mut storage = VariableStorage::new();
        let registry = TypeRegistry::new();

        let err = write_storage_lvalue(
            &mut storage,
            &registry,
            &DateTimeProfile::default(),
            None,
            &LValue::Name("missing".into()),
            Value::DInt(7),
        )
        .expect_err("unknown lvalue write must fail");

        assert_eq!(err, RuntimeError::UndefinedVariable("missing".into()));
        assert!(storage.get_global("missing").is_none());
    }

    #[test]
    fn writes_array_element_without_eval_context() {
        let mut storage = VariableStorage::new();
        storage.set_global(
            "arr",
            Value::Array(Box::new(
                crate::value::ArrayValue::from_untyped_parts(
                    vec![Value::DInt(1), Value::DInt(2), Value::DInt(3)],
                    vec![(0, 2)],
                )
                .unwrap(),
            )),
        );
        let registry = TypeRegistry::new();

        write_storage_lvalue(
            &mut storage,
            &registry,
            &DateTimeProfile::default(),
            None,
            &LValue::Index {
                target: Box::new(LValue::Name("arr".into())),
                indices: vec![Expr::Literal(Value::DInt(1))],
            },
            Value::DInt(9),
        )
        .unwrap();

        let Value::Array(array) = storage.get_global("arr").cloned().unwrap() else {
            panic!("expected array");
        };
        assert_eq!(array.elements()[1], Value::DInt(9));
    }

    #[test]
    fn writes_deref_target_without_eval_context() {
        let mut storage = VariableStorage::new();
        storage.set_global("x", Value::DInt(5));
        let reference = storage.ref_for_global("x").expect("global ref");
        let registry = TypeRegistry::new();

        write_storage_lvalue(
            &mut storage,
            &registry,
            &DateTimeProfile::default(),
            None,
            &LValue::Deref(Box::new(Expr::Ref(LValue::Name("x".into())))),
            Value::DInt(11),
        )
        .unwrap();

        assert_eq!(storage.get_global("x"), Some(&Value::DInt(11)));
        let stored = storage.read_by_ref(reference).cloned().unwrap();
        assert_eq!(stored, Value::DInt(11));
    }

    #[test]
    fn writes_struct_field_without_eval_context() {
        let mut storage = VariableStorage::new();
        let mut fields = indexmap::IndexMap::new();
        fields.insert(SmolStr::new("x"), Value::DInt(1));
        storage.set_global(
            "st",
            Value::Struct(Arc::new(crate::value::StructValue::from_untyped_parts(
                SmolStr::new("ST"),
                fields,
            ))),
        );
        let registry = TypeRegistry::new();

        write_storage_lvalue(
            &mut storage,
            &registry,
            &DateTimeProfile::default(),
            None,
            &LValue::Field {
                target: Box::new(LValue::Name("st".into())),
                field: "x".into(),
            },
            Value::DInt(3),
        )
        .unwrap();

        let Value::Struct(st) = storage.get_global("st").cloned().unwrap() else {
            panic!("expected struct");
        };
        assert_eq!(st.field("x"), Some(&Value::DInt(3)));
    }

    #[test]
    fn writes_nested_struct_array_element_without_eval_context() {
        let mut storage = VariableStorage::new();
        let mut outer_fields = indexmap::IndexMap::new();
        outer_fields.insert(
            SmolStr::new("arr"),
            Value::Array(Box::new(
                crate::value::ArrayValue::from_untyped_parts(
                    vec![Value::DInt(1), Value::DInt(2), Value::DInt(3)],
                    vec![(0, 2)],
                )
                .unwrap(),
            )),
        );
        storage.set_global(
            "outer",
            Value::Struct(Arc::new(crate::value::StructValue::from_untyped_parts(
                SmolStr::new("Outer"),
                outer_fields,
            ))),
        );
        let registry = TypeRegistry::new();

        write_storage_lvalue(
            &mut storage,
            &registry,
            &DateTimeProfile::default(),
            None,
            &LValue::Index {
                target: Box::new(LValue::Field {
                    target: Box::new(LValue::Name("outer".into())),
                    field: "arr".into(),
                }),
                indices: vec![Expr::Literal(Value::DInt(1))],
            },
            Value::DInt(9),
        )
        .unwrap();

        let Value::Struct(outer) = storage.get_global("outer").cloned().unwrap() else {
            panic!("expected struct");
        };
        let Value::Array(array) = outer.field("arr").cloned().unwrap() else {
            panic!("expected array field");
        };
        assert_eq!(array.elements()[1], Value::DInt(9));
    }

    #[test]
    fn writes_nested_array_of_struct_field_without_eval_context() {
        let mut storage = VariableStorage::new();
        storage.set_global(
            "items",
            Value::Array(Box::new(
                crate::value::ArrayValue::from_untyped_parts(
                    vec![
                        Value::Struct(Arc::new(crate::value::StructValue::from_untyped_parts(
                            SmolStr::new("Item"),
                            indexmap::IndexMap::from([(SmolStr::new("value"), Value::DInt(1))]),
                        ))),
                        Value::Struct(Arc::new(crate::value::StructValue::from_untyped_parts(
                            SmolStr::new("Item"),
                            indexmap::IndexMap::from([(SmolStr::new("value"), Value::DInt(2))]),
                        ))),
                    ],
                    vec![(0, 1)],
                )
                .unwrap(),
            )),
        );
        let registry = TypeRegistry::new();

        write_storage_lvalue(
            &mut storage,
            &registry,
            &DateTimeProfile::default(),
            None,
            &LValue::Field {
                target: Box::new(LValue::Index {
                    target: Box::new(LValue::Name("items".into())),
                    indices: vec![Expr::Literal(Value::DInt(1))],
                }),
                field: "value".into(),
            },
            Value::DInt(7),
        )
        .unwrap();

        let Value::Array(items) = storage.get_global("items").cloned().unwrap() else {
            panic!("expected array");
        };
        let Value::Struct(item) = items.elements()[1].clone() else {
            panic!("expected struct element");
        };
        assert_eq!(item.field("value"), Some(&Value::DInt(7)));
    }
}
