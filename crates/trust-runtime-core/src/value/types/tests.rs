use super::*;
use crate::memory::MemoryLocation;
use alloc::vec;
#[cfg(feature = "hir")]
use trust_hir::types::{StructField, UnionVariant};

#[test]
fn primitive_from_conversions_preserve_exact_runtime_tags() {
    assert_eq!(Value::from(true), Value::Bool(true));
    assert_eq!(Value::from(i16::MIN), Value::Int(i16::MIN));
    assert_eq!(Value::from(i32::MIN), Value::DInt(i32::MIN));
    assert_eq!(Value::from(i64::MIN), Value::LInt(i64::MIN));
    assert_eq!(Value::from(u8::MAX), Value::USInt(u8::MAX));
    assert_eq!(Value::from(u16::MAX), Value::UInt(u16::MAX));
}

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

#[test]
fn normalize_assignment_materializes_safe_numeric_widening_tags() {
    assert_eq!(
        normalize_assignment_for_target(&Value::DInt(0), Value::Int(200)),
        Value::DInt(200)
    );
    assert_eq!(
        normalize_assignment_for_target(&Value::Real(0.0), Value::Int(1)),
        Value::Real(1.0)
    );
    assert_eq!(
        normalize_assignment_for_target(&Value::LReal(0.0), Value::Real(1.25)),
        Value::LReal(1.25)
    );
    assert_eq!(
        normalize_assignment_for_target(&Value::UDInt(0), Value::UInt(7)),
        Value::UDInt(7)
    );
    assert_eq!(
        normalize_assignment_for_target(&Value::DWord(0), Value::Word(16)),
        Value::DWord(16)
    );
}

#[test]
fn normalize_assignment_preserves_null_reference_and_leaves_non_widening_values() {
    assert_eq!(
        normalize_assignment_for_target(&Value::Reference(Some(dummy_ref())), Value::Null),
        Value::Reference(None)
    );
    assert_eq!(
        normalize_assignment_for_target(&Value::Int(0), Value::DInt(32_000)),
        Value::DInt(32_000)
    );
    assert_eq!(
        normalize_assignment_for_target(&Value::Bool(false), Value::Int(1)),
        Value::Int(1)
    );
    assert_eq!(
        normalize_assignment_for_target(&Value::Real(0.0), Value::DInt(16_777_217)),
        Value::DInt(16_777_217),
        "DINT cannot be widened implicitly to REAL without possible precision loss"
    );
    assert_eq!(
        normalize_assignment_for_target(&Value::LReal(0.0), Value::LInt(9_007_199_254_740_993)),
        Value::LInt(9_007_199_254_740_993),
        "LINT cannot be widened implicitly to LREAL without possible precision loss"
    );
}

