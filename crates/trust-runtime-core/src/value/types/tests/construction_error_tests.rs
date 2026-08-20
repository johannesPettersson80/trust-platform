use alloc::vec;
use trust_hir::TypeId;

use super::super::{EnumValueError, ValueConstructionError};

#[test]
fn construction_error_messages_and_enum_conversion_are_stable() {
    let errors = [
        (
            ValueConstructionError::UnknownType(TypeId(91)),
            "unknown type id 91",
        ),
        (
            ValueConstructionError::UnknownTypeName("Missing".into()),
            "unknown type 'Missing'",
        ),
        (
            ValueConstructionError::AliasCycle(TypeId(92)),
            "alias cycle while resolving type id 92",
        ),
        (
            ValueConstructionError::NotStruct(TypeId(93)),
            "type id 93 is not a struct",
        ),
        (
            ValueConstructionError::NotArray(TypeId(94)),
            "type id 94 is not an array",
        ),
        (
            ValueConstructionError::NotStructOrUnion(TypeId(95)),
            "type id 95 is not a struct or union",
        ),
        (
            ValueConstructionError::UnsupportedType(TypeId(96)),
            "type id 96 cannot be represented as a runtime value",
        ),
        (
            ValueConstructionError::InvalidArrayBounds {
                dimensions: vec![(2, 1)],
            },
            "invalid array dimensions [(2, 1)]",
        ),
        (
            ValueConstructionError::ArrayDimensionsMismatch {
                expected: vec![(1, 2)],
                actual: vec![(0, 1)],
            },
            "array dimensions mismatch: expected [(1, 2)], got [(0, 1)]",
        ),
        (
            ValueConstructionError::ArrayElementCountMismatch {
                expected: 2,
                actual: 1,
            },
            "array element count mismatch: expected 2, got 1",
        ),
        (
            ValueConstructionError::ArrayElementTypeMismatch {
                index: 1,
                expected: TypeId::INT,
                actual: "BOOL",
            },
            "array element 1 type mismatch: expected type id 4, got BOOL",
        ),
        (
            ValueConstructionError::MissingField {
                type_name: "Point".into(),
                field_name: "x".into(),
            },
            "missing field 'Point.x'",
        ),
        (
            ValueConstructionError::ExtraField {
                type_name: "Point".into(),
                field_name: "z".into(),
            },
            "extra field 'Point.z'",
        ),
        (
            ValueConstructionError::FieldTypeMismatch {
                type_name: "Point".into(),
                field_name: "x".into(),
                expected: TypeId::INT,
                actual: "BOOL",
            },
            "field 'Point.x' type mismatch: expected type id 4, got BOOL",
        ),
        (
            ValueConstructionError::TypeMismatch {
                expected: TypeId::INT,
                actual: "BOOL",
            },
            "value type mismatch: expected type id 4, got BOOL",
        ),
    ];
    for (error, expected) in errors {
        assert_eq!(error.to_string(), expected);
    }

    let enum_errors = [
        (
            EnumValueError::UnknownType(TypeId(97)),
            "unknown enum type id 97",
        ),
        (
            EnumValueError::UnknownTypeName("Missing".into()),
            "unknown enum type 'Missing'",
        ),
        (
            EnumValueError::AliasCycle(TypeId(98)),
            "alias cycle while resolving enum type id 98",
        ),
        (
            EnumValueError::NotEnum(TypeId(99)),
            "type id 99 is not an enum",
        ),
        (
            EnumValueError::UnknownVariant {
                type_name: "Mode".into(),
                variant_name: "Missing".into(),
            },
            "unknown enum variant 'Mode#Missing'",
        ),
        (
            EnumValueError::NumericMismatch {
                type_name: "Mode".into(),
                variant_name: "Manual".into(),
                expected: 1,
                actual: 2,
            },
            "enum variant 'Mode#Manual' has value 1, got 2",
        ),
    ];
    for (error, expected) in enum_errors {
        assert_eq!(error.to_string(), expected);
    }

    let enum_error = EnumValueError::UnknownVariant {
        type_name: "Mode".into(),
        variant_name: "Missing".into(),
    };
    assert_eq!(
        ValueConstructionError::from(enum_error.clone()),
        ValueConstructionError::Enum(enum_error)
    );
}
