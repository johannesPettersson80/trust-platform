//! The store-access seam over the flat store, for the engine states (specification 34 §6.2, MEM-10).
//!
//! `EngineState` forwards each `StoreAccess`/`StoreMutate` method to the flat store's reader or
//! writer, which read and write the globals and the persistent instances in the cells and the locals
//! and the per-call instances in the `Value` half the store keeps for them (C1-9b, C1-9c, C1-19e). The
//! seam adds no charge and no allocation of its own, so the engine keeps every `charge_*`, `before_*`
//! and `check_*` call exactly where it is.
//!
//! `RuntimeState` implements both halves through the same reader and writer. Its `data` field stays
//! private to `engine::api`, which reaches it through `engine_parts_mut` (the public surface still
//! exposes no mutable storage: `api.rs`'s `storage_mut` compile-fail doc test).

use super::{EngineData, EngineState, RuntimeState};
use crate::error::RuntimeError;
use crate::memory::cell_access::{CellArm, CellReader, CellWriter};
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
        self.data
            .reader(self.prepared)
            .read_path(location, offset, path)
    }

    #[inline]
    fn read_slot<'a>(
        &'a self,
        location: MemoryLocation,
        offset: usize,
    ) -> Result<ValueView<'a>, VmTrap> {
        self.data.reader(self.prepared).read_slot(location, offset)
    }

    #[inline]
    fn read_reference<'a>(&'a self, reference: ValueRefView<'a>) -> Result<ValueView<'a>, VmTrap> {
        self.data.reader(self.prepared).read_reference(reference)
    }

    #[inline]
    fn read_instance_var<'a>(&'a self, instance: InstanceId, name: &str) -> Option<ValueView<'a>> {
        self.data
            .reader(self.prepared)
            .read_instance_var(instance, name)
    }

    #[inline]
    fn read_global<'a>(&'a self, name: &str) -> Option<ValueView<'a>> {
        self.data.reader(self.prepared).read_global(name)
    }

    #[inline]
    fn global_name(&self, offset: usize) -> Option<&str> {
        self.data.reader(self.prepared).global_name(offset)
    }

    #[inline]
    fn read_instance_field<'a>(
        &'a self,
        instance: InstanceId,
        name: &str,
    ) -> Result<ValueView<'a>, VmTrap> {
        self.data
            .reader(self.prepared)
            .read_instance_field(instance, name)
    }

    #[inline]
    fn ref_for_global(&self, name: &str) -> Option<ValueRef> {
        self.data.reader(self.prepared).ref_for_global(name)
    }

    #[inline]
    fn ref_for_instance(&self, instance: InstanceId, name: &str) -> Option<ValueRef> {
        self.data
            .reader(self.prepared)
            .ref_for_instance(instance, name)
    }

    #[inline]
    fn snapshot_instance(&self, instance: InstanceId) -> Option<InstanceData> {
        self.data.reader(self.prepared).snapshot_instance(instance)
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
        self.data
            .reader(self.prepared)
            .prepare_write(location, offset, path, value)
    }

    #[inline]
    fn generation(&self) -> Generation {
        self.data.reader(self.prepared).generation()
    }

    #[inline]
    fn instance_type_name(&self, instance: InstanceId) -> Option<&SmolStr> {
        self.data.reader(self.prepared).instance_type_name(instance)
    }

    #[inline]
    fn instance_parent(&self, instance: InstanceId) -> Result<Option<InstanceId>, VmTrap> {
        self.data.reader(self.prepared).instance_parent(instance)
    }

    #[inline]
    fn ref_for_instance_recursive(&self, instance: InstanceId, name: &str) -> Option<ValueRef> {
        self.data
            .reader(self.prepared)
            .ref_for_instance_recursive(instance, name)
    }

    #[inline]
    fn resolved_instance_field_ref(&self, instance: InstanceId, name: &str) -> Option<ValueRef> {
        self.data
            .reader(self.prepared)
            .resolved_instance_field_ref(instance, name)
    }

    #[inline]
    fn declared_instance_field_offset(&self, instance: InstanceId, name: &str) -> Option<usize> {
        self.data
            .reader(self.prepared)
            .declared_instance_field_offset(instance, name)
    }

    // Locals and retained variables are `Value`-half state (C1-9d2a, R3).
    #[inline]
    fn read_local<'a>(&'a self, name: &str) -> Option<ValueView<'a>> {
        self.data.storage.read_local(name)
    }

    #[inline]
    fn ref_for_local(&self, name: &str) -> Option<ValueRef> {
        StoreAccess::ref_for_local(&self.data.storage, name)
    }

    #[inline]
    fn read_retain<'a>(&'a self, name: &str) -> Option<ValueView<'a>> {
        self.data.storage.read_retain(name)
    }
}

