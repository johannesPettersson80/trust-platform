use super::*;

use crate::value::{ArrayValue, StructValue};
use indexmap::IndexMap;

fn profile() -> DateTimeProfile {
    DateTimeProfile::default()
}

fn registry() -> TypeRegistry {
    TypeRegistry::new()
}

fn literal(value: Value) -> Expr {
    Expr::Literal(value)
}

fn array(elements: Vec<Value>, dimensions: Vec<(i64, i64)>) -> Value {
    Value::Array(Box::new(
        ArrayValue::from_untyped_parts(elements, dimensions).expect("valid test array"),
    ))
}

fn structure(type_name: &str, fields: &[(&str, Value)]) -> Value {
    Value::Struct(Arc::new(StructValue::from_untyped_parts(
        type_name.into(),
        fields
            .iter()
            .map(|(name, value)| (SmolStr::new(*name), value.clone()))
            .collect::<IndexMap<_, _>>(),
    )))
}

fn read(
    storage: &VariableStorage,
    current_instance: Option<InstanceId>,
    target: &LValue,
) -> Result<Value, RuntimeError> {
    read_storage_lvalue(storage, &registry(), &profile(), current_instance, target)
}

fn write(
    storage: &mut VariableStorage,
    current_instance: Option<InstanceId>,
    target: &LValue,
    value: Value,
) -> Result<(), RuntimeError> {
    write_storage_lvalue(
        storage,
        &registry(),
        &profile(),
        current_instance,
        target,
        value,
    )
}

#[test]
fn lvalue_to_expression_preserves_nested_target_shape() {
    let target = LValue::Field {
        target: Box::new(LValue::Index {
            target: Box::new(LValue::Name("items".into())),
            indices: vec![literal(Value::Int(2))],
        }),
        field: "value".into(),
    };

    let Expr::Field { target, field } = expr_from_lvalue(&target) else {
        panic!("expected field expression");
    };
    assert_eq!(field, "value");
    let Expr::Index { target, indices } = target.as_ref() else {
        panic!("expected index expression");
    };
    assert_eq!(indices.len(), 1);
    assert!(matches!(target.as_ref(), Expr::Name(name) if name == "items"));
}

#[test]
fn lvalue_read_uses_expression_name_precedence_including_retain() {
    let mut storage = VariableStorage::new();
    storage.set_retain("value", Value::DInt(1));
    storage.set_global("value", Value::DInt(2));
    let instance = storage.create_instance("OWNER");
    assert!(storage.set_instance_var(instance, "value", Value::DInt(3)));
    storage.push_frame("METHOD");
    assert!(storage.set_local("value", Value::DInt(4)));

    assert_eq!(
        read(&storage, Some(instance), &LValue::Name("value".into())),
        Ok(Value::DInt(4))
    );
}

#[test]
fn simple_write_uses_local_instance_global_then_retain_precedence() {
    let mut storage = VariableStorage::new();
    storage.set_retain("local_wins", Value::Int(1));
    storage.set_global("local_wins", Value::Int(2));
    storage.set_retain("instance_wins", Value::Int(3));
    storage.set_global("instance_wins", Value::Int(4));
    storage.set_retain("global_wins", Value::Int(5));
    storage.set_global("global_wins", Value::Int(6));
    storage.set_retain("retain_only", Value::Int(7));

    let instance = storage.create_instance("OWNER");
    assert!(storage.set_instance_var(instance, "local_wins", Value::Int(8)));
    assert!(storage.set_instance_var(instance, "instance_wins", Value::Int(9)));
    storage.push_frame("METHOD");
    assert!(storage.set_local("local_wins", Value::Int(10)));

    for (name, value) in [
        ("local_wins", Value::Int(20)),
        ("instance_wins", Value::Int(21)),
        ("global_wins", Value::Int(22)),
        ("retain_only", Value::Int(23)),
    ] {
        write(
            &mut storage,
            Some(instance),
            &LValue::Name(name.into()),
            value,
        )
        .expect("existing name should be writable");
    }

    assert_eq!(storage.get_local("local_wins"), Some(&Value::Int(20)));
    assert_eq!(
        storage.get_instance_var(instance, "local_wins"),
        Some(&Value::Int(8))
    );
    assert_eq!(storage.get_global("local_wins"), Some(&Value::Int(2)));
    assert_eq!(storage.get_retain("local_wins"), Some(&Value::Int(1)));

    assert_eq!(
        storage.get_instance_var(instance, "instance_wins"),
        Some(&Value::Int(21))
    );
    assert_eq!(storage.get_global("instance_wins"), Some(&Value::Int(4)));
    assert_eq!(storage.get_retain("instance_wins"), Some(&Value::Int(3)));

    assert_eq!(storage.get_global("global_wins"), Some(&Value::Int(22)));
    assert_eq!(storage.get_retain("global_wins"), Some(&Value::Int(5)));
    assert_eq!(storage.get_retain("retain_only"), Some(&Value::Int(23)));
}

