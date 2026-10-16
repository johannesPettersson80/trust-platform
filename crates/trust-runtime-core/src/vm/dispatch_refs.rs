use smol_str::SmolStr;

use crate::error::RuntimeError;
use crate::memory::MemoryLocation;
use crate::value::{
    materialize_value_path, read_value_path_borrowed, single_ref_index, string_element_position,
    RefSegment, Value, ValueRef,
};

use super::context::ReferenceContext;
use super::errors::VmTrap;
use super::frames::{FrameStack, VmFrame};
use super::materialize_borrowed_value;
use super::module::{invalid_bytecode, VmModule, VmRef};
use super::type_policy::vm_type_for_path;

/// Read a static reference after policy and clone-budget checks.
pub fn load_ref(
    runtime: &impl ReferenceContext,
    module: &VmModule,
    frames: &FrameStack,
    ref_idx: u32,
) -> Result<Value, VmTrap> {
    match peek_ref(runtime, module, frames, ref_idx) {
        Ok(value) => {
            runtime.before_value_clone(value).map_err(VmTrap::Runtime)?;
            return Ok(materialize_borrowed_value(value).0);
        }
        Err(VmTrap::NullReference) => {}
        Err(error) => return Err(error),
    }
    // A string element is materialized as CHAR/WCHAR, so it has no borrowed Value.
    let reference = load_ref_view(runtime, module, frames, ref_idx)?;
    dynamic_load_view(runtime, frames, reference)
}

/// Borrow a static reference value after read-policy checks.
pub fn peek_ref<'a>(
    runtime: &'a impl ReferenceContext,
    module: &'a VmModule,
    frames: &'a FrameStack,
    ref_idx: u32,
) -> Result<&'a Value, VmTrap> {
    let address = load_ref_view(runtime, module, frames, ref_idx)?;
    runtime
        .check_reference_read(address)
        .map_err(VmTrap::Runtime)?;
    let reference = module
        .refs
        .get(ref_idx as usize)
        .ok_or_else(|| VmTrap::InvalidRefIndex(ref_idx))?;

    match reference {
        VmRef::InitializerResult {
            initializer_id,
            path,
        } => {
            let reference = runtime.initializer_reference(*initializer_id, path)?;
            runtime
                .storage()
                .read_by_ref_parts(reference.location, reference.offset, path)
                .ok_or_else(|| VmTrap::NullReference)
        }

        VmRef::Global { offset, path } if path.is_empty() => runtime
            .storage()
            .read_global_slot_by_offset(*offset)
            .ok_or_else(|| VmTrap::NullReference),
        VmRef::Global { offset, path } => runtime
            .storage()
            .read_by_ref_parts(MemoryLocation::Global, *offset, path)
            .ok_or_else(|| VmTrap::NullReference),
        VmRef::Local { offset, path, .. } => {
            let frame = frames.current().ok_or_else(|| VmTrap::CallStackUnderflow)?;
            if path.is_empty() {
                let slot = frame.local_slot_index(ref_idx)?;
                frame
                    .locals
                    .get(slot)
                    .ok_or_else(|| VmTrap::InvalidLocalRef {
                        ref_index: ref_idx,
                        start: frame.local_ref_start,
                        count: frame.local_ref_count,
                    })
            } else {
                let slot = *offset;
                peek_vm_local_ref(frame, slot, path)
            }
        }
        _ => runtime
            .storage()
            .read_by_ref_parts(address.location, address.offset, address.path)
            .ok_or_else(|| VmTrap::NullReference),
    }
}

/// Resolve a static descriptor to a runtime reference value.
pub fn load_ref_addr(
    runtime: &impl ReferenceContext,
    module: &VmModule,
    frames: &FrameStack,
    ref_idx: u32,
) -> Result<ValueRef, VmTrap> {
    let reference = load_ref_view(runtime, module, frames, ref_idx)?;
    runtime
        .before_reference_path_clone(reference)
        .map_err(VmTrap::Runtime)?;
    Ok(reference.to_owned())
}

