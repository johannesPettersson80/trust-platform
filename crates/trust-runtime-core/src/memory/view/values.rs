//! The `Value` store's side of the store-access seam: [`VariableStorage`](crate::memory::VariableStorage)
//! as a [`StoreAccess`](crate::memory::view::StoreAccess) and a
//! [`StoreMutate`](crate::memory::view::StoreMutate).
//!
//! Each method forwards to the same storage function the engine called before the seam existed (C1-8a
//! note §3), so no result, trap, error, charge or allocation changes (specification 34 §6.2, MEM-10).
//! The cell arm implements the same traits in `memory::cell_access` and `memory::cell_write`.

use super::{PreparedWrite, PreparedWriteKind, StoreAccess, StoreMutate, ValueView};
use crate::error::RuntimeError;
use crate::memory::{Generation, InstanceData, InstanceId, MemoryLocation, VariableStorage};
use crate::value::{RefSegment, Value, ValueRef, ValueRefView};
use crate::vm::VmTrap;
use smol_str::SmolStr;

impl StoreAccess for VariableStorage {
    #[inline]
    fn read_path<'a>(
        &'a self,
        location: MemoryLocation,
        offset: usize,
        path: &[RefSegment],
    ) -> Result<ValueView<'a>, VmTrap> {
        let value = if path.is_empty() {
            self.read_direct_slot_by_location(location, offset)
        } else {
            self.read_by_ref_parts(location, offset, path)
        };
        value.map(ValueView::borrowed).ok_or(VmTrap::NullReference)
    }

    #[inline]
    fn read_slot<'a>(
        &'a self,
        location: MemoryLocation,
        offset: usize,
    ) -> Result<ValueView<'a>, VmTrap> {
        self.read_direct_slot_by_location(location, offset)
            .map(ValueView::borrowed)
            .ok_or(VmTrap::NullReference)
    }

    #[inline]
    fn read_reference<'a>(&'a self, reference: ValueRefView<'a>) -> Result<ValueView<'a>, VmTrap> {
        if reference.generation != self.generation() {
            return Err(VmTrap::NullReference);
        }
        self.read_by_ref_parts(reference.location, reference.offset, reference.path)
            .map(ValueView::borrowed)
            .ok_or(VmTrap::NullReference)
    }

    #[inline]
    fn read_instance_var<'a>(&'a self, instance: InstanceId, name: &str) -> Option<ValueView<'a>> {
        self.get_instance_var(instance, name)
            .map(ValueView::borrowed)
    }

    #[inline]
    fn read_global<'a>(&'a self, name: &str) -> Option<ValueView<'a>> {
        self.get_global(name).map(ValueView::borrowed)
    }

    #[inline]
    fn global_name(&self, offset: usize) -> Option<&str> {
        self.values
            .globals
            .get_index(offset)
            .map(|(name, _)| name.as_str())
    }

    #[inline]
    fn ref_for_global(&self, name: &str) -> Option<ValueRef> {
        self.ref_for_global(name)
    }

    #[inline]
    fn ref_for_instance(&self, instance: InstanceId, name: &str) -> Option<ValueRef> {
        self.ref_for_instance(instance, name)
    }

    #[inline]
    fn snapshot_instance(&self, instance: InstanceId) -> Option<InstanceData> {
        self.get_instance(instance).cloned()
    }

    #[inline]
    fn read_instance_field<'a>(
        &'a self,
        instance: InstanceId,
        name: &str,
    ) -> Result<ValueView<'a>, VmTrap> {
        let reference = self
            .ref_for_instance_recursive(instance, name)
            .ok_or(VmTrap::NullReference)?;
        if reference.generation != self.generation() {
            return Err(VmTrap::NullReference);
        }
        self.read_by_ref_parts(
            reference.location,
            reference.offset as usize,
            &reference.path,
        )
        .map(ValueView::borrowed)
        .ok_or(VmTrap::NullReference)
    }

    #[inline]
    fn member<'a>(
        &'a self,
        view: &ValueView<'a>,
        segment: &RefSegment,
    ) -> Result<ValueView<'a>, VmTrap> {
        view.member(segment).ok_or(VmTrap::NullReference)
    }

    #[inline]
    fn text<'a>(&'a self, view: &ValueView<'a>) -> Option<&'a str> {
        view.text()
    }

    #[inline]
    fn borrow<'a>(&'a self, view: &ValueView<'a>) -> Option<&'a Value> {
        view.borrowed_value()
    }

    #[inline]
    fn materialize(&self, view: &ValueView<'_>) -> Result<Value, RuntimeError> {
        view.materialize()
    }

    #[inline]
    fn prepare_write(
        &self,
        location: MemoryLocation,
        offset: usize,
        path: &[RefSegment],
        value: &Value,
    ) -> Result<Option<PreparedWrite>, RuntimeError> {
        self.prepare_by_ref_parts_write(location, offset, path, value)
            .map(|staged| {
                staged.map(|staged| {
                    PreparedWrite(PreparedWriteKind::Values {
                        consumed: 0,
                        staged,
                    })
                })
            })
    }

    #[inline]
    fn generation(&self) -> Generation {
        self.generation()
    }

    #[inline]
    fn instance_type_name(&self, instance: InstanceId) -> Option<&SmolStr> {
        self.get_instance(instance).map(|data| &data.type_name)
    }

    #[inline]
    fn instance_parent(&self, instance: InstanceId) -> Result<Option<InstanceId>, VmTrap> {
        self.get_instance(instance)
            .map(|data| data.parent)
            .ok_or(VmTrap::NullReference)
    }

    #[inline]
    fn ref_for_instance_recursive(&self, instance: InstanceId, name: &str) -> Option<ValueRef> {
        self.ref_for_instance_recursive(instance, name)
    }

    #[inline]
    fn resolved_instance_field_ref(&self, instance: InstanceId, name: &str) -> Option<ValueRef> {
        self.resolved_instance_field_ref(instance, name)
    }

    #[inline]
    fn declared_instance_field_offset(&self, instance: InstanceId, name: &str) -> Option<usize> {
        self.declared_instance_field_offset(instance, name)
    }

    #[inline]
    fn read_local<'a>(&'a self, name: &str) -> Option<ValueView<'a>> {
        self.get_local(name).map(ValueView::borrowed)
    }

    #[inline]
    fn ref_for_local(&self, name: &str) -> Option<ValueRef> {
        self.ref_for_local(name)
    }

    #[inline]
    fn read_retain<'a>(&'a self, name: &str) -> Option<ValueView<'a>> {
        self.get_retain(name).map(ValueView::borrowed)
    }
}

