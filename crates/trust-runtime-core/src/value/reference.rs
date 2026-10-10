use alloc::{string::String, vec, vec::Vec};

use smol_str::SmolStr;

use crate::{error::RuntimeError, memory::MemoryLocation};

/// Reference path segment within composite values.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum RefSegment {
    /// Array/string index path segment.
    Index(RefIndices),
    /// Struct/union field path segment.
    Field(SmolStr),
}

/// Reference path indices for one array/string segment.
pub type RefIndices = Vec<i64>;
/// Reference path within a composite value.
pub type RefPath = Vec<RefSegment>;

/// Reference to a value in memory.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ValueRef {
    /// Memory area that owns the referenced root value.
    pub location: MemoryLocation,
    /// Slot offset within the owning memory area.
    pub offset: usize,
    /// Nested path from the root value to the referenced sub-value.
    pub path: RefPath,
}

/// Borrowed reference used by checks and traversal without cloning a path.
#[derive(Debug, Clone, Copy)]
pub struct ValueRefView<'a> {
    /// Memory area containing the root.
    pub location: MemoryLocation,
    /// Physical root slot.
    pub offset: usize,
    /// Borrowed compound path.
    pub path: &'a [RefSegment],
}

impl ValueRefView<'_> {
    /// Materialize an address when the program explicitly creates a reference.
    pub fn to_owned(self) -> ValueRef {
        ValueRef {
            location: self.location,
            offset: self.offset,
            path: self.path.into(),
        }
    }
}

impl ValueRef {
    /// Borrow the address without allocating its compound path.
    pub fn as_view(&self) -> ValueRefView<'_> {
        ValueRefView {
            location: self.location,
            offset: self.offset,
            path: &self.path,
        }
    }
}

pub fn ref_indices_from_iter<I>(indices: I) -> RefIndices
where
    I: IntoIterator<Item = i64>,
{
    indices.into_iter().collect()
}

#[inline]
#[must_use]
pub fn single_ref_index(index: i64) -> RefSegment {
    RefSegment::Index(vec![index])
}

#[inline]
#[must_use]
pub fn array_offset_i64(dimensions: &[(i64, i64)], indices: &[i64]) -> Option<usize> {
    if dimensions.len() != indices.len() {
        return None;
    }
    let mut offset: i128 = 0;
    let mut stride: i128 = 1;
    for ((lower, upper), index) in dimensions.iter().zip(indices).rev() {
        if index < lower || index > upper {
            return None;
        }
        let lower = i128::from(*lower);
        let upper = i128::from(*upper);
        let index = i128::from(*index);
        let len = upper.checked_sub(lower)?.checked_add(1)?;
        let relative = index.checked_sub(lower)?;
        offset = offset.checked_add(relative.checked_mul(stride)?)?;
        stride = stride.checked_mul(len)?;
    }
    usize::try_from(offset).ok()
}

#[inline]
pub fn checked_array_offset_i64(
    dimensions: &[(i64, i64)],
    indices: &[i64],
) -> Result<usize, RuntimeError> {
    if dimensions.len() != indices.len() {
        return Err(RuntimeError::TypeMismatch);
    }
    for ((lower, upper), index) in dimensions.iter().zip(indices) {
        if index < lower || index > upper {
            return Err(RuntimeError::IndexOutOfBounds {
                index: *index,
                lower: *lower,
                upper: *upper,
            });
        }
    }
    array_offset_i64(dimensions, indices).ok_or(RuntimeError::TypeMismatch)
}

/// Parsed IEC partial access suffix such as `%X0`, `%B1`, `%W0`, or `%D0`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartialAccess {
    /// Bit access.
    Bit(u8),
    /// Byte access.
    Byte(u8),
    /// Word access.
    Word(u8),
    /// Double-word access.
    DWord(u8),
}

/// Error produced by reading or writing a partial access.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartialAccessError {
    /// Partial access index outside the valid width of the target value.
    IndexOutOfBounds { index: i64, lower: i64, upper: i64 },
    /// Partial access is not valid for the target value type.
    TypeMismatch,
}