fn load_ref_view<'a>(
    runtime: &impl ReferenceContext,
    module: &'a VmModule,
    frames: &FrameStack,
    ref_idx: u32,
) -> Result<crate::value::ValueRefView<'a>, VmTrap> {
    let reference = module
        .refs
        .get(ref_idx as usize)
        .ok_or_else(|| VmTrap::InvalidRefIndex(ref_idx))?;
    let (location, offset, path) = match reference {
        VmRef::InitializerResult {
            initializer_id,
            path,
        } => return runtime.initializer_reference(*initializer_id, path),
        VmRef::Global { offset, path } => (MemoryLocation::Global, *offset, path.as_slice()),
        VmRef::Local { offset, path, .. } => (
            MemoryLocation::Local(
                frames
                    .current()
                    .ok_or_else(|| VmTrap::CallStackUnderflow)?
                    .reference_frame_id(),
            ),
            *offset,
            path.as_slice(),
        ),
        _ => runtime_access_target(
            runtime,
            reference,
            frames.current().ok_or_else(|| VmTrap::CallStackUnderflow)?,
        )?,
    };
    Ok(crate::value::ValueRefView {
        location,
        offset,
        path,
    })
}

/// Write a static reference through type, lifetime and storage-policy gates.
pub fn store_ref(
    runtime: &mut impl ReferenceContext,
    module: &VmModule,
    frames: &mut FrameStack,
    ref_idx: u32,
    value: Value,
) -> Result<(), VmTrap> {
    let reference = module
        .refs
        .get(ref_idx as usize)
        .ok_or_else(|| VmTrap::InvalidRefIndex(ref_idx))?;
    let value = match module.ref_type(ref_idx) {
        Some(ty) => runtime
            .normalize_assignment(module, ty, value)
            .map_err(VmTrap::Runtime)?,
        None => value,
    };
    let address = load_ref_view(runtime, module, frames, ref_idx)?;
    runtime
        .check_reference_write(address, &value)
        .map_err(VmTrap::Runtime)?;

    match reference {
        VmRef::InitializerResult {
            initializer_id,
            path,
        } => {
            let reference = runtime.initializer_reference(*initializer_id, path)?;
            if runtime
                .write_typed_storage(reference.location, reference.offset, reference.path, value)
                .map_err(VmTrap::Runtime)?
            {
                Ok(())
            } else {
                Err(VmTrap::NullReference)
            }
        }

        VmRef::Global { offset, path } if path.is_empty() => {
            if runtime
                .storage_mut()
                .write_global_slot_by_offset(*offset, value)
            {
                Ok(())
            } else {
                Err(VmTrap::NullReference)
            }
        }
        VmRef::Global { offset, path } => {
            if runtime
                .write_typed_storage(MemoryLocation::Global, *offset, path, value)
                .map_err(VmTrap::Runtime)?
            {
                Ok(())
            } else {
                Err(VmTrap::NullReference)
            }
        }
        VmRef::Local { offset, path, .. } => {
            let frame = frames
                .current_mut()
                .ok_or_else(|| VmTrap::CallStackUnderflow)?;
            if path.is_empty() {
                frame.store_local(ref_idx, value)
            } else {
                let slot = *offset;
                write_vm_local_ref(frame, slot, path, value, runtime)
            }
        }
        _ => {
            if runtime
                .write_typed_storage(address.location, address.offset, address.path, value)
                .map_err(VmTrap::Runtime)?
            {
                Ok(())
            } else {
                Err(VmTrap::NullReference)
            }
        }
    }
}

/// Consume a non-null reference operand or return a VM trap.
pub fn pop_reference(stack: &mut super::stack::OperandStack) -> Result<ValueRef, VmTrap> {
    let value = stack.pop()?;
    match value {
        Value::Reference(Some(reference)) => Ok(reference),
        Value::Reference(None) => Err(VmTrap::NullReference),
        _ => Err(VmTrap::Runtime(RuntimeError::TypeMismatch)),
    }
}

/// Extend an owned reference with a checked field selection.
pub fn dynamic_ref_field(
    runtime: &impl ReferenceContext,
    frames: &FrameStack,
    mut reference: ValueRef,
    field: SmolStr,
) -> Result<ValueRef, VmTrap> {
    let target = peek_dynamic_ref(runtime, frames, &reference)?;
    match target {
        Value::Struct(struct_value) => {
            if !struct_value.contains_field(field.as_str()) {
                return Err(VmTrap::Runtime(RuntimeError::UndefinedField(field)));
            }
            reserve_reference_segment(runtime, &mut reference, 0)?;
            reference.path.push(RefSegment::Field(field));
            Ok(reference)
        }
        Value::Instance(instance_id) => runtime
            .storage()
            .resolved_instance_field_ref(*instance_id, field.as_str())
            .ok_or_else(|| VmTrap::Runtime(RuntimeError::UndefinedField(field))),
        _ => Err(VmTrap::Runtime(RuntimeError::TypeMismatch)),
    }
}

