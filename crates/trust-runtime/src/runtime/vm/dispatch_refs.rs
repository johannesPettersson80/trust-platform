use smol_str::SmolStr;

use crate::error::RuntimeError;
use crate::memory::{FrameId, InstanceId, MemoryLocation};
use crate::value::{RefSegment, Value, ValueRef};

use super::super::core::Runtime;
use super::call::VM_LOCAL_SENTINEL_FRAME_ID;
use super::errors::VmTrap;
use super::frames::{FrameStack, VmFrame};
use super::{VmModule, VmRef};

pub(super) fn load_ref(
    runtime: &Runtime,
    module: &VmModule,
    frames: &FrameStack,
    ref_idx: u32,
) -> Result<Value, VmTrap> {
    let reference = module
        .refs
        .get(ref_idx as usize)
        .ok_or(VmTrap::InvalidRefIndex(ref_idx))?;

    match reference {
        VmRef::Local { offset, path, .. } => {
            let frame = frames.current().ok_or(VmTrap::CallStackUnderflow)?;
            if path.is_empty() {
                frame.load_local(ref_idx)
            } else {
                let slot = *offset;
                read_vm_local_ref(frame, slot, path)
            }
        }
        _ => {
            let frame = frames.current().ok_or(VmTrap::CallStackUnderflow)?;
            let (location, offset, path) = runtime_access_target(reference, frame)?;
            runtime
                .storage
                .read_by_ref_parts(location, offset, path)
                .cloned()
                .ok_or(VmTrap::NullReference)
        }
    }
}

pub(super) fn load_ref_addr(
    module: &VmModule,
    frames: &FrameStack,
    ref_idx: u32,
) -> Result<ValueRef, VmTrap> {
    let reference = module
        .refs
        .get(ref_idx as usize)
        .ok_or(VmTrap::InvalidRefIndex(ref_idx))?;
    match reference {
        VmRef::Local { offset, path, .. } => {
            let _ = frames.current().ok_or(VmTrap::CallStackUnderflow)?;
            Ok(ValueRef {
                location: MemoryLocation::Local(FrameId(VM_LOCAL_SENTINEL_FRAME_ID)),
                offset: *offset,
                path: path.clone(),
            })
        }
        _ => {
            let frame = frames.current().ok_or(VmTrap::CallStackUnderflow)?;
            let (location, offset, path) = runtime_access_target(reference, frame)?;
            Ok(ValueRef {
                location,
                offset,
                path: path.to_vec(),
            })
        }
    }
}

pub(super) fn store_ref(
    runtime: &mut Runtime,
    module: &VmModule,
    frames: &mut FrameStack,
    ref_idx: u32,
    value: Value,
) -> Result<(), VmTrap> {
    let reference = module
        .refs
        .get(ref_idx as usize)
        .ok_or(VmTrap::InvalidRefIndex(ref_idx))?;

    match reference {
        VmRef::Local { offset, path, .. } => {
            let frame = frames.current_mut().ok_or(VmTrap::CallStackUnderflow)?;
            if path.is_empty() {
                frame.store_local(ref_idx, value)
            } else {
                let slot = *offset;
                write_vm_local_ref(frame, slot, path, value)
            }
        }
        _ => {
            let frame = frames.current().ok_or(VmTrap::CallStackUnderflow)?;
            let (location, offset, path) = runtime_access_target(reference, frame)?;
            if runtime
                .storage
                .write_by_ref_parts(location, offset, path, value)
            {
                Ok(())
            } else {
                Err(VmTrap::NullReference)
            }
        }
    }
}

pub(super) fn pop_reference(stack: &mut super::stack::OperandStack) -> Result<ValueRef, VmTrap> {
    let value = stack.pop()?;
    match value {
        Value::Reference(Some(reference)) => Ok(reference),
        Value::Reference(None) => Err(VmTrap::NullReference),
        _ => Err(VmTrap::Runtime(RuntimeError::TypeMismatch)),
    }
}

pub(super) fn dynamic_ref_field(
    runtime: &Runtime,
    frames: &FrameStack,
    mut reference: ValueRef,
    field: SmolStr,
) -> Result<ValueRef, VmTrap> {
    let target = dynamic_load_ref(runtime, frames, &reference)?;
    match target {
        Value::Struct(struct_value) => {
            if !struct_value.fields.contains_key(field.as_str()) {
                return Err(VmTrap::Runtime(RuntimeError::UndefinedField(field)));
            }
            reference.path.push(RefSegment::Field(field));
            Ok(reference)
        }
        Value::Instance(instance_id) => runtime
            .storage
            .ref_for_instance_recursive(instance_id, field.as_str())
            .ok_or(VmTrap::Runtime(RuntimeError::UndefinedField(field))),
        _ => Err(VmTrap::Runtime(RuntimeError::TypeMismatch)),
    }
}

