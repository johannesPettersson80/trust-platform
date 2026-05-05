use super::*;
use alloc::vec;
#[cfg(feature = "hir")]
use trust_hir::types::StructField;

#[cfg(feature = "hir")]
#[test]
fn enum_value_new_resolves_alias_to_canonical_enum_type() {
    let mut registry = TypeRegistry::new();
    let base = registry.register_enum(
        "Solo",
        TypeId::INT,
        vec![("S0".into(), 0), ("S1".into(), 1)],
    );
    let alias = registry.register(
        "AliasSolo",
        Type::Alias {
            name: "AliasSolo".into(),
            target: base,
        },
    );

    let from_base = EnumValue::new(&registry, base, "S1").expect("base enum value");
    let from_alias = EnumValue::new(&registry, alias, "s1").expect("alias enum value");

    assert_eq!(from_alias.type_name().as_str(), "Solo");
    assert_eq!(from_alias.variant_name().as_str(), "S1");
    assert_eq!(from_alias.numeric_value(), 1);
    assert_eq!(from_alias, from_base);
}

#[cfg(feature = "hir")]
#[test]
fn enum_value_from_serialized_parts_canonicalizes_and_validates_numeric_value() {
    let mut registry = TypeRegistry::new();
    registry.register_enum(
        "Solo",
        TypeId::INT,
        vec![("S0".into(), 0), ("S1".into(), 1)],
    );

    let value = EnumValue::from_serialized_parts(&registry, "SOLO", "s1", 1)
        .expect("serialized enum value");
    assert_eq!(value.type_name().as_str(), "Solo");
    assert_eq!(value.variant_name().as_str(), "S1");

    let error = EnumValue::from_serialized_parts(&registry, "SOLO", "S1", 0)
        .expect_err("numeric mismatch should fail");
    assert!(matches!(error, EnumValueError::NumericMismatch { .. }));
}

#[cfg(feature = "hir")]
#[test]
fn struct_value_new_canonicalizes_alias_fields_and_rejects_type_drift() {
    let mut registry = TypeRegistry::new();
    let point = registry.register_struct(
        "Point",
        vec![
            StructField {
                name: "x".into(),
                type_id: TypeId::INT,
                address: None,
                default_initializer: None,
            },
            StructField {
                name: "y".into(),
                type_id: TypeId::INT,
                address: None,
                default_initializer: None,
            },
        ],
    );
    let alias = registry.register(
        "PointAlias",
        Type::Alias {
            name: "PointAlias".into(),
            target: point,
        },
    );
    let fields = [("Y".into(), Value::Int(2)), ("X".into(), Value::Int(1))]
        .into_iter()
        .collect();

    let value = StructValue::new(&registry, alias, fields).expect("alias-backed struct");

    assert_eq!(value.type_name().as_str(), "Point");
    assert_eq!(
        value.fields().keys().cloned().collect::<Vec<_>>(),
        vec![SmolStr::new("x"), SmolStr::new("y")]
    );
    assert_eq!(value.fields().get("x"), Some(&Value::Int(1)));
    assert_eq!(value.fields().get("y"), Some(&Value::Int(2)));

    let bad_fields = [("x".into(), Value::Bool(true)), ("y".into(), Value::Int(2))]
        .into_iter()
        .collect();
    let error =
        StructValue::new(&registry, point, bad_fields).expect_err("wrong field type must fail");
    assert!(matches!(
        error,
        ValueConstructionError::FieldTypeMismatch { .. }
    ));

    let missing_error = StructValue::new(
        &registry,
        point,
        [(SmolStr::new("x"), Value::Int(1))].into_iter().collect(),
    )
    .expect_err("missing field must fail");
    assert!(matches!(
        missing_error,
        ValueConstructionError::MissingField { .. }
    ));

    let extra_error = StructValue::new(
        &registry,
        point,
        [
            (SmolStr::new("x"), Value::Int(1)),
            (SmolStr::new("y"), Value::Int(2)),
            (SmolStr::new("z"), Value::Int(3)),
        ]
        .into_iter()
        .collect(),
    )
    .expect_err("extra field must fail");
    assert!(matches!(
        extra_error,
        ValueConstructionError::ExtraField { .. }
    ));
}

#[test]
fn struct_value_mutator_updates_existing_fields_only() {
    let mut value = StructValue::from_untyped_parts(
        "Point".into(),
        [
            (SmolStr::new("x"), Value::Int(1)),
            (SmolStr::new("y"), Value::Int(2)),
        ]
        .into_iter()
        .collect(),
    );

    assert!(value.contains_field("x"));
    assert_eq!(value.field("x"), Some(&Value::Int(1)));
    assert!(value.set_existing_field("x".into(), Value::Int(10)));
    assert!(!value.set_existing_field("z".into(), Value::Int(99)));
    assert_eq!(value.field("x"), Some(&Value::Int(10)));
    assert!(!value.contains_field("z"));
}

