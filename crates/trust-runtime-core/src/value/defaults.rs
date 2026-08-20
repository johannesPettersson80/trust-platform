use alloc::{string::String, sync::Arc, vec::Vec};

use indexmap::IndexMap;
use smol_str::SmolStr;
use trust_hir::types::{ArrayDimensionExt, TypeRegistry};
use trust_hir::{Type, TypeId};

use super::{
    ArrayValue, DateTimeProfile, DateTimeValue, DateValue, Duration, EnumValue, LDateTimeValue,
    LDateValue, LTimeOfDayValue, StructValue, TimeOfDayValue, Value,
};

/// Errors when computing default values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DefaultValueError {
    /// Type ID is not registered.
    UnknownType,
    /// Type is not supported by the runtime value system yet.
    UnsupportedType,
    /// Enum has no variants to select a default.
    EmptyEnum,
    /// Array dimensions are invalid.
    InvalidArrayBounds,
}

/// Default value for a type ID using the provided registry and profile.
pub fn default_value_for_type_id(
    type_id: TypeId,
    registry: &TypeRegistry,
    profile: &DateTimeProfile,
) -> Result<Value, DefaultValueError> {
    let ty = registry
        .get(type_id)
        .ok_or(DefaultValueError::UnknownType)?;
    if registry.is_named_value_type(type_id) {
        let Type::Enum { base, .. } = ty else {
            return Err(DefaultValueError::UnsupportedType);
        };
        return default_value_for_type_id(*base, registry, profile);
    }
    default_value_for_type(ty, registry, profile)
}

fn default_value_for_type(
    ty: &Type,
    registry: &TypeRegistry,
    profile: &DateTimeProfile,
) -> Result<Value, DefaultValueError> {
    match ty {
        Type::Bool => Ok(Value::Bool(false)),
        Type::SInt => Ok(Value::SInt(0)),
        Type::Int => Ok(Value::Int(0)),
        Type::DInt => Ok(Value::DInt(0)),
        Type::LInt => Ok(Value::LInt(0)),
        Type::USInt => Ok(Value::USInt(0)),
        Type::UInt => Ok(Value::UInt(0)),
        Type::UDInt => Ok(Value::UDInt(0)),
        Type::ULInt => Ok(Value::ULInt(0)),
        Type::Real => Ok(Value::Real(0.0)),
        Type::LReal => Ok(Value::LReal(0.0)),
        Type::Byte => Ok(Value::Byte(0)),
        Type::Word => Ok(Value::Word(0)),
        Type::DWord => Ok(Value::DWord(0)),
        Type::LWord => Ok(Value::LWord(0)),
        Type::Time => Ok(Value::Time(Duration::ZERO)),
        Type::LTime => Ok(Value::LTime(Duration::ZERO)),
        Type::Date => Ok(Value::Date(DateValue::new(profile.epoch.ticks()))),
        Type::LDate => Ok(Value::LDate(LDateValue::new(0))),
        Type::Tod => Ok(Value::Tod(TimeOfDayValue::new(0))),
        Type::LTod => Ok(Value::LTod(LTimeOfDayValue::new(0))),
        Type::Dt => Ok(Value::Dt(DateTimeValue::new(profile.epoch.ticks()))),
        Type::Ldt => Ok(Value::Ldt(LDateTimeValue::new(0))),
        Type::String { .. } => Ok(Value::String(SmolStr::new(""))),
        Type::WString { .. } => Ok(Value::WString(String::new())),
        Type::Char => Ok(Value::Char(0)),
        Type::WChar => Ok(Value::WChar(0)),
        Type::Array {
            element,
            dimensions,
        } => {
            if dimensions.iter().any(ArrayDimensionExt::is_wildcard) {
                return Ok(Value::Array(Box::new(ArrayValue::from_canonical_parts(
                    Vec::new(),
                    dimensions.clone(),
                ))));
            }
            let total = array_len(dimensions)?;
            let mut elements = Vec::with_capacity(total);
            for _ in 0..total {
                elements.push(default_value_for_type_id(*element, registry, profile)?);
            }
            Ok(Value::Array(Box::new(ArrayValue::from_canonical_parts(
                elements,
                dimensions.clone(),
            ))))
        }
        Type::Struct { name, fields } => {
            let mut values = IndexMap::new();
            for field in fields {
                let field_value = default_value_for_type_id(field.type_id, registry, profile)?;
                values.insert(field.name.clone(), field_value);
            }
            Ok(Value::Struct(Arc::new(StructValue::from_canonical_parts(
                name.clone(),
                values,
            ))))
        }
        Type::Enum { name, values, .. } => {
            let (variant_name, numeric_value) =
                values.first().ok_or(DefaultValueError::EmptyEnum)?;
            Ok(Value::Enum(Box::new(EnumValue::from_canonical_parts(
                name.clone(),
                variant_name.clone(),
                *numeric_value,
            ))))
        }
        Type::Alias { target, .. } => default_value_for_type_id(*target, registry, profile),
        Type::Reference { .. } | Type::Pointer { .. } => Ok(Value::Reference(None)),
        Type::Subrange { base, lower, .. } => int_value_of_base(*base, *lower),
        Type::Null | Type::Interface { .. } => Ok(Value::Null),
        Type::Union { name, variants } => {
            let mut values = IndexMap::new();
            for variant in variants {
                let variant_value = default_value_for_type_id(variant.type_id, registry, profile)?;
                values.insert(variant.name.clone(), variant_value);
            }
            Ok(Value::Struct(Arc::new(StructValue::from_canonical_parts(
                name.clone(),
                values,
            ))))
        }
        Type::Unknown
        | Type::Void
        | Type::FunctionBlock { .. }
        | Type::Class { .. }
        | Type::Any
        | Type::AnyDerived
        | Type::AnyElementary
        | Type::AnyMagnitude
        | Type::AnyInt
        | Type::AnyUnsigned
        | Type::AnySigned
        | Type::AnyReal
        | Type::AnyNum
        | Type::AnyDuration
        | Type::AnyBit
        | Type::AnyChars
        | Type::AnyString
        | Type::AnyChar
        | Type::AnyDate => Err(DefaultValueError::UnsupportedType),
    }
}

