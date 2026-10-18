//! The store-access seam the engine reaches variable storage through.
//!
//! `StoreAccess` reads variables and stages typed path writes while storage is
//! borrowed immutably; `StoreMutate` publishes a prepared write or restores a
//! slot with `&mut self`. A read returns an opaque `ValueView`; the only `&`
//! [`Value`] a caller can obtain is
//! [`StoreAccess::borrow`](crate::memory::view::StoreAccess::borrow), the C1-8
//! compatibility read that C1-9 removes, so the rest of the API can switch to
//! the cell store behind the same API (design
//! `~/projects/.artifacts/bounded-c/design.md` lines 361-367; specification 34
//! §6.2, MEM-10). The traits never charge: every `charge_*`, `before_*` and
//! `check_*` call stays on the engine, where it is today.
//!
//! This slice implements the seam over today's `VariableStorage` (the `Value`
//! tree). Each method forwards to the same storage function the engine calls today
//! (C1-8a note §3), so no result, trap, error, charge or allocation changes.

use super::{
    Generation, InstanceData, InstanceId, MemoryLocation, PreparedStorageWrite, VariableStorage,
};
use crate::error::RuntimeError;
use crate::value::{read_value_path_borrowed, RefSegment, Value, ValueRef, ValueRefView};
use crate::vm::VmTrap;
use smol_str::SmolStr;

/// A variable, or one path inside it, as the store holds it.
///
/// Opaque: only [`StoreAccess`] observes a view, its field is private and, apart
/// from the compatibility read [`StoreAccess::borrow`], no method hands out a
/// `&`[`Value`] into storage.
pub struct ValueView<'a> {
    value: &'a Value,
}

impl<'a> ValueView<'a> {
    /// A view borrowing `value` from storage.
    fn borrowed(value: &'a Value) -> Self {
        Self { value }
    }

    /// Step one compound segment into a child borrowed from the same storage, or
    /// `None` when the child is absent.
    fn member(&self, segment: &RefSegment) -> Option<Self> {
        read_value_path_borrowed(self.value, core::slice::from_ref(segment)).map(Self::borrowed)
    }

    /// The stored text bytes of a text leaf, borrowed. `None` for any other value.
    fn text(&self) -> Option<&'a str> {
        match self.value {
            Value::String(text) => Some(text.as_str()),
            Value::WString(text) => Some(text.as_str()),
            _ => None,
        }
    }

    /// The owned `Value` of this view. The view already holds the resolved leaf,
    /// so this is the clone `materialize_by_ref_parts`/`materialize_value_path`
    /// end their walk with.
    fn materialize(&self) -> Value {
        self.value.clone()
    }
}

/// An opaque staged typed path write (specification 34 MEM-06).
///
/// [`StoreAccess::prepare_write`] returns one while storage is borrowed
/// immutably; [`StoreMutate::commit_write`] publishes it once. Its field stays
/// private so C1-9 can replace today's `PreparedStorageWrite` with a
/// cell-based patch.
pub struct PreparedWrite(PreparedStorageWrite);

impl PreparedWrite {
    /// Whether publishing this patch needs the engine's final deadline check,
    /// forwarding to `PreparedStorageWrite::requires_final_deadline_check`.
    pub fn requires_final_deadline_check(&self) -> bool {
        self.0.requires_final_deadline_check()
    }

    /// Run the caller's alias preflight over the staged patch before it is
    /// published, forwarding to `PreparedStorageWrite::validate_aliases`.
    pub fn validate_aliases(
        &self,
        check: &mut impl FnMut(&Value) -> Result<(), RuntimeError>,
    ) -> Result<(), RuntimeError> {
        self.0.validate_aliases(check)
    }
}

