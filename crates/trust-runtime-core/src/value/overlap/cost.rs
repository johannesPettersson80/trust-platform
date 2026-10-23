//! Destination-indexed upper bounds for the actual patch, alias and nested-backing work.
use super::*;

pub(crate) struct PhysicalWriteCost {
    pub(crate) work: usize,
    pub(crate) allocation: usize,
}

impl StructValue {
    pub(crate) fn physical_write_cost(
        &self,
        path: &[RefSegment],
    ) -> Result<Option<PhysicalWriteCost>, RuntimeError> {
        let Some(physical) = &self.physical else {
            return Ok(None);
        };
        let Some(selected) = physical.layout.selection(path) else {
            return Ok(None);
        };
        let layout = &physical.layout;
        let candidates = layout.affected_leaves(&selected.bits);
        let patch_bytes = usize::try_from(selected.bits.end.div_ceil(8) - selected.bits.start / 8)
            .map_err(|_| RuntimeError::Overflow)?;
        let mut allocation = patch_bytes
            .checked_add(
                candidates
                    .len()
                    .checked_mul(core::mem::size_of::<(usize, Value)>())
                    .ok_or(RuntimeError::Overflow)?,
            )
            .ok_or(RuntimeError::Overflow)?;
        let lookups = usize::BITS as usize
            - layout
                .leaves
                .len()
                .max(layout.children.len())
                .max(1)
                .leading_zeros() as usize;
        let mut work = lookups
            .checked_mul(layout.max_path_work.max(path_work(path)))
            .and_then(|n| n.checked_mul(8))
            .and_then(|n| n.checked_add(patch_bytes))
            .ok_or(RuntimeError::Overflow)?;
        if path.is_empty() || !selected.children.is_empty() {
            // Aggregate patches clear all selected bits, then may copy an entire
            // source backing (including sparse gaps), rather than only its leaves.
            work = add(work, (selected.bits.end - selected.bits.start) as usize)?;
            if path.is_empty() {
                work = add(work, (selected.bits.end - selected.bits.start) as usize)?;
            }
        }
        for &i in &layout.path_order[selected.leaves.clone()] {
            let leaf = &layout.leaves[i];
            work = add(
                work,
                192 + path_work(&leaf.path)
                    .checked_mul(4)
                    .ok_or(RuntimeError::Overflow)?,
            )?;
            if let Some(enumeration) = &leaf.enumeration {
                work = add(
                    work,
                    enumeration
                        .variants
                        .len()
                        .checked_mul(4)
                        .ok_or(RuntimeError::Overflow)?,
                )?;
            }
        }
        for &i in &layout.bit_order[candidates] {
            let leaf = &layout.leaves[i];
            work = add(work, 192 + path_work(&leaf.path))?;
            if let Some(enumeration) = &leaf.enumeration {
                work = add(
                    work,
                    enumeration
                        .variants
                        .len()
                        .checked_mul(2)
                        .ok_or(RuntimeError::Overflow)?,
                )?;
                // Alias reconstruction clones its type/variant names, not every enum in the root.
                let name = enumeration
                    .variants
                    .iter()
                    .map(|(name, _)| name.len())
                    .max()
                    .unwrap_or(0);
                let type_len = match read_struct_path(self, &leaf.path) {
                    Some(Value::Enum(v)) => v.type_name().len(),
                    _ => 0,
                };
                allocation = add(allocation, core::mem::size_of::<super::super::EnumValue>())?;
                allocation = add(allocation, name)?;
                allocation = add(allocation, type_len)?;
            }
        }
        for &i in &layout.child_path_order[selected.children] {
            let child = &layout.children[i];
            work = add(work, child.bits as usize)?;
            work = add(
                work,
                path_work(&child.path)
                    .checked_mul(2)
                    .ok_or(RuntimeError::Overflow)?,
            )?;
        }
        for i in layout.affected_children(&selected.bits) {
            let child = &layout.children[i];
            let overlap = (child.offset + child.bits)
                .min(selected.bits.end)
                .saturating_sub(child.offset.max(selected.bits.start));
            work = add(work, overlap as usize)?;
            work = add(work, path_work(&child.path))?;
        }
        if path.is_empty() {
            work = add(work, self.fields.len())?;
        }
        Ok(Some(PhysicalWriteCost { work, allocation }))
    }

    /// Indexed immutable paths of structures whose backing intersects this write.
    pub(crate) fn physical_affected_children<'a>(
        &'a self,
        path: &[RefSegment],
    ) -> Option<impl Iterator<Item = &'a [RefSegment]>> {
        let physical = self.physical.as_ref()?;
        let selected = physical.layout.selection(path)?;
        Some(
            physical
                .layout
                .affected_children(&selected.bits)
                .map(move |i| physical.layout.children[i].path.as_slice()),
        )
    }

    pub(crate) fn physical_whole_assignment_is_admitted(&self, input: &Value) -> bool {
        let (Some(expected), Value::Struct(source)) = (&self.physical, input) else {
            return false;
        };
        source
            .physical
            .as_ref()
            .is_some_and(|source| Arc::ptr_eq(&source.layout, &expected.layout))
    }
}