fn array_len(dimensions: &[(i64, i64)]) -> Result<usize, DefaultValueError> {
    let mut total: i128 = 1;
    for (lower, upper) in dimensions {
        if upper < lower {
            return Err(DefaultValueError::InvalidArrayBounds);
        }
        let len = (*upper as i128) - (*lower as i128) + 1;
        total *= len;
    }
    usize::try_from(total).map_err(|_| DefaultValueError::InvalidArrayBounds)
}

fn int_value_of_base(base: TypeId, value: i64) -> Result<Value, DefaultValueError> {
    match base {
        TypeId::SINT => Ok(Value::SInt(value as i8)),
        TypeId::INT => Ok(Value::Int(value as i16)),
        TypeId::DINT => Ok(Value::DInt(value as i32)),
        TypeId::LINT => Ok(Value::LInt(value)),
        TypeId::USINT => Ok(Value::USInt(value as u8)),
        TypeId::UINT => Ok(Value::UInt(value as u16)),
        TypeId::UDINT => Ok(Value::UDInt(value as u32)),
        TypeId::ULINT => Ok(Value::ULInt(value as u64)),
        _ => Err(DefaultValueError::UnsupportedType),
    }
}

#[cfg(test)]
mod tests {
    use super::{default_value_for_type_id, DefaultValueError};
    use crate::value::{
        DateTimeProfile, DateTimeValue, DateValue, Duration, LDateTimeValue, LDateValue,
        LTimeOfDayValue, TimeOfDayValue, Value,
    };
    use alloc::{string::String, vec};
    use trust_hir::types::{StructField, TypeRegistry, UnionVariant};
    use trust_hir::{Type, TypeId};

