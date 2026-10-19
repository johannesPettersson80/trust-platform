//! A structure with a physical layout read from its backing bytes (specification 34 MEM-12; C1-15).
//!
//! The flat store holds a `STRUCT OVERLAP`, and every structure nested in one, as its backing: the
//! bits of every member, the gaps between them and the bits of the nested structures it holds, with
//! each member that has no bits there (a text, a reference, a union, or an array of them) beside it.
//! Every leaf's value is a function of those bits, so the logical value the value store keeps beside
//! its backing is decoded from them: the same fields in declaration order, each leaf as the layout
//! reads it, each member beside the backing as its own bytes hold it, each nested structure that has
//! a layout of its own with its bits, and the root with the whole backing.

use super::*;
use crate::value::{
    ArrayValue, DateTimeValue, DateValue, Duration, EnumValue, LDateTimeValue, LDateValue,
    LTimeOfDayValue, TimeOfDayValue,
};
use crate::vm::PreparedModule;
use alloc::boxed::Box;

/// The value of type `type_id` at `path` inside the structure whose physical layout is `layout` and
/// whose backing is `bytes`: the whole structure for an empty path. `beside` reads a member that lies
/// beside the backing, given its path and type. `None` when the type or the layout names no such
/// value.
pub(crate) fn decode(
    prepared: &PreparedModule,
    layout: &Arc<PhysicalLayout>,
    bytes: &[u8],
    type_id: u32,
    path: &mut Vec<RefSegment>,
    beside: &mut dyn FnMut(&[RefSegment], u32) -> Option<Value>,
) -> Option<Value> {
    if path.len() > crate::bytecode::BYTECODE_MAX_TYPE_NESTING {
        return None;
    }
    let vm = &prepared.vm;
    if !path.is_empty() && crate::memory::layout::lies_beside_backing(&vm.types, type_id) {
        return beside(path, type_id);
    }
    let resolved = crate::vm::type_policy::resolved_alias_type(&vm.types, type_id, 0)?;
    let entry = vm.types.entries.get(resolved as usize)?;
    match &entry.data {
        TypeData::Struct { fields } => {
            let mut values = crate::collections::OrderedMap::default();
            for field in fields {
                let name = SmolStr::new(vm.strings.get(field.name_idx as usize)?);
                path.push(RefSegment::Field(name.clone()));
                let value = decode(prepared, layout, bytes, field.type_id, path, beside);
                path.pop();
                values.insert(name, value?);
            }
            let name = entry
                .name_idx
                .and_then(|index| vm.strings.get(index as usize))
                .map(SmolStr::new)
                .unwrap_or_default();
            let mut structure = StructValue::from_canonical_parts(name, values);
            structure.physical = if path.is_empty() {
                Some(PhysicalValue {
                    layout: Arc::clone(layout),
                    bytes: bytes.to_vec(),
                })
            } else {
                match (layout.child_at_path(path), vm.physical_layout(resolved)) {
                    (Some(child), Some(own)) => {
                        let mut child_bytes = alloc::vec![0; own.byte_len().ok()?];
                        for bit in 0..child.bits {
                            write_bits(
                                &mut child_bytes,
                                bit,
                                1,
                                read_bits(bytes, child.offset + bit, 1),
                            );
                        }
                        Some(PhysicalValue {
                            layout: own,
                            bytes: child_bytes,
                        })
                    }
                    _ => None,
                }
            };
            Some(Value::Struct(Arc::new(structure)))
        }
        TypeData::Array { elem_type_id, dims } => {
            let count = dims.iter().try_fold(1usize, |count, (lower, upper)| {
                let length = usize::try_from(upper.checked_sub(*lower)?.checked_add(1)?).ok()?;
                count.checked_mul(length)
            })?;
            let mut elements = Vec::new();
            elements.try_reserve_exact(count).ok()?;
            for flat in 0..count {
                let mut indices = alloc::vec![0i64; dims.len()];
                let mut remaining = flat;
                for (index, (lower, upper)) in indices.iter_mut().zip(dims).rev() {
                    let length = usize::try_from(upper - lower + 1).ok()?;
                    *index = lower.checked_add(i64::try_from(remaining % length).ok()?)?;
                    remaining /= length;
                }
                path.push(RefSegment::Index(indices));
                let element = decode(prepared, layout, bytes, *elem_type_id, path, beside);
                path.pop();
                elements.push(element?);
            }
            Some(Value::Array(Box::new(ArrayValue::from_canonical_parts(
                elements,
                dims.clone(),
            ))))
        }
        TypeData::Enum { .. } => {
            let leaf = layout.leaf_at_path(path)?;
            let name = entry
                .name_idx
                .and_then(|index| vm.strings.get(index as usize))
                .map(SmolStr::new)
                .unwrap_or_default();
            let template = Value::Enum(Box::new(EnumValue::from_canonical_parts(
                name,
                SmolStr::default(),
                0,
            )));
            scalar::value(&template, read_bits(bytes, leaf.offset, leaf.width), leaf)
        }
        _ => {
            let leaf = layout.leaf_at_path(path)?;
            let template = zero(leaf.primitive_id)?;
            scalar::value(&template, read_bits(bytes, leaf.offset, leaf.width), leaf)
        }
    }
}