#[must_use]
pub fn parse_partial_access(text: &str) -> Option<PartialAccess> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    if let Some(stripped) = text.strip_prefix('%') {
        let mut chars = stripped.chars();
        let prefix = chars.next()?;
        let digits: String = chars.collect();
        let index = parse_access_index(&digits)?;
        return match prefix.to_ascii_uppercase() {
            'X' => Some(PartialAccess::Bit(index)),
            'B' => Some(PartialAccess::Byte(index)),
            'W' => Some(PartialAccess::Word(index)),
            'D' => Some(PartialAccess::DWord(index)),
            _ => None,
        };
    }
    if text.chars().all(|c| c.is_ascii_digit() || c == '_') {
        let index = parse_access_index(text)?;
        return Some(PartialAccess::Bit(index));
    }
    None
}

fn parse_access_index(text: &str) -> Option<u8> {
    let cleaned: String = text.chars().filter(|c| *c != '_').collect();
    let value: u64 = cleaned.parse().ok()?;
    u8::try_from(value).ok()
}

use super::{normalize_assignment_for_target, read_string_element, write_string_element, Value};
#[cfg(feature = "hir")]
use trust_hir::types::TypeRegistry;
#[cfg(feature = "hir")]
use trust_hir::{Type, TypeId};

#[inline]
pub fn read_value_path_borrowed<'a>(value: &'a Value, path: &[RefSegment]) -> Option<&'a Value> {
    if path.is_empty() {
        return Some(value);
    }

    match &path[0] {
        RefSegment::Field(name) => match value {
            Value::Struct(struct_value) => struct_value
                .field(name.as_str())
                .and_then(|field| read_value_path_borrowed(field, &path[1..])),
            _ => None,
        },
        RefSegment::Index(indices) => match value {
            Value::Array(array) => {
                let offset = array_offset_i64(array.dimensions(), indices)?;
                array
                    .elements()
                    .get(offset)
                    .and_then(|element| read_value_path_borrowed(element, &path[1..]))
            }
            _ => None,
        },
    }
}

#[inline]
pub fn materialize_value_path(value: &Value, path: &[RefSegment]) -> Option<Value> {
    if path.is_empty() {
        return Some(value.clone());
    }

    match &path[0] {
        RefSegment::Field(name) => match value {
            Value::Struct(struct_value) => struct_value
                .field(name.as_str())
                .and_then(|field| materialize_value_path(field, &path[1..])),
            _ => None,
        },
        RefSegment::Index(indices) => match value {
            Value::Array(array) => {
                let offset = array_offset_i64(array.dimensions(), indices)?;
                array
                    .elements()
                    .get(offset)
                    .and_then(|element| materialize_value_path(element, &path[1..]))
            }
            Value::String(text) => {
                if !path[1..].is_empty() {
                    return None;
                }
                let index = single_string_index(indices)?;
                read_string_element(text.as_str(), index, false).ok()
            }
            Value::WString(text) => {
                if !path[1..].is_empty() {
                    return None;
                }
                let index = single_string_index(indices)?;
                read_string_element(text.as_str(), index, true).ok()
            }
            _ => None,
        },
    }
}

#[inline]
pub fn write_value_path(target: &mut Value, path: &[RefSegment], value: Value) -> bool {
    if path.is_empty() {
        *target = normalize_assignment_for_target(target, value);
        return true;
    }

    match &path[0] {
        RefSegment::Field(name) => match target {
            Value::Struct(struct_value) => alloc::sync::Arc::make_mut(struct_value)
                .field_mut(name.as_str())
                .map(|field| write_value_path(field, &path[1..], value))
                .unwrap_or(false),
            _ => false,
        },
        RefSegment::Index(indices) => match target {
            Value::Array(array) => {
                let offset = match array_offset_i64(array.dimensions(), indices) {
                    Some(offset) => offset,
                    None => return false,
                };
                array
                    .elements_mut()
                    .get_mut(offset)
                    .map(|element| write_value_path(element, &path[1..], value))
                    .unwrap_or(false)
            }
            Value::String(text) => write_string_path(text, indices, value, false)
                .map(|updated| {
                    *target = Value::String(updated.into());
                    true
                })
                .unwrap_or(false),
            Value::WString(text) => write_string_path(text, indices, value, true)
                .map(|updated| {
                    *target = Value::WString(updated);
                    true
                })
                .unwrap_or(false),
            _ => false,
        },
    }
}

