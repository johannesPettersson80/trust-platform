//! The store-access seam over today's `Value` storage, for the engine states.
//!
//! `EngineState` forwards each `StoreAccess`/`StoreMutate` method to the same
//! `VariableStorage` function the engine calls today (C1-8a note §3), adding no
//! charge and no allocation of its own (specification 34 §6.2, MEM-10). The seam
//! never charges, so the engine keeps every `charge_*`, `before_*` and `check_*`
//! call exactly where it is.
//!
//! `RuntimeState` implements `StoreAccess`, the read and prepare half. It cannot
//! implement `StoreMutate` here: `RuntimeState`'s `data` field is private to
//! `engine::api`, and its public surface deliberately exposes no mutable storage
//! (`api.rs`'s `storage_mut` compile-fail doc test). Its mutable half needs a
//! crate-visible accessor in `api.rs`, which this slice's file list does not
//! include; C1-8c owns that engineering surface.

use super::{EngineData, EngineState, EngineStore, RuntimeState};
use crate::error::RuntimeError;
use crate::memory::cell_access::{CellReader, CellWriter};
use crate::memory::view::{PreparedWrite, StoreAccess, StoreMutate, ValueView};
use crate::memory::VariableStorage;
use crate::memory::{Generation, InstanceData, InstanceId, MemoryLocation};
use crate::value::{RefSegment, Value, ValueRef, ValueRefView};
use crate::vm::PreparedModule;
use crate::vm::VmTrap;
use smol_str::SmolStr;