impl StoreMutate for EngineState<'_> {
    #[inline]
    fn write_slot(&mut self, location: MemoryLocation, offset: usize, value: Value) -> bool {
        let written = StoreMutate::write_slot(
            &mut writer(&mut *self.data, self.prepared),
            location,
            offset,
            value,
        );
        self.recycle_displaced_reference();
        written
    }

    #[inline]
    fn write_reference(
        &mut self,
        reference: ValueRefView<'_>,
        value: Value,
    ) -> Result<bool, RuntimeError> {
        let written = StoreMutate::write_reference(
            &mut writer(&mut *self.data, self.prepared),
            reference,
            value,
        );
        self.recycle_displaced_reference();
        written
    }

    #[inline]
    fn write_instance_var(
        &mut self,
        instance: InstanceId,
        name: impl Into<SmolStr>,
        value: Value,
    ) -> bool {
        StoreMutate::write_instance_var(
            &mut writer(&mut *self.data, self.prepared),
            instance,
            name,
            value,
        )
    }

    #[inline]
    fn set_global(&mut self, name: impl Into<SmolStr>, value: Value) -> bool {
        StoreMutate::set_global(&mut writer(&mut *self.data, self.prepared), name, value)
    }

    #[inline]
    fn try_create_instance(
        &mut self,
        type_name: impl Into<SmolStr>,
    ) -> Result<InstanceId, RuntimeError> {
        StoreMutate::try_create_instance(&mut writer(&mut *self.data, self.prepared), type_name)
    }

    #[inline]
    fn remove_instance(&mut self, id: InstanceId) -> bool {
        StoreMutate::remove_instance(&mut writer(&mut *self.data, self.prepared), id)
    }

    #[inline]
    fn retain_instances(&mut self, keep: impl FnMut(InstanceId) -> bool) -> usize {
        StoreMutate::retain_instances(&mut writer(&mut *self.data, self.prepared), keep)
    }

    #[inline]
    fn restore_instance(&mut self, instance: InstanceId, data: InstanceData) {
        StoreMutate::restore_instance(&mut writer(&mut *self.data, self.prepared), instance, data);
    }

    #[inline]
    fn commit_write(
        &mut self,
        path: &[RefSegment],
        value: Value,
        prepared: PreparedWrite,
        capacity: usize,
    ) -> bool {
        let written = StoreMutate::commit_write(
            &mut writer(&mut *self.data, self.prepared),
            path,
            value,
            prepared,
            capacity,
        );
        self.recycle_displaced_reference();
        written
    }

    #[inline]
    fn restore_slot(&mut self, location: MemoryLocation, offset: usize, value: Value) {
        StoreMutate::restore_slot(
            &mut writer(&mut *self.data, self.prepared),
            location,
            offset,
            value,
        );
    }
}