pub(super) fn dynamic_ref_index(
    runtime: &Runtime,
    frames: &FrameStack,
    mut reference: ValueRef,
    index: i64,
) -> Result<ValueRef, VmTrap> {
    // Support chained indexing for multidimensional arrays by extending a trailing
    // partial index segment (e.g. [i] -> [i, j]) against the base array dimensions.
    if let Some(RefSegment::Index(existing)) = reference.path.last().cloned() {
        let mut base_reference = reference.clone();
        let _ = base_reference.path.pop();
        if let Value::Array(array) = dynamic_load_ref(runtime, frames, &base_reference)? {
            if existing.len() < array.dimensions.len() {
                let (lower, upper) = array.dimensions[existing.len()];
                if index < lower || index > upper {
                    return Err(VmTrap::Runtime(RuntimeError::IndexOutOfBounds {
                        index,
                        lower,
                        upper,
                    }));
                }
                let mut combined = existing;
                combined.push(index);
                if let Some(RefSegment::Index(indices)) = reference.path.last_mut() {
                    *indices = combined;
                    return Ok(reference);
                }
            }
        }
    }

    let target = dynamic_load_ref(runtime, frames, &reference)?;
    match target {
        Value::Array(array) => {
            let Some((lower, upper)) = array.dimensions.first().copied() else {
                return Err(VmTrap::Runtime(RuntimeError::TypeMismatch));
            };
            if index < lower || index > upper {
                return Err(VmTrap::Runtime(RuntimeError::IndexOutOfBounds {
                    index,
                    lower,
                    upper,
                }));
            }
            reference.path.push(RefSegment::Index(vec![index]));
            Ok(reference)
        }
        _ => Err(VmTrap::Runtime(RuntimeError::TypeMismatch)),
    }
}

pub(super) fn dynamic_load_ref(
    runtime: &Runtime,
    frames: &FrameStack,
    reference: &ValueRef,
) -> Result<Value, VmTrap> {
    if matches!(
        reference.location,
        MemoryLocation::Local(FrameId(VM_LOCAL_SENTINEL_FRAME_ID))
    ) {
        let frame = frames.current().ok_or(VmTrap::CallStackUnderflow)?;
        return read_vm_local_ref(frame, reference.offset, &reference.path);
    }
    runtime
        .storage
        .read_by_ref(reference.clone())
        .cloned()
        .ok_or(VmTrap::NullReference)
}

pub(super) fn dynamic_store_ref(
    runtime: &mut Runtime,
    frames: &mut FrameStack,
    reference: &ValueRef,
    value: Value,
) -> Result<(), VmTrap> {
    if matches!(
        reference.location,
        MemoryLocation::Local(FrameId(VM_LOCAL_SENTINEL_FRAME_ID))
    ) {
        let frame = frames.current_mut().ok_or(VmTrap::CallStackUnderflow)?;
        return write_vm_local_ref(frame, reference.offset, &reference.path, value);
    }
    if runtime.storage.write_by_ref(reference.clone(), value) {
        Ok(())
    } else {
        Err(VmTrap::NullReference)
    }
}

fn read_vm_local_ref(frame: &VmFrame, offset: usize, path: &[RefSegment]) -> Result<Value, VmTrap> {
    let root = frame.locals.get(offset).ok_or(VmTrap::NullReference)?;
    read_value_path(root, path)
        .cloned()
        .ok_or(VmTrap::NullReference)
}

fn write_vm_local_ref(
    frame: &mut VmFrame,
    offset: usize,
    path: &[RefSegment],
    value: Value,
) -> Result<(), VmTrap> {
    let root = frame.locals.get_mut(offset).ok_or(VmTrap::NullReference)?;
    if write_value_path(root, path, value) {
        Ok(())
    } else {
        Err(VmTrap::NullReference)
    }
}

fn read_value_path<'a>(value: &'a Value, path: &[RefSegment]) -> Option<&'a Value> {
    if path.is_empty() {
        return Some(value);
    }
    match &path[0] {
        RefSegment::Field(name) => match value {
            Value::Struct(struct_value) => struct_value
                .fields
                .get(name)
                .and_then(|field| read_value_path(field, &path[1..])),
            _ => None,
        },
        RefSegment::Index(indices) => match value {
            Value::Array(array) => {
                let offset = array_offset_i64(&array.dimensions, indices)?;
                array
                    .elements
                    .get(offset)
                    .and_then(|element| read_value_path(element, &path[1..]))
            }
            _ => None,
        },
    }
}

fn write_value_path(target: &mut Value, path: &[RefSegment], value: Value) -> bool {
    if path.is_empty() {
        *target = value;
        return true;
    }

    match &path[0] {
        RefSegment::Field(name) => match target {
            Value::Struct(struct_value) => struct_value
                .fields
                .get_mut(name)
                .map(|field| write_value_path(field, &path[1..], value))
                .unwrap_or(false),
            _ => false,
        },
        RefSegment::Index(indices) => match target {
            Value::Array(array) => {
                let offset = match array_offset_i64(&array.dimensions, indices) {
                    Some(offset) => offset,
                    None => return false,
                };
                array
                    .elements
                    .get_mut(offset)
                    .map(|element| write_value_path(element, &path[1..], value))
                    .unwrap_or(false)
            }
            _ => false,
        },
    }
}

fn array_offset_i64(dimensions: &[(i64, i64)], indices: &[i64]) -> Option<usize> {
    if dimensions.len() != indices.len() {
        return None;
    }
    let mut offset: i128 = 0;
    let mut stride: i128 = 1;
    for ((lower, upper), index) in dimensions.iter().zip(indices).rev() {
        if index < lower || index > upper {
            return None;
        }
        let len = (*upper - *lower + 1) as i128;
        offset += (index - *lower) as i128 * stride;
        stride *= len;
    }
    usize::try_from(offset).ok()
}

pub(super) fn index_to_i64(value: Value) -> Result<i64, VmTrap> {
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
    reference: &'a VmRef,
    frame: &VmFrame,
) -> Result<(MemoryLocation, usize, &'a [RefSegment]), VmTrap> {
    match reference {
        VmRef::Global { offset, path } => Ok((MemoryLocation::Global, *offset, path.as_slice())),
        VmRef::Instance {
            owner_instance_id,
            offset,
            path,
        } => {
            let runtime_owner = if frame.instance_owner == Some(*owner_instance_id) {
                frame
                    .runtime_instance
                    .unwrap_or(InstanceId(*owner_instance_id))
            } else {
                InstanceId(*owner_instance_id)
            };
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