impl StoreAccess for EngineState<'_> {
    #[inline]
    fn read_path<'a>(
        &'a self,
        location: MemoryLocation,
        offset: usize,
        path: &[RefSegment],
    ) -> Result<ValueView<'a>, VmTrap> {
        match self.data.routed(self.prepared) {
            Routed::Values(store) => store.read_path(location, offset, path),
            Routed::Cells(reader) => reader.read_path(location, offset, path),
        }
    }

    #[inline]
    fn read_slot<'a>(
        &'a self,
        location: MemoryLocation,
        offset: usize,
    ) -> Result<ValueView<'a>, VmTrap> {
        match self.data.routed(self.prepared) {
            Routed::Values(store) => store.read_slot(location, offset),
            Routed::Cells(reader) => reader.read_slot(location, offset),
        }
    }

    #[inline]
    fn read_reference<'a>(&'a self, reference: ValueRefView<'a>) -> Result<ValueView<'a>, VmTrap> {
        match self.data.routed(self.prepared) {
            Routed::Values(store) => store.read_reference(reference),
            Routed::Cells(reader) => reader.read_reference(reference),
        }
    }

    #[inline]
    fn read_instance_var<'a>(&'a self, instance: InstanceId, name: &str) -> Option<ValueView<'a>> {
        match self.data.routed(self.prepared) {
            Routed::Values(store) => store.read_instance_var(instance, name),
            Routed::Cells(reader) => reader.read_instance_var(instance, name),
        }
    }

    #[inline]
    fn read_global<'a>(&'a self, name: &str) -> Option<ValueView<'a>> {
        match self.data.routed(self.prepared) {
            Routed::Values(store) => store.read_global(name),
            Routed::Cells(reader) => reader.read_global(name),
        }
    }

    #[inline]
    fn global_name(&self, offset: usize) -> Option<&SmolStr> {
        match self.data.routed(self.prepared) {
            Routed::Values(store) => store.global_name(offset),
            Routed::Cells(reader) => reader.global_name(offset),
        }
    }

    #[inline]
    fn read_instance_field<'a>(
        &'a self,
        instance: InstanceId,
        name: &str,
    ) -> Result<ValueView<'a>, VmTrap> {
        match self.data.routed(self.prepared) {
            Routed::Values(store) => store.read_instance_field(instance, name),
            Routed::Cells(reader) => reader.read_instance_field(instance, name),
        }
    }

    #[inline]
    fn ref_for_global(&self, name: &str) -> Option<ValueRef> {
        match self.data.routed(self.prepared) {
            Routed::Values(store) => store.ref_for_global(name),
            Routed::Cells(reader) => reader.ref_for_global(name),
        }
    }

    #[inline]
    fn ref_for_instance(&self, instance: InstanceId, name: &str) -> Option<ValueRef> {
        match self.data.routed(self.prepared) {
            Routed::Values(store) => store.ref_for_instance(instance, name),
            Routed::Cells(reader) => reader.ref_for_instance(instance, name),
        }
    }

    #[inline]
    fn snapshot_instance(&self, instance: InstanceId) -> Option<InstanceData> {
        match self.data.routed(self.prepared) {
            Routed::Values(store) => store.snapshot_instance(instance),
            Routed::Cells(reader) => reader.snapshot_instance(instance),
        }
    }

    #[inline]
    fn member<'a>(
        &'a self,
        view: &ValueView<'a>,
        segment: &RefSegment,
    ) -> Result<ValueView<'a>, VmTrap> {
        self.data.storage.member(view, segment)
    }

    #[inline]
    fn text<'a>(&'a self, view: &ValueView<'a>) -> Option<&'a str> {
        self.data.storage.text(view)
    }

    #[inline]
    fn borrow<'a>(&'a self, view: &ValueView<'a>) -> Option<&'a Value> {
        self.data.storage.borrow(view)
    }

    #[inline]
    fn materialize(&self, view: &ValueView<'_>) -> Result<Value, RuntimeError> {
        self.data.storage.materialize(view)
    }

    #[inline]
    fn prepare_write(
        &self,
        location: MemoryLocation,
        offset: usize,
        path: &[RefSegment],
        value: &Value,
    ) -> Result<Option<PreparedWrite>, RuntimeError> {
        match self.data.routed(self.prepared) {
            Routed::Values(store) => store.prepare_write(location, offset, path, value),
            Routed::Cells(reader) => reader.prepare_write(location, offset, path, value),
        }
    }

    #[inline]
    fn generation(&self) -> Generation {
        match self.data.routed(self.prepared) {
            Routed::Values(store) => store.generation(),
            Routed::Cells(reader) => reader.generation(),
        }
    }

    #[inline]
    fn instance_type_name(&self, instance: InstanceId) -> Option<&SmolStr> {
        match self.data.routed(self.prepared) {
            Routed::Values(store) => store.instance_type_name(instance),
            Routed::Cells(reader) => reader.instance_type_name(instance),
        }
    }

    #[inline]
    fn instance_parent(&self, instance: InstanceId) -> Result<Option<InstanceId>, VmTrap> {
        match self.data.routed(self.prepared) {
            Routed::Values(store) => store.instance_parent(instance),
            Routed::Cells(reader) => reader.instance_parent(instance),
        }
    }

    #[inline]
    fn ref_for_instance_recursive(&self, instance: InstanceId, name: &str) -> Option<ValueRef> {
        match self.data.routed(self.prepared) {
            Routed::Values(store) => store.ref_for_instance_recursive(instance, name),
            Routed::Cells(reader) => reader.ref_for_instance_recursive(instance, name),
        }
    }

    #[inline]
    fn resolved_instance_field_ref(&self, instance: InstanceId, name: &str) -> Option<ValueRef> {
        match self.data.routed(self.prepared) {
            Routed::Values(store) => store.resolved_instance_field_ref(instance, name),
            Routed::Cells(reader) => reader.resolved_instance_field_ref(instance, name),
        }
    }

    #[inline]
    fn declared_instance_field_offset(&self, instance: InstanceId, name: &str) -> Option<usize> {
        match self.data.routed(self.prepared) {
            Routed::Values(store) => store.declared_instance_field_offset(instance, name),
            Routed::Cells(reader) => reader.declared_instance_field_offset(instance, name),
        }
    }
}