/// Extend a reference with a borrowed field name.
#[cfg(any(feature = "hir", feature = "register-ir"))]
pub fn dynamic_ref_field_borrowed(
    runtime: &impl ReferenceContext,
    frames: &FrameStack,
    reference: &ValueRef,
    field: SmolStr,
) -> Result<ValueRef, VmTrap> {
    let target = peek_dynamic_ref(runtime, frames, reference)?;
    match target {
        Value::Struct(struct_value) => {
            if !struct_value.contains_field(field.as_str()) {
                return Err(VmTrap::Runtime(RuntimeError::UndefinedField(field)));
            }
            runtime
                .before_reference_path_clone(reference.as_view())
                .map_err(VmTrap::Runtime)?;
            // TIME-08: the added field segment costs one unit and its own bytes, as the
            // owned path in `reserve_reference_segment` does.
            runtime
                .before_reference_path_growth(core::mem::size_of::<RefSegment>(), 1)
                .map_err(VmTrap::Runtime)?;
            let capacity = reference
                .path
                .len()
                .checked_add(1)
                .ok_or_else(|| RuntimeError::Overflow)?;
            let mut path = alloc::vec::Vec::new();
            path.try_reserve_exact(capacity)
                .map_err(|_| RuntimeError::Overflow)?;
            path.extend_from_slice(&reference.path);
            let mut next = ValueRef {
                location: reference.location,
                offset: reference.offset,
                path,
            };
            next.path.push(RefSegment::Field(field));
            Ok(next)
        }
        Value::Instance(instance_id) => runtime
            .storage()
            .resolved_instance_field_ref(*instance_id, field.as_str())
            .ok_or_else(|| VmTrap::Runtime(RuntimeError::UndefinedField(field))),
        _ => Err(VmTrap::Runtime(RuntimeError::TypeMismatch)),
    }
}

/// Consume index operands and append a checked array selection.
pub fn dynamic_ref_index(
    runtime: &impl ReferenceContext,
    frames: &FrameStack,
    mut reference: ValueRef,
    index: i64,
) -> Result<ValueRef, VmTrap> {
    // Support chained indexing for multidimensional arrays by extending a trailing
    // partial index segment (e.g. [i] -> [i, j]) against the base array dimensions.
    if let Some(RefSegment::Index(existing)) = reference.path.last() {
        let base_path = &reference.path[..reference.path.len().saturating_sub(1)];
        if let Value::Array(array) =
            peek_dynamic_ref_path(runtime, frames, reference.as_view(), base_path)?
            && existing.len() < array.dimensions().len()
        {
            let (lower, upper) = array.dimensions()[existing.len()];
            if index < lower || index > upper {
                return Err(VmTrap::Runtime(RuntimeError::IndexOutOfBounds {
                    index,
                    lower,
                    upper,
                }));
            }
            // TIME-08: the added subscript costs one unit and its own bytes, whatever the
            // capacity of the trailing index vector.
            runtime
                .before_reference_path_growth(core::mem::size_of::<i64>(), 1)
                .map_err(VmTrap::Runtime)?;
            if let Some(RefSegment::Index(indices)) = reference.path.last_mut() {
                indices
                    .try_reserve_exact(1)
                    .map_err(|_| RuntimeError::Overflow)?;
                indices.push(index);
                return Ok(reference);
            }
        }
    }

    let target = peek_dynamic_ref(runtime, frames, &reference)?;
    match target {
        Value::Array(array) => {
            let Some((lower, upper)) = array.dimensions().first().copied() else {
                return Err(VmTrap::Runtime(RuntimeError::TypeMismatch));
            };
            if index < lower || index > upper {
                return Err(VmTrap::Runtime(RuntimeError::IndexOutOfBounds {
                    index,
                    lower,
                    upper,
                }));
            }
            reserve_reference_segment(runtime, &mut reference, 1)?;
            reference.path.push(single_ref_index(index));
            Ok(reference)
        }
        Value::String(text) => {
            runtime
                .before_reference_path_growth(0, text.len())
                .map_err(VmTrap::Runtime)?;
            string_element_position(text.as_str(), index).map_err(VmTrap::Runtime)?;
            reserve_reference_segment(runtime, &mut reference, 1)?;
            reference.path.push(single_ref_index(index));
            Ok(reference)
        }
        Value::WString(text) => {
            runtime
                .before_reference_path_growth(0, text.len())
                .map_err(VmTrap::Runtime)?;
            string_element_position(text.as_str(), index).map_err(VmTrap::Runtime)?;
            reserve_reference_segment(runtime, &mut reference, 1)?;
            reference.path.push(single_ref_index(index));
            Ok(reference)
        }
        _ => Err(VmTrap::Runtime(RuntimeError::TypeMismatch)),
    }
}

