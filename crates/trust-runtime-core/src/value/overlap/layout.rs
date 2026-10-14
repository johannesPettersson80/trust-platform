//! Validate relative extents and prepare scalar alias paths once per admitted type.
use super::*;
use core::mem::size_of;

use crate::bytecode::BYTECODE_MAX_TYPE_NESTING;

/// Complete storage extent. Located BOOL components occupy individual bits;
/// aggregate values and array element strides occupy complete bytes.
/// This also validates relative layouts that need no physical alias backing.
pub(crate) fn type_extent_bits(
    types: &TypeTable,
    id: u32,
    abi: crate::value::StorageAbi,
    charge: &mut dyn FnMut(usize, usize) -> Result<(), RuntimeError>,
) -> Result<u32, RuntimeError> {
    extent(types, id, abi, charge, 0)
}

// Validation and materialization share this walker regardless of budget owner.
// Keep the accounting callback dynamic so each owner does not clone the traversal.
#[inline(never)]
pub(crate) fn prepare_layouts(
    types: &TypeTable,
    strings: &[SmolStr],
    abi: crate::value::StorageAbi,
    charge: &mut dyn FnMut(usize, usize) -> Result<(), RuntimeError>,
) -> Result<Vec<Option<Arc<PhysicalLayout>>>, RuntimeError> {
    let mut result = reserved(types.entries.len(), charge)?;
    result.resize_with(types.entries.len(), || None);
    let mut selected = reserved(types.entries.len(), charge)?;
    selected.resize(types.entries.len(), false);
    for (index, entry) in types.entries.iter().enumerate() {
        charge(0, 1)?;
        if matches!(
            entry.kind,
            TypeKind::OverlappingStruct | TypeKind::LocatedStruct
        ) {
            type_extent_bits(types, index as u32, abi, charge)?;
        }
        if entry.kind == TypeKind::OverlappingStruct {
            mark_nested(types, index as u32, &mut selected, charge, 0)?;
        }
    }
    for (index, entry) in types.entries.iter().enumerate() {
        charge(0, 1)?;
        if !selected[index] || !matches!(entry.data, TypeData::Struct { .. }) {
            continue;
        }
        let mut builder = Builder {
            types,
            strings,
            abi,
            charge,
            leaves: Vec::new(),
            children: Vec::new(),
            opaque: Vec::new(),
        };
        let mut path = Vec::new();
        let bits = builder.visit(index as u32, 0, &mut path, 0, None)?;
        builder.reject_opaque_intersections()?;
        let layout =
            PhysicalLayout::new(bits, abi, builder.leaves, builder.children, builder.charge)?;
        (builder.charge)(2 * size_of::<usize>() + size_of::<PhysicalLayout>(), 1)?;
        result[index] = Some(Arc::new(layout));
    }
    Ok(result)
}

fn mark_nested(
    types: &TypeTable,
    id: u32,
    selected: &mut [bool],
    charge: &mut dyn FnMut(usize, usize) -> Result<(), RuntimeError>,
    depth: usize,
) -> Result<(), RuntimeError> {
    charge(0, 1)?;
    if depth > BYTECODE_MAX_TYPE_NESTING {
        return Err(RuntimeError::TypeMismatch);
    }
    let mark = selected
        .get_mut(id as usize)
        .ok_or(RuntimeError::TypeMismatch)?;
    if *mark {
        return Ok(());
    }
    *mark = true;
    match &types
        .entries
        .get(id as usize)
        .ok_or(RuntimeError::TypeMismatch)?
        .data
    {
        TypeData::Struct { fields } => {
            for field in fields {
                mark_nested(types, field.type_id, selected, charge, depth + 1)?;
            }
        }
        TypeData::Alias { target_type_id } => {
            mark_nested(types, *target_type_id, selected, charge, depth + 1)?;
        }
        TypeData::Array { elem_type_id, .. } => {
            mark_nested(types, *elem_type_id, selected, charge, depth + 1)?;
        }
        // Unions retain independent logical variants. They are opaque to aliases.
        _ => {}
    }
    Ok(())
}