#[cfg(feature = "hir")]
pub fn write_value_path_typed(
    target: &mut Value,
    path: &[RefSegment],
    value: Value,
    registry: &TypeRegistry,
) -> bool {
    if path.is_empty() {
        *target = normalize_assignment_for_target(target, value);
        return true;
    }

    match &path[0] {
        RefSegment::Field(name) => match target {
            Value::Struct(struct_value) => {
                let struct_value = alloc::sync::Arc::make_mut(struct_value);
                if path.len() == 1
                    && synchronize_overlap_field(struct_value, name, value.clone(), registry)
                {
                    return true;
                }
                struct_value
                    .field_mut(name.as_str())
                    .map(|field| write_value_path_typed(field, &path[1..], value, registry))
                    .unwrap_or(false)
            }
            _ => false,
        },
        RefSegment::Index(indices) => match target {
            Value::Array(array) => {
                let offset = match array_offset_i64(array.dimensions(), indices) {
                    Some(offset) => offset,
                    None => return false,
                };
                array
                    .elements_mut()
                    .get_mut(offset)
                    .map(|element| write_value_path_typed(element, &path[1..], value, registry))
                    .unwrap_or(false)
            }
            Value::String(text) => write_string_path(text, indices, value, false)
                .map(|updated| {
                    *target = Value::String(updated.into());
                    true
                })
                .unwrap_or(false),
            Value::WString(text) => write_string_path(text, indices, value, true)
                .map(|updated| {
                    *target = Value::WString(updated);
                    true
                })
                .unwrap_or(false),
            _ => false,
        },
    }
}

#[cfg(feature = "hir")]
fn synchronize_overlap_field(
    value: &mut super::StructValue,
    selected_name: &smol_str::SmolStr,
    selected_value: Value,
    registry: &TypeRegistry,
) -> bool {
    let Some(type_id) = registry.lookup(value.type_name().as_str()) else {
        return false;
    };
    if !registry.is_overlapping_struct_type(type_id) {
        return false;
    }
    let Some(Type::Struct { fields, .. }) = registry.get(type_id) else {
        return false;
    };
    let fields = fields.clone();
    let Some(selected) = fields
        .iter()
        .find(|field| field.name.eq_ignore_ascii_case(selected_name.as_str()))
    else {
        return false;
    };
    let Some(selected_address) = selected.address.as_deref() else {
        return false;
    };
    let Some(selected_template) = value.field(selected.name.as_str()).cloned() else {
        return false;
    };
    let selected_value = normalize_assignment_for_target(&selected_template, selected_value);
    let Some(selected_bits) = overlap_value_bits(&selected_value) else {
        return false;
    };
    let Some(selected_start) = relative_address_start_bit(selected_address) else {
        return false;
    };

    let layouts = fields
        .iter()
        .filter_map(|field| {
            let address = field.address.as_deref()?;
            let template = value.field(field.name.as_str())?.clone();
            let bits = overlap_value_bits(&template)?;
            let start = relative_address_start_bit(address)?;
            Some((field.name.clone(), start, bits, template, field.type_id))
        })
        .collect::<Vec<_>>();
    let Some(total_bits) = layouts
        .iter()
        .map(|(_, start, bits, _, _)| start.checked_add(bits.len()))
        .collect::<Option<Vec<_>>>()
        .and_then(|ends| ends.into_iter().max())
    else {
        return false;
    };
    let mut backing = vec![false; total_bits];
    for (_, start, bits, _, _) in &layouts {
        write_overlap_bits(&mut backing, *start, bits);
    }
    write_overlap_bits(&mut backing, selected_start, &selected_bits);

    for (name, start, bits, template, field_type) in layouts {
        let end = match start.checked_add(bits.len()) {
            Some(end) => end,
            None => return false,
        };
        let Some(updated) =
            overlap_value_from_bits(&template, field_type, &backing[start..end], registry)
        else {
            return false;
        };
        if !value.set_existing_field(name, updated) {
            return false;
        }
    }
    true
}

#[cfg(feature = "hir")]
fn relative_address_start_bit(address: &str) -> Option<usize> {
    let address = address.strip_prefix('%')?;
    let (unit, offset) = address.split_at(1);
    if !matches!(unit, "X" | "B" | "W" | "D" | "L") {
        return None;
    }
    let (byte, bit) = match offset.split_once('.') {
        Some((byte, bit)) if unit == "X" => {
            let bit = bit.parse::<usize>().ok()?;
            if bit > 7 {
                return None;
            }
            (byte.parse::<usize>().ok()?, bit)
        }
        Some(_) => return None,
        None => (offset.parse::<usize>().ok()?, 0),
    };
    byte.checked_mul(8)?.checked_add(bit)
}