impl StoreMutate for RuntimeState<'_> {
    #[inline]
    fn write_slot(&mut self, location: MemoryLocation, offset: usize, value: Value) -> bool {
        StoreMutate::write_slot(&mut self.writer(), location, offset, value)
    }

    #[inline]
    fn write_reference(
        &mut self,
        reference: ValueRefView<'_>,
        value: Value,
    ) -> Result<bool, RuntimeError> {
        StoreMutate::write_reference(&mut self.writer(), reference, value)
    }

    #[inline]
    fn write_instance_var(
        &mut self,
        instance: InstanceId,
        name: impl Into<SmolStr>,
        value: Value,
    ) -> bool {
        StoreMutate::write_instance_var(&mut self.writer(), instance, name, value)
    }

    #[inline]
    fn set_global(&mut self, name: impl Into<SmolStr>, value: Value) -> bool {
        StoreMutate::set_global(&mut self.writer(), name, value)
    }

    #[inline]
    fn try_create_instance(
        &mut self,
        type_name: impl Into<SmolStr>,
    ) -> Result<InstanceId, RuntimeError> {
        StoreMutate::try_create_instance(&mut self.writer(), type_name)
    }

    #[inline]
    fn remove_instance(&mut self, id: InstanceId) -> bool {
        StoreMutate::remove_instance(&mut self.writer(), id)
    }

    #[inline]
    fn retain_instances(&mut self, keep: impl FnMut(InstanceId) -> bool) -> usize {
        StoreMutate::retain_instances(&mut self.writer(), keep)
    }

    #[inline]
    fn restore_instance(&mut self, instance: InstanceId, data: InstanceData) {
        StoreMutate::restore_instance(&mut self.writer(), instance, data);
    }

    #[inline]
    fn commit_write(
        &mut self,
        path: &[RefSegment],
        value: Value,
        prepared: PreparedWrite,
        capacity: usize,
    ) -> bool {
        StoreMutate::commit_write(&mut self.writer(), path, value, prepared, capacity)
    }

    #[inline]
    fn restore_slot(&mut self, location: MemoryLocation, offset: usize, value: Value) {
        StoreMutate::restore_slot(&mut self.writer(), location, offset, value);
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
        self.reader().read_path(location, offset, path)
    }

    #[inline]
    fn read_slot<'a>(
        &'a self,
        location: MemoryLocation,
        offset: usize,
    ) -> Result<ValueView<'a>, VmTrap> {
        self.reader().read_slot(location, offset)
    }

    #[inline]
    fn read_reference<'a>(&'a self, reference: ValueRefView<'a>) -> Result<ValueView<'a>, VmTrap> {
        self.reader().read_reference(reference)
    }

    #[inline]
    fn read_instance_var<'a>(&'a self, instance: InstanceId, name: &str) -> Option<ValueView<'a>> {
        self.reader().read_instance_var(instance, name)
    }

    #[inline]
    fn read_global<'a>(&'a self, name: &str) -> Option<ValueView<'a>> {
        self.reader().read_global(name)
    }

    #[inline]
    fn global_name(&self, offset: usize) -> Option<&str> {
        self.reader().global_name(offset)
    }

    #[inline]
    fn read_instance_field<'a>(
        &'a self,
        instance: InstanceId,
        name: &str,
    ) -> Result<ValueView<'a>, VmTrap> {
        self.reader().read_instance_field(instance, name)
    }

    #[inline]
    fn ref_for_global(&self, name: &str) -> Option<ValueRef> {
        self.reader().ref_for_global(name)
    }

    #[inline]
    fn ref_for_instance(&self, instance: InstanceId, name: &str) -> Option<ValueRef> {
        self.reader().ref_for_instance(instance, name)
    }

    #[inline]
    fn snapshot_instance(&self, instance: InstanceId) -> Option<InstanceData> {
        self.reader().snapshot_instance(instance)
    }

    // A view carries what it reads through: these read the view, not a copy of the storage.
    #[inline]
    fn member<'a>(
        &'a self,
        view: &ValueView<'a>,
        segment: &RefSegment,
    ) -> Result<ValueView<'a>, VmTrap> {
        self.engine_parts().0.storage.member(view, segment)
    }

    #[inline]
    fn text<'a>(&'a self, view: &ValueView<'a>) -> Option<&'a str> {
        self.engine_parts().0.storage.text(view)
    }

    #[inline]
    fn borrow<'a>(&'a self, view: &ValueView<'a>) -> Option<&'a Value> {
        self.engine_parts().0.storage.borrow(view)
    }

    #[inline]
    fn materialize(&self, view: &ValueView<'_>) -> Result<Value, RuntimeError> {
        self.engine_parts().0.storage.materialize(view)
    }

    #[inline]
    fn prepare_write(
        &self,
        location: MemoryLocation,
        offset: usize,
        path: &[RefSegment],
        value: &Value,
    ) -> Result<Option<PreparedWrite>, RuntimeError> {
        self.reader().prepare_write(location, offset, path, value)
    }

    #[inline]
    fn generation(&self) -> Generation {
        self.reader().generation()
    }

    #[inline]
    fn instance_type_name(&self, instance: InstanceId) -> Option<&SmolStr> {
        self.reader().instance_type_name(instance)
    }

    #[inline]
    fn instance_parent(&self, instance: InstanceId) -> Result<Option<InstanceId>, VmTrap> {
        self.reader().instance_parent(instance)
    }

    #[inline]
    fn ref_for_instance_recursive(&self, instance: InstanceId, name: &str) -> Option<ValueRef> {
        self.reader().ref_for_instance_recursive(instance, name)
    }

    #[inline]
    fn resolved_instance_field_ref(&self, instance: InstanceId, name: &str) -> Option<ValueRef> {
        self.reader().resolved_instance_field_ref(instance, name)
    }

    #[inline]
    fn declared_instance_field_offset(&self, instance: InstanceId, name: &str) -> Option<usize> {
        self.reader().declared_instance_field_offset(instance, name)
    }

    // Locals and retained variables are `Value`-half state (C1-9d2a, R3).
    #[inline]
    fn read_local<'a>(&'a self, name: &str) -> Option<ValueView<'a>> {
        let (data, _) = self.engine_parts();
        data.storage.read_local(name)
    }

    #[inline]
    fn ref_for_local(&self, name: &str) -> Option<ValueRef> {
        let (data, _) = self.engine_parts();
        StoreAccess::ref_for_local(&data.storage, name)
    }

    #[inline]
    fn read_retain<'a>(&'a self, name: &str) -> Option<ValueView<'a>> {
        let (data, _) = self.engine_parts();
        data.storage.read_retain(name)
    }
}