/// Admit one new owned path segment that carries `subscripts` subscripts, then reserve its
/// slot. Each added segment and each added subscript costs one unit and its own bytes,
/// whatever the path vector's capacity (spec 34 TIME-08).
fn reserve_reference_segment(
    runtime: &impl ReferenceContext,
    reference: &mut ValueRef,
    subscripts: usize,
) -> Result<(), VmTrap> {
    let subscript_bytes = subscripts
        .checked_mul(core::mem::size_of::<i64>())
        .ok_or_else(|| RuntimeError::Overflow)?;
    let bytes = core::mem::size_of::<RefSegment>()
        .checked_add(subscript_bytes)
        .ok_or_else(|| RuntimeError::Overflow)?;
    let work = subscripts
        .checked_add(1)
        .ok_or_else(|| RuntimeError::Overflow)?;
    runtime
        .before_reference_path_growth(bytes, work)
        .map_err(VmTrap::Runtime)?;
    reference
        .path
        .try_reserve_exact(1)
        .map_err(|_| RuntimeError::Overflow)?;
    Ok(())
}

/// Borrow a dynamic target after resolving frame and instance ownership.
pub fn peek_dynamic_ref<'a>(
    runtime: &'a impl ReferenceContext,
    frames: &'a FrameStack,
    reference: &ValueRef,
) -> Result<&'a Value, VmTrap> {
    peek_dynamic_ref_path(runtime, frames, reference.as_view(), &reference.path)
}

fn peek_dynamic_ref_path<'a>(
    runtime: &'a impl ReferenceContext,
    frames: &'a FrameStack,
    reference: crate::value::ValueRefView<'_>,
    path: &[RefSegment],
) -> Result<&'a Value, VmTrap> {
    runtime
        .check_reference_read(reference)
        .map_err(VmTrap::Runtime)?;
    if frames.current().is_some_and(|frame| {
        reference.location == MemoryLocation::Local(frame.reference_frame_id())
    }) {
        let frame = frames.current().ok_or_else(|| VmTrap::CallStackUnderflow)?;
        let root = frame
            .locals
            .get(reference.offset)
            .ok_or_else(|| VmTrap::NullReference)?;
        return read_value_path_borrowed(root, path).ok_or_else(|| VmTrap::NullReference);
    }
    runtime
        .storage()
        .read_by_ref_parts(reference.location, reference.offset, path)
        .ok_or_else(|| VmTrap::NullReference)
}

/// Read a dynamic target with policy and pre-copy charging.
pub fn dynamic_load_ref(
    runtime: &impl ReferenceContext,
    frames: &FrameStack,
    reference: &ValueRef,
) -> Result<Value, VmTrap> {
    dynamic_load_view(runtime, frames, reference.as_view())
}

fn dynamic_load_view(
    runtime: &impl ReferenceContext,
    frames: &FrameStack,
    reference: crate::value::ValueRefView<'_>,
) -> Result<Value, VmTrap> {
    match peek_dynamic_ref_path(runtime, frames, reference, reference.path) {
        Ok(value) => {
            runtime.before_value_clone(value).map_err(VmTrap::Runtime)?;
            Ok(materialize_borrowed_value(value).0)
        }
        Err(VmTrap::NullReference) => {
            let (last, prefix) = reference
                .path
                .split_last()
                .ok_or_else(|| VmTrap::NullReference)?;
            let parent = peek_dynamic_ref_path(runtime, frames, reference, prefix)?;
            runtime
                .before_value_clone(parent)
                .map_err(VmTrap::Runtime)?;
            charge_materialize_path_work(runtime, parent, core::slice::from_ref(last))
                .map_err(VmTrap::Runtime)?;
            materialize_value_path(parent, core::slice::from_ref(last))
                .ok_or_else(|| VmTrap::NullReference)
        }
        Err(error) => Err(error),
    }
}

