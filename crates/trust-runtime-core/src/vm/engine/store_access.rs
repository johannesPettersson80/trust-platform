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

use super::{EngineState, RuntimeState};
use crate::error::RuntimeError;
use crate::memory::view::{PreparedWrite, StoreAccess, StoreMutate, ValueView};
use crate::memory::{InstanceId, MemoryLocation};
use crate::value::{RefSegment, Value, ValueRefView};
use crate::vm::VmTrap;

impl StoreAccess for EngineState<'_> {
    #[inline]
    fn read_path<'a>(
        &'a self,
        location: MemoryLocation,
        offset: usize,
        path: &[RefSegment],
    ) -> Result<ValueView<'a>, VmTrap> {
        self.data.storage.read_path(location, offset, path)
    }

    #[inline]
    fn read_slot<'a>(
        &'a self,
        location: MemoryLocation,
        offset: usize,
    ) -> Result<ValueView<'a>, VmTrap> {
        self.data.storage.read_slot(location, offset)
    }

    #[inline]
    fn read_reference<'a>(&'a self, reference: ValueRefView<'a>) -> Result<ValueView<'a>, VmTrap> {
        self.data.storage.read_reference(reference)
    }

    #[inline]
    fn read_instance_var<'a>(&'a self, instance: InstanceId, name: &str) -> Option<ValueView<'a>> {
        self.data.storage.read_instance_var(instance, name)
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
            .storage
            .prepare_write(location, offset, path, value)
    }
}

impl StoreMutate for EngineState<'_> {
    #[inline]
    fn write_slot(&mut self, location: MemoryLocation, offset: usize, value: Value) -> bool {
        self.data.storage.write_slot(location, offset, value)
    }

    #[inline]
    fn write_reference(
        &mut self,
        reference: ValueRefView<'_>,
        value: Value,
    ) -> Result<bool, RuntimeError> {
        self.data.storage.write_reference(reference, value)
    }

    #[inline]
    fn write_instance_var(&mut self, instance: InstanceId, name: &str, value: Value) -> bool {
        self.data.storage.write_instance_var(instance, name, value)
    }

    #[inline]
    fn commit_write(
        &mut self,
        path: &[RefSegment],
        value: Value,
        prepared: PreparedWrite,
        capacity: usize,
    ) -> bool {
        self.data
            .storage
            .commit_write(path, value, prepared, capacity)
    }

    #[inline]
    fn restore_slot(&mut self, location: MemoryLocation, offset: usize, value: Value) {
        self.data.storage.restore_slot(location, offset, value);
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
        self.storage().read_path(location, offset, path)
    }

    #[inline]
    fn read_slot<'a>(
        &'a self,
        location: MemoryLocation,
        offset: usize,
    ) -> Result<ValueView<'a>, VmTrap> {
        self.storage().read_slot(location, offset)
    }

    #[inline]
    fn read_reference<'a>(&'a self, reference: ValueRefView<'a>) -> Result<ValueView<'a>, VmTrap> {
        self.storage().read_reference(reference)
    }

    #[inline]
    fn read_instance_var<'a>(&'a self, instance: InstanceId, name: &str) -> Option<ValueView<'a>> {
        self.storage().read_instance_var(instance, name)
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
        self.storage().prepare_write(location, offset, path, value)
    }
}