impl StoreMutate for VariableStorage {
    #[inline]
    fn write_slot(&mut self, location: MemoryLocation, offset: usize, value: Value) -> bool {
        self.write_direct_slot_by_location(location, offset, value)
    }

    #[inline]
    fn write_reference(
        &mut self,
        reference: ValueRefView<'_>,
        value: Value,
    ) -> Result<bool, RuntimeError> {
        if reference.generation != self.generation() {
            return Err(RuntimeError::ReferenceLifetime);
        }
        self.write_by_ref_parts_checked(reference.location, reference.offset, reference.path, value)
    }

    #[inline]
    fn write_instance_var(
        &mut self,
        instance: InstanceId,
        name: impl Into<SmolStr>,
        value: Value,
    ) -> bool {
        self.set_instance_var(instance, name, value)
    }

    #[inline]
    fn set_global(&mut self, name: impl Into<SmolStr>, value: Value) -> bool {
        // The inherent method owns the insertion; this forwards to it. The `Value` store keeps
        // every name it is given, so every call stores.
        self.set_global(name, value);
        true
    }

    #[inline]
    fn try_create_instance(
        &mut self,
        type_name: impl Into<SmolStr>,
    ) -> Result<InstanceId, RuntimeError> {
        // The inherent method owns identity issue; this forwards to it.
        self.try_create_instance(type_name)
    }

    #[inline]
    fn remove_instance(&mut self, id: InstanceId) -> bool {
        // The inherent method owns release; this forwards to it and drops the record.
        self.remove_instance(id).is_some()
    }

    #[inline]
    fn retain_instances(&mut self, keep: impl FnMut(InstanceId) -> bool) -> usize {
        // The inherent method owns the batch release; this forwards to it.
        self.retain_instances(keep)
    }

    #[inline]
    fn restore_instance(&mut self, instance: InstanceId, data: InstanceData) {
        // The initializer rollback writes the copy back over the live record.
        if let Some(record) = self.get_instance_mut(instance) {
            *record = data;
        }
    }

    #[inline]
    fn commit_write(
        &mut self,
        path: &[RefSegment],
        value: Value,
        prepared: PreparedWrite,
        capacity: usize,
    ) -> bool {
        match prepared.0 {
            PreparedWriteKind::Values { consumed, staged } => path
                .get(consumed..)
                .is_some_and(|path| self.commit_prepared_write(path, value, staged, capacity)),
            // A cell patch is published by the cell arm, which holds the cells; it is never
            // handed to the `Value` store, so this store refuses it.
            PreparedWriteKind::Cells(_) => false,
        }
    }

    #[inline]
    fn restore_slot(&mut self, location: MemoryLocation, offset: usize, value: Value) {
        self.restore_direct_slot(location, offset, value);
    }
}