fn extent(
    types: &TypeTable,
    id: u32,
    abi: crate::value::StorageAbi,
    charge: &mut dyn FnMut(usize, usize) -> Result<(), RuntimeError>,
    depth: usize,
) -> Result<u32, RuntimeError> {
    charge(0, 1)?;
    if depth > BYTECODE_MAX_TYPE_NESTING {
        return Err(RuntimeError::TypeMismatch);
    }
    let entry = types
        .entries
        .get(id as usize)
        .ok_or(RuntimeError::TypeMismatch)?;
    match &entry.data {
        TypeData::Alias { target_type_id } => {
            extent(types, *target_type_id, abi, charge, depth + 1)
        }
        TypeData::Subrange { .. } => {
            let IntegerShape { width, .. } = integer_base(types, id, charge, depth)?;
            Ok(u32::from(width))
        }
        TypeData::Primitive {
            prim_id,
            max_length,
        } => match prim_id {
            24 | 25 if *max_length != 0 => u32::from(*max_length)
                .checked_mul(if *prim_id == 24 { 8 } else { 16 })
                .ok_or(RuntimeError::Overflow),
            _ => scalar::width(*prim_id, abi)
                .map(u32::from)
                .ok_or(RuntimeError::TypeMismatch),
        },
        TypeData::Enum { base_type_id, .. } => {
            let IntegerShape { width, .. } = integer_base(types, *base_type_id, charge, depth + 1)?;
            Ok(u32::from(width))
        }
        TypeData::Array { elem_type_id, dims } => {
            let count = array_len(dims, charge)?;
            let stride = byte_extent(extent(types, *elem_type_id, abi, charge, depth + 1)?)?;
            stride.checked_mul(count).ok_or(RuntimeError::Overflow)
        }
        TypeData::Struct { fields } => {
            let located = matches!(
                entry.kind,
                TypeKind::OverlappingStruct | TypeKind::LocatedStruct
            );
            let mut ranges = if entry.kind == TypeKind::LocatedStruct {
                reserved(fields.len(), charge)?
            } else {
                Vec::new()
            };
            let mut end = 0;
            for field in fields {
                charge(0, 1)?;
                let start = placement(types, field, located, end, charge)?;
                let bits = extent(types, field.type_id, abi, charge, depth + 1)?;
                let last = start.checked_add(bits).ok_or(RuntimeError::Overflow)?;
                if entry.kind == TypeKind::LocatedStruct {
                    ranges.push((start, last));
                }
                end = end.max(last);
            }
            sort_ranges(&mut ranges, charge)?;
            let mut prior_end = 0;
            for (start, last) in ranges {
                charge(0, 1)?;
                if start < prior_end {
                    return Err(RuntimeError::TypeMismatch);
                }
                prior_end = last;
            }
            byte_extent(end)
        }
        TypeData::Union { fields } => {
            let mut size = 0;
            for field in fields {
                charge(0, 1)?;
                if field.bit_offset.is_some() {
                    return Err(RuntimeError::TypeMismatch);
                }
                size = size.max(byte_extent(extent(
                    types,
                    field.type_id,
                    abi,
                    charge,
                    depth + 1,
                )?)?);
            }
            Ok(size)
        }
        TypeData::Reference { .. } => Ok(usize::BITS),
        TypeData::Pou { .. } | TypeData::Interface { .. } => Err(RuntimeError::TypeMismatch),
    }
}

fn placement(
    types: &TypeTable,
    field: &crate::bytecode::Field,
    located: bool,
    end: u32,
    charge: &mut dyn FnMut(usize, usize) -> Result<(), RuntimeError>,
) -> Result<u32, RuntimeError> {
    let start = match (located, field.bit_offset) {
        (true, Some(offset)) => offset,
        (false, None) => byte_extent(end)?,
        _ => return Err(RuntimeError::TypeMismatch),
    };
    if start % 8 != 0 && !is_bool(types, field.type_id, charge)? {
        return Err(RuntimeError::TypeMismatch);
    }
    Ok(start)
}

