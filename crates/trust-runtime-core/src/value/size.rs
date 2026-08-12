use trust_hir::types::{TypeRegistry, POINTER_REFERENCE_HANDLE_SIZE_BYTES};
use trust_hir::{Type, TypeId};

use crate::value::{string_element_count, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SizeOfError {
    UnknownType,
    UnsupportedType,
    Overflow,
}

pub fn size_of_type(type_id: TypeId, registry: &TypeRegistry) -> Result<u64, SizeOfError> {
    let ty = registry.get(type_id).ok_or(SizeOfError::UnknownType)?;
    match ty {
        Type::Alias { target, .. } => size_of_type(*target, registry),
        Type::Subrange { base, .. } => size_of_type(*base, registry),
        Type::Enum { base, .. } => size_of_type(*base, registry),
        Type::Array {
            element,
            dimensions,
        } => {
            if dimensions
                .iter()
                .any(|(lower, upper)| *lower == 0 && *upper == i64::MAX)
            {
                return Err(SizeOfError::UnsupportedType);
            }
            let element_size = size_of_type(*element, registry)?;
            let len = array_len_bits(dimensions).ok_or(SizeOfError::UnsupportedType)?;
            element_size.checked_mul(len).ok_or(SizeOfError::Overflow)
        }
        Type::Struct { fields, .. } => {
            let mut total = 0u64;
            for field in fields {
                let size = size_of_type(field.type_id, registry)?;
                total = total.checked_add(size).ok_or(SizeOfError::Overflow)?;
            }
            Ok(total)
        }
        Type::Union { variants, .. } => {
            let mut max = 0u64;
            for variant in variants {
                let size = size_of_type(variant.type_id, registry)?;
                max = max.max(size);
            }
            Ok(max)
        }
        Type::String { max_len } => max_len.map(u64::from).ok_or(SizeOfError::UnsupportedType),
        Type::WString { max_len } => max_len
            .map(|len| u64::from(len) * 2)
            .ok_or(SizeOfError::UnsupportedType),
        Type::Reference { .. } | Type::Pointer { .. } => Ok(POINTER_REFERENCE_HANDLE_SIZE_BYTES),
        Type::Time | Type::Date | Type::Tod | Type::Dt => Ok(4),
        Type::LTime | Type::LDate | Type::LTod | Type::Ldt => Ok(8),
        _ => {
            let bits = ty.bit_size().ok_or(SizeOfError::UnsupportedType)?;
            Ok(u64::from(bits.div_ceil(8)))
        }
    }
}

pub fn size_of_value(registry: &TypeRegistry, value: &Value) -> Result<u64, SizeOfError> {
    let size = match value {
        Value::Bool(_) => 1,
        Value::SInt(_) | Value::USInt(_) | Value::Byte(_) | Value::Char(_) => 1,
        Value::Int(_) | Value::UInt(_) | Value::Word(_) | Value::WChar(_) => 2,
        Value::DInt(_) | Value::UDInt(_) | Value::DWord(_) | Value::Real(_) => 4,
        Value::LInt(_) | Value::ULInt(_) | Value::LWord(_) | Value::LReal(_) => 8,
        Value::Time(_) | Value::Date(_) | Value::Tod(_) | Value::Dt(_) => 4,
        Value::LTime(_) | Value::LDate(_) | Value::LTod(_) | Value::Ldt(_) => 8,
        Value::String(value) => string_element_count(value.as_str()) as u64,
        Value::WString(value) => (string_element_count(value.as_str()) as u64) * 2,
        Value::Array(array) => {
            let element_size = match array.elements().first() {
                Some(value) => size_of_value(registry, value)?,
                None => 0,
            };
            let len = array_len_bits(array.dimensions()).ok_or(SizeOfError::UnsupportedType)?;
            element_size.checked_mul(len).ok_or(SizeOfError::Overflow)?
        }
        Value::Struct(struct_value) => {
            let mut total = 0u64;
            for value in struct_value.fields().values() {
                let size = size_of_value(registry, value)?;
                total = total.checked_add(size).ok_or(SizeOfError::Overflow)?;
            }
            total
        }
        Value::Enum(enum_value) => {
            let type_id = registry
                .lookup(enum_value.type_name().as_str())
                .ok_or(SizeOfError::UnsupportedType)?;
            size_of_type(type_id, registry)?
        }
        Value::Reference(_) => POINTER_REFERENCE_HANDLE_SIZE_BYTES,
        Value::Instance(_) => u64::try_from(core::mem::size_of::<crate::memory::InstanceId>())
            .map_err(|_| SizeOfError::Overflow)?,
        Value::Null => return Err(SizeOfError::UnsupportedType),
    };
    Ok(size)
}

fn array_len_bits(dimensions: &[(i64, i64)]) -> Option<u64> {
    let mut total: i128 = 1;
    for (lower, upper) in dimensions {
        let len = i128::from(*upper) - i128::from(*lower) + 1;
        if len <= 0 {
            return None;
        }
        total = total.checked_mul(len)?;
    }
    u64::try_from(total).ok()
}

#[cfg(test)]
mod tests {
    use super::{array_len_bits, size_of_type, size_of_value, SizeOfError};
    use crate::value::{ArrayValue, StructValue, Value};
    use alloc::{boxed::Box, sync::Arc, vec};
    use smol_str::SmolStr;
    use trust_hir::types::{
        StructField, TypeRegistry, UnionVariant, POINTER_REFERENCE_HANDLE_SIZE_BYTES,
    };
    use trust_hir::{Type, TypeId};

    #[test]
    fn string_value_size_counts_character_elements() {
        let registry = TypeRegistry::default();
        assert_eq!(
            size_of_value(&registry, &Value::String("ÄB".into())).unwrap(),
            2
        );
        assert_eq!(
            size_of_value(&registry, &Value::WString("ÄB".into())).unwrap(),
            4
        );
    }

    #[test]
    fn sizeof_contract_covers_declared_and_runtime_shapes_and_rejections() {
        let mut registry = TypeRegistry::new();

        for (type_id, expected) in [
            (TypeId::BOOL, 1),
            (TypeId::INT, 2),
            (TypeId::DINT, 4),
            (TypeId::LREAL, 8),
            (TypeId::TIME, 4),
            (TypeId::LDT, 8),
        ] {
            assert_eq!(size_of_type(type_id, &registry), Ok(expected));
        }

        let string = registry.register_string_with_length(13);
        let wstring = registry.register_wstring_with_length(13);
        let array = registry.register_array(TypeId::INT, vec![(1, 2), (-1, 0)]);
        let structure = registry.register_struct(
            "SizedStruct",
            vec![
                StructField {
                    name: "a".into(),
                    type_id: TypeId::INT,
                    address: None,
                    default_initializer: None,
                },
                StructField {
                    name: "b".into(),
                    type_id: TypeId::LREAL,
                    address: None,
                    default_initializer: None,
                },
            ],
        );
        let union = registry.register_union(
            "SizedUnion",
            vec![
                UnionVariant {
                    name: "small".into(),
                    type_id: TypeId::INT,
                    address: None,
                    default_initializer: None,
                },
                UnionVariant {
                    name: "large".into(),
                    type_id: TypeId::LREAL,
                    address: None,
                    default_initializer: None,
                },
            ],
        );
        let enumeration =
            registry.register_enum("SizedEnum", TypeId::INT, vec![("Zero".into(), 0)]);
        let subrange = registry.register(
            "SizedSubrange",
            Type::Subrange {
                base: TypeId::DINT,
                lower: -10,
                upper: 10,
            },
        );
        let alias = registry.register(
            "SizedAlias",
            Type::Alias {
                name: "SizedAlias".into(),
                target: TypeId::LINT,
            },
        );
        let reference = registry.register_reference(TypeId::INT);
        let pointer = registry.register_pointer(TypeId::INT);

        for (type_id, expected) in [
            (string, 13),
            (wstring, 26),
            (array, 8),
            (structure, 10),
            (union, 8),
            (enumeration, 2),
            (subrange, 4),
            (alias, 8),
            (reference, POINTER_REFERENCE_HANDLE_SIZE_BYTES),
            (pointer, POINTER_REFERENCE_HANDLE_SIZE_BYTES),
        ] {
            assert_eq!(size_of_type(type_id, &registry), Ok(expected));
        }

        let wildcard = registry.register_array(TypeId::INT, vec![(0, i64::MAX)]);
        let reversed = registry.register_array(TypeId::INT, vec![(1, 0)]);
        let overflowing = registry.register_array(TypeId::LREAL, vec![(i64::MIN, 0)]);
        assert_eq!(
            size_of_type(TypeId::STRING, &registry),
            Err(SizeOfError::UnsupportedType)
        );
        assert_eq!(
            size_of_type(wildcard, &registry),
            Err(SizeOfError::UnsupportedType)
        );
        assert_eq!(
            size_of_type(reversed, &registry),
            Err(SizeOfError::UnsupportedType)
        );
        assert_eq!(
            size_of_type(overflowing, &registry),
            Err(SizeOfError::Overflow)
        );
        assert_eq!(
            size_of_type(TypeId(900_001), &registry),
            Err(SizeOfError::UnknownType)
        );

        assert_eq!(array_len_bits(&[(1, 2), (-1, 0)]), Some(4));
        assert_eq!(array_len_bits(&[(1, 0)]), None);
        assert_eq!(array_len_bits(&[(i64::MIN, i64::MAX)]), None);

        let array_value = Value::Array(Box::new(ArrayValue::from_canonical_parts(
            vec![Value::Int(1), Value::Int(2)],
            vec![(0, 1)],
        )));
        let struct_value = Value::Struct(Arc::new(StructValue::from_canonical_parts(
            SmolStr::new("RuntimeShape"),
            [
                (SmolStr::new("a"), Value::Int(1)),
                (SmolStr::new("b"), Value::LReal(2.0)),
            ]
            .into_iter()
            .collect(),
        )));
        for (value, expected) in [
            (Value::Bool(false), 1),
            (Value::DInt(1), 4),
            (Value::LReal(1.0), 8),
            (Value::String("ÄB".into()), 2),
            (Value::WString("ÄB".into()), 4),
            (array_value, 4),
            (struct_value, 10),
            (Value::Reference(None), POINTER_REFERENCE_HANDLE_SIZE_BYTES),
        ] {
            assert_eq!(size_of_value(&registry, &value), Ok(expected), "{value:?}");
        }
        assert_eq!(
            size_of_value(&registry, &Value::Null),
            Err(SizeOfError::UnsupportedType)
        );
    }
}
