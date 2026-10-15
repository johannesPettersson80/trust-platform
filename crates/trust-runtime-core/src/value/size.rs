#[cfg(feature = "hir")]
use trust_hir::types::TypeRegistry;
const POINTER_REFERENCE_HANDLE_SIZE_BYTES: u64 = core::mem::size_of::<usize>() as u64;
#[cfg(feature = "hir")]
use trust_hir::{Type, TypeId};

use crate::value::{string_element_count, StorageAbi, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SizeOfError {
    UnknownType,
    UnsupportedType,
    Overflow,
}

#[cfg(feature = "hir")]
pub fn size_of_type(type_id: TypeId, registry: &TypeRegistry) -> Result<u64, SizeOfError> {
    size_of_type_with_abi(type_id, registry, StorageAbi::default())
}

/// Calculate a source-associated type under an explicitly selected artifact ABI.
#[cfg(feature = "hir")]
pub fn size_of_type_with_abi(
    type_id: TypeId,
    registry: &TypeRegistry,
    abi: StorageAbi,
) -> Result<u64, SizeOfError> {
    let ty = registry.get(type_id).ok_or(SizeOfError::UnknownType)?;
    match ty {
        Type::Alias { target, .. } => size_of_type_with_abi(*target, registry, abi),
        Type::Subrange { base, .. } => size_of_type_with_abi(*base, registry, abi),
        Type::Enum { base, .. } => size_of_type_with_abi(*base, registry, abi),
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
            let element_size = size_of_type_with_abi(*element, registry, abi)?;
            let len = array_len_bits(dimensions).ok_or(SizeOfError::UnsupportedType)?;
            element_size.checked_mul(len).ok_or(SizeOfError::Overflow)
        }
        Type::Struct { fields, .. } => {
            if abi == StorageAbi::UniformTemporal64
                && fields.iter().any(|field| field.address.is_some())
            {
                let mut end = 0u64;
                for field in fields {
                    let address = field
                        .address
                        .as_deref()
                        .ok_or(SizeOfError::UnsupportedType)?;
                    let suffix = address
                        .strip_prefix('%')
                        .ok_or(SizeOfError::UnsupportedType)?;
                    let address = crate::io_address::IoAddress::parse(
                        &alloc::format!("%M{suffix}").to_ascii_uppercase(),
                    )
                    .map_err(|_| SizeOfError::UnsupportedType)?;
                    if address.wildcard || address.path.len() != 1 {
                        return Err(SizeOfError::UnsupportedType);
                    }
                    let boolean = hir_is_bool(field.type_id, registry)?;
                    if address.bit != 0 && !boolean {
                        return Err(SizeOfError::UnsupportedType);
                    }
                    let size = size_of_type_with_abi(field.type_id, registry, abi)?;
                    let width = if boolean {
                        1
                    } else {
                        size.checked_mul(8).ok_or(SizeOfError::Overflow)?
                    };
                    let offset = u64::from(address.byte)
                        .checked_mul(8)
                        .and_then(|offset| offset.checked_add(u64::from(address.bit)))
                        .ok_or(SizeOfError::Overflow)?;
                    end = end.max(offset.checked_add(width).ok_or(SizeOfError::Overflow)?);
                }
                return Ok(end.div_ceil(8));
            }
            let mut total = 0u64;
            for field in fields {
                let size = size_of_type_with_abi(field.type_id, registry, abi)?;
                total = total.checked_add(size).ok_or(SizeOfError::Overflow)?;
            }
            Ok(total)
        }
        Type::Union { variants, .. } => {
            let mut max = 0u64;
            for variant in variants {
                let size = size_of_type_with_abi(variant.type_id, registry, abi)?;
                max = max.max(size);
            }
            Ok(max)
        }
        Type::String { max_len } => max_len.map(u64::from).ok_or(SizeOfError::UnsupportedType),
        Type::WString { max_len } => max_len
            .map(|len| u64::from(len) * 2)
            .ok_or(SizeOfError::UnsupportedType),
        Type::Reference { .. } | Type::Pointer { .. } => Ok(POINTER_REFERENCE_HANDLE_SIZE_BYTES),
        Type::Time | Type::Date | Type::Tod | Type::Dt => Ok(abi.short_temporal_bytes()),
        Type::LTime | Type::LDate | Type::LTod | Type::Ldt => Ok(8),
        _ => {
            let bits = ty.bit_size().ok_or(SizeOfError::UnsupportedType)?;
            Ok(u64::from(bits.div_ceil(8)))
        }
    }
}