fn is_bool(
    types: &TypeTable,
    mut id: u32,
    charge: &mut dyn FnMut(usize, usize) -> Result<(), RuntimeError>,
) -> Result<bool, RuntimeError> {
    for _ in 0..=BYTECODE_MAX_TYPE_NESTING {
        charge(0, 1)?;
        match &types
            .entries
            .get(id as usize)
            .ok_or(RuntimeError::TypeMismatch)?
            .data
        {
            TypeData::Alias { target_type_id } => id = *target_type_id,
            TypeData::Primitive { prim_id: 1, .. } => return Ok(true),
            _ => return Ok(false),
        }
    }
    Err(RuntimeError::TypeMismatch)
}

fn byte_extent(bits: u32) -> Result<u32, RuntimeError> {
    bits.checked_add(7)
        .map(|bits| (bits / 8) * 8)
        .ok_or(RuntimeError::Overflow)
}

fn array_len(
    dims: &[(i64, i64)],
    charge: &mut dyn FnMut(usize, usize) -> Result<(), RuntimeError>,
) -> Result<u32, RuntimeError> {
    if dims.is_empty() {
        return Err(RuntimeError::TypeMismatch);
    }
    dims.iter().try_fold(1u32, |count, (lower, upper)| {
        charge(0, 1)?;
        let length = upper
            .checked_sub(*lower)
            .and_then(|n| n.checked_add(1))
            .and_then(|n| u32::try_from(n).ok())
            .filter(|n| *n != 0)
            .ok_or(RuntimeError::Overflow)?;
        count.checked_mul(length).ok_or(RuntimeError::Overflow)
    })
}

struct IntegerShape {
    width: u8,
    signed: bool,
    range: Option<(i64, i64)>,
}

fn integer_base(
    types: &TypeTable,
    id: u32,
    charge: &mut dyn FnMut(usize, usize) -> Result<(), RuntimeError>,
    depth: usize,
) -> Result<IntegerShape, RuntimeError> {
    charge(0, 1)?;
    if depth > BYTECODE_MAX_TYPE_NESTING {
        return Err(RuntimeError::TypeMismatch);
    }
    match &types
        .entries
        .get(id as usize)
        .ok_or(RuntimeError::TypeMismatch)?
        .data
    {
        TypeData::Alias { target_type_id } => {
            integer_base(types, *target_type_id, charge, depth + 1)
        }
        TypeData::Subrange {
            base_type_id,
            lower,
            upper,
        } => {
            let IntegerShape {
                width,
                signed,
                range,
            } = integer_base(types, *base_type_id, charge, depth + 1)?;
            if !integer_fits(*lower, width, signed) || !integer_fits(*upper, width, signed) {
                return Err(RuntimeError::TypeMismatch);
            }
            let range = merge_range(range, Some((*lower, *upper)))?;
            Ok(IntegerShape {
                width,
                signed,
                range,
            })
        }
        TypeData::Primitive {
            prim_id: id @ 6..=13,
            ..
        } => Ok(IntegerShape {
            width: scalar::width(*id, crate::value::StorageAbi::default())
                .ok_or(RuntimeError::TypeMismatch)?,
            signed: *id <= 9,
            range: None,
        }),
        _ => Err(RuntimeError::TypeMismatch),
    }
}

fn integer_fits(value: i64, width: u8, signed: bool) -> bool {
    if signed {
        width == 64 || (-(1i64 << (width - 1))..(1i64 << (width - 1))).contains(&value)
    } else {
        value >= 0 && (width == 64 || (value as u64) < (1u64 << width))
    }
}

