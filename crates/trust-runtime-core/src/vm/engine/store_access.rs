//! The store-access seam over today's `Value` storage, for the engine states.
//!
//! `EngineState` forwards each `StoreAccess`/`StoreMutate` method to the same
//! `VariableStorage` function the engine calls today (C1-8a note §3), adding no
//! charge and no allocation of its own (specification 34 §6.2, MEM-10). The seam
//! never charges, so the engine keeps every `charge_*`, `before_*` and `check_*`
//! call exactly where it is.
//!
//! `RuntimeState` implements both halves, routed through the same `EngineStore` choice as
//! `EngineState` (C1-9c). Its `data` field stays private to `engine::api`, which reaches it through
//! `engine_parts_mut` (the public surface still exposes no mutable storage: `api.rs`'s
//! `storage_mut` compile-fail doc test). All writes go through `RoutedMut`, the one place that
//! chooses between the `Value` store and the cell arm for a write.

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
    fn global_name(&self, offset: usize) -> Option<&str> {
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

    // Locals and retained variables are `Value`-half state, whichever arm the engine reads (C1-9d2a, R3).
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
            &mut routed_mut(&mut *self.data, self.prepared),
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
            &mut routed_mut(&mut *self.data, self.prepared),
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
            &mut routed_mut(&mut *self.data, self.prepared),
            instance,
            name,
            value,
        )
    }

    #[inline]
    fn set_global(&mut self, name: impl Into<SmolStr>, value: Value) -> bool {
        StoreMutate::set_global(&mut routed_mut(&mut *self.data, self.prepared), name, value)
    }

    #[inline]
    fn try_create_instance(
        &mut self,
        type_name: impl Into<SmolStr>,
    ) -> Result<InstanceId, RuntimeError> {
        StoreMutate::try_create_instance(&mut routed_mut(&mut *self.data, self.prepared), type_name)
    }

    #[inline]
    fn remove_instance(&mut self, id: InstanceId) -> bool {
        StoreMutate::remove_instance(&mut routed_mut(&mut *self.data, self.prepared), id)
    }

    #[inline]
    fn retain_instances(&mut self, keep: impl FnMut(InstanceId) -> bool) -> usize {
        StoreMutate::retain_instances(&mut routed_mut(&mut *self.data, self.prepared), keep)
    }

    #[inline]
    fn restore_instance(&mut self, instance: InstanceId, data: InstanceData) {
        StoreMutate::restore_instance(
            &mut routed_mut(&mut *self.data, self.prepared),
            instance,
            data,
        );
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
            &mut routed_mut(&mut *self.data, self.prepared),
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
            &mut routed_mut(&mut *self.data, self.prepared),
            location,
            offset,
            value,
        );
    }
}

