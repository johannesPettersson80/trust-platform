//! Resource image synchronization using the shared host/portable codecs.
use super::*;
use crate::bytecode::{InitializationTarget, InitializerEntry};
use crate::io_address::IoAddress;
use crate::io_image::{
    read_flat_image, validate_process_image_address, write_flat_image, IoAddressKey, IoValueCodec,
};
use crate::memory::{IoArea, MemoryLocation};
use crate::value::{
    size_of_value_with_abi_and_visitor, write_partial_access, PartialAccess, RefSegment,
    SizeOfError,
};
use crate::vm::{
    construction::values::ValueConstructionContext, module::VmRef, sizeof_type_from_table_with_abi,
};
use alloc::format;
use core::mem::size_of;

mod admission;
mod staging;

impl EngineState<'_> {
    pub(in crate::vm::engine) fn set_hierarchical_input(
        &mut self,
        address: &IoAddress,
        value: Value,
    ) -> Result<(), RuntimeError> {
        if address.area != IoArea::Input || address.path.len() < 2 {
            return Err(RuntimeError::TypeMismatch);
        }
        let binding = self
            .prepared
            .io_bindings
            .iter()
            .find(|binding| binding.address == *address)
            .ok_or_else(|| RuntimeError::InvalidIoAddress("unbound hierarchical input".into()))?;
        let value = if let Some(ty) = binding.type_id {
            self.normalize_assignment_value(ty, value)?
        } else {
            value
        };
        let value = binding.codec.encode(value, address.size)?;
        self.charge_hierarchical_entry(address)?;
        self.data
            .images
            .hierarchical
            .try_reserve(1)
            .map_err(|_| RuntimeError::Overflow)?;
        let key = IoAddressKey::from(address);
        self.check_entry_deadline()?;
        self.data.images.hierarchical.insert(key, value);
        Ok(())
    }

    pub(in crate::vm::engine) fn read_named_bool(
        &self,
        name_idx: u32,
    ) -> Result<bool, RuntimeError> {
        let name = self
            .prepared
            .vm
            .strings
            .get(name_idx as usize)
            .ok_or_else(|| {
                invalid_bytecode(smol_str::SmolStr::new_static("invalid task SINGLE name"))
            })?;
        let (_, declaration) = self
            .prepared
            .global(name)
            .ok_or_else(|| RuntimeError::InvalidExecutionState)?;
        match self
            .data
            .storage
            .read_global_slot_by_offset(declaration.slot as usize)
        {
            Some(Value::Bool(value)) => Ok(*value),
            _ => Err(RuntimeError::TypeMismatch),
        }
    }

    pub(in crate::vm::engine) fn execute_task_fb(
        &mut self,
        ref_idx: u32,
    ) -> Result<(), RuntimeError> {
        let reference = self.persistent_io_reference(ref_idx)?;
        self.check_read(reference.as_view())?;
        let instance = match self.data.storage.read_by_ref_ref(&reference) {
            Some(Value::Instance(id)) => *id,
            Some(_) => return Err(RuntimeError::TypeMismatch),
            None => return Err(RuntimeError::NullReference),
        };
        let pou = *self
            .data
            .construction
            .instance_templates
            .get(&instance)
            .ok_or_else(|| RuntimeError::NullReference)?;
        self.execute_pou(pou, Some(instance))
    }

    pub(in crate::vm::engine) fn persistent_io_reference(
        &mut self,
        ref_idx: u32,
    ) -> Result<ValueRef, RuntimeError> {
        let prepared = self.prepared;
        let reference = prepared.vm.refs.get(ref_idx as usize).ok_or_else(|| {
            invalid_bytecode(smol_str::SmolStr::new_static(
                "invalid persistent reference",
            ))
        })?;
        let (location, offset, path) = match reference {
            VmRef::Global { offset, path } => (MemoryLocation::Global, *offset, path),
            VmRef::Retain { offset, path } => (MemoryLocation::Retain, *offset, path),
            VmRef::Instance {
                owner_instance_id,
                offset,
                path,
            } => {
                let (index, _) = self
                    .prepared
                    .root_for_instance_owner(*owner_instance_id)
                    .ok_or_else(|| RuntimeError::NullReference)?;
                let instance = self
                    .data
                    .construction
                    .roots
                    .get(index)
                    .copied()
                    .flatten()
                    .ok_or_else(|| RuntimeError::NullReference)?;
                (MemoryLocation::Instance(instance), *offset, path)
            }
            _ => return Err(RuntimeError::TypeMismatch),
        };
        let mut bytes = path
            .len()
            .checked_mul(size_of::<RefSegment>())
            .ok_or_else(|| RuntimeError::Overflow)?;
        for segment in path {
            let extra = match segment {
                RefSegment::Field(name) => name.len(),
                RefSegment::Index(indices) => indices
                    .len()
                    .checked_mul(size_of::<i64>())
                    .ok_or_else(|| RuntimeError::Overflow)?,
            };
            bytes = bytes
                .checked_add(extra)
                .ok_or_else(|| RuntimeError::Overflow)?;
        }
        self.charge_allocation(bytes)?;
        let path = path.clone();
        Ok(ValueRef {
            location,
            offset,
            path,
        })
    }

    fn read_image_address(&mut self, address: &IoAddress) -> Result<Value, RuntimeError> {
        validate_process_image_address(address)?;
        if address.path.len() > 1 {
            self.charge_allocation(
                address
                    .path
                    .len()
                    .checked_mul(size_of::<u32>())
                    .ok_or_else(|| RuntimeError::Overflow)?,
            )?;
            let key = IoAddressKey::from(address);
            let value = self.data.images.hierarchical.get(&key).ok_or_else(|| {
                RuntimeError::InvalidIoAddress(format!("hier {:?}", address.path).into())
            })?;
            let bytes = self.value_clone_charge(value, 0)?;
            self.charge_allocation(bytes)?;
            return self
                .data
                .images
                .hierarchical
                .get(&key)
                .cloned()
                .ok_or_else(|| RuntimeError::NullReference);
        }
        if let crate::io_address::IoSize::Bytes(len) = address.size {
            self.charge_allocation(len as usize)?;
        }
        let area = match address.area {
            IoArea::Input => &self.data.images.inputs,
            IoArea::Output => &self.data.images.outputs,
            IoArea::Memory => &self.data.images.memory,
        };
        self.check_image_window(address, area.len())?;
        read_flat_image(area, address)
    }
    fn charge_hierarchical_entry(&mut self, address: &IoAddress) -> Result<(), RuntimeError> {
        let bytes = address
            .path
            .len()
            .checked_mul(size_of::<u32>())
            .and_then(|n| {
                n.checked_add(
                    2 * (size_of::<IoAddressKey>() + size_of::<Value>() + 4 * size_of::<usize>()),
                )
            })
            .ok_or_else(|| RuntimeError::Overflow)?;
        self.charge_allocation(bytes)
    }
    fn check_image_window(&self, address: &IoAddress, len: usize) -> Result<(), RuntimeError> {
        if address
            .flat_byte_end()?
            .is_some_and(|end| end as usize > len)
        {
            return Err(RuntimeError::InvalidIoAddress(
                "address exceeds admitted process image".into(),
            ));
        }
        Ok(())
    }

    pub(in crate::vm::engine) fn commit_configuration_target(
        &mut self,
        entry: &InitializerEntry,
        value: Value,
    ) -> Result<(), RuntimeError> {
        match entry.target_kind {
            InitializationTarget::Reference => {
                let reference = self.persistent_io_reference(
                    entry.target_idx.ok_or_else(|| RuntimeError::TypeMismatch)?,
                )?;
                let value = if let Some(access) =
                    PartialAccess::from_wire(entry.partial_kind, entry.partial_index)
                        .map_err(crate::vm::dispatch::partial_access_error_to_runtime)?
                {
                    self.check_read(reference.as_view())?;
                    let target = self
                        .data
                        .storage
                        .materialize_by_ref_ref(&reference)
                        .ok_or_else(|| RuntimeError::NullReference)?;
                    write_partial_access(target, access, value)
                        .map_err(crate::vm::dispatch::partial_access_error_to_runtime)?
                } else {
                    value
                };
                self.check_write(reference.as_view(), &value)?;
                let value = self.normalize_reference_value(reference.as_view(), value)?;
                self.charge_storage_path_write(
                    &self.data.storage,
                    reference.location,
                    reference.offset,
                    &reference.path,
                    &value,
                )?;
                let prepared = self
                    .data
                    .storage
                    .prepare_by_ref_parts_write(
                        reference.location,
                        reference.offset,
                        &reference.path,
                        &value,
                    )?
                    .ok_or_else(|| RuntimeError::NullReference)?;
                self.check_entry_deadline()?;
                if !self
                    .data
                    .storage
                    .commit_prepared_write(&reference.path, value, prepared)
                {
                    return Err(RuntimeError::NullReference);
                }
                Ok(())
            }
            InitializationTarget::DirectIo => {
                if entry.partial_kind != 0 || entry.partial_index != 0 {
                    return Err(RuntimeError::TypeMismatch);
                }
                self.check_value_lifetime(&value, None)?;
                let prepared = self.prepared;
                let text = prepared
                    .vm
                    .strings
                    .get(entry.target_idx.ok_or_else(|| RuntimeError::TypeMismatch)? as usize)
                    .ok_or_else(|| RuntimeError::TypeMismatch)?;
                self.charge_allocation(
                    text.len()
                        .checked_mul(32)
                        .ok_or_else(|| RuntimeError::Overflow)?,
                )?;
                let raw_address = IoAddress::parse(text)?;
                let ty = self
                    .prepared
                    .vm
                    .ref_type(entry.result_ref_idx)
                    .ok_or_else(|| RuntimeError::TypeMismatch)?;
                let codec = IoValueCodec::from_type(
                    Some(ty),
                    &self.prepared.vm.types,
                    &self.prepared.vm.strings,
                )?;
                let address = codec.address(raw_address)?;
                let value = codec.encode(value, address.size)?;
                if address.path.len() > 1 {
                    self.charge_hierarchical_entry(&address)?;
                    self.data
                        .images
                        .hierarchical
                        .insert(IoAddressKey::from(&address), value);
                    return Ok(());
                }
                let len = match address.area {
                    IoArea::Input => self.data.images.inputs.len(),
                    IoArea::Output => self.data.images.outputs.len(),
                    IoArea::Memory => self.data.images.memory.len(),
                };
                self.check_image_window(&address, len)?;
                let area = match address.area {
                    IoArea::Input => &mut self.data.images.inputs,
                    IoArea::Output => &mut self.data.images.outputs,
                    IoArea::Memory => &mut self.data.images.memory,
                };
                write_flat_image(area, &address, value)
            }
            _ => Err(RuntimeError::TypeMismatch),
        }
    }

    pub(in crate::vm::engine) fn sizeof_runtime_value(
        &self,
        value: &Value,
    ) -> Result<u64, RuntimeError> {
        size_of_value_with_abi_and_visitor(
            value,
            self.prepared.storage_abi(),
            &mut |name| {
                self.sizeof_named_value(name, false)?
                    .ok_or_else(|| RuntimeError::from(SizeOfError::UnsupportedType))
            },
            &mut |name| self.sizeof_named_value(name, true),
            &mut &mut |units| self.charge_work_units(units),
        )
    }

    fn sizeof_named_value(
        &self,
        name: &str,
        relative_structure_only: bool,
    ) -> Result<Option<u64>, RuntimeError> {
        let Some(type_idx) = self.prepared.named_type(name) else {
            return Ok(None);
        };
        if relative_structure_only
            && !matches!(
                self.prepared.vm.types.entries[type_idx as usize].kind,
                crate::bytecode::TypeKind::OverlappingStruct
                    | crate::bytecode::TypeKind::LocatedStruct
            )
        {
            return Ok(None);
        }
        sizeof_type_from_table_with_abi(
            &self.prepared.vm.types,
            type_idx,
            self.prepared.storage_abi(),
            &mut &mut |units| self.charge_work_units(units),
        )
        .map(Some)
    }
}

/// Borrow an admitted hierarchical image value without creating a mutable
/// dispatcher context or a second process image. Includes physical inputs.
pub(super) fn read_hierarchical_image_ref<'a>(
    data: &'a EngineData,
    prepared: &PreparedModule,
    address: &IoAddress,
) -> Option<&'a Value> {
    if address.path.len() < 2
        || validate_process_image_address(address).is_err()
        || !prepared
            .io_bindings
            .iter()
            .any(|binding| binding.address == *address)
    {
        return None;
    }
    data.images.hierarchical.get(&IoAddressKey::from(address))
}

/// Borrow a published hierarchical value without constructing a mutable context.
pub(super) fn read_hierarchical_output<'a>(
    data: &'a EngineData,
    prepared: &PreparedModule,
    address: &IoAddress,
) -> Option<&'a Value> {
    if !matches!(address.area, IoArea::Output | IoArea::Memory)
        || address.path.len() < 2
        || !prepared
            .io_bindings
            .iter()
            .any(|binding| binding.address == *address)
    {
        return None;
    }
    data.images.hierarchical.get(&IoAddressKey::from(address))
}