/// Reads one variable, or one path inside it, from the store behind the engine.
pub trait StoreAccess {
    /// The view of `path` inside the slot at `location`/`offset`.
    fn read_path<'a>(
        &'a self,
        location: MemoryLocation,
        offset: usize,
        path: &[RefSegment],
    ) -> Result<ValueView<'a>, VmTrap>;

    /// The view of a whole slot, without a path.
    fn read_slot<'a>(
        &'a self,
        location: MemoryLocation,
        offset: usize,
    ) -> Result<ValueView<'a>, VmTrap>;

    /// The view of a reference's slot and path, in this storage's generation.
    fn read_reference<'a>(&'a self, reference: ValueRefView<'a>) -> Result<ValueView<'a>, VmTrap>;

    /// The view of one instance variable selected by its declared name.
    ///
    /// A C1-8b forwarding method: today's storage names an instance's variables by
    /// string in an ordered map, which a computed cell range cannot carry, and a
    /// built-in function-block executor holds no layout to resolve the name to an
    /// offset. C1-9 resolves the name through the instance template's layout at
    /// this boundary and retires the method (C1-8b note §5; specification 34 §6.2,
    /// MEM-10). Callers: `vm::edge`, `stdlib::fbs::instance`, `vm::engine::cycle`,
    /// whose retained capture reads each retained program variable this way, and the
    /// C1-8c `vm::engine::initialization::staging`,
    /// `vm::engine::retained_graph`, `vm::engine::restart` and
    /// `vm::engine::retain_apply`.
    fn read_instance_var<'a>(&'a self, instance: InstanceId, name: &str) -> Option<ValueView<'a>>;

    /// The view of one global variable selected by its declared name.
    ///
    /// A C1-8c forwarding method, the global counterpart of
    /// [`StoreAccess::read_instance_var`]: today's storage names globals by string in
    /// an ordered map, while a cell range is reached by offset and the retained
    /// capture holds a declaration's name rather than its slot. C1-9 resolves the
    /// name through the prepared layout at this boundary and retires the method
    /// (specification 34 §6.2, MEM-10). Callers: `vm::engine::cycle`,
    /// `vm::engine::initialization::staging`, `vm::engine::retained_graph` and
    /// `vm::engine::retain_apply`.
    fn read_global<'a>(&'a self, name: &str) -> Option<ValueView<'a>>;

    /// The declared name of the global at slot `offset`, in declaration order.
    ///
    /// A C1-8c forwarding method, the name counterpart of
    /// [`StoreAccess::read_global`]: the retained graph rebinds a retained reference
    /// to the candidate's global of the same name, and finds that name from the slot
    /// the reference holds. A name is declaration metadata, not a value, so no stored
    /// value leaves the seam here. C1-9 serves the name from the prepared global
    /// declaration (specification 34 §6.2, MEM-10). Callers:
    /// `vm::engine::retained_graph`.
    fn global_name(&self, offset: usize) -> Option<&SmolStr>;

    /// The view of one instance member resolved through the instance's inheritance
    /// chain.
    ///
    /// A C1-8c forwarding method: today's storage resolves the member by name
    /// against the instance and then its ancestors, and the path-write charge walk
    /// needs that resolution before it may borrow the member. C1-9 resolves the name
    /// through the instance template's layout chain at this boundary and retires the
    /// method (specification 34 §6.2, MEM-10). Callers: `vm::engine::path_write`.
    fn read_instance_field<'a>(
        &'a self,
        instance: InstanceId,
        name: &str,
    ) -> Result<ValueView<'a>, VmTrap>;

    /// The reference to the global variable named `name`, in this storage's generation.
    ///
    /// A C1-8c forwarding method, the reference counterpart of
    /// [`StoreAccess::read_global`]: the retained graph rebinds a retained reference
    /// to the candidate's global of the same name, and needs the slot the candidate
    /// holds it in. C1-9 resolves the name through the prepared layout at this
    /// boundary and retires the method (specification 34 §6.2, MEM-10). Callers:
    /// `vm::engine::retained_graph`.
    fn ref_for_global(&self, name: &str) -> Option<ValueRef>;

    /// The reference to the instance variable `name` of `instance`, in this storage's
    /// generation.
    ///
    /// A C1-8c forwarding method, the reference counterpart of
    /// [`StoreAccess::read_instance_var`]. C1-9 resolves the name through the instance
    /// template's layout at this boundary and retires the method (specification 34
    /// §6.2, MEM-10). Callers: `vm::engine::retained_graph`.
    fn ref_for_instance(&self, instance: InstanceId, name: &str) -> Option<ValueRef>;

    /// An owned copy of one instance record, taken before an initializer may write it.
    ///
    /// A C1-8c forwarding method for the initializer rollback (specification 34
    /// MEM-09). Like the saved root of [`StoreMutate::restore_slot`], the copy is a
    /// journal: it goes back only through [`StoreMutate::restore_instance`]. C1-9
    /// materializes the instance's cells here. Callers:
    /// `vm::engine::initialization::staging`.
    fn snapshot_instance(&self, instance: InstanceId) -> Option<InstanceData>;

    /// Step into one compound segment without cloning the parent.
    fn member<'a>(
        &'a self,
        view: &ValueView<'a>,
        segment: &RefSegment,
    ) -> Result<ValueView<'a>, VmTrap>;

    /// The text bytes of a text leaf, borrowed, without cloning.
    fn text<'a>(&'a self, view: &ValueView<'a>) -> Option<&'a str>;

    /// Borrow the stored leaf a view names, without copying it.
    ///
    /// This is the C1-8 compatibility read: it hands out a `&`[`Value`] into
    /// today's `Value`-tree storage, which the C1-9 cell store cannot serve. C1-9
    /// removes it by moving every caller to view-based reads and charges. The
    /// C1-8a callers are `read_storage_path` and `read_storage_slot` in
    /// `vm::dispatch_refs` and `charge_storage_path_write` in
    /// `vm::engine::path_write` (C1-8a note §4); the C1-8b callers are
    /// `peek_vm_reference_path` and `write_vm_reference` in `vm::call::bindings`,
    /// `VmFbOutSource::read`, `VmFbFieldBinding::read` and `VmWriteTarget::peek` in
    /// `vm::call::bindings::targets`, `EdgeInputTransaction::begin` in `vm::edge`,
    /// `snapshot_root` in `vm::engine::output_transaction`, and the instance read
    /// helper in `stdlib::fbs::instance`; the C1-8c callers are `read_named_bool`,
    /// `execute_task_fb` and `commit_configuration_target` in `vm::engine::io`,
    /// `binding_patch` in `vm::engine::io::staging`, `read_checked_reference` in
    /// `vm::engine::access`, `execute_program_root` and `fill_retained_payload`
    /// in `vm::engine::cycle`, `seed_source_value` and the owned-instance walk of
    /// `initializer_seed` in `vm::engine::initialization::staging`,
    /// `collect_owned_instance_graph` in `vm::engine::initialization::construction`,
    /// `charge_path_write_inner` in `vm::engine::path_write`,
    /// `import_retained_globals` and `RetainedGraph::instance` in
    /// `vm::engine::retained_graph`, `restore_retained_program_values` in
    /// `vm::engine::restart`, and `require_serializable_retained_values` and
    /// `apply_retained_inner` in `vm::engine::retain_apply`. The engine reads the stored
    /// value through this for its copy charge and step-into decisions while today's
    /// storage is a `Value` tree.
    fn borrow<'a>(&'a self, view: &ValueView<'a>) -> Option<&'a Value>;

    /// The owned `Value` of a view; the only allocating read.
    fn materialize(&self, view: &ValueView<'_>) -> Result<Value, RuntimeError>;

    /// Stage a typed path write while storage is still borrowed immutably.
    fn prepare_write(
        &self,
        location: MemoryLocation,
        offset: usize,
        path: &[RefSegment],
        value: &Value,
    ) -> Result<Option<PreparedWrite>, RuntimeError>;

    /// The installation generation of this store. Every reference it returns carries
    /// it, and reference reads and writes resolve only references of this generation
    /// (specification 34 §11.3, MEM-07).
    ///
    /// A C1-9 P3 forwarding method for the shared reference paths, which build references
    /// into the store. Callers: `vm::dispatch_refs`, `vm::call::bindings::targets`.
    fn generation(&self) -> Generation;

    /// The declared type name of `instance`, or `None` when no such instance exists.
    ///
    /// A C1-9 P3 forwarding method for the shared call and reference paths. Callers:
    /// `vm::call`, `vm::reference_attempt`.
    fn instance_type_name(&self, instance: InstanceId) -> Option<&SmolStr>;

    /// The parent of `instance`, which `SUPER` names: `Ok(None)` when it has no parent,
    /// and `NullReference` when no such instance exists (specification 34 §6.2, MEM-10).
    ///
    /// A C1-9 P3 forwarding method for the `SUPER` instructions. Callers: `vm::dispatch`,
    /// `vm::register_ir`.
    fn instance_parent(&self, instance: InstanceId) -> Result<Option<InstanceId>, VmTrap>;

    /// The reference to field `name` of `instance`, searched through its inheritance
    /// chain, in this store's generation.
    ///
    /// A C1-9 P3 forwarding method. Callers: `vm::dispatch`, `vm::call::bindings::targets`.
    fn ref_for_instance_recursive(&self, instance: InstanceId, name: &str) -> Option<ValueRef>;

    /// The reference to field `name` of `instance`: the field the instance declares,
    /// else an inherited one, in this store's generation.
    ///
    /// A C1-9 P3 forwarding method. Callers: `vm::dispatch_refs`, `vm::register_ir`.
    fn resolved_instance_field_ref(&self, instance: InstanceId, name: &str) -> Option<ValueRef>;

    /// The position of field `name` among the fields `instance` declares itself, or
    /// `None` when it declares no such field.
    ///
    /// A C1-9 P3 forwarding method. Callers: `vm::call::bindings::targets`.
    fn declared_instance_field_offset(&self, instance: InstanceId, name: &str) -> Option<usize>;
}