#[cfg(feature = "hir")]
fn hir_is_bool(mut ty: TypeId, registry: &TypeRegistry) -> Result<bool, SizeOfError> {
    for _ in 0..128 {
        match registry.get(ty).ok_or(SizeOfError::UnknownType)? {
            Type::Alias { target, .. } => ty = *target,
            Type::Bool => return Ok(true),
            _ => return Ok(false),
        }
    }
    Err(SizeOfError::UnsupportedType)
}

#[cfg(feature = "hir")]
pub fn size_of_value(registry: &TypeRegistry, value: &Value) -> Result<u64, SizeOfError> {
    size_of_value_with_abi(registry, value, StorageAbi::default())
}

/// Calculate a source-associated value under its installed artifact ABI.
#[cfg(feature = "hir")]
pub fn size_of_value_with_abi(
    registry: &TypeRegistry,
    value: &Value,
    abi: StorageAbi,
) -> Result<u64, SizeOfError> {
    size_of_value_with_abi_and_visitor(
        value,
        abi,
        &mut |name| {
            let type_id = registry.lookup(name).ok_or(SizeOfError::UnsupportedType)?;
            size_of_type_with_abi(type_id, registry, abi)
        },
        &mut |name| {
            let Some(type_id) = registry.lookup(name) else {
                return Ok(None);
            };
            match registry.get(type_id) {
                Some(Type::Struct { fields, .. })
                    if fields.iter().any(|field| field.address.is_some()) =>
                {
                    size_of_type_with_abi(type_id, registry, abi).map(Some)
                }
                _ => Ok(None),
            }
        },
        &mut |_| Ok(()),
    )
}

/// Shared runtime-value traversal; enum storage size comes from immutable type metadata.
/// The visitor permits the portable execution context to charge traversal work.
pub fn size_of_value_with<E: From<SizeOfError>>(
    value: &Value,
    enum_size: &mut impl FnMut(&str) -> Result<u64, E>,
    visit: &mut impl FnMut(usize) -> Result<(), E>,
) -> Result<u64, E> {
    size_of_value_with_abi_and_visitor(
        value,
        StorageAbi::default(),
        enum_size,
        &mut |_| Ok(None),
        visit,
    )
}

/// Shared runtime-value traversal with explicit legacy/current storage semantics.
/// `structure_size` supplies the admitted extent for relative-layout structures;
/// ordinary structures retain their dynamic field-size behavior when it returns `None`.
pub fn size_of_value_with_abi_and_visitor<E: From<SizeOfError>>(
    value: &Value,
    abi: StorageAbi,
    enum_size: &mut impl FnMut(&str) -> Result<u64, E>,
    structure_size: &mut impl FnMut(&str) -> Result<Option<u64>, E>,
    visit: &mut impl FnMut(usize) -> Result<(), E>,
) -> Result<u64, E> {
    size_of_value_inner(value, abi, enum_size, structure_size, visit, 0)
}

impl From<SizeOfError> for crate::error::RuntimeError {
    fn from(error: SizeOfError) -> Self {
        match error {
            SizeOfError::Overflow => Self::Overflow,
            SizeOfError::UnknownType | SizeOfError::UnsupportedType => Self::TypeMismatch,
        }
    }
}