    #[test]
    fn defaults_for_core_elementary_values_match_runtime_contract() {
        let registry = TypeRegistry::new();
        let profile = DateTimeProfile::default();
        let cases = [
            (TypeId::BOOL, Value::Bool(false)),
            (TypeId::SINT, Value::SInt(0)),
            (TypeId::INT, Value::Int(0)),
            (TypeId::DINT, Value::DInt(0)),
            (TypeId::LINT, Value::LInt(0)),
            (TypeId::USINT, Value::USInt(0)),
            (TypeId::UINT, Value::UInt(0)),
            (TypeId::UDINT, Value::UDInt(0)),
            (TypeId::ULINT, Value::ULInt(0)),
            (TypeId::REAL, Value::Real(0.0)),
            (TypeId::LREAL, Value::LReal(0.0)),
            (TypeId::BYTE, Value::Byte(0)),
            (TypeId::WORD, Value::Word(0)),
            (TypeId::DWORD, Value::DWord(0)),
            (TypeId::LWORD, Value::LWord(0)),
            (TypeId::TIME, Value::Time(Duration::ZERO)),
            (TypeId::LTIME, Value::LTime(Duration::ZERO)),
            (
                TypeId::DATE,
                Value::Date(DateValue::new(profile.epoch.ticks())),
            ),
            (TypeId::LDATE, Value::LDate(LDateValue::new(0))),
            (TypeId::TOD, Value::Tod(TimeOfDayValue::new(0))),
            (TypeId::LTOD, Value::LTod(LTimeOfDayValue::new(0))),
            (
                TypeId::DT,
                Value::Dt(DateTimeValue::new(profile.epoch.ticks())),
            ),
            (TypeId::LDT, Value::Ldt(LDateTimeValue::new(0))),
            (TypeId::STRING, Value::String("".into())),
            (TypeId::WSTRING, Value::WString(String::new())),
            (TypeId::CHAR, Value::Char(0)),
            (TypeId::WCHAR, Value::WChar(0)),
        ];

        for (type_id, expected) in cases {
            assert_eq!(
                default_value_for_type_id(type_id, &registry, &profile),
                Ok(expected),
                "wrong default for {}",
                type_id.builtin_name().expect("elementary type name")
            );
        }
    }

