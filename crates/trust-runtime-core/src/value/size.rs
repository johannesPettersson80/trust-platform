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
    use super::size_of_value;
    use crate::value::Value;
    use trust_hir::types::TypeRegistry;

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
}