/// Reading one character scans the text up to its index: TEXT work (spec 34 TIME-08).
pub(crate) fn charge_materialize_path_work(
    runtime: &impl ReferenceContext,
    root: &Value,
    path: &[RefSegment],
) -> Result<(), RuntimeError> {
    let Some((RefSegment::Index(_), prefix)) = path.split_last() else {
        return Ok(());
    };
    let Some(value) = read_value_path_borrowed(root, prefix) else {
        return Ok(());
    };
    let bytes = match value {
        Value::String(text) => text.len(),
        Value::WString(text) => text.len(),
        _ => return Ok(()),
    };
    let work = bytes.checked_mul(2).ok_or_else(|| RuntimeError::Overflow)?;
    runtime.charge_text_work(work)
}

/// Write a dynamic target with policy, type and allocation checks.
pub fn dynamic_store_ref(
    runtime: &mut impl ReferenceContext,
    module: &VmModule,
    frames: &mut FrameStack,
    reference: &ValueRef,
    value: Value,
) -> Result<(), VmTrap> {
    runtime
        .check_reference_write(reference.as_view(), &value)
        .map_err(VmTrap::Runtime)?;
    let frame = frames.current().ok_or_else(|| VmTrap::CallStackUnderflow)?;
    let value = normalize_dynamic_store_value(runtime, module, frame, reference, value)?;
    if frames
        .current()
        .is_some_and(|frame| frame.owns_local_reference(reference))
    {
        let frame = frames
            .current_mut()
            .ok_or_else(|| VmTrap::CallStackUnderflow)?;
        return write_vm_local_ref(frame, reference.offset, &reference.path, value, runtime);
    }
    if runtime
        .write_typed_storage(reference.location, reference.offset, &reference.path, value)
        .map_err(VmTrap::Runtime)?
    {
        Ok(())
    } else {
        Err(VmTrap::NullReference)
    }
}

fn peek_vm_local_ref<'a>(
    frame: &'a VmFrame,
    offset: usize,
    path: &[RefSegment],
) -> Result<&'a Value, VmTrap> {
    let root = frame
        .locals
        .get(offset)
        .ok_or_else(|| VmTrap::NullReference)?;
    read_value_path_borrowed(root, path).ok_or_else(|| VmTrap::NullReference)
}

fn write_vm_local_ref(
    frame: &mut VmFrame,
    offset: usize,
    path: &[RefSegment],
    value: Value,
    runtime: &impl ReferenceContext,
) -> Result<(), VmTrap> {
    let root = frame
        .locals
        .get_mut(offset)
        .ok_or_else(|| VmTrap::NullReference)?;
    runtime
        .before_path_write(root, path, &value)
        .map_err(VmTrap::Runtime)?;
    if runtime
        .write_typed_local_checked(root, path, value)
        .map_err(VmTrap::Runtime)?
    {
        Ok(())
    } else {
        Err(VmTrap::NullReference)
    }
}

fn normalize_dynamic_store_value(
    runtime: &impl ReferenceContext,
    module: &VmModule,
    frame: &VmFrame,
    reference: &ValueRef,
    value: Value,
) -> Result<Value, VmTrap> {
    let Some(type_idx) = dynamic_ref_type(runtime, module, frame, reference)? else {
        return Ok(value);
    };
    runtime
        .normalize_assignment(module, type_idx, value)
        .map_err(VmTrap::Runtime)
}