#[test]
fn inherited_instance_write_updates_nearest_owner_and_respects_shadowing() {
    let mut storage = VariableStorage::new();
    let base = storage.create_instance("BASE");
    let child = storage.create_instance("CHILD");
    storage.get_instance_mut(child).expect("child").parent = Some(base);
    assert!(storage.set_instance_var(base, "inherited", Value::DInt(1)));

    write(
        &mut storage,
        Some(child),
        &LValue::Name("inherited".into()),
        Value::DInt(2),
    )
    .expect("inherited field should be writable");
    assert_eq!(
        storage.get_instance_var(base, "inherited"),
        Some(&Value::DInt(2))
    );

    assert!(storage.set_instance_var(child, "inherited", Value::DInt(3)));
    write(
        &mut storage,
        Some(child),
        &LValue::Name("inherited".into()),
        Value::DInt(4),
    )
    .expect("shadowing child field should be writable");
    assert_eq!(
        storage.get_instance_var(child, "inherited"),
        Some(&Value::DInt(4))
    );
    assert_eq!(
        storage.get_instance_var(base, "inherited"),
        Some(&Value::DInt(2))
    );
}

#[test]
fn unknown_name_write_creates_no_storage_in_any_area() {
    let mut storage = VariableStorage::new();
    let instance = storage.create_instance("OWNER");
    storage.push_frame("METHOD");

    assert_eq!(
        write(
            &mut storage,
            Some(instance),
            &LValue::Name("missing".into()),
            Value::Int(1)
        ),
        Err(RuntimeError::UndefinedVariable("missing".into()))
    );
    assert_eq!(storage.get_local("missing"), None);
    assert_eq!(storage.get_instance_var(instance, "missing"), None);
    assert_eq!(storage.get_global("missing"), None);
    assert_eq!(storage.get_retain("missing"), None);
}

#[test]
fn qualified_field_write_targets_exact_existing_storage_name_first() {
    let mut storage = VariableStorage::new();
    storage.set_global("Config", structure("Config", &[("Value", Value::DInt(10))]));
    storage.set_global("Config.Value", Value::DInt(20));
    let target = LValue::Field {
        target: Box::new(LValue::Name("Config".into())),
        field: "Value".into(),
    };

    write(&mut storage, None, &target, Value::DInt(30))
        .expect("qualified storage name should be writable");

    assert_eq!(storage.get_global("Config.Value"), Some(&Value::DInt(30)));
    let Value::Struct(config) = storage.get_global("Config").expect("config") else {
        panic!("expected structure");
    };
    assert_eq!(config.field("Value"), Some(&Value::DInt(10)));
}

#[test]
fn array_write_honors_multidimensional_lower_bounds_and_preserves_siblings() {
    let mut storage = VariableStorage::new();
    storage.set_global(
        "grid",
        array(
            vec![
                Value::Int(10),
                Value::Int(11),
                Value::Int(12),
                Value::Int(13),
            ],
            vec![(5, 6), (-1, 0)],
        ),
    );
    let target = LValue::Index {
        target: Box::new(LValue::Name("grid".into())),
        indices: vec![literal(Value::Int(6)), literal(Value::Int(-1))],
    };

    write(&mut storage, None, &target, Value::Int(99)).expect("valid array write");

    let Value::Array(grid) = storage.get_global("grid").expect("grid") else {
        panic!("expected array");
    };
    assert_eq!(
        grid.elements(),
        &[
            Value::Int(10),
            Value::Int(11),
            Value::Int(99),
            Value::Int(13),
        ]
    );
}

