//! Admitted relative layouts and synchronized structure backing bits.
//!
//! Layout is immutable and shared. Each value owns its backing bytes; copies
//! therefore retain hidden/gap bits without sharing mutable physical storage.
use super::{read_value_path_borrowed, RefSegment, StructValue, Value};
use crate::bytecode::{TypeData, TypeKind, TypeTable};
use crate::error::RuntimeError;
use alloc::{sync::Arc, vec::Vec};
use smol_str::SmolStr;

mod checkpoint;
mod cost;
mod index;
mod layout;
mod scalar;
mod writer;
pub(crate) use layout::prepare_layouts;
pub(crate) use writer::PreparedPhysicalWrite;

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct PhysicalLayout {
    pub(crate) bits: u32,
    abi: crate::value::StorageAbi,
    leaves: Vec<Leaf>,
    children: Vec<Child>,
    path_order: Vec<usize>,
    bit_order: Vec<usize>,
    bit_max_end: Vec<u32>,
    child_path_order: Vec<usize>,
    child_bit_order: Vec<usize>,
    child_max_end: Vec<u32>,
    max_path_work: usize,
}

#[derive(Debug, PartialEq, Eq)]
struct Leaf {
    path: Vec<RefSegment>,
    offset: u32,
    width: u8,
    primitive_id: u16,
    enumeration: Option<Enumeration>,
    range: Option<(i64, i64)>,
}

#[derive(Debug, PartialEq, Eq)]
struct Enumeration {
    signed: bool,
    variants: Vec<(SmolStr, i64)>,
}

#[derive(Debug, PartialEq, Eq)]
struct Child {
    path: Vec<RefSegment>,
    offset: u32,
    bits: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PhysicalValue {
    layout: Arc<PhysicalLayout>,
    bytes: Vec<u8>,
}

impl StructValue {
    /// Compare opaque admitted physical storage without changing logical value equality.
    /// Persistence owners use this to detect hidden-byte or layout changes without
    /// encoding a temporary checkpoint on every save decision.
    pub fn physical_checkpoint_eq(&self, other: &Self) -> bool {
        self.physical == other.physical
    }

    /// Attach only the layout selected from this installation's admitted type.
    pub(crate) fn with_physical_layout(
        mut self,
        layout: Option<Arc<PhysicalLayout>>,
    ) -> Result<Self, RuntimeError> {
        let Some(layout) = layout else {
            self.physical = None;
            return Ok(self);
        };
        if self.has_physical_layout(Some(&layout)) {
            return Ok(self);
        }
        writer::initialize(&mut self, layout, true, None)?;
        Ok(self)
    }

    /// Fresh construction defaults initialize backing in canonical declaration order.
    /// Already materialized/retained values use with_physical_layout's strict admission.
    pub(crate) fn initialize_physical_layout(
        mut self,
        layout: Option<Arc<PhysicalLayout>>,
    ) -> Result<Self, RuntimeError> {
        let Some(layout) = layout else {
            self.physical = None;
            return Ok(self);
        };
        if self.has_physical_layout(Some(&layout)) {
            return Ok(self);
        }
        writer::initialize(&mut self, layout, false, None)?;
        Ok(self)
    }

    /// Strictly reattach rebuilt fields while preserving compatible source gap bits.
    pub(crate) fn with_physical_layout_preserving(
        mut self,
        layout: Option<Arc<PhysicalLayout>>,
        source: &StructValue,
    ) -> Result<Self, RuntimeError> {
        let Some(layout) = layout else {
            self.physical = None;
            return Ok(self);
        };
        if let Some(physical) = &source.physical
            && !physical.layout.storage_compatible(&layout)
        {
            return Err(RuntimeError::TypeMismatch);
        }
        writer::initialize(&mut self, layout, true, source.physical.as_ref())?;
        Ok(self)
    }

    pub(crate) fn physical_layout(&self) -> Option<Arc<PhysicalLayout>> {
        self.physical
            .as_ref()
            .map(|value| Arc::clone(&value.layout))
    }

    pub(crate) fn physical_canonical_path<'a>(
        &'a self,
        path: &[RefSegment],
    ) -> Option<&'a [RefSegment]> {
        self.physical.as_ref()?.layout.canonical_path(path)
    }

    /// Rebind an equivalent admitted layout without reconstructing hidden bits.
    /// The caller charges the structural comparison before calling this method.
    pub(crate) fn rebind_physical_layout(
        &mut self,
        layout: Option<Arc<PhysicalLayout>>,
    ) -> Result<(), RuntimeError> {
        match (&mut self.physical, layout) {
            (None, None) => Ok(()),
            (Some(value), Some(layout)) if value.layout.storage_compatible(&layout) => {
                value.layout = layout;
                Ok(())
            }
            _ => Err(RuntimeError::TypeMismatch),
        }
    }

    pub(crate) fn physical_byte_len(&self) -> usize {
        self.physical.as_ref().map_or(0, |value| value.bytes.len())
    }

    pub(crate) fn has_physical_layout(&self, layout: Option<&Arc<PhysicalLayout>>) -> bool {
        match (&self.physical, layout) {
            (None, None) => true,
            (Some(value), Some(layout)) => Arc::ptr_eq(&value.layout, layout),
            _ => false,
        }
    }
}

impl PhysicalLayout {
    pub(crate) fn byte_len(&self) -> Result<usize, RuntimeError> {
        usize::try_from(u64::from(self.bits).div_ceil(8)).map_err(|_| RuntimeError::Overflow)
    }

