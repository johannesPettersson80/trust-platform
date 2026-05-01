use smol_str::SmolStr;

use crate::error::RuntimeError;
use crate::memory::{FrameId, InstanceId, MemoryLocation};
use crate::value::{
    materialize_value_path, read_value_path_borrowed, single_ref_index, string_element_position,
    write_value_path, RefSegment, Value, ValueRef,
};
#[cfg(test)]
use crate::value::{ref_indices_from_iter, RefPath};

use super::super::core::Runtime;
use super::call::VM_LOCAL_SENTINEL_FRAME_ID;
use super::errors::VmTrap;
use super::frames::{FrameStack, VmFrame};
use super::{materialize_borrowed_value, VmModule, VmRef};

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
                read_vm_local_ref(frame, *offset, path)
            }
        }
        _ => peek_ref(runtime, module, frames, ref_idx)
            .map(|value| materialize_borrowed_value(value).0),
    }
}

pub(super) fn peek_ref<'a>(
    runtime: &'a Runtime,
    module: &'a VmModule,
    frames: &'a FrameStack,
    ref_idx: u32,
) -> Result<&'a Value, VmTrap> {
    let reference = module
        .refs
        .get(ref_idx as usize)
        .ok_or(VmTrap::InvalidRefIndex(ref_idx))?;

    match reference {
        VmRef::Global { offset, path } if path.is_empty() => runtime
            .storage
            .read_global_slot_by_offset(*offset)
            .ok_or(VmTrap::NullReference),
        VmRef::Global { offset, path } => runtime
            .storage
            .read_by_ref_parts(MemoryLocation::Global, *offset, path)
            .ok_or(VmTrap::NullReference),
        VmRef::Local { offset, path, .. } => {
            let frame = frames.current().ok_or(VmTrap::CallStackUnderflow)?;
            if path.is_empty() {
                let slot = frame.local_slot_index(ref_idx)?;
                frame.locals.get(slot).ok_or(VmTrap::InvalidLocalRef {
                    ref_index: ref_idx,
                    start: frame.local_ref_start,
                    count: frame.local_ref_count,
                })
            } else {
                let slot = *offset;
                peek_vm_local_ref(frame, slot, path)
            }
        }
        _ => {
            let frame = frames.current().ok_or(VmTrap::CallStackUnderflow)?;
            let (location, offset, path) = runtime_access_target(reference, frame)?;
            runtime
                .storage
                .read_by_ref_parts(location, offset, path)
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
                path: path.into(),
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
        VmRef::Global { offset, path } if path.is_empty() => {
            if runtime.storage.write_global_slot_by_offset(*offset, value) {
                Ok(())
            } else {
                Err(VmTrap::NullReference)
            }
        }
        VmRef::Global { offset, path } => {
            if runtime
                .storage
                .write_by_ref_parts(MemoryLocation::Global, *offset, path, value)
            {
                Ok(())
            } else {
                Err(VmTrap::NullReference)
            }
        }
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
    let target = peek_dynamic_ref(runtime, frames, &reference)?;
    match target {
        Value::Struct(struct_value) => {
            if !struct_value.contains_field(field.as_str()) {
                return Err(VmTrap::Runtime(RuntimeError::UndefinedField(field)));
            }
            reference.path.push(RefSegment::Field(field));
            Ok(reference)
        }
        Value::Instance(instance_id) => runtime
            .storage
            .resolved_instance_field_ref(*instance_id, field.as_str())
            .ok_or(VmTrap::Runtime(RuntimeError::UndefinedField(field))),
        _ => Err(VmTrap::Runtime(RuntimeError::TypeMismatch)),
    }
}

pub(super) fn dynamic_ref_field_borrowed(
    runtime: &Runtime,
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
            let mut next = reference.clone();
            next.path.push(RefSegment::Field(field));
            Ok(next)
        }
        Value::Instance(instance_id) => runtime
            .storage
            .resolved_instance_field_ref(*instance_id, field.as_str())
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
    if let Some(RefSegment::Index(existing)) = reference.path.last() {
        let base_path = &reference.path[..reference.path.len().saturating_sub(1)];
        if let Value::Array(array) = peek_dynamic_ref_path(runtime, frames, &reference, base_path)?
        {
            if existing.len() < array.dimensions().len() {
                let (lower, upper) = array.dimensions()[existing.len()];
                if index < lower || index > upper {
                    return Err(VmTrap::Runtime(RuntimeError::IndexOutOfBounds {
                        index,
                        lower,
                        upper,
                    }));
                }
                let mut combined = existing.clone();
                combined.push(index);
                if let Some(RefSegment::Index(indices)) = reference.path.last_mut() {
                    *indices = combined;
                    return Ok(reference);
                }
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
            reference.path.push(single_ref_index(index));
            Ok(reference)
        }
        Value::String(text) => {
            string_element_position(text.as_str(), index).map_err(VmTrap::Runtime)?;
            reference.path.push(single_ref_index(index));
            Ok(reference)
        }
        Value::WString(text) => {
            string_element_position(text.as_str(), index).map_err(VmTrap::Runtime)?;
            reference.path.push(single_ref_index(index));
            Ok(reference)
        }
        _ => Err(VmTrap::Runtime(RuntimeError::TypeMismatch)),
    }
}

pub(super) fn peek_dynamic_ref<'a>(
    runtime: &'a Runtime,
    frames: &'a FrameStack,
    reference: &ValueRef,
) -> Result<&'a Value, VmTrap> {
    peek_dynamic_ref_path(runtime, frames, reference, &reference.path)
}