fn size_of_value_inner<E: From<SizeOfError>>(
    value: &Value,
    abi: StorageAbi,
    enum_size: &mut impl FnMut(&str) -> Result<u64, E>,
    structure_size: &mut impl FnMut(&str) -> Result<Option<u64>, E>,
    visit: &mut impl FnMut(usize) -> Result<(), E>,
    depth: usize,
) -> Result<u64, E> {
    if depth >= 128 {
        return Err(SizeOfError::UnsupportedType.into());
    }
    visit(1)?;
    let size = match value {
        Value::Bool(_) => 1,
        Value::SInt(_) | Value::USInt(_) | Value::Byte(_) | Value::Char(_) => 1,
        Value::Int(_) | Value::UInt(_) | Value::Word(_) | Value::WChar(_) => 2,
        Value::DInt(_) | Value::UDInt(_) | Value::DWord(_) | Value::Real(_) => 4,
        Value::LInt(_) | Value::ULInt(_) | Value::LWord(_) | Value::LReal(_) => 8,
        Value::Time(_) | Value::Date(_) | Value::Tod(_) | Value::Dt(_) => {
            abi.short_temporal_bytes()
        }
        Value::LTime(_) | Value::LDate(_) | Value::LTod(_) | Value::Ldt(_) => 8,
        Value::String(value) => {
            visit(value.len())?;
            string_element_count(value.as_str()) as u64
        }
        Value::WString(value) => {
            visit(value.len())?;
            (string_element_count(value.as_str()) as u64) * 2
        }
        Value::Array(array) => {
            let element_size = match array.elements().first() {
                Some(value) => {
                    size_of_value_inner(value, abi, enum_size, structure_size, visit, depth + 1)?
                }
                None => 0,
            };
            visit(array.dimensions().len())?;
            let len = array_len_bits(array.dimensions()).ok_or(SizeOfError::UnsupportedType)?;
            element_size.checked_mul(len).ok_or(SizeOfError::Overflow)?
        }
        Value::Struct(struct_value) => {
            if abi == StorageAbi::UniformTemporal64 {
                if let Some(size) = structure_size(struct_value.type_name())? {
                    return Ok(size);
                }
                if struct_value.physical.is_some() {
                    return Ok(struct_value.physical_byte_len() as u64);
                }
            }
            let mut total = 0u64;
            for value in struct_value.fields().values() {
                let size =
                    size_of_value_inner(value, abi, enum_size, structure_size, visit, depth + 1)?;
                total = total.checked_add(size).ok_or(SizeOfError::Overflow)?;
            }
            total
        }
        Value::Enum(enum_value) => enum_size(enum_value.type_name().as_str())?,
        Value::Reference(_) => POINTER_REFERENCE_HANDLE_SIZE_BYTES,
        Value::Instance(_) => u64::try_from(core::mem::size_of::<crate::memory::InstanceId>())
            .map_err(|_| SizeOfError::Overflow)?,
        Value::Null => return Err(SizeOfError::UnsupportedType.into()),
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

#[cfg(all(test, feature = "hir"))]
mod tests {
    use super::{
        array_len_bits, size_of_type, size_of_type_with_abi, size_of_value, size_of_value_with_abi,
        SizeOfError,
    };
    use crate::value::{ArrayValue, Duration, StorageAbi, StructValue, Value};
    use alloc::{boxed::Box, sync::Arc, vec};
    use smol_str::SmolStr;
    use trust_hir::types::{
        StructField, TypeRegistry, UnionVariant, POINTER_REFERENCE_HANDLE_SIZE_BYTES,
    };
    #[cfg(feature = "hir")]
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
    fn source_metadata_and_dynamic_values_agree_on_relative_extent_and_legacy_history() {
        let mut registry = TypeRegistry::new();
        let structure = registry.register_struct(
            "LocatedTime",
            vec![
                StructField {
                    name: "bit".into(),
                    type_id: TypeId::BOOL,
                    address: Some("%x0.3".into()),
                    default_initializer: None,
                },
                StructField {
                    name: "time".into(),
                    type_id: TypeId::TIME,
                    address: Some("%l8".into()),
                    default_initializer: None,
                },
            ],
        );
        let array = registry.register_array(structure, vec![(2, 4)]);
        let value = Value::Struct(Arc::new(StructValue::from_canonical_parts(
            "LocatedTime".into(),
            [
                ("bit".into(), Value::Bool(false)),
                ("time".into(), Value::Time(Duration::from_nanos(i64::MIN))),
            ]
            .into_iter()
            .collect(),
        )));
        for (abi, expected) in [
            (StorageAbi::UniformTemporal64, 16),
            (StorageAbi::LegacyTemporal32, 5),
        ] {
            assert_eq!(
                size_of_type_with_abi(structure, &registry, abi),
                Ok(expected)
            );
            assert_eq!(size_of_value_with_abi(&registry, &value, abi), Ok(expected));
            assert_eq!(
                size_of_type_with_abi(array, &registry, abi),
                Ok(expected * 3)
            );
        }
    }

    #[test]
    fn sizeof_contract_covers_declared_and_runtime_shapes_and_rejections() {
        let mut registry = TypeRegistry::new();

        for (type_id, expected) in [
            (TypeId::BOOL, 1),
            (TypeId::INT, 2),
            (TypeId::DINT, 4),
            (TypeId::LREAL, 8),
            (TypeId::TIME, 8),
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

    #[test]
    fn lreal_array_of_two_pow_twenty_nine_elements_has_the_same_size_on_every_target() {
        // 2^29 LREAL elements occupy 2^32 bytes, one more than a 32-bit usize holds. Sizes are
        // computed in u64, so a 32-bit target (the i686 lane) reports the exact extent instead
        // of wrapping to zero.
        let mut registry = TypeRegistry::new();
        let array = registry.register_array(TypeId::LREAL, vec![(0, (1_i64 << 29) - 1)]);
        assert_eq!(size_of_type(array, &registry), Ok(4_294_967_296));
    }
}