/// Publishes a prepared write, or restores a saved slot, with `&mut self`.
pub trait StoreMutate {
    /// Replace one whole slot, normalized for its target.
    fn write_slot(&mut self, location: MemoryLocation, offset: usize, value: Value) -> bool;

    /// Write through a reference in this storage's generation. A reference of
    /// another generation is refused with [`RuntimeError::ReferenceLifetime`]
    /// and nothing is written (specification 34 §11.3, MEM-07).
    fn write_reference(
        &mut self,
        reference: ValueRefView<'_>,
        value: Value,
    ) -> Result<bool, RuntimeError>;

    /// Replace, or insert, one instance variable selected by its declared name,
    /// without normalizing it for a target.
    ///
    /// A C1-8b forwarding method, the write counterpart of
    /// [`StoreAccess::read_instance_var`]: a name may name a member not yet present
    /// in today's value map, so the seam's slot write (which requires an existing
    /// offset) cannot express it. The name is admitted as an owned `SmolStr` so a
    /// caller that already holds one moves it instead of building a second, keeping
    /// today's allocation. C1-9 resolves the name through the instance template's
    /// layout at this boundary and retires the method. Callers: `vm::edge`,
    /// `stdlib::fbs::instance`, `vm::engine::initialization`,
    /// `vm::engine::retained_graph`, `vm::engine::restart` and
    /// `vm::engine::retain_apply`.
    fn write_instance_var(
        &mut self,
        instance: InstanceId,
        name: impl Into<SmolStr>,
        value: Value,
    ) -> bool;