impl StoreMutate for EngineState<'_> {
    #[inline]
    fn write_slot(&mut self, location: MemoryLocation, offset: usize, value: Value) -> bool {
        match routed_mut(&mut *self.data, self.prepared) {
            RoutedMut::Values(store) => StoreMutate::write_slot(store, location, offset, value),
            RoutedMut::Cells(mut writer) => {
                StoreMutate::write_slot(&mut writer, location, offset, value)
            }
        }
    }

    #[inline]
    fn write_reference(
        &mut self,
        reference: ValueRefView<'_>,
        value: Value,
    ) -> Result<bool, RuntimeError> {
        match routed_mut(&mut *self.data, self.prepared) {
            RoutedMut::Values(store) => StoreMutate::write_reference(store, reference, value),
            RoutedMut::Cells(mut writer) => {
                StoreMutate::write_reference(&mut writer, reference, value)
            }
        }
    }

    #[inline]
    fn write_instance_var(
        &mut self,
        instance: InstanceId,
        name: impl Into<SmolStr>,
        value: Value,
    ) -> bool {
        match routed_mut(&mut *self.data, self.prepared) {
            RoutedMut::Values(store) => {
                StoreMutate::write_instance_var(store, instance, name, value)
            }
            RoutedMut::Cells(mut writer) => {
                StoreMutate::write_instance_var(&mut writer, instance, name, value)
            }
        }
    }

    #[inline]
    fn set_global(&mut self, name: impl Into<SmolStr>, value: Value) -> bool {
        match routed_mut(&mut *self.data, self.prepared) {
            RoutedMut::Values(store) => StoreMutate::set_global(store, name, value),
            RoutedMut::Cells(mut writer) => StoreMutate::set_global(&mut writer, name, value),
        }
    }

    #[inline]
    fn try_create_instance(
        &mut self,
        type_name: impl Into<SmolStr>,
    ) -> Result<InstanceId, RuntimeError> {
        match routed_mut(&mut *self.data, self.prepared) {
            RoutedMut::Values(store) => StoreMutate::try_create_instance(store, type_name),
            RoutedMut::Cells(mut writer) => {
                StoreMutate::try_create_instance(&mut writer, type_name)
            }
        }
    }

    #[inline]
    fn remove_instance(&mut self, id: InstanceId) -> bool {
        match routed_mut(&mut *self.data, self.prepared) {
            RoutedMut::Values(store) => StoreMutate::remove_instance(store, id),
            RoutedMut::Cells(mut writer) => StoreMutate::remove_instance(&mut writer, id),
        }
    }

    #[inline]
    fn retain_instances(&mut self, keep: impl FnMut(InstanceId) -> bool) -> usize {
        match routed_mut(&mut *self.data, self.prepared) {
            RoutedMut::Values(store) => StoreMutate::retain_instances(store, keep),
            RoutedMut::Cells(mut writer) => StoreMutate::retain_instances(&mut writer, keep),
        }
    }

    #[inline]
    fn restore_instance(&mut self, instance: InstanceId, data: InstanceData) {
        match routed_mut(&mut *self.data, self.prepared) {
            RoutedMut::Values(store) => StoreMutate::restore_instance(store, instance, data),
            RoutedMut::Cells(mut writer) => {
                StoreMutate::restore_instance(&mut writer, instance, data)
            }
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
        match routed_mut(&mut *self.data, self.prepared) {
            RoutedMut::Values(store) => {
                StoreMutate::commit_write(store, path, value, prepared, capacity)
            }
            RoutedMut::Cells(mut writer) => {
                StoreMutate::commit_write(&mut writer, path, value, prepared, capacity)
            }
        }
    }

    #[inline]
    fn restore_slot(&mut self, location: MemoryLocation, offset: usize, value: Value) {
        match routed_mut(&mut *self.data, self.prepared) {
            RoutedMut::Values(store) => StoreMutate::restore_slot(store, location, offset, value),
            RoutedMut::Cells(mut writer) => {
                StoreMutate::restore_slot(&mut writer, location, offset, value)
            }
        }
    }
}

