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
    let path = match value {
        Value::Struct(value) => value.physical_canonical_path(path).unwrap_or(path),
        _ => path,
    };
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
    let path = match value {
        Value::Struct(value) => value.physical_canonical_path(path).unwrap_or(path),
        _ => path,
    };
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

/// Compatibility wrapper for callers whose API cannot expose a typed storage fault.
#[inline]
pub fn write_value_path(target: &mut Value, path: &[RefSegment], value: Value) -> bool {
    write_value_path_checked(target, path, value).unwrap_or(false)
}

/// Checked shared assignment owner. A refused physical write leaves all backing and aliases intact.
pub fn write_value_path_checked(
    target: &mut Value,
    path: &[RefSegment],
    value: Value,
) -> Result<bool, crate::error::RuntimeError> {
    let value = match read_value_path_borrowed(target, path) {
        Some(template) => normalize_assignment_for_target(template, value),
        None => value,
    };
    let Some(prepared) = prepare_value_path_write(target, path, &value)? else {
        return Ok(false);
    };
    Ok(commit_value_path_write(target, path, value, prepared))
}

/// Detached alias preflight shared by execution and finite engineering-write boundaries.
pub(crate) enum PreparedValuePathWrite {
    Direct,
    Physical {
        depth: usize,
        layout: alloc::sync::Arc<super::overlap::PhysicalLayout>,
        patch: super::overlap::PreparedPhysicalWrite,
    },
}

impl PreparedValuePathWrite {
    pub(crate) fn requires_final_deadline_check(&self) -> bool {
        matches!(self, Self::Physical { .. })
    }
    pub(crate) fn validate_aliases(
        &self,
        check: &mut impl FnMut(&Value) -> Result<(), crate::error::RuntimeError>,
    ) -> Result<(), crate::error::RuntimeError> {
        match self {
            Self::Direct => Ok(()),
            Self::Physical { patch, .. } => patch.validate_aliases(check),
        }
    }
}

pub(crate) fn prepare_value_path_write(
    mut target: &Value,
    path: &[RefSegment],
    input: &Value,
) -> Result<Option<PreparedValuePathWrite>, crate::error::RuntimeError> {
    for depth in 0..=path.len() {
        let rest = &path[depth..];
        if let Value::Struct(structure) = target {
            if rest.is_empty() && structure.physical_whole_assignment_is_admitted(input) {
                return Ok(Some(PreparedValuePathWrite::Direct));
            }
            if let Some(layout) = structure.physical_layout() {
                let canonical = layout.canonical_path(rest).unwrap_or(rest);
                if let Some(patch) = structure.prepare_physical_write(canonical, input)? {
                    return Ok(Some(PreparedValuePathWrite::Physical {
                        depth,
                        layout,
                        patch,
                    }));
                }
            }
        }
        let Some((segment, remaining)) = rest.split_first() else {
            return Ok(Some(PreparedValuePathWrite::Direct));
        };
        target = match (target, segment) {
            (Value::Struct(value), RefSegment::Field(name)) => match value.field(name) {
                Some(value) => value,
                None => return Ok(None),
            },
            (Value::Array(value), RefSegment::Index(indices)) => {
                let Some(offset) = array_offset_i64(value.dimensions(), indices) else {
                    return Ok(None);
                };
                let Some(value) = value.elements().get(offset) else {
                    return Ok(None);
                };
                value
            }
            (Value::String(_) | Value::WString(_), RefSegment::Index(_))
                if remaining.is_empty() =>
            {
                return Ok(Some(PreparedValuePathWrite::Direct))
            }
            _ => return Ok(None),
        };
    }
    Ok(None)
}

pub(crate) fn commit_value_path_write(
    target: &mut Value,
    path: &[RefSegment],
    value: Value,
    prepared: PreparedValuePathWrite,
) -> bool {
    match prepared {
        PreparedValuePathWrite::Direct => write_value_path_raw(target, path, value),
        PreparedValuePathWrite::Physical {
            depth,
            layout,
            patch,
        } => {
            let Some(prefix) = path.get(..depth) else {
                return false;
            };
            let Some(suffix) = path.get(depth..) else {
                return false;
            };
            let Some(root) = value_path_mut_raw(target, prefix) else {
                return false;
            };
            let Value::Struct(structure) = root else {
                return false;
            };
            let path = layout.canonical_path(suffix).unwrap_or(suffix);
            alloc::sync::Arc::make_mut(structure).commit_physical_write(path, value, patch)
        }
    }
}

fn value_path_mut_raw<'a>(mut value: &'a mut Value, path: &[RefSegment]) -> Option<&'a mut Value> {
    for segment in path {
        value = match (value, segment) {
            (Value::Struct(structure), RefSegment::Field(name)) => {
                alloc::sync::Arc::make_mut(structure)
                    .fields
                    .get_mut(name.as_str())?
            }
            (Value::Array(array), RefSegment::Index(indices)) => {
                let offset = array_offset_i64(array.dimensions(), indices)?;
                array.elements_mut().get_mut(offset)?
            }
            _ => return None,
        };
    }
    Some(value)
}