impl StoreMutate for RuntimeState<'_> {
    #[inline]
    fn write_slot(&mut self, location: MemoryLocation, offset: usize, value: Value) -> bool {
        StoreMutate::write_slot(&mut self.routed_for_write(), location, offset, value)
    }

    #[inline]
    fn write_reference(
        &mut self,
        reference: ValueRefView<'_>,
        value: Value,
    ) -> Result<bool, RuntimeError> {
        StoreMutate::write_reference(&mut self.routed_for_write(), reference, value)
    }

    #[inline]
    fn write_instance_var(
        &mut self,
        instance: InstanceId,
        name: impl Into<SmolStr>,
        value: Value,
    ) -> bool {
        StoreMutate::write_instance_var(&mut self.routed_for_write(), instance, name, value)
    }

    #[inline]
    fn set_global(&mut self, name: impl Into<SmolStr>, value: Value) -> bool {
        StoreMutate::set_global(&mut self.routed_for_write(), name, value)
    }

    #[inline]
    fn try_create_instance(
        &mut self,
        type_name: impl Into<SmolStr>,
    ) -> Result<InstanceId, RuntimeError> {
        StoreMutate::try_create_instance(&mut self.routed_for_write(), type_name)
    }

    #[inline]
    fn remove_instance(&mut self, id: InstanceId) -> bool {
        StoreMutate::remove_instance(&mut self.routed_for_write(), id)
    }

    #[inline]
    fn retain_instances(&mut self, keep: impl FnMut(InstanceId) -> bool) -> usize {
        StoreMutate::retain_instances(&mut self.routed_for_write(), keep)
    }

    #[inline]
    fn restore_instance(&mut self, instance: InstanceId, data: InstanceData) {
        StoreMutate::restore_instance(&mut self.routed_for_write(), instance, data);
    }

    #[inline]
    fn commit_write(
        &mut self,
        path: &[RefSegment],
        value: Value,
        prepared: PreparedWrite,
        capacity: usize,
    ) -> bool {
        StoreMutate::commit_write(
            &mut self.routed_for_write(),
            path,
            value,
            prepared,
            capacity,
        )
    }

    #[inline]
    fn restore_slot(&mut self, location: MemoryLocation, offset: usize, value: Value) {
        StoreMutate::restore_slot(&mut self.routed_for_write(), location, offset, value);
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
    fn global_name(&self, offset: usize) -> Option<&str> {
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

    // Locals and retained variables are `Value`-half state, whichever arm the engine reads (C1-9d2a, R3).
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

/// The store one engine state reads through, borrowed for one call (C1-9b).
pub(super) enum Routed<'a> {
    /// Today's `Value` storage.
    Values(&'a VariableStorage),
    /// The cell arm, read through the prepared module that places its cells.
    Cells(CellReader<'a>),
}

/// The store one engine state writes through, borrowed for one call (C1-9c): the `Value` storage or the
/// cell arm's writer. Its [`StoreMutate`] implementation is the single routing of writes.
pub(super) enum RoutedMut<'a> {
    /// Today's `Value` storage.
    Values(&'a mut VariableStorage),
    /// The cell arm's writer.
    Cells(CellWriter<'a>),
}

/// The store an engine state writes through (C1-9c): the same `EngineStore` choice that reads use.
fn routed_mut<'a>(data: &'a mut EngineData, prepared: &'a PreparedModule) -> RoutedMut<'a> {
    let EngineData { storage, store, .. } = data;
    routed_mut_fields(storage, store, prepared)
}

/// The store the engine writes through, borrowed from its two fields alone, so that a caller can hold
/// the lifecycle tables meanwhile (construction, C1-9c).
pub(super) fn routed_mut_fields<'a>(
    storage: &'a mut VariableStorage,
    store: &'a mut EngineStore,
    prepared: &'a PreparedModule,
) -> RoutedMut<'a> {
    match store {
        EngineStore::Values => RoutedMut::Values(storage),
        EngineStore::Cells(arm) => RoutedMut::Cells(CellWriter::new(prepared, arm, storage)),
    }
}

impl RoutedMut<'_> {
    /// Create the persistent instance of template `pou` that the construction root at index `root`
    /// holds, and answer its identity (C1-9d2b-1, R2). The `Value` store issues the identity as it always
    /// did; the cell arm records the instance in its root's range, from the store's one counter.
    pub(super) fn create_root_instance(
        &mut self,
        root: usize,
        pou: u32,
        type_name: &str,
    ) -> Result<InstanceId, RuntimeError> {
        match self {
            RoutedMut::Values(store) => StoreMutate::try_create_instance(&mut **store, type_name),
            RoutedMut::Cells(writer) => writer.create_persistent_instance(root, pou),
        }
    }

    /// Create an instance of template `pou` that a call owns: on the activation stack of the cell
    /// store, above the frame that runs (specification 34 MEM-12; C1-11c), and as a record of the
    /// `Value` store.
    pub(super) fn create_call_instance(
        &mut self,
        pou: u32,
        type_name: &str,
    ) -> Result<InstanceId, RuntimeError> {
        match self {
            RoutedMut::Values(store) => StoreMutate::try_create_instance(&mut **store, type_name),
            RoutedMut::Cells(writer) => writer.create_call_instance(pou),
        }
    }

    /// Give the instance `child` the parent `parent`, or none; `false` when `child` does not exist.
    pub(super) fn set_instance_parent(
        &mut self,
        child: InstanceId,
        parent: Option<InstanceId>,
    ) -> bool {
        match self {
            RoutedMut::Values(store) => store
                .get_instance_mut(child)
                .map(|record| record.parent = parent)
                .is_some(),
            RoutedMut::Cells(writer) => writer.set_instance_parent(child, parent),
        }
    }
}

impl EngineState<'_> {
    /// Give the path buffers of the reference that a write into a reference leaf replaced back to the
    /// engine's reference buffers (spec 34 ARCH-05), so a reference variable that takes a new
    /// reference in every scan allocates nothing once the engine is warm.
    fn recycle_displaced_reference(&mut self) {
        if let EngineStore::Cells(arm) = &mut self.data.store
            && let Some(reference) = arm.take_displaced_reference()
        {
            self.data.resources.references.recycle(reference.path);
        }
    }

    /// Whether the engine holds its globals and persistent instances in cells.
    pub(super) fn holds_cells(&self) -> bool {
        matches!(self.data.store, EngineStore::Cells(_))
    }

    /// Create the persistent instance of the construction root at index `root` (C1-9d2b-1, R2).
    pub(super) fn create_root_instance(
        &mut self,
        root: usize,
        pou: u32,
        type_name: &str,
    ) -> Result<InstanceId, RuntimeError> {
        routed_mut(&mut *self.data, self.prepared).create_root_instance(root, pou, type_name)
    }

    /// Create an instance of template `pou` that a call owns (see [`RoutedMut::create_call_instance`]).
    pub(super) fn create_call_instance(
        &mut self,
        pou: u32,
        type_name: &str,
    ) -> Result<InstanceId, RuntimeError> {
        routed_mut(&mut *self.data, self.prepared).create_call_instance(pou, type_name)
    }

    /// Whether an instance that a call creates now lies on the activation stack (specification 34
    /// MEM-12; C1-11c, C1-19c4): on the cell store while a frame is on the stack. The instances an
    /// entry or construction creates outside any call stay in the `Value` half.
    pub(super) fn call_instances_on_stack(&self) -> bool {
        matches!(&self.data.store, EngineStore::Cells(arm) if arm.holds_frames())
    }

    /// Give the instance `child` the parent `parent`, or none; `false` when `child` does not exist.
    pub(super) fn set_instance_parent(
        &mut self,
        child: InstanceId,
        parent: Option<InstanceId>,
    ) -> bool {
        routed_mut(&mut *self.data, self.prepared).set_instance_parent(child, parent)
    }

    /// Begin a top-level execution entry on the store the engine holds (specification 34 §4.3.1, HOST-02;
    /// C1-9d2b-2, R4). The activation identities restart at zero in the `Value` half's registry on either
    /// arm. The instance identities restart from the entry floor of the one counter that issues them: the
    /// `Value` half's on the `Value` store, and the cell store's on the cell arm, where the `Value` half
    /// keeps no floor of its own.
    pub(super) fn begin_execution_entry(&mut self) -> Result<(), RuntimeError> {
        match &mut self.data.store {
            EngineStore::Values => self.data.storage.begin_execution_entry(),
            EngineStore::Cells(arm) => {
                self.data.storage.begin_activation_entry()?;
                arm.begin_execution_entry();
                Ok(())
            }
        }
    }

    /// The number of live instances, persistent and per-call (specification 34 MEM-09).
    pub(super) fn live_instance_count(&self) -> usize {
        let per_call = self.data.storage.instances().len();
        match &self.data.store {
            EngineStore::Values => per_call,
            EngineStore::Cells(arm) => arm.persistent_count().saturating_add(per_call),
        }
    }

    /// The number of members `instance` holds, or `None` when no such instance exists.
    pub(super) fn instance_member_count(&self, instance: InstanceId) -> Option<usize> {
        match self.data.routed(self.prepared) {
            Routed::Values(store) => store
                .get_instance(instance)
                .map(|data| data.variables.len()),
            Routed::Cells(reader) => reader.instance_member_count(instance),
        }
    }

    /// The declared name of member `ordinal` of `instance`, in declaration order.
    pub(super) fn instance_member_name(
        &self,
        instance: InstanceId,
        ordinal: usize,
    ) -> Option<SmolStr> {
        match self.data.routed(self.prepared) {
            Routed::Values(store) => store
                .get_instance(instance)?
                .variables
                .get_index(ordinal)
                .map(|(name, _)| name.clone()),
            Routed::Cells(reader) => reader
                .instance_member_name(instance, ordinal)
                .map(SmolStr::new),
        }
    }

    /// The view of member `ordinal` of `instance`, in declaration order.
    pub(super) fn instance_member_view(
        &self,
        instance: InstanceId,
        ordinal: usize,
    ) -> Option<ValueView<'_>> {
        match self.data.routed(self.prepared) {
            Routed::Values(store) => store
                .read_slot(MemoryLocation::Instance(instance), ordinal)
                .ok(),
            Routed::Cells(reader) => reader.instance_member_view(instance, ordinal),
        }
    }
}