impl StoreAccess for RuntimeState<'_> {
    #[inline]
    fn read_path<'a>(
        &'a self,
        location: MemoryLocation,
        offset: usize,
        path: &[RefSegment],
    ) -> Result<ValueView<'a>, VmTrap> {
        match self.routed() {
            Routed::Values(store) => store.read_path(location, offset, path),
            Routed::Cells(reader) => reader.read_path(location, offset, path),
        }
    }

    #[inline]
    fn read_slot<'a>(
        &'a self,
        location: MemoryLocation,
        offset: usize,
    ) -> Result<ValueView<'a>, VmTrap> {
        match self.routed() {
            Routed::Values(store) => store.read_slot(location, offset),
            Routed::Cells(reader) => reader.read_slot(location, offset),
        }
    }

    #[inline]
    fn read_reference<'a>(&'a self, reference: ValueRefView<'a>) -> Result<ValueView<'a>, VmTrap> {
        match self.routed() {
            Routed::Values(store) => store.read_reference(reference),
            Routed::Cells(reader) => reader.read_reference(reference),
        }
    }

    #[inline]
    fn read_instance_var<'a>(&'a self, instance: InstanceId, name: &str) -> Option<ValueView<'a>> {
        match self.routed() {
            Routed::Values(store) => store.read_instance_var(instance, name),
            Routed::Cells(reader) => reader.read_instance_var(instance, name),
        }
    }

    #[inline]
    fn read_global<'a>(&'a self, name: &str) -> Option<ValueView<'a>> {
        match self.routed() {
            Routed::Values(store) => store.read_global(name),
            Routed::Cells(reader) => reader.read_global(name),
        }
    }

    #[inline]
    fn global_name(&self, offset: usize) -> Option<&SmolStr> {
        match self.routed() {
            Routed::Values(store) => store.global_name(offset),
            Routed::Cells(reader) => reader.global_name(offset),
        }
    }

    #[inline]
    fn read_instance_field<'a>(
        &'a self,
        instance: InstanceId,
        name: &str,
    ) -> Result<ValueView<'a>, VmTrap> {
        match self.routed() {
            Routed::Values(store) => store.read_instance_field(instance, name),
            Routed::Cells(reader) => reader.read_instance_field(instance, name),
        }
    }

    #[inline]
    fn ref_for_global(&self, name: &str) -> Option<ValueRef> {
        match self.routed() {
            Routed::Values(store) => store.ref_for_global(name),
            Routed::Cells(reader) => reader.ref_for_global(name),
        }
    }

    #[inline]
    fn ref_for_instance(&self, instance: InstanceId, name: &str) -> Option<ValueRef> {
        match self.routed() {
            Routed::Values(store) => store.ref_for_instance(instance, name),
            Routed::Cells(reader) => reader.ref_for_instance(instance, name),
        }
    }

    #[inline]
    fn snapshot_instance(&self, instance: InstanceId) -> Option<InstanceData> {
        match self.routed() {
            Routed::Values(store) => store.snapshot_instance(instance),
            Routed::Cells(reader) => reader.snapshot_instance(instance),
        }
    }

    #[inline]
    fn member<'a>(
        &'a self,
        view: &ValueView<'a>,
        segment: &RefSegment,
    ) -> Result<ValueView<'a>, VmTrap> {
        self.storage().member(view, segment)
    }

    #[inline]
    fn text<'a>(&'a self, view: &ValueView<'a>) -> Option<&'a str> {
        self.storage().text(view)
    }

    #[inline]
    fn borrow<'a>(&'a self, view: &ValueView<'a>) -> Option<&'a Value> {
        self.storage().borrow(view)
    }

    #[inline]
    fn materialize(&self, view: &ValueView<'_>) -> Result<Value, RuntimeError> {
        self.storage().materialize(view)
    }

    #[inline]
    fn prepare_write(
        &self,
        location: MemoryLocation,
        offset: usize,
        path: &[RefSegment],
        value: &Value,
    ) -> Result<Option<PreparedWrite>, RuntimeError> {
        match self.routed() {
            Routed::Values(store) => store.prepare_write(location, offset, path, value),
            Routed::Cells(reader) => reader.prepare_write(location, offset, path, value),
        }
    }

    #[inline]
    fn generation(&self) -> Generation {
        match self.routed() {
            Routed::Values(store) => store.generation(),
            Routed::Cells(reader) => reader.generation(),
        }
    }

    #[inline]
    fn instance_type_name(&self, instance: InstanceId) -> Option<&SmolStr> {
        match self.routed() {
            Routed::Values(store) => store.instance_type_name(instance),
            Routed::Cells(reader) => reader.instance_type_name(instance),
        }
    }

    #[inline]
    fn instance_parent(&self, instance: InstanceId) -> Result<Option<InstanceId>, VmTrap> {
        match self.routed() {
            Routed::Values(store) => store.instance_parent(instance),
            Routed::Cells(reader) => reader.instance_parent(instance),
        }
    }

    #[inline]
    fn ref_for_instance_recursive(&self, instance: InstanceId, name: &str) -> Option<ValueRef> {
        match self.routed() {
            Routed::Values(store) => store.ref_for_instance_recursive(instance, name),
            Routed::Cells(reader) => reader.ref_for_instance_recursive(instance, name),
        }
    }

    #[inline]
    fn resolved_instance_field_ref(&self, instance: InstanceId, name: &str) -> Option<ValueRef> {
        match self.routed() {
            Routed::Values(store) => store.resolved_instance_field_ref(instance, name),
            Routed::Cells(reader) => reader.resolved_instance_field_ref(instance, name),
        }
    }

    #[inline]
    fn declared_instance_field_offset(&self, instance: InstanceId, name: &str) -> Option<usize> {
        match self.routed() {
            Routed::Values(store) => store.declared_instance_field_offset(instance, name),
            Routed::Cells(reader) => reader.declared_instance_field_offset(instance, name),
        }
    }
}