fn merge_range(
    a: Option<(i64, i64)>,
    b: Option<(i64, i64)>,
) -> Result<Option<(i64, i64)>, RuntimeError> {
    let range = match (a, b) {
        (Some((al, au)), Some((bl, bu))) => Some((al.max(bl), au.min(bu))),
        (a, b) => a.or(b),
    };
    if range.is_some_and(|(lower, upper)| lower > upper) {
        return Err(RuntimeError::TypeMismatch);
    }
    Ok(range)
}

#[derive(Clone, Copy)]
struct OpaqueRange {
    start: u32,
    end: u32,
}

struct Builder<'a> {
    types: &'a TypeTable,
    strings: &'a [SmolStr],
    abi: crate::value::StorageAbi,
    charge: &'a mut dyn FnMut(usize, usize) -> Result<(), RuntimeError>,
    leaves: Vec<Leaf>,
    children: Vec<Child>,
    opaque: Vec<OpaqueRange>,
}

impl Builder<'_> {
    fn visit(
        &mut self,
        id: u32,
        base: u32,
        path: &mut Vec<RefSegment>,
        depth: usize,
        range: Option<(i64, i64)>,
    ) -> Result<u32, RuntimeError> {
        (self.charge)(0, 1)?;
        if depth > BYTECODE_MAX_TYPE_NESTING {
            return Err(RuntimeError::TypeMismatch);
        }
        let entry = self
            .types
            .entries
            .get(id as usize)
            .ok_or(RuntimeError::TypeMismatch)?;
        match &entry.data {
            TypeData::Alias { target_type_id } => {
                self.visit(*target_type_id, base, path, depth + 1, range)
            }
            TypeData::Subrange { base_type_id, .. } => {
                let IntegerShape {
                    range: declared_range,
                    ..
                } = integer_base(self.types, id, self.charge, depth)?;
                self.visit(
                    *base_type_id,
                    base,
                    path,
                    depth + 1,
                    merge_range(range, declared_range)?,
                )
            }
            TypeData::Primitive {
                prim_id: 24 | 25, ..
            }
            | TypeData::Union { .. }
            | TypeData::Reference { .. } => {
                let bits = extent(self.types, id, self.abi, self.charge, depth)?;
                let end = base.checked_add(bits).ok_or(RuntimeError::Overflow)?;
                push(
                    &mut self.opaque,
                    OpaqueRange { start: base, end },
                    self.charge,
                )?;
                Ok(bits)
            }
            TypeData::Primitive { prim_id, .. } => {
                let width = scalar::width(*prim_id, self.abi).ok_or(RuntimeError::TypeMismatch)?;
                self.leaf(path, base, width, *prim_id, None, range)?;
                Ok(u32::from(width))
            }
            TypeData::Enum {
                base_type_id,
                variants,
            } => {
                let IntegerShape {
                    width,
                    signed,
                    range: base_range,
                } = integer_base(self.types, *base_type_id, self.charge, depth + 1)?;
                let range = merge_range(range, base_range)?;
                let mut names = reserved(variants.len(), self.charge)?;
                for variant in variants {
                    (self.charge)(0, 1)?;
                    let valid_width = integer_fits(variant.value, width, signed);
                    if !valid_width
                        || range.is_some_and(|(lo, hi)| variant.value < lo || variant.value > hi)
                    {
                        return Err(RuntimeError::TypeMismatch);
                    }
                    names.push((
                        self.strings
                            .get(variant.name_idx as usize)
                            .ok_or(RuntimeError::TypeMismatch)?
                            .clone(),
                        variant.value,
                    ));
                }
                let primitive = integer_primitive_id(self.types, *base_type_id, self.charge)?;
                self.leaf(
                    path,
                    base,
                    width,
                    primitive,
                    Some(Enumeration {
                        signed,
                        variants: names,
                    }),
                    range,
                )?;
                Ok(u32::from(width))
            }
            TypeData::Struct { fields } => {
                let located = matches!(
                    entry.kind,
                    TypeKind::OverlappingStruct | TypeKind::LocatedStruct
                );
                let mut end = 0u32;
                for field in fields {
                    let relative = placement(self.types, field, located, end, self.charge)?;
                    let offset = base.checked_add(relative).ok_or(RuntimeError::Overflow)?;
                    let name = self
                        .strings
                        .get(field.name_idx as usize)
                        .ok_or(RuntimeError::TypeMismatch)?
                        .clone();
                    push(path, RefSegment::Field(name), self.charge)?;
                    let width = self.visit(field.type_id, offset, path, depth + 1, None)?;
                    path.pop();
                    end = end.max(relative.checked_add(width).ok_or(RuntimeError::Overflow)?);
                }
                let bits = byte_extent(end)?;
                if !path.is_empty() {
                    let child = Child {
                        path: clone_path(path, self.charge)?,
                        offset: base,
                        bits,
                    };
                    push(&mut self.children, child, self.charge)?;
                }
                Ok(bits)
            }
            TypeData::Array { elem_type_id, dims } => {
                let count = array_len(dims, self.charge)?;
                let stride = byte_extent(extent(
                    self.types,
                    *elem_type_id,
                    self.abi,
                    self.charge,
                    depth + 1,
                )?)?;
                for index in 0..count {
                    let mut ordinal = u64::from(index);
                    let mut indices = reserved(dims.len(), self.charge)?;
                    indices.resize(dims.len(), 0);
                    for (dimension, (lower, upper)) in dims.iter().enumerate().rev() {
                        (self.charge)(0, 1)?;
                        let length = upper
                            .checked_sub(*lower)
                            .and_then(|n| n.checked_add(1))
                            .and_then(|n| u64::try_from(n).ok())
                            .filter(|n| *n != 0)
                            .ok_or(RuntimeError::Overflow)?;
                        indices[dimension] = lower
                            .checked_add(
                                i64::try_from(ordinal % length)
                                    .map_err(|_| RuntimeError::Overflow)?,
                            )
                            .ok_or(RuntimeError::Overflow)?;
                        ordinal /= length;
                    }
                    push(path, RefSegment::Index(indices), self.charge)?;
                    let offset = index
                        .checked_mul(stride)
                        .and_then(|n| base.checked_add(n))
                        .ok_or(RuntimeError::Overflow)?;
                    self.visit(*elem_type_id, offset, path, depth + 1, None)?;
                    path.pop();
                }
                count.checked_mul(stride).ok_or(RuntimeError::Overflow)
            }
            _ => Err(RuntimeError::TypeMismatch),
        }
    }

    fn leaf(
        &mut self,
        path: &[RefSegment],
        offset: u32,
        width: u8,
        primitive_id: u16,
        enumeration: Option<Enumeration>,
        range: Option<(i64, i64)>,
    ) -> Result<(), RuntimeError> {
        offset
            .checked_add(u32::from(width))
            .ok_or(RuntimeError::Overflow)?;
        let path = clone_path(path, self.charge)?;
        push(
            &mut self.leaves,
            Leaf {
                path,
                offset,
                width,
                primitive_id,
                enumeration,
                range,
            },
            self.charge,
        )
    }

    fn reject_opaque_intersections(&mut self) -> Result<(), RuntimeError> {
        if self.opaque.is_empty() {
            return Ok(());
        }
        let count = self
            .opaque
            .len()
            .checked_add(self.leaves.len())
            .ok_or(RuntimeError::Overflow)?;
        let mut intervals = reserved(count, self.charge)?;
        for range in &self.opaque {
            intervals.push((range.start, range.end, true));
        }
        for leaf in &self.leaves {
            intervals.push((
                leaf.offset,
                leaf.offset
                    .checked_add(u32::from(leaf.width))
                    .ok_or(RuntimeError::Overflow)?,
                false,
            ));
        }
        use crate::sort::{heap_sort, Operation};
        heap_sort::<RuntimeError>(intervals.len(), &mut |operation| match operation {
            Operation::Charge => {
                (self.charge)(0, 1)?;
                Ok(false)
            }
            Operation::Less(a, b) => Ok(intervals[a] < intervals[b]),
            Operation::Swap(a, b) => {
                intervals.swap(a, b);
                Ok(false)
            }
        })?;
        let mut prior_end = 0;
        let mut prior_opaque_end = 0;
        for (start, end, opaque) in intervals {
            (self.charge)(0, 1)?;
            if start < prior_opaque_end || (opaque && start < prior_end) {
                return Err(RuntimeError::TypeMismatch);
            }
            prior_end = prior_end.max(end);
            if opaque {
                prior_opaque_end = prior_opaque_end.max(end);
            }
        }
        Ok(())
    }
}