#[cfg(feature = "hir")]
fn overlap_value_bits(value: &Value) -> Option<Vec<bool>> {
    let (bytes, bit_len) = match value {
        Value::Bool(value) => (vec![u8::from(*value)], 1),
        Value::SInt(value) => (value.to_le_bytes().to_vec(), 8),
        Value::Int(value) => (value.to_le_bytes().to_vec(), 16),
        Value::DInt(value) => (value.to_le_bytes().to_vec(), 32),
        Value::LInt(value) => (value.to_le_bytes().to_vec(), 64),
        Value::USInt(value) | Value::Byte(value) | Value::Char(value) => (vec![*value], 8),
        Value::UInt(value) | Value::Word(value) | Value::WChar(value) => {
            (value.to_le_bytes().to_vec(), 16)
        }
        Value::UDInt(value) | Value::DWord(value) => (value.to_le_bytes().to_vec(), 32),
        Value::ULInt(value) | Value::LWord(value) => (value.to_le_bytes().to_vec(), 64),
        Value::Real(value) => (value.to_bits().to_le_bytes().to_vec(), 32),
        Value::LReal(value) => (value.to_bits().to_le_bytes().to_vec(), 64),
        Value::Time(value) => (value.as_nanos().to_le_bytes()[..4].to_vec(), 32),
        Value::LTime(value) => (value.as_nanos().to_le_bytes().to_vec(), 64),
        Value::Date(value) => (value.ticks().to_le_bytes()[..4].to_vec(), 32),
        Value::LDate(value) => (value.nanos().to_le_bytes().to_vec(), 64),
        Value::Tod(value) => (value.ticks().to_le_bytes()[..4].to_vec(), 32),
        Value::LTod(value) => (value.nanos().to_le_bytes().to_vec(), 64),
        Value::Dt(value) => (value.ticks().to_le_bytes()[..4].to_vec(), 32),
        Value::Ldt(value) => (value.nanos().to_le_bytes().to_vec(), 64),
        Value::Enum(value) => (value.numeric_value().to_le_bytes().to_vec(), 64),
        _ => return None,
    };
    Some(
        (0..bit_len)
            .map(|bit| bytes[bit / 8] & (1 << (bit % 8)) != 0)
            .collect(),
    )
}

#[cfg(feature = "hir")]
fn write_overlap_bits(backing: &mut [bool], start: usize, bits: &[bool]) {
    for (offset, bit) in bits.iter().enumerate() {
        if let Some(slot) = backing.get_mut(start + offset) {
            *slot = *bit;
        }
    }
}

