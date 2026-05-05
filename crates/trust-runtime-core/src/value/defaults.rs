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
    use crate::value::{DateTimeProfile, Duration, Value};
    use trust_hir::types::TypeRegistry;
    use trust_hir::{Type, TypeId};

    #[test]
    fn defaults_for_core_elementary_values_match_runtime_contract() {
        let registry = TypeRegistry::new();
        let profile = DateTimeProfile::default();

        assert_eq!(
            default_value_for_type_id(TypeId::BOOL, &registry, &profile),
            Ok(Value::Bool(false))
        );
        assert_eq!(
            default_value_for_type_id(TypeId::TIME, &registry, &profile),
            Ok(Value::Time(Duration::ZERO))
        );
        assert_eq!(
            default_value_for_type_id(TypeId::STRING, &registry, &profile),
            Ok(Value::String("".into()))
        );
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