fn clone_path(
    path: &[RefSegment],
    charge: &mut dyn FnMut(usize, usize) -> Result<(), RuntimeError>,
) -> Result<Vec<RefSegment>, RuntimeError> {
    let mut result = reserved(path.len(), charge)?;
    for segment in path {
        let segment = match segment {
            RefSegment::Field(name) => RefSegment::Field(name.clone()),
            RefSegment::Index(indices) => {
                let mut owned = reserved(indices.len(), charge)?;
                owned.extend_from_slice(indices);
                RefSegment::Index(owned)
            }
        };
        result.push(segment);
    }
    Ok(result)
}

fn reserved<T>(
    count: usize,
    charge: &mut dyn FnMut(usize, usize) -> Result<(), RuntimeError>,
) -> Result<Vec<T>, RuntimeError> {
    charge(
        count
            .checked_mul(size_of::<T>())
            .ok_or(RuntimeError::Overflow)?,
        count,
    )?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| RuntimeError::Overflow)?;
    Ok(values)
}

fn push<T>(
    values: &mut Vec<T>,
    value: T,
    charge: &mut dyn FnMut(usize, usize) -> Result<(), RuntimeError>,
) -> Result<(), RuntimeError> {
    if values.len() == values.capacity() {
        let next = values
            .capacity()
            .max(8)
            .checked_mul(2)
            .ok_or(RuntimeError::Overflow)?;
        let bytes = next
            .checked_sub(values.capacity())
            .and_then(|n| n.checked_mul(size_of::<T>()))
            .ok_or(RuntimeError::Overflow)?;
        charge(bytes, values.len())?;
        values
            .try_reserve_exact(next - values.len())
            .map_err(|_| RuntimeError::Overflow)?;
    }
    charge(0, 1)?;
    values.push(value);
    Ok(())
}