    #[test]
    fn compound_defaults_preserve_declared_shape_order_and_identity() {
        let mut registry = TypeRegistry::new();
        let profile = DateTimeProfile::default();

        let fixed_array = registry.register_array(TypeId::DINT, vec![(1, 2), (-1, 0)]);
        let Value::Array(array) =
            default_value_for_type_id(fixed_array, &registry, &profile).expect("fixed array")
        else {
            panic!("fixed array must retain the array runtime tag");
        };
        assert_eq!(array.dimensions(), &[(1, 2), (-1, 0)]);
        assert_eq!(array.elements(), vec![Value::DInt(0); 4]);

        let wildcard_array = registry.register_array(TypeId::BOOL, vec![(0, i64::MAX)]);
        let Value::Array(array) =
            default_value_for_type_id(wildcard_array, &registry, &profile).expect("wildcard array")
        else {
            panic!("wildcard array must retain the array runtime tag");
        };
        assert_eq!(array.dimensions(), &[(0, i64::MAX)]);
        assert!(array.elements().is_empty());

        let record = registry.register_struct(
            "Record",
            vec![
                StructField {
                    name: "count".into(),
                    type_id: TypeId::DINT,
                    address: None,
                    default_initializer: None,
                },
                StructField {
                    name: "enabled".into(),
                    type_id: TypeId::BOOL,
                    address: None,
                    default_initializer: None,
                },
            ],
        );
        let Value::Struct(record) =
            default_value_for_type_id(record, &registry, &profile).expect("struct")
        else {
            panic!("struct must retain the struct runtime tag");
        };
        assert_eq!(record.type_name().as_str(), "Record");
        assert_eq!(
            record
                .fields()
                .iter()
                .map(|(name, value)| (name.as_str(), value))
                .collect::<vec::Vec<_>>(),
            vec![("count", &Value::DInt(0)), ("enabled", &Value::Bool(false))]
        );

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
                    name: "flag".into(),
                    type_id: TypeId::BOOL,
                    address: None,
                    default_initializer: None,
                },
            ],
        );
        let Value::Struct(choice) =
            default_value_for_type_id(choice, &registry, &profile).expect("union")
        else {
            panic!("union defaults use the portable struct representation");
        };
        assert_eq!(choice.type_name().as_str(), "Choice");
        assert_eq!(
            choice
                .fields()
                .iter()
                .map(|(name, value)| (name.as_str(), value))
                .collect::<vec::Vec<_>>(),
            vec![("number", &Value::Int(0)), ("flag", &Value::Bool(false))]
        );

        let mode = registry.register_enum(
            "Mode",
            TypeId::INT,
            vec![("Manual".into(), 7), ("Auto".into(), 9)],
        );
        let Value::Enum(mode) = default_value_for_type_id(mode, &registry, &profile).expect("enum")
        else {
            panic!("enum must retain the enum runtime tag");
        };
        assert_eq!(mode.type_name().as_str(), "Mode");
        assert_eq!(mode.variant_name().as_str(), "Manual");
        assert_eq!(mode.numeric_value(), 7);

        let alias = registry.register(
            "Counter",
            Type::Alias {
                name: "Counter".into(),
                target: TypeId::DINT,
            },
        );
        assert_eq!(
            default_value_for_type_id(alias, &registry, &profile),
            Ok(Value::DInt(0))
        );

        let reference = registry.register_reference(TypeId::DINT);
        let pointer = registry.register_pointer(TypeId::BOOL);
        assert_eq!(
            default_value_for_type_id(reference, &registry, &profile),
            Ok(Value::Reference(None))
        );
        assert_eq!(
            default_value_for_type_id(pointer, &registry, &profile),
            Ok(Value::Reference(None))
        );
        assert_eq!(
            default_value_for_type_id(TypeId::NULL, &registry, &profile),
            Ok(Value::Null)
        );

        for (base, lower, expected) in [
            (TypeId::SINT, -8, Value::SInt(-8)),
            (TypeId::INT, -16, Value::Int(-16)),
            (TypeId::DINT, -32, Value::DInt(-32)),
            (TypeId::LINT, -64, Value::LInt(-64)),
            (TypeId::USINT, 8, Value::USInt(8)),
            (TypeId::UINT, 16, Value::UInt(16)),
            (TypeId::UDINT, 32, Value::UDInt(32)),
            (TypeId::ULINT, 64, Value::ULInt(64)),
        ] {
            let subrange = registry.register(
                format!("Subrange{}", base.0),
                Type::Subrange {
                    base,
                    lower,
                    upper: lower + 1,
                },
            );
            assert_eq!(
                default_value_for_type_id(subrange, &registry, &profile),
                Ok(expected)
            );
        }
    }

    #[test]
    fn defaults_reject_unknown_type_ids() {
        let registry = TypeRegistry::new();
        let profile = DateTimeProfile::default();

        assert_eq!(
            default_value_for_type_id(TypeId(u32::MAX), &registry, &profile),
            Err(DefaultValueError::UnknownType)
        );
    }

    #[test]
    fn default_construction_rejects_invalid_or_unsupported_types() {
        let mut registry = TypeRegistry::new();
        let profile = DateTimeProfile::default();
        let invalid_array = registry.register_array(TypeId::INT, vec![(3, 2)]);
        let empty_enum = registry.register_enum("Empty", TypeId::INT, vec![]);
        let invalid_subrange = registry.register(
            "BooleanSubrange",
            Type::Subrange {
                base: TypeId::BOOL,
                lower: 0,
                upper: 1,
            },
        );

        assert_eq!(
            default_value_for_type_id(invalid_array, &registry, &profile),
            Err(DefaultValueError::InvalidArrayBounds)
        );
        assert_eq!(
            default_value_for_type_id(empty_enum, &registry, &profile),
            Err(DefaultValueError::EmptyEnum)
        );
        assert_eq!(
            default_value_for_type_id(invalid_subrange, &registry, &profile),
            Err(DefaultValueError::UnsupportedType)
        );
        assert_eq!(
            default_value_for_type_id(TypeId::VOID, &registry, &profile),
            Err(DefaultValueError::UnsupportedType)
        );
    }

    #[test]
    fn interface_defaults_to_explicit_null_reference() {
        let mut registry = TypeRegistry::new();
        let profile = DateTimeProfile::default();
        let interface = registry.register(
            "IService",
            Type::Interface {
                name: "IService".into(),
            },
        );

        assert_eq!(
            default_value_for_type_id(interface, &registry, &profile),
            Ok(Value::Null)
        );
    }
}