/// A value of the primitive `id`, whose kind tells [`scalar::value`] how to read a leaf's bits.
fn zero(id: u16) -> Option<Value> {
    Some(match id {
        1 => Value::Bool(false),
        2 => Value::Byte(0),
        3 => Value::Word(0),
        4 => Value::DWord(0),
        5 => Value::LWord(0),
        6 => Value::SInt(0),
        7 => Value::Int(0),
        8 => Value::DInt(0),
        9 => Value::LInt(0),
        10 => Value::USInt(0),
        11 => Value::UInt(0),
        12 => Value::UDInt(0),
        13 => Value::ULInt(0),
        14 => Value::Real(0.0),
        15 => Value::LReal(0.0),
        16 => Value::Time(Duration::from_nanos(0)),
        17 => Value::LTime(Duration::from_nanos(0)),
        18 => Value::Date(DateValue::new(0)),
        19 => Value::LDate(LDateValue::new(0)),
        20 => Value::Tod(TimeOfDayValue::new(0)),
        21 => Value::LTod(LTimeOfDayValue::new(0)),
        22 => Value::Dt(DateTimeValue::new(0)),
        23 => Value::Ldt(LDateTimeValue::new(0)),
        26 => Value::Char(0),
        27 => Value::WChar(0),
        _ => return None,
    })
}

impl PhysicalLayout {
    /// The number of scalar leaves of the layout.
    pub(crate) fn leaf_count(&self) -> usize {
        self.leaves.len()
    }

    /// The path of scalar leaf `leaf` from the structure.
    pub(crate) fn leaf_path(&self, leaf: u32) -> Option<&[RefSegment]> {
        self.leaves
            .get(leaf as usize)
            .map(|leaf| leaf.path.as_slice())
    }

    /// The first scalar leaf, in path order, below the member `segment` of the node at `prefix`, or
    /// `None` when no leaf lies below that member. No path is built.
    pub(crate) fn step_below(&self, prefix: &[RefSegment], segment: &RefSegment) -> Option<u32> {
        let depth = prefix.len();
        let range = self.leaves_below_path(prefix);
        self.path_order[range].iter().find_map(|&index| {
            let next = self.leaves[index].path.get(depth)?;
            compare_paths(core::slice::from_ref(next), core::slice::from_ref(segment))
                .is_eq()
                .then(|| u32::try_from(index).ok())
                .flatten()
        })
    }

    /// The scalar leaf `leaf` read from `bytes`, the root's backing, without allocating: `None` for an
    /// enumeration leaf, whose value names its type and variant.
    pub(crate) fn scalar_leaf(&self, leaf: u32, bytes: &[u8]) -> Option<Value> {
        let leaf = self.leaves.get(leaf as usize)?;
        if leaf.enumeration.is_some() {
            return None;
        }
        scalar::value(
            &zero(leaf.primitive_id)?,
            read_bits(bytes, leaf.offset, leaf.width),
            leaf,
        )
    }
}

/// The type of the node at `path` inside a value of the root type `type_id`: each member named by its
/// declared name, in any case, and each element of an array.
pub(crate) fn node_type(
    prepared: &PreparedModule,
    type_id: u32,
    path: &[RefSegment],
) -> Option<u32> {
    let vm = &prepared.vm;
    path.iter().try_fold(type_id, |current, segment| {
        let resolved = crate::vm::type_policy::resolved_alias_type(&vm.types, current, 0)?;
        match (&vm.types.entries.get(resolved as usize)?.data, segment) {
            (TypeData::Struct { fields }, RefSegment::Field(name)) => fields
                .iter()
                .find(|field| {
                    vm.strings
                        .get(field.name_idx as usize)
                        .is_some_and(|declared| declared.eq_ignore_ascii_case(name))
                })
                .map(|field| field.type_id),
            (TypeData::Array { elem_type_id, .. }, RefSegment::Index(_)) => Some(*elem_type_id),
            _ => None,
        }
    })
}