/// The store one engine state reads through, borrowed for one call (C1-9b).
enum Routed<'a> {
    /// Today's `Value` storage.
    Values(&'a VariableStorage),
    /// The cell arm, read through the prepared module that places its cells.
    Cells(CellReader<'a>),
}

/// The store one engine state writes through, borrowed for one call (C1-9c).
enum RoutedMut<'a> {
    /// Today's `Value` storage.
    Values(&'a mut VariableStorage),
    /// The cell arm's writer.
    Cells(CellWriter<'a>),
}

/// The store an engine state writes through (C1-9c): the same `EngineStore` choice that reads use.
fn routed_mut<'a>(data: &'a mut EngineData, prepared: &'a PreparedModule) -> RoutedMut<'a> {
    let EngineData { storage, store, .. } = data;
    match store {
        EngineStore::Values => RoutedMut::Values(storage),
        EngineStore::Cells(arm) => RoutedMut::Cells(CellWriter::new(prepared, arm)),
    }
}

impl RuntimeState<'_> {
    /// The store this runtime state reads through (C1-9b).
    fn routed(&self) -> Routed<'_> {
        let (data, prepared) = self.engine_parts();
        data.routed(prepared)
    }
}

impl EngineData {
    /// The store the engine reads through: the arm `EngineStore` holds.
    fn routed<'a>(&'a self, prepared: &'a PreparedModule) -> Routed<'a> {
        match &self.store {
            EngineStore::Values => Routed::Values(&self.storage),
            EngineStore::Cells(arm) => Routed::Cells(CellReader::new(prepared, arm)),
        }
    }
}