    pub(crate) fn construction_work(&self) -> Result<usize, RuntimeError> {
        self.leaves
            .iter()
            .try_fold(0usize, |sum, leaf| {
                sum.checked_add(192)?
                    .checked_add(path_work(&leaf.path).checked_mul(4)?)
                    .and_then(|sum| {
                        sum.checked_add(
                            leaf.enumeration
                                .as_ref()
                                .map_or(Some(0), |e| e.variants.len().checked_mul(6))?,
                        )
                    })
            })
            .and_then(|n| {
                self.children.iter().try_fold(n, |n, child| {
                    n.checked_add((child.bits as usize).checked_mul(2)?)?
                        .checked_add(path_work(&child.path).checked_mul(child.path.len().max(1))?)
                })
            })
            .ok_or(RuntimeError::Overflow)
    }
}

fn read_bits(bytes: &[u8], start: u32, width: u8) -> u64 {
    let mut value = 0;
    for bit in 0..u32::from(width) {
        let index = u64::from(start) + u64::from(bit);
        if let Some(byte) = usize::try_from(index / 8)
            .ok()
            .and_then(|index| bytes.get(index))
        {
            value |= u64::from((byte >> (index % 8)) & 1) << bit;
        }
    }
    value
}

fn write_bits(bytes: &mut [u8], start: u32, width: u8, value: u64) {
    for bit in 0..u32::from(width) {
        let index = u64::from(start) + u64::from(bit);
        if let Some(byte) = usize::try_from(index / 8)
            .ok()
            .and_then(|index| bytes.get_mut(index))
        {
            let mask = 1 << (index % 8);
            *byte = (*byte & !mask) | (((value >> bit) as u8 & 1) << (index % 8));
        }
    }
}

fn read_struct_path<'a>(value: &'a StructValue, path: &[RefSegment]) -> Option<&'a Value> {
    let (RefSegment::Field(name), rest) = path.split_first()? else {
        return None;
    };
    read_value_path_borrowed(value.field(name)?, rest)
}

fn struct_path_mut<'a>(value: &'a mut StructValue, path: &[RefSegment]) -> Option<&'a mut Value> {
    let (RefSegment::Field(name), rest) = path.split_first()? else {
        return None;
    };
    value_path_mut(value.fields.get_mut(name.as_str())?, rest)
}

fn value_path_mut<'a>(value: &'a mut Value, path: &[RefSegment]) -> Option<&'a mut Value> {
    let Some((segment, rest)) = path.split_first() else {
        return Some(value);
    };
    let value = match (value, segment) {
        (Value::Struct(value), RefSegment::Field(name)) => {
            Arc::make_mut(value).fields.get_mut(name.as_str())?
        }
        (Value::Array(value), RefSegment::Index(indices)) => {
            let offset = super::array_offset_i64(value.dimensions(), indices)?;
            value.elements_mut().get_mut(offset)?
        }
        _ => return None,
    };
    value_path_mut(value, rest)
}

fn write_struct_path_raw(target: &mut StructValue, path: &[RefSegment], value: Value) -> bool {
    let Some(slot) = struct_path_mut(target, path) else {
        return false;
    };
    *slot = value;
    true
}

fn index_order(
    count: usize,
    charge: &mut dyn FnMut(usize, usize) -> Result<(), RuntimeError>,
) -> Result<Vec<usize>, RuntimeError> {
    charge(
        count
            .checked_mul(core::mem::size_of::<usize>())
            .ok_or(RuntimeError::Overflow)?,
        count,
    )?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| RuntimeError::Overflow)?;
    values.extend(0..count);
    Ok(values)
}

fn sort_order(
    order: &mut [usize],
    charge: &mut dyn FnMut(usize, usize) -> Result<(), RuntimeError>,
    comparison_work: usize,
    less: &mut dyn FnMut(usize, usize) -> bool,
) -> Result<(), RuntimeError> {
    use crate::sort::{heap_sort, Operation};
    heap_sort(order.len(), &mut |operation| match operation {
        Operation::Charge => {
            charge(0, comparison_work)?;
            Ok(false)
        }
        Operation::Less(a, b) => Ok(less(order[a], order[b])),
        Operation::Swap(a, b) => {
            order.swap(a, b);
            Ok(false)
        }
    })
}

fn compare_paths(a: &[RefSegment], b: &[RefSegment]) -> core::cmp::Ordering {
    use core::cmp::Ordering;
    for (a, b) in a.iter().zip(b) {
        let order = match (a, b) {
            (RefSegment::Field(a), RefSegment::Field(b)) => a
                .bytes()
                .map(|b| b.to_ascii_lowercase())
                .cmp(b.bytes().map(|b| b.to_ascii_lowercase())),
            (RefSegment::Index(a), RefSegment::Index(b)) => a.cmp(b),
            (RefSegment::Field(_), RefSegment::Index(_)) => Ordering::Less,
            (RefSegment::Index(_), RefSegment::Field(_)) => Ordering::Greater,
        };
        if order != Ordering::Equal {
            return order;
        }
    }
    a.len().cmp(&b.len())
}

fn path_work(path: &[RefSegment]) -> usize {
    path.iter().fold(path.len(), |sum, part| {
        sum.saturating_add(match part {
            RefSegment::Field(name) => name.len(),
            RefSegment::Index(indices) => indices.len(),
        })
    })
}

#[cfg(test)]
mod writer_tests;

fn path_suffix<'a>(path: &'a [RefSegment], prefix: &[RefSegment]) -> Option<&'a [RefSegment]> {
    (path.len() >= prefix.len() && compare_paths(&path[..prefix.len()], prefix).is_eq())
        .then(|| &path[prefix.len()..])
}

#[cfg(test)]
mod text_work_overlap_cost_tests;