/// Determine the declared type reached by a dynamic reference.
pub fn dynamic_ref_type(
    runtime: &impl ReferenceContext,
    module: &VmModule,
    frame: &VmFrame,
    reference: &ValueRef,
) -> Result<Option<u32>, VmTrap> {
    if frame.owns_local_reference(reference) {
        if reference.offset >= frame.local_ref_count as usize {
            return Ok(None);
        }
        let Some(offset) = u32::try_from(reference.offset).ok() else {
            return Ok(None);
        };
        let Some(base_type) = frame
            .local_ref_start
            .checked_add(offset)
            .and_then(|ref_idx| module.ref_type(ref_idx))
        else {
            return Ok(None);
        };
        return Ok(vm_type_for_path(module, base_type, &reference.path));
    }

    let candidates = runtime
        .dynamic_reference_candidates(module, reference.as_view())
        .map_err(VmTrap::Runtime)?;
    let count = candidates.map_or(module.refs.len(), <[u32]>::len);
    let mut resolved = None;
    for ordinal in 0..count {
        let ref_idx = match candidates {
            Some(ids) => ids[ordinal],
            None => u32::try_from(ordinal).map_err(|_| VmTrap::InvalidRefIndex(u32::MAX))?,
        };
        let candidate = module
            .refs
            .get(ref_idx as usize)
            .ok_or_else(|| VmTrap::InvalidRefIndex(ref_idx))?;
        let path = match (reference.location, candidate) {
            (MemoryLocation::Global, VmRef::Global { offset, path })
            | (MemoryLocation::Instance(_), VmRef::Instance { offset, path, .. })
                if *offset == reference.offset =>
            {
                path
            }
            _ => continue,
        };
        let Some(suffix) = reference.path.as_slice().strip_prefix(path.as_slice()) else {
            continue;
        };
        let Some(base_type_idx) = module.ref_type(ref_idx) else {
            continue;
        };
        let (location, offset, _) = match runtime_access_target(runtime, candidate, frame) {
            Ok(target) => target,
            Err(VmTrap::Runtime(RuntimeError::NullReference)) => continue,
            Err(VmTrap::Runtime(error)) => return Err(VmTrap::Runtime(error)),
            Err(_) => continue,
        };
        if location == reference.location && offset == reference.offset {
            let Some(type_idx) = vm_type_for_path(module, base_type_idx, suffix) else {
                continue;
            };
            if resolved.is_some_and(|existing| existing != type_idx) {
                return Err(VmTrap::Runtime(invalid_bytecode(
                    "conflicting type metadata for runtime reference",
                )));
            }
            resolved = Some(type_idx);
        }
    }
    Ok(resolved)
}

/// Convert an IEC integer index to the common checked signed representation.
pub fn index_to_i64(value: Value) -> Result<i64, VmTrap> {
    match value {
        Value::SInt(v) => Ok(v as i64),
        Value::Int(v) => Ok(v as i64),
        Value::DInt(v) => Ok(v as i64),
        Value::LInt(v) => Ok(v),
        Value::USInt(v) => Ok(v as i64),
        Value::UInt(v) => Ok(v as i64),
        Value::UDInt(v) => Ok(v as i64),
        Value::ULInt(v) => {
            i64::try_from(v).map_err(|_| VmTrap::Runtime(RuntimeError::TypeMismatch))
        }
        _ => Err(VmTrap::Runtime(RuntimeError::TypeMismatch)),
    }
}

fn runtime_access_target<'a>(
    runtime: &impl ReferenceContext,
    reference: &'a VmRef,
    frame: &VmFrame,
) -> Result<(MemoryLocation, usize, &'a [RefSegment]), VmTrap> {
    match reference {
        VmRef::InitializerResult { .. } => {
            Err(VmTrap::UnsupportedRefLocation("initializer-result"))
        }

        VmRef::Global { offset, path } => Ok((MemoryLocation::Global, *offset, path.as_slice())),
        VmRef::Instance {
            owner_instance_id,
            offset,
            path,
        } => {
            let runtime_owner = runtime.resolve_instance_owner(*owner_instance_id, frame)?;
            Ok((
                MemoryLocation::Instance(runtime_owner),
                *offset,
                path.as_slice(),
            ))
        }
        VmRef::Local {
            owner_frame_id,
            offset,
            path,
        } => Err(VmTrap::UnsupportedRefLocation(if path.is_empty() {
            let _ = owner_frame_id;
            let _ = offset;
            "local"
        } else {
            "local-path"
        })),
        VmRef::Retain { offset, path } => {
            let _ = (offset, path);
            Err(VmTrap::UnsupportedRefLocation("retain"))
        }
        VmRef::Io { area, offset, path } => {
            let _ = (area, offset, path);
            Err(VmTrap::UnsupportedRefLocation("io"))
        }
    }
}

#[cfg(test)]
#[path = "dispatch_refs_tests.rs"]
mod dispatch_refs_tests;