#[test]
fn struct_value_clone_and_equality_preserve_field_identity() {
    let value = StructValue::from_untyped_parts(
        "Point".into(),
        [
            (SmolStr::new("x"), Value::Int(1)),
            (SmolStr::new("y"), Value::Bool(true)),
        ]
        .into_iter()
        .collect(),
    );

    let cloned = value.clone();

    assert_eq!(cloned, value);
    assert_eq!(cloned.type_name(), value.type_name());
    assert_eq!(
        cloned.fields().keys().collect::<Vec<_>>(),
        value.fields().keys().collect::<Vec<_>>()
    );
    assert_eq!(cloned.field("x"), Some(&Value::Int(1)));
    assert_eq!(cloned.field("y"), Some(&Value::Bool(true)));
}

#[cfg(feature = "hir")]
#[test]
fn array_value_new_canonicalizes_alias_and_rejects_shape_or_type_drift() {
    let mut registry = TypeRegistry::new();
    let base = registry.register_array(TypeId::INT, vec![(1, 2)]);
    let alias = registry.register(
        "IntArrayAlias",
        Type::Alias {
            name: "IntArrayAlias".into(),
            target: base,
        },
    );

    let value = ArrayValue::new(&registry, alias, vec![Value::Int(1), Value::Int(2)])
        .expect("alias-backed array");

    assert_eq!(value.dimensions(), &[(1, 2)]);
    assert_eq!(value.elements(), &[Value::Int(1), Value::Int(2)]);

    let count_error = ArrayValue::new(&registry, base, vec![Value::Int(1)])
        .expect_err("wrong element count must fail");
    assert!(matches!(
        count_error,
        ValueConstructionError::ArrayElementCountMismatch { .. }
    ));

    let type_error = ArrayValue::new(&registry, base, vec![Value::Int(1), Value::Bool(false)])
        .expect_err("wrong element type must fail");
    assert!(matches!(
        type_error,
        ValueConstructionError::ArrayElementTypeMismatch { .. }
    ));

    let bounds_error = ArrayValue::from_untyped_parts(Vec::new(), vec![(2, 1)])
        .expect_err("invalid array bounds must fail");
    assert!(matches!(
        bounds_error,
        ValueConstructionError::InvalidArrayBounds { .. }
    ));
}

#[test]
fn array_value_mutators_preserve_shape_contract() {
    let mut value =
        ArrayValue::from_untyped_parts(vec![Value::Int(1), Value::Int(2)], vec![(1, 2)])
            .expect("array value");

    value.elements_mut()[1] = Value::Int(20);
    assert_eq!(value.elements(), &[Value::Int(1), Value::Int(20)]);
    value
        .set_dimensions(vec![(0, 1)])
        .expect("same element count dimensions");
    assert_eq!(value.dimensions(), &[(0, 1)]);

    let error = value
        .set_dimensions(vec![(0, 2)])
        .expect_err("different element count must fail");
    assert!(matches!(
        error,
        ValueConstructionError::ArrayElementCountMismatch { .. }
    ));
}

#[test]
fn array_value_clone_and_equality_preserve_shape_and_elements() {
    let value = ArrayValue::from_untyped_parts(
        vec![
            Value::Int(1),
            Value::Bool(false),
            Value::String("tag".into()),
        ],
        vec![(1, 3)],
    )
    .expect("array value");

    let cloned = value.clone();

    assert_eq!(cloned, value);
    assert_eq!(cloned.dimensions(), value.dimensions());
    assert_eq!(cloned.elements(), value.elements());
}

#[cfg(feature = "hir")]
#[test]
fn array_value_new_validates_array_of_struct_elements() {
    let mut registry = TypeRegistry::new();
    let point = registry.register_struct(
        "Point",
        vec![StructField {
            name: "x".into(),
            type_id: TypeId::INT,
            address: None,
            default_initializer: None,
        }],
    );
    let point_array = registry.register_array(point, vec![(1, 2)]);
    let first = StructValue::new(
        &registry,
        point,
        [(SmolStr::new("x"), Value::Int(1))].into_iter().collect(),
    )
    .expect("first point");
    let second = StructValue::new(
        &registry,
        point,
        [(SmolStr::new("x"), Value::Int(2))].into_iter().collect(),
    )
    .expect("second point");

    let value = ArrayValue::new(
        &registry,
        point_array,
        vec![
            Value::Struct(Arc::new(first)),
            Value::Struct(Arc::new(second)),
        ],
    )
    .expect("array of structs");

    assert_eq!(value.dimensions(), &[(1, 2)]);
    assert_eq!(value.elements().len(), 2);

    let error = ArrayValue::new(
        &registry,
        point_array,
        vec![
            Value::Struct(Arc::new(StructValue::from_untyped_parts(
                "Point".into(),
                [(SmolStr::new("x"), Value::Int(1))].into_iter().collect(),
            ))),
            Value::Bool(false),
        ],
    )
    .expect_err("array element type drift must fail");
    assert!(matches!(
        error,
        ValueConstructionError::ArrayElementTypeMismatch { .. }
    ));
}

#[cfg(feature = "hir")]
#[test]
fn interface_type_accepts_null_and_instance_values() {
    let mut registry = TypeRegistry::new();
    let interface = registry.register(
        "IService",
        Type::Interface {
            name: "IService".into(),
        },
    );

    assert!(value_matches_type(&registry, interface, &Value::Null));
    assert!(value_matches_type(
        &registry,
        interface,
        &Value::Instance(InstanceId(7))
    ));
    assert!(!value_matches_type(
        &registry,
        interface,
        &Value::Bool(false)
    ));
}