impl StoreMutate for RoutedMut<'_> {
    #[inline]
    fn write_slot(&mut self, location: MemoryLocation, offset: usize, value: Value) -> bool {
        match self {
            RoutedMut::Values(store) => {
                StoreMutate::write_slot(&mut **store, location, offset, value)
            }
            RoutedMut::Cells(writer) => StoreMutate::write_slot(writer, location, offset, value),
        }
    }

    #[inline]
    fn write_reference(
        &mut self,
        reference: ValueRefView<'_>,
        value: Value,
    ) -> Result<bool, RuntimeError> {
        match self {
            RoutedMut::Values(store) => {
                StoreMutate::write_reference(&mut **store, reference, value)
            }
            RoutedMut::Cells(writer) => StoreMutate::write_reference(writer, reference, value),
        }
    }

    #[inline]
    fn write_instance_var(
        &mut self,
        instance: InstanceId,
        name: impl Into<SmolStr>,
        value: Value,
    ) -> bool {
        match self {
            RoutedMut::Values(store) => {
                StoreMutate::write_instance_var(&mut **store, instance, name, value)
            }
            RoutedMut::Cells(writer) => {
                StoreMutate::write_instance_var(writer, instance, name, value)
            }
        }
    }

    #[inline]
    fn set_global(&mut self, name: impl Into<SmolStr>, value: Value) -> bool {
        match self {
            RoutedMut::Values(store) => StoreMutate::set_global(&mut **store, name, value),
            RoutedMut::Cells(writer) => StoreMutate::set_global(writer, name, value),
        }
    }

    #[inline]
    fn try_create_instance(
        &mut self,
        type_name: impl Into<SmolStr>,
    ) -> Result<InstanceId, RuntimeError> {
        match self {
            RoutedMut::Values(store) => StoreMutate::try_create_instance(&mut **store, type_name),
            RoutedMut::Cells(writer) => StoreMutate::try_create_instance(writer, type_name),
        }
    }

    #[inline]
    fn remove_instance(&mut self, id: InstanceId) -> bool {
        match self {
            RoutedMut::Values(store) => StoreMutate::remove_instance(&mut **store, id),
            RoutedMut::Cells(writer) => StoreMutate::remove_instance(writer, id),
        }
    }

    #[inline]
    fn retain_instances(&mut self, keep: impl FnMut(InstanceId) -> bool) -> usize {
        match self {
            RoutedMut::Values(store) => StoreMutate::retain_instances(&mut **store, keep),
            RoutedMut::Cells(writer) => StoreMutate::retain_instances(writer, keep),
        }
    }

    #[inline]
    fn restore_instance(&mut self, instance: InstanceId, data: InstanceData) {
        match self {
            RoutedMut::Values(store) => StoreMutate::restore_instance(&mut **store, instance, data),
            RoutedMut::Cells(writer) => StoreMutate::restore_instance(writer, instance, data),
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
        match self {
            RoutedMut::Values(store) => {
                StoreMutate::commit_write(&mut **store, path, value, prepared, capacity)
            }
            RoutedMut::Cells(writer) => {
                StoreMutate::commit_write(writer, path, value, prepared, capacity)
            }
        }
    }

    #[inline]
    fn restore_slot(&mut self, location: MemoryLocation, offset: usize, value: Value) {
        match self {
            RoutedMut::Values(store) => {
                StoreMutate::restore_slot(&mut **store, location, offset, value)
            }
            RoutedMut::Cells(writer) => StoreMutate::restore_slot(writer, location, offset, value),
        }
    }
}