impl PhysicalLayout {
    /// Scratch and owned enum replacements used when initially admitting backing.
    /// Backing byte_len is charged separately by the construction owner.
    pub(crate) fn initialization_staging_allocation(
        &self,
        value: &StructValue,
    ) -> Result<usize, RuntimeError> {
        let mut bytes = self
            .leaves
            .len()
            .checked_mul(core::mem::size_of::<(usize, Value)>())
            .ok_or(RuntimeError::Overflow)?;
        for leaf in &self.leaves {
            if let Some(enumeration) = &leaf.enumeration {
                let name = enumeration
                    .variants
                    .iter()
                    .map(|(name, _)| name.len())
                    .max()
                    .unwrap_or(0);
                let type_len = match read_struct_path(value, &leaf.path) {
                    Some(Value::Enum(v)) => v.type_name().len(),
                    _ => 0,
                };
                bytes = add(bytes, core::mem::size_of::<super::super::EnumValue>())?;
                bytes = add(bytes, name)?;
                bytes = add(bytes, type_len)?;
            }
        }
        // Fresh outer backing can still publish an alias into a shared canonical
        // child. Copying its Arc propagates sharing to nested child Arcs.
        for &i in &self.child_path_order {
            let child = &self.children[i];
            let mut shared = false;
            for length in 1..=child.path.len() {
                if let Some(Value::Struct(value)) = read_struct_path(value, &child.path[..length]) {
                    shared |= Arc::strong_count(value) > 1;
                }
            }
            if shared && let Some(Value::Struct(child)) = read_struct_path(value, &child.path) {
                bytes = add(bytes, structure_copy_bytes(child)?)?;
            } else if let Some(Value::Struct(child)) = read_struct_path(value, &child.path)
                && Arc::weak_count(child) > 0
            {
                bytes = add(
                    bytes,
                    core::mem::size_of::<StructValue>() + 2 * core::mem::size_of::<usize>(),
                )?;
            }
        }
        Ok(bytes)
    }
}

fn structure_copy_bytes(value: &StructValue) -> Result<usize, RuntimeError> {
    let mut bytes = add(
        core::mem::size_of::<StructValue>() + 2 * core::mem::size_of::<usize>(),
        value.physical_byte_len(),
    )?;
    bytes = add(
        bytes,
        value
            .fields
            .len()
            .checked_mul(4 * core::mem::size_of::<(SmolStr, Value)>())
            .ok_or(RuntimeError::Overflow)?,
    )?;
    for value in value.fields.values() {
        bytes = add(bytes, owned_copy_bytes(value, 0)?)?;
    }
    Ok(bytes)
}

fn owned_copy_bytes(value: &Value, depth: usize) -> Result<usize, RuntimeError> {
    if depth > crate::bytecode::BYTECODE_MAX_TYPE_NESTING {
        return Err(RuntimeError::TypeMismatch);
    }
    let mut bytes = core::mem::size_of::<Value>();
    match value {
        Value::WString(value) => bytes = add(bytes, value.len())?,
        Value::Enum(_) => bytes = add(bytes, core::mem::size_of::<super::super::EnumValue>())?,
        Value::Array(value) => {
            bytes = add(bytes, core::mem::size_of::<super::super::ArrayValue>())?;
            bytes = add(
                bytes,
                value
                    .dimensions()
                    .len()
                    .checked_mul(core::mem::size_of::<(i64, i64)>())
                    .ok_or(RuntimeError::Overflow)?,
            )?;
            for value in value.elements() {
                bytes = add(bytes, owned_copy_bytes(value, depth + 1)?)?;
            }
        }
        Value::Reference(Some(value)) => {
            bytes = add(
                bytes,
                value
                    .path
                    .len()
                    .checked_mul(core::mem::size_of::<RefSegment>())
                    .ok_or(RuntimeError::Overflow)?,
            )?;
            for part in &value.path {
                if let RefSegment::Index(indices) = part {
                    bytes = add(
                        bytes,
                        indices
                            .len()
                            .checked_mul(core::mem::size_of::<i64>())
                            .ok_or(RuntimeError::Overflow)?,
                    )?;
                }
            }
        }
        _ => {}
    }
    Ok(bytes)
}

fn add(a: usize, b: usize) -> Result<usize, RuntimeError> {
    a.checked_add(b).ok_or(RuntimeError::Overflow)
}