fn sort_ranges(
    values: &mut [(u32, u32)],
    charge: &mut dyn FnMut(usize, usize) -> Result<(), RuntimeError>,
) -> Result<(), RuntimeError> {
    use crate::sort::{heap_sort, Operation};
    heap_sort(values.len(), &mut |operation| match operation {
        Operation::Charge => {
            charge(0, 1)?;
            Ok(false)
        }
        Operation::Less(a, b) => Ok(values[a] < values[b]),
        Operation::Swap(a, b) => {
            values.swap(a, b);
            Ok(false)
        }
    })
}

#[cfg(test)]
#[path = "layout_tests.rs"]
mod tests;

fn integer_primitive_id(
    types: &TypeTable,
    mut id: u32,
    charge: &mut dyn FnMut(usize, usize) -> Result<(), RuntimeError>,
) -> Result<u16, RuntimeError> {
    for _ in 0..=BYTECODE_MAX_TYPE_NESTING {
        charge(0, 1)?;
        match &types
            .entries
            .get(id as usize)
            .ok_or(RuntimeError::TypeMismatch)?
            .data
        {
            TypeData::Alias { target_type_id } => id = *target_type_id,
            TypeData::Subrange { base_type_id, .. } => id = *base_type_id,
            TypeData::Primitive {
                prim_id: id @ 6..=13,
                ..
            } => return Ok(*id),
            _ => return Err(RuntimeError::TypeMismatch),
        }
    }
    Err(RuntimeError::TypeMismatch)
}