#[test]
fn invalid_array_arity_type_and_bounds_leave_storage_unchanged() {
    let original = array(vec![Value::Int(10), Value::Int(11)], vec![(5, 6)]);
    let cases = [
        (Vec::new(), RuntimeError::TypeMismatch),
        (vec![Value::Bool(true)], RuntimeError::TypeMismatch),
        (
            vec![Value::Int(7)],
            RuntimeError::IndexOutOfBounds {
                index: 7,
                lower: 5,
                upper: 6,
            },
        ),
    ];

    for (indices, expected) in cases {
        let mut storage = VariableStorage::new();
        storage.set_global("arr", original.clone());
        let target = LValue::Index {
            target: Box::new(LValue::Name("arr".into())),
            indices: indices.into_iter().map(literal).collect(),
        };

        assert_eq!(
            write(&mut storage, None, &target, Value::Int(99)),
            Err(expected)
        );
        assert_eq!(storage.get_global("arr"), Some(&original));
    }
}

#[test]
fn indexing_nonarray_target_fails_without_mutation() {
    let mut storage = VariableStorage::new();
    storage.set_global("scalar", Value::Int(10));
    let target = LValue::Index {
        target: Box::new(LValue::Name("scalar".into())),
        indices: vec![literal(Value::Int(1))],
    };

    assert_eq!(
        write(&mut storage, None, &target, Value::Int(99)),
        Err(RuntimeError::TypeMismatch)
    );
    assert_eq!(storage.get_global("scalar"), Some(&Value::Int(10)));
}

#[test]
fn structure_write_changes_existing_field_only() {
    let mut storage = VariableStorage::new();
    storage.set_global(
        "record",
        structure(
            "Record",
            &[("left", Value::Int(1)), ("right", Value::Int(2))],
        ),
    );

    write(
        &mut storage,
        None,
        &LValue::Field {
            target: Box::new(LValue::Name("record".into())),
            field: "left".into(),
        },
        Value::Int(9),
    )
    .expect("existing field");

    let Value::Struct(record) = storage.get_global("record").expect("record") else {
        panic!("expected structure");
    };
    assert_eq!(record.field("left"), Some(&Value::Int(9)));
    assert_eq!(record.field("right"), Some(&Value::Int(2)));
}

#[test]
fn missing_structure_field_is_not_added_and_scalar_field_is_type_error() {
    let original = structure("Record", &[("value", Value::Int(1))]);
    let mut storage = VariableStorage::new();
    storage.set_global("record", original.clone());
    storage.set_global("scalar", Value::Int(2));

    assert_eq!(
        write(
            &mut storage,
            None,
            &LValue::Field {
                target: Box::new(LValue::Name("record".into())),
                field: "missing".into(),
            },
            Value::Int(9)
        ),
        Err(RuntimeError::UndefinedField("missing".into()))
    );
    assert_eq!(storage.get_global("record"), Some(&original));

    assert_eq!(
        write(
            &mut storage,
            None,
            &LValue::Field {
                target: Box::new(LValue::Name("scalar".into())),
                field: "field".into(),
            },
            Value::Int(9)
        ),
        Err(RuntimeError::TypeMismatch)
    );
    assert_eq!(storage.get_global("scalar"), Some(&Value::Int(2)));
}

#[test]
fn structure_write_uses_copy_on_write_for_shared_values() {
    let shared = structure("Record", &[("value", Value::Int(1))]);
    let mut storage = VariableStorage::new();
    storage.set_global("left", shared.clone());
    storage.set_global("right", shared);

    write(
        &mut storage,
        None,
        &LValue::Field {
            target: Box::new(LValue::Name("left".into())),
            field: "value".into(),
        },
        Value::Int(9),
    )
    .expect("left field write");

    let Value::Struct(left) = storage.get_global("left").expect("left") else {
        panic!("expected left structure");
    };
    let Value::Struct(right) = storage.get_global("right").expect("right") else {
        panic!("expected right structure");
    };
    assert_eq!(left.field("value"), Some(&Value::Int(9)));
    assert_eq!(right.field("value"), Some(&Value::Int(1)));
}