fn peek_dynamic_ref_path<'a>(
    runtime: &'a Runtime,
    frames: &'a FrameStack,
    reference: &ValueRef,
    path: &[RefSegment],
) -> Result<&'a Value, VmTrap> {
    if matches!(
        reference.location,
        MemoryLocation::Local(FrameId(VM_LOCAL_SENTINEL_FRAME_ID))
    ) {
        let frame = frames.current().ok_or(VmTrap::CallStackUnderflow)?;
        let root = frame
            .locals
            .get(reference.offset)
            .ok_or(VmTrap::NullReference)?;
        return read_value_path_borrowed(root, path).ok_or(VmTrap::NullReference);
    }
    runtime
        .storage
        .read_by_ref_parts(reference.location, reference.offset, path)
        .ok_or(VmTrap::NullReference)
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
        let root = frame
            .locals
            .get(reference.offset)
            .ok_or(VmTrap::NullReference)?;
        return materialize_value_path(root, &reference.path).ok_or(VmTrap::NullReference);
    }
    runtime
        .storage
        .materialize_by_ref_ref(reference)
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
    if runtime.storage_mut().write_by_ref_ref(reference, value) {
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
    let root = frame.locals.get(offset).ok_or(VmTrap::NullReference)?;
    read_value_path_borrowed(root, path).ok_or(VmTrap::NullReference)
}