fn write_value_path_raw(target: &mut Value, path: &[RefSegment], value: Value) -> bool {
    let Some((segment, rest)) = path.split_first() else {
        *target = value;
        return true;
    };
    match (target, segment) {
        (Value::Struct(structure), RefSegment::Field(name)) => {
            alloc::sync::Arc::make_mut(structure)
                .fields
                .get_mut(name.as_str())
                .is_some_and(|field| write_value_path_raw(field, rest, value))
        }
        (Value::Array(array), RefSegment::Index(indices)) => {
            let Some(offset) = array_offset_i64(array.dimensions(), indices) else {
                return false;
            };
            array
                .elements_mut()
                .get_mut(offset)
                .is_some_and(|element| write_value_path_raw(element, rest, value))
        }
        (Value::String(text), RefSegment::Index(indices)) if rest.is_empty() => {
            let Some(updated) = write_string_path(text, indices, value, false) else {
                return false;
            };
            *text = updated.into();
            true
        }
        (Value::WString(text), RefSegment::Index(indices)) if rest.is_empty() => {
            let Some(updated) = write_string_path(text, indices, value, true) else {
                return false;
            };
            *text = updated;
            true
        }
        _ => false,
    }
}

#[cfg(feature = "hir")]
pub fn write_value_path_typed(
    target: &mut Value,
    path: &[RefSegment],
    value: Value,
    registry: &TypeRegistry,
) -> bool {
    if matches!(target, Value::Struct(structure) if structure.physical.is_some()) {
        return write_value_path(target, path, value);
    }
    if path.is_empty() {
        *target = normalize_assignment_for_target(target, value);
        return true;
    }

    match &path[0] {
        RefSegment::Field(name) => match target {
            Value::Struct(struct_value) => {
                let struct_value = alloc::sync::Arc::make_mut(struct_value);
                if path.len() == 1
                    && let Some(result) =
                        synchronize_overlap_field(struct_value, name, value.clone(), registry)
                {
                    return result;
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
) -> Option<bool> {
    let type_id = registry.lookup(value.type_name().as_str())?;
    if !registry.is_overlapping_struct_type(type_id) {
        return None;
    }
    Some(synchronize_legacy_overlap(
        value,
        selected_name,
        selected_value,
        registry,
        type_id,
    ))
}

#[cfg(feature = "hir")]
fn synchronize_legacy_overlap(
    value: &mut super::StructValue,
    selected_name: &smol_str::SmolStr,
    selected_value: Value,
    registry: &TypeRegistry,
    type_id: TypeId,
) -> bool {
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

    let mut updates = Vec::with_capacity(layouts.len());
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
        updates.push((name, updated));
    }
    for (name, updated) in updates {
        if let Some(slot) = value.fields.get_mut(name.as_str()) {
            *slot = updated;
        }
    }
    true
}

#[cfg(feature = "hir")]
fn relative_address_start_bit(address: &str) -> Option<usize> {
    let address = address.strip_prefix('%')?;
    let (unit, offset) = address.split_at_checked(1)?;
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
        Value::Time(value) => (value.as_nanos().to_le_bytes().to_vec(), 64),
        Value::LTime(value) => (value.as_nanos().to_le_bytes().to_vec(), 64),
        Value::Date(value) => (value.ticks().to_le_bytes().to_vec(), 64),
        Value::LDate(value) => (value.nanos().to_le_bytes().to_vec(), 64),
        Value::Tod(value) => (value.ticks().to_le_bytes().to_vec(), 64),
        Value::LTod(value) => (value.nanos().to_le_bytes().to_vec(), 64),
        Value::Dt(value) => (value.ticks().to_le_bytes().to_vec(), 64),
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
        Value::Time(_) => Value::Time(super::Duration::from_nanos(i64::from_le_bytes(bytes))),
        Value::LTime(_) => Value::LTime(super::Duration::from_nanos(i64::from_le_bytes(bytes))),
        Value::Date(_) => Value::Date(super::DateValue::new(i64::from_le_bytes(bytes))),
        Value::LDate(_) => Value::LDate(super::LDateValue::new(i64::from_le_bytes(bytes))),
        Value::Tod(_) => Value::Tod(super::TimeOfDayValue::new(i64::from_le_bytes(bytes))),
        Value::LTod(_) => Value::LTod(super::LTimeOfDayValue::new(i64::from_le_bytes(bytes))),
        Value::Dt(_) => Value::Dt(super::DateTimeValue::new(i64::from_le_bytes(bytes))),
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
    fn checked_string_path_write_keeps_option_rejection_and_original_text() {
        let mut text = crate::value::Value::String("abc".into());
        assert!(super::write_value_path_checked(
            &mut text,
            &[single_ref_index(2)],
            crate::value::Value::Char(b'Z')
        )
        .unwrap());
        assert_eq!(text, crate::value::Value::String("aZc".into()));
        assert!(!super::write_value_path_checked(
            &mut text,
            &[single_ref_index(9)],
            crate::value::Value::Char(b'Q')
        )
        .unwrap());
        assert_eq!(text, crate::value::Value::String("aZc".into()));
    }

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

#[cfg(all(test, feature = "hir"))]
mod legacy_overlap_address_tests {
    #[test]
    fn malformed_relative_address_refuses_without_splitting_invalid_utf8_boundary() {
        for address in ["%", "%é1", "%🦀1", ""] {
            assert_eq!(super::relative_address_start_bit(address), None);
        }
        assert_eq!(super::relative_address_start_bit("%B2"), Some(16));
    }
}