/// The writer of the flat store an engine state holds, borrowed for one call (C1-9c).
fn writer<'a>(data: &'a mut EngineData, prepared: &'a PreparedModule) -> CellWriter<'a> {
    let EngineData { storage, store, .. } = data;
    store_writer(storage, store, prepared)
}

/// The writer of the flat store, borrowed from its two fields alone, so that a caller can hold the
/// lifecycle tables meanwhile (construction, C1-9c).
pub(super) fn store_writer<'a>(
    storage: &'a mut VariableStorage,
    store: &'a mut CellArm,
    prepared: &'a PreparedModule,
) -> CellWriter<'a> {
    CellWriter::new(prepared, store, storage)
}

impl EngineState<'_> {
    /// Give the path buffers of the reference that a write into a reference leaf replaced back to the
    /// engine's reference buffers (spec 34 ARCH-05), so a reference variable that takes a new
    /// reference in every scan allocates nothing once the engine is warm.
    fn recycle_displaced_reference(&mut self) {
        if let Some(reference) = self.data.store.take_displaced_reference() {
            self.data.resources.references.recycle(reference.path);
        }
    }

    /// Create the persistent instance of template `pou` that the construction root at index `root`
    /// holds, recorded in the root's range with an identity from the store's one counter (C1-9d2b-1,
    /// R2).
    pub(super) fn create_root_instance(
        &mut self,
        root: usize,
        pou: u32,
    ) -> Result<InstanceId, RuntimeError> {
        writer(&mut *self.data, self.prepared).create_persistent_instance(root, pou)
    }

    /// Create an instance of template `pou` that a call owns, on the activation stack above the frame
    /// that runs (specification 34 MEM-12; C1-11c).
    pub(super) fn create_call_instance(&mut self, pou: u32) -> Result<InstanceId, RuntimeError> {
        writer(&mut *self.data, self.prepared).create_call_instance(pou)
    }

    /// Give the instance `child` the parent `parent`, or none; `false` when `child` does not exist.
    pub(super) fn set_instance_parent(
        &mut self,
        child: InstanceId,
        parent: Option<InstanceId>,
    ) -> bool {
        writer(&mut *self.data, self.prepared).set_instance_parent(child, parent)
    }

    /// Begin a top-level execution entry (specification 34 §4.3.1, HOST-02; C1-9d2b-2, R4). The
    /// activation identities restart at zero in the `Value` half's registry, and the instance identities
    /// from the entry floor of the store's one counter, which issues them; the `Value` half keeps no
    /// floor of its own.
    pub(super) fn begin_execution_entry(&mut self) -> Result<(), RuntimeError> {
        self.data.storage.begin_activation_entry()?;
        self.data.store.begin_execution_entry();
        Ok(())
    }

    /// The number of live instances, persistent and per-call (specification 34 MEM-09).
    pub(super) fn live_instance_count(&self) -> usize {
        let per_call = self.data.storage.instances().len();
        self.data.store.persistent_count().saturating_add(per_call)
    }

    /// The number of members `instance` holds, or `None` when no such instance exists.
    pub(super) fn instance_member_count(&self, instance: InstanceId) -> Option<usize> {
        self.data
            .reader(self.prepared)
            .instance_member_count(instance)
    }

    /// The declared name of member `ordinal` of `instance`, in declaration order.
    pub(super) fn instance_member_name(
        &self,
        instance: InstanceId,
        ordinal: usize,
    ) -> Option<SmolStr> {
        self.data
            .reader(self.prepared)
            .instance_member_name(instance, ordinal)
            .map(SmolStr::new)
    }

    /// The view of member `ordinal` of `instance`, in declaration order.
    pub(super) fn instance_member_view(
        &self,
        instance: InstanceId,
        ordinal: usize,
    ) -> Option<ValueView<'_>> {
        self.data
            .reader(self.prepared)
            .instance_member_view(instance, ordinal)
    }
}