#[test]
fn instance_field_write_updates_recursive_owner_without_rebuilding_holder() {
    let mut storage = VariableStorage::new();
    let base = storage.create_instance("BASE");
    let child = storage.create_instance("CHILD");
    storage.get_instance_mut(child).expect("child").parent = Some(base);
    assert!(storage.set_instance_var(base, "value", Value::Int(1)));
    storage.set_global("object", Value::Instance(child));

    write(
        &mut storage,
        None,
        &LValue::Field {
            target: Box::new(LValue::Name("object".into())),
            field: "value".into(),
        },
        Value::Int(8),
    )
    .expect("instance field");

    assert_eq!(
        storage.get_instance_var(base, "value"),
        Some(&Value::Int(8))
    );
    assert_eq!(storage.get_global("object"), Some(&Value::Instance(child)));
}

#[test]
fn missing_instance_field_is_not_created() {
    let mut storage = VariableStorage::new();
    let instance = storage.create_instance("OWNER");
    storage.set_global("object", Value::Instance(instance));

    assert_eq!(
        write(
            &mut storage,
            None,
            &LValue::Field {
                target: Box::new(LValue::Name("object".into())),
                field: "missing".into(),
            },
            Value::Int(8)
        ),
        Err(RuntimeError::UndefinedField("missing".into()))
    );
    assert_eq!(storage.get_instance_var(instance, "missing"), None);
}

#[test]
fn partial_access_write_changes_only_selected_bits() {
    let mut storage = VariableStorage::new();
    storage.set_global("bits", Value::Byte(0b1010_0000));
    let target = LValue::Field {
        target: Box::new(LValue::Name("bits".into())),
        field: "%X1".into(),
    };

    write(&mut storage, None, &target, Value::Bool(true)).expect("bit write");
    assert_eq!(storage.get_global("bits"), Some(&Value::Byte(0b1010_0010)));
}

#[test]
fn invalid_partial_access_write_is_atomic() {
    let cases = [
        (
            "%X8",
            Value::Bool(true),
            RuntimeError::IndexOutOfBounds {
                index: 8,
                lower: 0,
                upper: 7,
            },
        ),
        ("%B0", Value::Bool(true), RuntimeError::TypeMismatch),
    ];
    for (field, value, expected) in cases {
        let mut storage = VariableStorage::new();
        storage.set_global("bits", Value::Byte(0b1010_0000));
        assert_eq!(
            write(
                &mut storage,
                None,
                &LValue::Field {
                    target: Box::new(LValue::Name("bits".into())),
                    field: field.into(),
                },
                value
            ),
            Err(expected)
        );
        assert_eq!(storage.get_global("bits"), Some(&Value::Byte(0b1010_0000)));
    }
}

#[test]
fn dereference_write_accepts_ref_expression_and_existing_reference_value() {
    let mut storage = VariableStorage::new();
    storage.set_global("x", Value::DInt(1));
    let reference = storage.ref_for_global("x").expect("reference");

    write(
        &mut storage,
        None,
        &LValue::Deref(Box::new(Expr::Ref(LValue::Name("x".into())))),
        Value::DInt(2),
    )
    .expect("REF target write");
    assert_eq!(storage.get_global("x"), Some(&Value::DInt(2)));

    write(
        &mut storage,
        None,
        &LValue::Deref(Box::new(literal(Value::Reference(Some(reference))))),
        Value::DInt(3),
    )
    .expect("reference value write");
    assert_eq!(storage.get_global("x"), Some(&Value::DInt(3)));
}