#[cfg(feature = "hir")]
fn overlap_value_from_bits(
    template: &Value,
    field_type: TypeId,
    bits: &[bool],
    registry: &TypeRegistry,
) -> Option<Value> {
    let mut bytes = [0u8; 8];
    for (bit, set) in bits.iter().copied().enumerate() {
        if set {
            bytes[bit / 8] |= 1 << (bit % 8);
        }
    }
    Some(match template {
        Value::Bool(_) => Value::Bool(bits.first().copied().unwrap_or(false)),
        Value::SInt(_) => Value::SInt(i8::from_le_bytes([bytes[0]])),
        Value::Int(_) => Value::Int(i16::from_le_bytes(bytes[..2].try_into().ok()?)),
        Value::DInt(_) => Value::DInt(i32::from_le_bytes(bytes[..4].try_into().ok()?)),
        Value::LInt(_) => Value::LInt(i64::from_le_bytes(bytes)),
        Value::USInt(_) => Value::USInt(bytes[0]),
        Value::UInt(_) => Value::UInt(u16::from_le_bytes(bytes[..2].try_into().ok()?)),
        Value::UDInt(_) => Value::UDInt(u32::from_le_bytes(bytes[..4].try_into().ok()?)),
        Value::ULInt(_) => Value::ULInt(u64::from_le_bytes(bytes)),
        Value::Byte(_) => Value::Byte(bytes[0]),
        Value::Word(_) => Value::Word(u16::from_le_bytes(bytes[..2].try_into().ok()?)),
        Value::DWord(_) => Value::DWord(u32::from_le_bytes(bytes[..4].try_into().ok()?)),
        Value::LWord(_) => Value::LWord(u64::from_le_bytes(bytes)),
        Value::Real(_) => Value::Real(f32::from_bits(u32::from_le_bytes(
            bytes[..4].try_into().ok()?,
        ))),
        Value::LReal(_) => Value::LReal(f64::from_bits(u64::from_le_bytes(bytes))),
        Value::Char(_) => Value::Char(bytes[0]),
        Value::WChar(_) => Value::WChar(u16::from_le_bytes(bytes[..2].try_into().ok()?)),
        Value::Time(_) => Value::Time(super::Duration::from_nanos(i64::from(i32::from_le_bytes(
            bytes[..4].try_into().ok()?,
        )))),
        Value::LTime(_) => Value::LTime(super::Duration::from_nanos(i64::from_le_bytes(bytes))),
        Value::Date(_) => Value::Date(super::DateValue::new(i64::from(i32::from_le_bytes(
            bytes[..4].try_into().ok()?,
        )))),
        Value::LDate(_) => Value::LDate(super::LDateValue::new(i64::from_le_bytes(bytes))),
        Value::Tod(_) => Value::Tod(super::TimeOfDayValue::new(i64::from(i32::from_le_bytes(
            bytes[..4].try_into().ok()?,
        )))),
        Value::LTod(_) => Value::LTod(super::LTimeOfDayValue::new(i64::from_le_bytes(bytes))),
        Value::Dt(_) => Value::Dt(super::DateTimeValue::new(i64::from(i32::from_le_bytes(
            bytes[..4].try_into().ok()?,
        )))),
        Value::Ldt(_) => Value::Ldt(super::LDateTimeValue::new(i64::from_le_bytes(bytes))),
        Value::Enum(_) => {
            let Type::Enum { values, .. } = registry.get(field_type)? else {
                return None;
            };
            let numeric = i64::from_le_bytes(bytes);
            let (name, _) = values.iter().find(|(_, value)| *value == numeric)?;
            Value::Enum(alloc::boxed::Box::new(
                super::EnumValue::new(registry, field_type, name.as_str()).ok()?,
            ))
        }
        _ => return None,
    })
}

#[inline]
pub fn single_string_index(indices: &[i64]) -> Option<i64> {
    if indices.len() != 1 {
        return None;
    }
    Some(indices[0])
}

#[inline]
fn write_string_path(text: &str, indices: &[i64], value: Value, wide: bool) -> Option<String> {
    let index = single_string_index(indices)?;
    write_string_element(text, index, value, wide).ok()
}

#[cfg(test)]
mod tests {
    use super::{
        array_offset_i64, checked_array_offset_i64, parse_partial_access, ref_indices_from_iter,
        single_ref_index, PartialAccess, RefPath, RefSegment,
    };
    use crate::error::RuntimeError;
    use alloc::vec;

    #[test]
    fn array_offset_handles_extreme_bounds_without_overflow() {
        assert_eq!(
            array_offset_i64(&[(i64::MIN, i64::MAX)], &[i64::MIN]),
            Some(0)
        );
        assert_eq!(
            array_offset_i64(&[(i64::MIN, i64::MAX)], &[i64::MAX]),
            usize::try_from(u64::MAX).ok()
        );
    }

    #[test]
    fn checked_array_offset_preserves_bounds_error() {
        assert_eq!(
            checked_array_offset_i64(&[(0, 1)], &[2]),
            Err(RuntimeError::IndexOutOfBounds {
                index: 2,
                lower: 0,
                upper: 1,
            })
        );
    }

    #[test]
    fn common_ref_path_helpers_preserve_segment_order() {
        let path: RefPath = vec![
            RefSegment::Field("items".into()),
            single_ref_index(1),
            single_ref_index(2),
            RefSegment::Field("value".into()),
        ];

        assert_eq!(ref_indices_from_iter([1, 2, 3]), vec![1, 2, 3]);
        assert_eq!(path.len(), 4);
    }

    #[test]
    fn partial_access_parser_accepts_iec_suffixes_and_bare_bits() {
        assert_eq!(parse_partial_access("%X1"), Some(PartialAccess::Bit(1)));
        assert_eq!(parse_partial_access("%B2"), Some(PartialAccess::Byte(2)));
        assert_eq!(parse_partial_access("%W3"), Some(PartialAccess::Word(3)));
        assert_eq!(parse_partial_access("%D4"), Some(PartialAccess::DWord(4)));
        assert_eq!(parse_partial_access("1_0"), Some(PartialAccess::Bit(10)));
        assert_eq!(parse_partial_access("%Q1"), None);
    }
}