    /// Replace, or insert, one global variable selected by its declared name.
    ///
    /// A C1-8c forwarding method, the write counterpart of
    /// [`StoreAccess::read_global`]: today's storage names globals by string in an
    /// ordered map, while a cell range is published by offset. C1-9 resolves the
    /// name through the prepared layout at this boundary and retires the method
    /// (specification 34 §6.2, MEM-10). Callers:
    /// `vm::engine::initialization::construction`, `vm::engine::retained_graph` and
    /// `vm::engine::retain_apply`.
    fn set_global(&mut self, name: impl Into<SmolStr>, value: Value);

    /// Allocate a fresh instance identity of `type_name`.
    ///
    /// A C1-8c forwarding method, the structural counterpart of
    /// [`StoreMutate::write_instance_var`]: an identity is issued by the store today,
    /// and C1-9 issues it against the instance region (specification 34 §6.2,
    /// MEM-10). Callers: `vm::engine::initialization::construction`.
    fn try_create_instance(
        &mut self,
        type_name: impl Into<SmolStr>,
    ) -> Result<InstanceId, RuntimeError>;

    /// Release one instance and everything that named it. Returns whether the instance
    /// existed.
    ///
    /// A C1-8c forwarding method: the rollback of a reserve that failed after the
    /// identity was issued. The released record is dropped inside the store, so no
    /// value tree leaves the seam. C1-9 releases the instance's region (specification
    /// 34 §6.2, MEM-10). Callers: `vm::engine::initialization::construction`.
    fn remove_instance(&mut self, id: InstanceId) -> bool;

    /// Release every instance `keep` refuses, in one pass, and return how many were
    /// released.
    ///
    /// A C1-8c forwarding method: the retirement of an owner's instances (TIME-08,
    /// specification 34 MEM-09). C1-9 releases the region of each refused instance.
    /// Callers: `vm::engine::initialization::construction`.
    fn retain_instances(&mut self, keep: impl FnMut(InstanceId) -> bool) -> usize;

    /// Put a snapshot of an instance record back over the instance's current record,
    /// without normalizing any value. An absent instance is left unchanged.
    ///
    /// A C1-8c forwarding method, the write counterpart of
    /// [`StoreAccess::snapshot_instance`], used only by the initializer rollback
    /// (specification 34 MEM-09). C1-9 restores the instance's cells. Callers:
    /// `vm::engine::initialization::staging`.
    fn restore_instance(&mut self, instance: InstanceId, data: InstanceData);

    /// Publish the prepared patch once, applying the receiving text capacity.
    fn commit_write(
        &mut self,
        path: &[RefSegment],
        value: Value,
        prepared: PreparedWrite,
        capacity: usize,
    ) -> bool;

    /// Put a saved root back without normalization (journals only).
    fn restore_slot(&mut self, location: MemoryLocation, offset: usize, value: Value);
}

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
    fn global_name(&self, offset: usize) -> Option<&SmolStr> {
        self.globals.get_index(offset).map(|(name, _)| name)
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
        Some(view.value)
    }

    #[inline]
    fn materialize(&self, view: &ValueView<'_>) -> Result<Value, RuntimeError> {
        Ok(view.materialize())
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
            .map(|prepared| prepared.map(PreparedWrite))
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
    fn set_global(&mut self, name: impl Into<SmolStr>, value: Value) {
        // The inherent method owns the insertion; this forwards to it.
        self.set_global(name, value);
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
        self.commit_prepared_write(path, value, prepared.0, capacity)
    }

    #[inline]
    fn restore_slot(&mut self, location: MemoryLocation, offset: usize, value: Value) {
        self.restore_direct_slot(location, offset, value);
    }
}

#[cfg(all(test, feature = "std"))]
mod tests;