#[test]
fn dereference_write_distinguishes_empty_nonreference_and_stale_values() {
    let mut storage = VariableStorage::new();
    storage.set_global("x", Value::DInt(1));
    let reference = storage.ref_for_global("x").expect("reference");

    for (expression, expected) in [
        (literal(Value::Reference(None)), RuntimeError::NullReference),
        (literal(Value::DInt(1)), RuntimeError::TypeMismatch),
    ] {
        assert_eq!(
            write(
                &mut storage,
                None,
                &LValue::Deref(Box::new(expression)),
                Value::DInt(9)
            ),
            Err(expected)
        );
        assert_eq!(storage.get_global("x"), Some(&Value::DInt(1)));
    }

    storage.reset_runtime_values(false);
    assert_eq!(
        write(
            &mut storage,
            None,
            &LValue::Deref(Box::new(literal(Value::Reference(Some(reference))))),
            Value::DInt(9)
        ),
        Err(RuntimeError::NullReference)
    );
}

#[test]
fn dereference_of_index_reference_updates_only_selected_element() {
    let mut storage = VariableStorage::new();
    storage.set_global(
        "arr",
        array(vec![Value::Int(10), Value::Int(20)], vec![(5, 6)]),
    );
    let reference = Expr::Ref(LValue::Index {
        target: Box::new(LValue::Name("arr".into())),
        indices: vec![literal(Value::Int(6))],
    });

    write(
        &mut storage,
        None,
        &LValue::Deref(Box::new(reference)),
        Value::Int(99),
    )
    .expect("indexed reference write");

    let Value::Array(arr) = storage.get_global("arr").expect("arr") else {
        panic!("expected array");
    };
    assert_eq!(arr.elements(), &[Value::Int(10), Value::Int(99)]);
}

#[test]
fn retain_only_name_can_be_written_but_cannot_be_ref_target() {
    let mut storage = VariableStorage::new();
    storage.set_retain("retained", Value::Int(1));

    write(
        &mut storage,
        None,
        &LValue::Name("retained".into()),
        Value::Int(2),
    )
    .expect("direct retained helper write");
    assert_eq!(storage.get_retain("retained"), Some(&Value::Int(2)));

    assert_eq!(
        write(
            &mut storage,
            None,
            &LValue::Deref(Box::new(Expr::Ref(LValue::Name("retained".into())))),
            Value::Int(3)
        ),
        Err(RuntimeError::UndefinedVariable("retained".into()))
    );
    assert_eq!(storage.get_retain("retained"), Some(&Value::Int(2)));
}

#[test]
fn integer_index_normalization_rejects_unsigned_host_overflow() {
    assert_eq!(index_to_i64(Value::ULInt(4)), Ok(4));
    assert_eq!(index_to_i64(Value::LWord(5)), Ok(5));
    assert_eq!(
        index_to_i64(Value::ULInt(u64::MAX)),
        Err(RuntimeError::Overflow)
    );
    assert_eq!(
        index_to_i64(Value::LWord(u64::MAX)),
        Err(RuntimeError::Overflow)
    );
    assert_eq!(
        index_to_i64(Value::Bool(true)),
        Err(RuntimeError::TypeMismatch)
    );
}

#[test]
fn array_offset_preserves_exact_error_partitions() {
    assert_eq!(array_offset(&[(5, 6)], &[Value::Int(6)]), Ok(1));
    assert_eq!(
        array_offset(&[(5, 6)], &[]),
        Err(RuntimeError::TypeMismatch)
    );
    assert_eq!(
        array_offset(&[(5, 6)], &[Value::Bool(true)]),
        Err(RuntimeError::TypeMismatch)
    );
    assert_eq!(
        array_offset(&[(5, 6)], &[Value::Int(7)]),
        Err(RuntimeError::IndexOutOfBounds {
            index: 7,
            lower: 5,
            upper: 6,
        })
    );
}

#[test]
fn partial_access_error_mapping_preserves_bounds_and_type_classes() {
    assert_eq!(
        partial_access_error_to_runtime(PartialAccessError::IndexOutOfBounds {
            index: 8,
            lower: 0,
            upper: 7,
        }),
        RuntimeError::IndexOutOfBounds {
            index: 8,
            lower: 0,
            upper: 7,
        }
    );
    assert_eq!(
        partial_access_error_to_runtime(PartialAccessError::TypeMismatch),
        RuntimeError::TypeMismatch
    );
}
