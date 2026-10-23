//! Physical relative-layout extents; legacy SIZEOF is a separate compatibility rule.
use super::*;
use crate::types::POINTER_REFERENCE_HANDLE_SIZE_BYTES;

impl SymbolCollector<'_> {
    pub(super) fn type_storage_bit_width(&self, type_id: TypeId) -> Option<u128> {
        self.storage_bits(type_id, &mut Vec::new())
    }

    fn storage_bits(&self, id: TypeId, stack: &mut Vec<TypeId>) -> Option<u128> {
        if stack.len() >= 128 || stack.contains(&id) {
            return None;
        }
        stack.push(id);
        let result = (|| {
            let ty = self.table.type_by_id(id)?;
            match ty {
                Type::Alias { target, .. } => self.storage_bits(*target, stack),
                Type::Subrange { base, .. } | Type::Enum { base, .. } => {
                    self.storage_bits(*base, stack)
                }
                Type::Time | Type::Date | Type::Tod | Type::Dt => {
                    u128::from(self.temporal_storage.bytes()).checked_mul(8)
                }
                Type::Array {
                    element,
                    dimensions,
                } => {
                    let stride = byte_bits(self.storage_bits(*element, stack)?)?;
                    dimensions.iter().try_fold(stride, |total, (lower, upper)| {
                        if *lower == 0 && *upper == i64::MAX {
                            return None;
                        }
                        let count = upper.checked_sub(*lower)?.checked_add(1)?;
                        total.checked_mul(u128::try_from(count).ok()?)
                    })
                }
                Type::Struct { fields, .. }
                    if fields.iter().any(|field| field.address.is_some()) =>
                {
                    let mut end = 0;
                    for field in fields {
                        let width = self.storage_bits(field.type_id, stack)?;
                        let (_, field_end) = super::types::relative_address_bit_interval(
                            field.address.as_deref()?,
                            width,
                        )?;
                        end = end.max(field_end);
                    }
                    byte_bits(end)
                }
                Type::Struct { fields, .. } => fields.iter().try_fold(0u128, |total, field| {
                    total.checked_add(byte_bits(self.storage_bits(field.type_id, stack)?)?)
                }),
                Type::Union { variants, .. } => {
                    variants.iter().try_fold(0u128, |largest, field| {
                        Some(largest.max(byte_bits(self.storage_bits(field.type_id, stack)?)?))
                    })
                }
                Type::String {
                    max_len: Some(length),
                } => u128::from(*length).checked_mul(8),
                Type::WString {
                    max_len: Some(length),
                } => u128::from(*length).checked_mul(16),
                Type::Reference { .. } | Type::Pointer { .. } => {
                    u128::from(POINTER_REFERENCE_HANDLE_SIZE_BYTES).checked_mul(8)
                }
                _ => ty.bit_size().map(u128::from),
            }
        })();
        stack.pop();
        result
    }
}

fn byte_bits(bits: u128) -> Option<u128> {
    bits.div_ceil(8).checked_mul(8)
}