fn read_vm_local_ref(frame: &VmFrame, offset: usize, path: &[RefSegment]) -> Result<Value, VmTrap> {
    let root = frame.locals.get(offset).ok_or(VmTrap::NullReference)?;
    materialize_value_path(root, path).ok_or(VmTrap::NullReference)
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

#[cfg(test)]
mod tests {
    use super::*;
    use indexmap::IndexMap;

    use crate::memory::FrameId;
    use crate::runtime::vm::frames::VmFrame;
    use crate::value::{ArrayValue, StructValue};
    use crate::Runtime;

    #[test]
    fn peek_dynamic_ref_borrows_global_storage_value() {
        let mut runtime = Runtime::new();
        runtime.storage_mut().set_global(
            "CELL",
            Value::Struct(std::sync::Arc::new(StructValue::from_untyped_parts(
                SmolStr::new("CELL_T"),
                IndexMap::from([(SmolStr::new("ACC"), Value::DInt(7))]),
            ))),
        );
        let reference = runtime
            .storage()
            .ref_for_global("CELL")
            .expect("global ref");
        let frames = FrameStack::default();

        let peeked = peek_dynamic_ref(&runtime, &frames, &reference).expect("peek global");
        let direct = runtime
            .storage()
            .read_by_ref_ref(&reference)
            .expect("direct global read");

        assert!(std::ptr::eq(peeked, direct));
    }

    #[test]
    fn peek_dynamic_ref_borrows_local_sentinel_value() {
        let mut frames = FrameStack::default();
        frames
            .push(VmFrame {
                pou_id: 0,
                return_pc: 0,
                code_start: 0,
                code_end: 0,
                local_ref_start: 0,
                local_ref_count: 1,
                locals: vec![Value::Struct(std::sync::Arc::new(
                    StructValue::from_untyped_parts(
                        SmolStr::new("LOCAL_T"),
                        IndexMap::from([(SmolStr::new("ACC"), Value::DInt(11))]),
                    ),
                ))],
                runtime_instance: None,
                instance_owner: None,
            })
            .expect("push frame");
        let runtime = Runtime::new();
        let reference = ValueRef {
            location: MemoryLocation::Local(FrameId(VM_LOCAL_SENTINEL_FRAME_ID)),
            offset: 0,
            path: [RefSegment::Field(SmolStr::new("ACC"))]
                .into_iter()
                .collect(),
        };

        let peeked = peek_dynamic_ref(&runtime, &frames, &reference).expect("peek local");
        let frame = frames.current().expect("current frame");
        let direct =
            read_value_path_borrowed(frame.locals.first().expect("local slot"), &reference.path)
                .expect("direct local read");

        assert!(std::ptr::eq(peeked, direct));
    }

    #[test]
    fn dynamic_ref_field_resolves_instance_field_reference() {
        let mut runtime = Runtime::new();
        let instance = runtime.storage_mut().create_instance("FB");
        assert!(runtime
            .storage_mut()
            .set_instance_var(instance, "ACC", Value::DInt(19)));
        runtime
            .storage_mut()
            .set_global("HOLDER", Value::Instance(instance));
        let holder = runtime
            .storage()
            .ref_for_global("HOLDER")
            .expect("holder ref");
        let frames = FrameStack::default();

        let resolved = dynamic_ref_field(&runtime, &frames, holder, SmolStr::new("ACC"))
            .expect("resolve instance field");
        let expected = runtime
            .storage()
            .ref_for_instance_recursive(instance, "ACC")
            .expect("expected ref");

        assert_eq!(resolved, expected);
    }

    #[test]
    fn dynamic_ref_index_extends_partial_index_against_array_shape() {
        let mut runtime = Runtime::new();
        runtime.storage_mut().set_global(
            "GRID",
            Value::Array(Box::new(
                ArrayValue::from_untyped_parts(
                    vec![
                        Value::DInt(1),
                        Value::DInt(2),
                        Value::DInt(3),
                        Value::DInt(4),
                    ],
                    vec![(0, 1), (0, 1)],
                )
                .unwrap(),
            )),
        );
        let mut reference = runtime.storage().ref_for_global("GRID").expect("grid ref");
        reference.path.push(single_ref_index(0));
        let frames = FrameStack::default();

        let resolved =
            dynamic_ref_index(&runtime, &frames, reference, 1).expect("extend partial index");

        assert_eq!(
            resolved.path,
            [RefSegment::Index(ref_indices_from_iter([0, 1]))]
                .into_iter()
                .collect::<RefPath>()
        );
    }

    #[test]
    fn dynamic_ref_index_extends_nested_partial_index_against_array_shape() {
        let mut runtime = Runtime::new();
        runtime.storage_mut().set_global(
            "HOLDER",
            Value::Struct(std::sync::Arc::new(StructValue::from_untyped_parts(
                SmolStr::new("GRID_HOLDER"),
                IndexMap::from([(
                    SmolStr::new("GRID"),
                    Value::Array(Box::new(
                        ArrayValue::from_untyped_parts(
                            vec![
                                Value::DInt(1),
                                Value::DInt(2),
                                Value::DInt(3),
                                Value::DInt(4),
                            ],
                            vec![(0, 1), (0, 1)],
                        )
                        .unwrap(),
                    )),
                )]),
            ))),
        );
        let mut reference = runtime
            .storage()
            .ref_for_global("HOLDER")
            .expect("holder ref");
        reference.path.push(RefSegment::Field(SmolStr::new("GRID")));
        reference.path.push(single_ref_index(0));
        let frames = FrameStack::default();

        let resolved = dynamic_ref_index(&runtime, &frames, reference, 1)
            .expect("extend nested partial index");

        assert_eq!(
            resolved.path,
            [
                RefSegment::Field(SmolStr::new("GRID")),
                RefSegment::Index(ref_indices_from_iter([0, 1])),
            ]
            .into_iter()
            .collect::<RefPath>()
        );
    }

    #[test]
    fn read_and_write_value_path_handle_extreme_array_bounds_without_overflow() {
        let mut value = Value::Array(Box::new(ArrayValue::from_canonical_parts(
            vec![Value::DInt(7)],
            vec![(i64::MIN, i64::MAX)],
        )));
        let path = [RefSegment::Index(ref_indices_from_iter([i64::MIN]))];

        let read = read_value_path_borrowed(&value, &path).expect("read extreme lower bound");
        assert_eq!(read, &Value::DInt(7));

        assert!(write_value_path(&mut value, &path, Value::DInt(9)));
        let updated =
            read_value_path_borrowed(&value, &path).expect("read updated extreme lower bound");
        assert_eq!(updated, &Value::DInt(9));
    }

    #[test]
    fn read_and_write_value_path_non_ascii_string_uses_character_elements() {
        let mut value = Value::String("ÄBC".into());
        let path = [RefSegment::Index(ref_indices_from_iter([1]))];

        let read = materialize_value_path(&value, &path).expect("read non-ascii string element");
        assert_eq!(read, Value::Char(0xC4));

        assert!(write_value_path(
            &mut value,
            &[RefSegment::Index(ref_indices_from_iter([2]))],
            Value::Char(b'X')
        ));
        assert_eq!(value, Value::String("ÄXC".into()));
    }
}