fn dummy_ref() -> ValueRef {
    ValueRef {
        location: MemoryLocation::Global,
        offset: 0,
        path: Vec::new(),
    }
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

#[cfg(feature = "hir")]
#[test]
fn array_serialized_construction_rejects_shape_type_and_registry_drift() {
    let mut registry = TypeRegistry::new();
    let array = registry.register_array(TypeId::INT, vec![(1, 2), (-1, 0)]);

    let value = ArrayValue::from_serialized_parts(
        &registry,
        array,
        vec![(1, 2), (-1, 0)],
        vec![Value::Int(1), Value::Int(2), Value::Int(3), Value::Int(4)],
    )
    .expect("canonical serialized array");
    assert_eq!(value.dimensions(), &[(1, 2), (-1, 0)]);
    assert_eq!(
        value.elements(),
        &[Value::Int(1), Value::Int(2), Value::Int(3), Value::Int(4)]
    );

    assert_eq!(
        ArrayValue::from_serialized_parts(
            &registry,
            array,
            vec![(0, 1), (-1, 0)],
            vec![Value::Int(1), Value::Int(2), Value::Int(3), Value::Int(4)],
        ),
        Err(ValueConstructionError::ArrayDimensionsMismatch {
            expected: vec![(1, 2), (-1, 0)],
            actual: vec![(0, 1), (-1, 0)],
        })
    );
    assert_eq!(
        ArrayValue::new(
            &registry,
            array,
            vec![Value::Int(1), Value::Int(2), Value::Int(3)]
        ),
        Err(ValueConstructionError::ArrayElementCountMismatch {
            expected: 4,
            actual: 3,
        })
    );
    assert_eq!(
        ArrayValue::new(
            &registry,
            array,
            vec![
                Value::Int(1),
                Value::Int(2),
                Value::Bool(false),
                Value::Int(4),
            ]
        ),
        Err(ValueConstructionError::ArrayElementTypeMismatch {
            index: 2,
            expected: TypeId::INT,
            actual: "BOOL",
        })
    );
    assert_eq!(
        ArrayValue::new(&registry, TypeId::BOOL, Vec::new()),
        Err(ValueConstructionError::NotArray(TypeId::BOOL))
    );
    let unknown = TypeId(900_001);
    assert_eq!(
        ArrayValue::new(&registry, unknown, Vec::new()),
        Err(ValueConstructionError::UnknownType(unknown))
    );

    let cycle = register_alias_cycle(&mut registry, "ArrayCycle");
    assert_eq!(
        ArrayValue::new(&registry, cycle, Vec::new()),
        Err(ValueConstructionError::AliasCycle(cycle))
    );

    let invalid_bounds = registry.register_array(TypeId::INT, vec![(2, 1)]);
    assert_eq!(
        ArrayValue::new(&registry, invalid_bounds, Vec::new()),
        Err(ValueConstructionError::InvalidArrayBounds {
            dimensions: vec![(2, 1)],
        })
    );
    assert_eq!(
        ArrayValue::from_untyped_parts(Vec::new(), vec![(i64::MIN, i64::MAX)]),
        Err(ValueConstructionError::InvalidArrayBounds {
            dimensions: vec![(i64::MIN, i64::MAX)],
        })
    );
}

#[cfg(feature = "hir")]
#[test]
fn struct_serialized_construction_preserves_union_order_and_nested_identity() {
    let mut registry = TypeRegistry::new();
    let choice = registry.register_union(
        "Choice",
        vec![
            UnionVariant {
                name: "number".into(),
                type_id: TypeId::INT,
                address: None,
                default_initializer: None,
            },
            UnionVariant {
                name: "enabled".into(),
                type_id: TypeId::BOOL,
                address: None,
                default_initializer: None,
            },
        ],
    );
    let fields = [
        (SmolStr::new("ENABLED"), Value::Bool(true)),
        (SmolStr::new("NUMBER"), Value::Int(7)),
    ]
    .into_iter()
    .collect();
    let mut value =
        StructValue::from_serialized_parts(&registry, "choice", fields).expect("union value");

    assert_eq!(value.type_name().as_str(), "Choice");
    assert_eq!(
        value
            .fields()
            .keys()
            .map(SmolStr::as_str)
            .collect::<Vec<_>>(),
        ["number", "enabled"]
    );
    assert_eq!(value.field("number"), Some(&Value::Int(7)));
    assert_eq!(value.field("NUMBER"), None);
    *value.field_mut("number").expect("canonical mutable field") = Value::Int(8);
    assert_eq!(value.field("number"), Some(&Value::Int(8)));

    assert_eq!(
        StructValue::from_serialized_parts(&registry, "Absent", IndexMap::new()),
        Err(ValueConstructionError::UnknownTypeName("Absent".into()))
    );
    assert_eq!(
        StructValue::new(&registry, TypeId::BOOL, IndexMap::new()),
        Err(ValueConstructionError::NotStructOrUnion(TypeId::BOOL))
    );
    let unknown = TypeId(900_002);
    assert_eq!(
        StructValue::new(&registry, unknown, IndexMap::new()),
        Err(ValueConstructionError::UnknownType(unknown))
    );
    let cycle = register_alias_cycle(&mut registry, "StructCycle");
    assert_eq!(
        StructValue::new(&registry, cycle, IndexMap::new()),
        Err(ValueConstructionError::AliasCycle(cycle))
    );

    let wrapper = registry.register_struct(
        "Wrapper",
        vec![StructField {
            name: "choice".into(),
            type_id: choice,
            address: None,
            default_initializer: None,
        }],
    );
    let canonical_choice = StructValue::new(
        &registry,
        choice,
        [
            (SmolStr::new("number"), Value::Int(1)),
            (SmolStr::new("enabled"), Value::Bool(false)),
        ]
        .into_iter()
        .collect(),
    )
    .expect("canonical nested union");
    StructValue::new(
        &registry,
        wrapper,
        [(
            SmolStr::new("choice"),
            Value::Struct(Arc::new(canonical_choice)),
        )]
        .into_iter()
        .collect(),
    )
    .expect("matching nested identity");

    let wrong_identity = StructValue::from_untyped_parts(
        "OtherChoice".into(),
        [
            (SmolStr::new("number"), Value::Int(1)),
            (SmolStr::new("enabled"), Value::Bool(false)),
        ]
        .into_iter()
        .collect(),
    );
    assert!(matches!(
        StructValue::new(
            &registry,
            wrapper,
            [(
                SmolStr::new("choice"),
                Value::Struct(Arc::new(wrong_identity)),
            )]
            .into_iter()
            .collect(),
        ),
        Err(ValueConstructionError::FieldTypeMismatch {
            type_name,
            field_name,
            expected,
            actual: "STRUCT",
        }) if type_name == "Wrapper" && field_name == "choice" && expected == choice
    ));
}

#[cfg(feature = "hir")]
#[test]
fn enum_construction_rejects_registry_variant_and_numeric_identity_drift() {
    let mut registry = TypeRegistry::new();
    let mode = registry.register_enum(
        "Mode",
        TypeId::INT,
        vec![("Manual".into(), -1), ("Automatic".into(), 4)],
    );

    assert_eq!(
        EnumValue::new(&registry, TypeId::BOOL, "FALSE"),
        Err(EnumValueError::NotEnum(TypeId::BOOL))
    );
    let unknown = TypeId(900_003);
    assert_eq!(
        EnumValue::new(&registry, unknown, "Manual"),
        Err(EnumValueError::UnknownType(unknown))
    );
    assert_eq!(
        EnumValue::from_serialized_parts(&registry, "Absent", "Manual", -1),
        Err(EnumValueError::UnknownTypeName("Absent".into()))
    );
    assert_eq!(
        EnumValue::new(&registry, mode, "Unknown"),
        Err(EnumValueError::UnknownVariant {
            type_name: "Mode".into(),
            variant_name: "Unknown".into(),
        })
    );
    assert_eq!(
        EnumValue::new_with_numeric(&registry, mode, "automatic", 5),
        Err(EnumValueError::NumericMismatch {
            type_name: "Mode".into(),
            variant_name: "Automatic".into(),
            expected: 4,
            actual: 5,
        })
    );

    let cycle = register_alias_cycle(&mut registry, "EnumCycle");
    assert_eq!(
        EnumValue::new(&registry, cycle, "Manual"),
        Err(EnumValueError::AliasCycle(cycle))
    );

    let canonical = EnumValue::from_canonical_parts("Mode".into(), "Manual".into(), -1);
    let alternate_label = EnumValue::from_canonical_parts("Mode".into(), "Alias".into(), -1);
    let other_type = EnumValue::from_canonical_parts("Other".into(), "Manual".into(), -1);
    let other_number = EnumValue::from_canonical_parts("Mode".into(), "Manual".into(), 4);
    assert_eq!(canonical, alternate_label);
    assert_ne!(canonical, other_type);
    assert_ne!(canonical, other_number);
    assert_eq!(canonical.type_name().as_str(), "Mode");
    assert_eq!(canonical.variant_name().as_str(), "Manual");
    assert_eq!(canonical.numeric_value(), -1);
}

#[cfg(feature = "hir")]
#[test]
fn declared_type_matching_covers_elementary_and_recursive_value_boundaries() {
    let mut registry = TypeRegistry::new();
    let profile = crate::value::DateTimeProfile::default();
    let elementary = [
        TypeId::BOOL,
        TypeId::SINT,
        TypeId::INT,
        TypeId::DINT,
        TypeId::LINT,
        TypeId::USINT,
        TypeId::UINT,
        TypeId::UDINT,
        TypeId::ULINT,
        TypeId::REAL,
        TypeId::LREAL,
        TypeId::BYTE,
        TypeId::WORD,
        TypeId::DWORD,
        TypeId::LWORD,
        TypeId::TIME,
        TypeId::LTIME,
        TypeId::DATE,
        TypeId::LDATE,
        TypeId::TOD,
        TypeId::LTOD,
        TypeId::DT,
        TypeId::LDT,
        TypeId::STRING,
        TypeId::WSTRING,
        TypeId::CHAR,
        TypeId::WCHAR,
        TypeId::NULL,
    ];
    for type_id in elementary {
        let value = crate::value::default_value_for_type_id(type_id, &registry, &profile)
            .expect("elementary default");
        assert!(
            value_matches_type(&registry, type_id, &value),
            "{type_id:?} must accept its exact runtime tag"
        );
        assert!(
            !value_matches_type(&registry, type_id, &Value::Instance(InstanceId(41))),
            "{type_id:?} must reject a different runtime tag"
        );
    }

    let subrange = registry.register(
        "Small",
        Type::Subrange {
            base: TypeId::INT,
            lower: -2,
            upper: 2,
        },
    );
    assert!(value_matches_type(&registry, subrange, &Value::Int(-2)));
    assert!(value_matches_type(&registry, subrange, &Value::Int(2)));
    assert!(!value_matches_type(&registry, subrange, &Value::Int(3)));
    assert!(!value_matches_type(&registry, subrange, &Value::DInt(1)));

    let reference = registry.register_reference(TypeId::INT);
    let pointer = registry.register_pointer(TypeId::INT);
    for type_id in [reference, pointer] {
        assert!(value_matches_type(
            &registry,
            type_id,
            &Value::Reference(None)
        ));
        assert!(value_matches_type(&registry, type_id, &Value::Null));
        assert!(!value_matches_type(&registry, type_id, &Value::Int(0)));
    }

    let mode = registry.register_enum(
        "Mode",
        TypeId::INT,
        vec![("Manual".into(), 0), ("Automatic".into(), 1)],
    );
    let other_mode = registry.register_enum("OtherMode", TypeId::INT, vec![("Manual".into(), 0)]);
    let mode_value = Value::Enum(Box::new(
        EnumValue::new(&registry, mode, "Manual").expect("mode"),
    ));
    assert!(value_matches_type(&registry, mode, &mode_value));
    assert!(!value_matches_type(&registry, other_mode, &mode_value));

    let point = registry.register_struct(
        "Point",
        vec![StructField {
            name: "x".into(),
            type_id: TypeId::INT,
            address: None,
            default_initializer: None,
        }],
    );
    let point_value = Value::Struct(Arc::new(
        StructValue::new(
            &registry,
            point,
            [(SmolStr::new("x"), Value::Int(1))].into_iter().collect(),
        )
        .expect("point"),
    ));
    assert!(value_matches_type(&registry, point, &point_value));
    let wrong_point = Value::Struct(Arc::new(StructValue::from_untyped_parts(
        "Point".into(),
        [(SmolStr::new("x"), Value::Bool(false))]
            .into_iter()
            .collect(),
    )));
    assert!(!value_matches_type(&registry, point, &wrong_point));

    let points = registry.register_array(point, vec![(0, 0)]);
    let array_value = Value::Array(Box::new(
        ArrayValue::new(&registry, points, vec![point_value]).expect("point array"),
    ));
    assert!(value_matches_type(&registry, points, &array_value));
    let wrong_shape = Value::Array(Box::new(ArrayValue::from_canonical_parts(
        vec![Value::Int(1)],
        vec![(1, 1)],
    )));
    assert!(!value_matches_type(&registry, points, &wrong_shape));

    let unsupported = registry.register("Opaque", Type::Unknown);
    assert!(!value_matches_type(&registry, unsupported, &Value::Null));
    assert!(!value_matches_type(
        &registry,
        TypeId(900_004),
        &Value::Null
    ));
}

#[cfg(feature = "hir")]
fn register_alias_cycle(registry: &mut TypeRegistry, prefix: &str) -> TypeId {
    let first = registry.reserve(format!("{prefix}A"));
    let second = registry.reserve(format!("{prefix}B"));
    registry.replace(
        first,
        Type::Alias {
            name: format!("{prefix}A").into(),
            target: second,
        },
    );
    registry.replace(
        second,
        Type::Alias {
            name: format!("{prefix}B").into(),
            target: first,
        },
    );
    first
}

#[cfg(feature = "hir")]
mod construction_error_tests;