impl RuntimeState<'_> {
    /// The writer of the flat store this runtime state holds (C1-9c).
    fn writer(&mut self) -> CellWriter<'_> {
        let (data, prepared) = self.engine_parts_mut();
        writer(data, prepared)
    }

    /// The reader of the flat store this runtime state holds (C1-9b).
    fn reader(&self) -> CellReader<'_> {
        let (data, prepared) = self.engine_parts();
        data.reader(prepared)
    }
}

impl EngineData {
    /// The reader of the flat store, with the `Value` half it keeps for the locals and the per-call
    /// instances.
    pub(super) fn reader<'a>(&'a self, prepared: &'a PreparedModule) -> CellReader<'a> {
        CellReader::new(prepared, &self.store, &self.storage)
    }

    /// Move the next instance identity of the store this record holds, so that a test reaches the
    /// identity limit without issuing 2^32 identities: the flat store issues the identities of its
    /// instances, the `Value` half those of the instances it keeps.
    #[cfg(any(test, feature = "test-support"))]
    pub(super) fn set_next_instance_id_for_test(&mut self, next: u32) {
        self.storage.set_next_instance_id_for_test(next);
        self.store.cells_mut().set_next_instance_id_for_test(next);
    }

    /// Move the installation generation of the store this record holds, so that a test reaches the
    /// last generation without 2^32 installations.
    #[cfg(test)]
    pub(super) fn set_generation_for_test(&mut self, generation: crate::memory::Generation) {
        self.storage.set_generation(generation);
        self.store.cells_mut().set_generation_for_test(generation);
    }
}