impl RuntimeState<'_> {
    /// The store this runtime state writes through (C1-9c).
    fn routed_for_write(&mut self) -> RoutedMut<'_> {
        let (data, prepared) = self.engine_parts_mut();
        routed_mut(data, prepared)
    }

    /// The store this runtime state reads through (C1-9b).
    fn routed(&self) -> Routed<'_> {
        let (data, prepared) = self.engine_parts();
        data.routed(prepared)
    }
}

impl EngineData {
    /// The store the engine reads through: the arm `EngineStore` holds, with the `Value` half the arm
    /// shares with the engine (locals and per-call instances).
    pub(super) fn routed<'a>(&'a self, prepared: &'a PreparedModule) -> Routed<'a> {
        match &self.store {
            EngineStore::Values => Routed::Values(&self.storage),
            EngineStore::Cells(arm) => Routed::Cells(CellReader::new(prepared, arm, &self.storage)),
        }
    }

    /// Move the next instance identity of the store this record holds, so that a test reaches the
    /// identity limit without issuing 2^32 identities: the flat store issues the identities of its
    /// instances, the `Value` half those of the instances it keeps.
    #[cfg(any(test, feature = "test-support"))]
    pub(super) fn set_next_instance_id_for_test(&mut self, next: u32) {
        self.storage.set_next_instance_id_for_test(next);
        if let EngineStore::Cells(arm) = &mut self.store {
            arm.cells_mut().set_next_instance_id_for_test(next);
        }
    }

    /// Move the installation generation of the store this record holds, so that a test reaches the
    /// last generation without 2^32 installations.
    #[cfg(test)]
    pub(super) fn set_generation_for_test(&mut self, generation: crate::memory::Generation) {
        self.storage.set_generation(generation);
        if let EngineStore::Cells(arm) = &mut self.store {
            arm.cells_mut().set_generation_for_test(generation);
        }
    }
}
